use std::cell::Cell;
use std::ops::Index;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::fmt::{self, Debug, Display};
use std::thread::ThreadId;
use std::hash::{Hash, Hasher};

use tracing::{Event, Level, Metadata, Subscriber};
use tracing::level_filters::LevelFilter;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};

use tracing_subscriber::prelude::*;
use tracing_subscriber::layer::{Context, Layered};
use tracing_subscriber::registry::{LookupSpan, SpanRef};
use tracing_subscriber::{reload, filter, Layer, Registry};
use tracing_subscriber::field::RecordFields;

use tinyvec::TinyVec;
use yansi::{Condition, Paint, Painted, Style};

use crate::config::{Config, CliColors};
use crate::util::Formatter;

pub trait PaintExt: Sized {
    fn emoji(self) -> Painted<&'static str>;
}

impl PaintExt for Painted<&'static str> {
    /// Paint::masked(), but hidden on Windows due to broken output. See #1122.
    fn emoji(self) -> Painted<&'static str> {
        #[cfg(windows)] { Paint::new("").mask() }
        #[cfg(not(windows))] { self.mask() }
    }
}

#[derive(Default)]
pub struct IdentHasher(u128);

impl Hasher for IdentHasher {
    fn finish(&self) -> u64 {
        self.0 as u64
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 << 8) | (byte as u128);
        }
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0 << 64) | (i as u128);
    }

    fn write_u128(&mut self, i: u128) {
        self.0 = i;
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RequestId {
    thread: ThreadId,
    span: Id,
}

thread_local! {
    pub static CURRENT_REQUEST_ID: Cell<Option<u128>> = Cell::new(None);
}

impl RequestId {
    fn new(span: &Id) -> Self {
        thread_local! {
            pub static THREAD_ID: Cell<Option<ThreadId>> = Cell::new(None);
        }

        RequestId {
            span: span.clone(),
            thread: THREAD_ID.get().unwrap_or_else(|| {
                let id = std::thread::current().id();
                THREAD_ID.set(Some(id));
                id
            }),
        }
    }

    fn of<R: for<'a> LookupSpan<'a>>(span: &SpanRef<'_, R>) -> Option<u128> {
        span.extensions().get::<Self>().map(|id| id.as_u128())
    }

    fn current() -> Option<u128> {
        CURRENT_REQUEST_ID.get()
    }

    fn layer() -> RequestIdLayer {
        RequestIdLayer
    }

    fn as_u128(&self) -> u128 {
        let mut hasher = IdentHasher::default();
        self.hash(&mut hasher);
        hasher.0
    }
}

struct RequestIdLayer;

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for RequestIdLayer {
    fn on_new_span(&self, _: &Attributes<'_>, id: &Id, ctxt: Context<'_, S>) {
        let span = ctxt.span(id).expect("new_span: span does not exist");
        if span.name() == "request" {
            span.extensions_mut().replace(RequestId::new(id));
        }
    }

    fn on_enter(&self, id: &Id, ctxt: Context<'_, S>) {
        let span = ctxt.span(id).expect("enter: span does not exist");
        if span.name() == "request" {
            CURRENT_REQUEST_ID.set(RequestId::of(&span));
        }
    }

    fn on_exit(&self, id: &Id, ctxt: Context<'_, S>) {
        let span = ctxt.span(id).expect("enter: span does not exist");
        if span.name() == "request" {
            CURRENT_REQUEST_ID.set(None);
        }
    }
}

pub(crate) fn init(config: Option<&Config>) {
    type RocketSubscriber = Layered<RequestIdLayer, Registry>;

    static HANDLE: OnceLock<reload::Handle<RocketFmt, RocketSubscriber>> = OnceLock::new();

    // Do nothing if there's no config and we've already initialized.
    if config.is_none() && HANDLE.get().is_some() {
        return;
    }

    let cli_colors = config.map(|c| c.cli_colors).unwrap_or(CliColors::Auto);
    let log_level = config.map(|c| c.log_level).unwrap_or(Some(Level::INFO));
    let (layer, reload_handle) = reload::Layer::new(RocketFmt::new(cli_colors, log_level));
    let result = tracing_subscriber::registry()
        .with(RequestId::layer())
        .with(layer)
        .try_init();

    if result.is_ok() {
        assert!(HANDLE.set(reload_handle).is_ok());
    } if let Some(handle) = HANDLE.get() {
        assert!(handle.modify(|layer| layer.set(cli_colors, log_level)).is_ok());
    }
}

pub(crate) struct Data {
    // start: Instant,
    map: TinyVec<[(&'static str, String); 3]>,
}

impl Data {
    pub fn new<T: RecordFields>(attrs: T) -> Self {
        let mut data = Data {
            // start: Instant::now(),
            map: TinyVec::new(),
        };

        attrs.record(&mut data);
        data
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.map.iter()
            .find(|(k, _)| k == &key)
            .map(|(_, v)| v.as_str())
    }
}

impl Index<&str> for Data {
    type Output = str;

    fn index(&self, index: &str) -> &Self::Output {
        self.get(index).unwrap_or("[internal error: missing key]")
    }
}

impl Visit for Data {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.map.push((field.name(), format!("{:?}", value)));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.map.push((field.name(), value.into()));
    }
}

#[derive(Default)]
struct RocketFmt {
    depth: AtomicU8,
    filter: filter::Targets,
    default_style: Style,
}

struct DisplayVisit<F>(F);

impl<F: FnMut(&Field, &dyn fmt::Display)> Visit for DisplayVisit<F> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        (self.0)(field, &Formatter(|f| value.fmt(f)));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        (self.0)(field, &value)
    }
}

trait DisplayFields {
    fn record_display<F: FnMut(&Field, &dyn fmt::Display)>(&self, f: F);
}

impl<T: RecordFields> DisplayFields for T {
    fn record_display<F: FnMut(&Field, &dyn fmt::Display)>(&self, f: F) {
        self.record(&mut DisplayVisit(f));
    }
}

impl RocketFmt {
    fn new(cli_colors: CliColors, level: impl Into<LevelFilter>) -> Self {
        let mut this = Self {
            depth: AtomicU8::new(0),
            filter: filter::Targets::new(),
            default_style: Style::new(),
            // _subscriber: PhantomData,
        };

        this.set(cli_colors, level.into());
        this
    }

    fn set(&mut self, cli_colors: CliColors, level: impl Into<LevelFilter>) {
        self.default_style = Style::new().whenever(match cli_colors {
            CliColors::Always => Condition::ALWAYS,
            CliColors::Auto => Condition::DEFAULT,
            CliColors::Never => Condition::NEVER,
        });

        self.filter = filter::Targets::new()
            .with_default(level.into())
            .with_target("rustls", LevelFilter::OFF)
            .with_target("hyper", LevelFilter::OFF);
    }

    fn indent(&self) -> &'static str {
        static INDENT: &[&str] = &["", "   ", "      "];
        INDENT.get(self.depth()).copied().unwrap_or("         ")
    }

    fn marker(&self) -> &'static str {
        static MARKER: &[&str] = &["", ">> ", ":: "];
        MARKER.get(self.depth()).copied().unwrap_or("-- ")
    }

    fn depth(&self) -> usize {
        self.depth.load(Ordering::Acquire) as usize
    }

    fn style(&self, metadata: &Metadata<'_>) -> Style {
        match *metadata.level() {
            Level::ERROR => self.default_style.red(),
            Level::WARN => self.default_style.yellow(),
            Level::INFO => self.default_style.blue(),
            Level::DEBUG => self.default_style.green(),
            Level::TRACE => self.default_style.magenta(),
        }
    }

    fn print_prefix(&self, meta: &Metadata<'_>) {
        let (i, m, s) = (self.indent(), self.marker(), self.style(meta));
        match *meta.level() {
            Level::WARN => print!("{i}{m}{} ", "warning:".paint(s).bold()),
            Level::ERROR => print!("{i}{m}{} ", "error:".paint(s).bold()),
            Level::INFO => print!("{i}{m}"),
            level => print!("{i}{m}[{} {}] ", level.paint(s).bold(), meta.target()),
        }

        if let Some(id) = RequestId::current() {
            print!("[{id:x}] ");
        }
    }

    fn print<F: RecordFields>(&self, metadata: &Metadata<'_>, data: F) {
        let style = self.style(metadata);
        let fields = metadata.fields();
        if !fields.is_empty() {
            self.print_prefix(metadata);
        }

        let message = fields.field("message");
        if let Some(message_field) = &message {
            data.record_display(|field: &Field, value: &dyn Display| {
                if field == message_field {
                    for (i, line) in value.to_string().lines().enumerate() {
                        if i != 0 {
                            print!("{}{} ", self.indent(), "++".paint(style).dim());
                        }

                        println!("{}", line.paint(style));
                    }
                }
            });
        }

        if message.is_some() && fields.len() > 1 {
            print!("{}{} ", self.indent(), "++".paint(style).dim());
            self.println_compact_fields(metadata, data)
        } else if message.is_none() && !fields.is_empty() {
            self.println_compact_fields(metadata, data);
        }
    }

    fn println_compact_fields<F: RecordFields>(&self, metadata: &Metadata<'_>, data: F) {
        self.print_compact_fields(metadata, data);
        println!();
    }

    fn print_compact_fields<F: RecordFields>(&self, metadata: &Metadata<'_>, data: F) {
        let key_style = self.style(metadata).bold();
        let val_style = self.style(metadata).primary();

        let mut printed = false;
        data.record_display(|field: &Field, val: &dyn Display| {
            let key = field.name();
            if key != "message" {
                if printed { print!(" "); }
                print!("{}: {}", key.paint(key_style), val.paint(val_style));
                printed = true;
            }
        });
    }

    fn print_fields<F>(&self, metadata: &Metadata<'_>, fields: F)
        where F: RecordFields
    {
        let style = self.style(metadata);
        fields.record_display(|key: &Field, value: &dyn Display| {
            if key.name() != "message" {
                self.print_prefix(metadata);
                println!("{}: {}", key.paint(style), value.paint(style).primary());
            }
        })
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for RocketFmt {
    fn enabled(&self, metadata: &Metadata<'_>, _: Context<'_, S>) -> bool {
        self.filter.would_enable(metadata.target(), metadata.level())
    }

    fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
        let (meta, data) = (event.metadata(), Data::new(event));
        let style = self.style(meta);
        match meta.name() {
            "config" => self.print_fields(meta, event),
            "liftoff" => {
                self.print_prefix(meta);
                println!("{}{} {}", "🚀 ".paint(style).emoji(),
                    "Rocket has launched from".paint(style).primary().bold(),
                    &data["endpoint"].paint(style).primary().bold().underline());
            }
            _ => self.print(meta, event),
        }
    }

    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctxt: Context<'_, S>) {
        let data = Data::new(attrs);
        let span = ctxt.span(id).expect("new_span: span does not exist");
        let style = self.style(span.metadata());
        if &data["count"] != "0" {
            let name = span.name();
            let emoji = match name {
                "config" => "🔧 ",
                "routes" => "📬 ",
                "catchers" => "🚧 ",
                "fairings" => "📦 ",
                "shield" => "🛡️ ",
                "request" => "● ",
                _ => "",
            };

            self.print_prefix(span.metadata());
            print!("{}{}", emoji.paint(style).emoji(), name.paint(style).bold());
            if let Some(id) = RequestId::of(&span) {
                print!(" [{id:x}]");
            }

            if !attrs.fields().is_empty() {
                print!(" {}", "(".paint(style));
                self.print_compact_fields(span.metadata(), attrs);
                print!("{}", ")".paint(style));
            }

            println!();
        }

        span.extensions_mut().replace(data);
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctxt: Context<'_, S>) {
        let span = ctxt.span(id).expect("new_span: span does not exist");
        match span.extensions_mut().get_mut::<Data>() {
            Some(data) => values.record(data),
            None => span.extensions_mut().insert(Data::new(values)),
        }

        self.print_prefix(span.metadata());
        self.println_compact_fields(span.metadata(), values);
    }

    fn on_enter(&self, _: &Id, _: Context<'_, S>) {
        self.depth.fetch_add(1, Ordering::AcqRel);
    }

    fn on_exit(&self, _: &Id, _: Context<'_, S>) {
        self.depth.fetch_sub(1, Ordering::AcqRel);
    }
}
