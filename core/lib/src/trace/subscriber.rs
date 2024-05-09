use std::marker::PhantomData;
use std::ops::Index;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};
use std::fmt::{self, Debug, Display};
// use std::time::Instant;

use tracing::{Event, Level, Metadata, Subscriber};
use tracing::level_filters::LevelFilter;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};

use tracing_subscriber::prelude::*;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{reload, filter, Layer, Registry};
use tracing_subscriber::field::RecordFields;

use figment::Source::File as RelPath;
use yansi::{Condition, Paint, Painted, Style};
use tinyvec::TinyVec;

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

pub(crate) fn init(config: Option<&Config>) {
    static HANDLE: OnceLock<reload::Handle<RocketFmt<Registry>, Registry>> = OnceLock::new();

    // Do nothing if there's no config and we've already initialized.
    if config.is_none() && HANDLE.get().is_some() {
        return;
    }

    let cli_colors = config.map(|c| c.cli_colors).unwrap_or(CliColors::Auto);
    let log_level = config.map(|c| c.log_level).unwrap_or(Some(Level::INFO));
    let (layer, reload_handle) = reload::Layer::new(RocketFmt::new(cli_colors, log_level));
    let result = tracing_subscriber::registry()
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
    map: TinyVec<[(&'static str, String); 2]>,
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
struct RocketFmt<S> {
    depth: AtomicU8,
    filter: filter::Targets,
    default_style: Style,
    _subscriber: PhantomData<fn() -> S>
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

impl<S: Subscriber + for<'a> LookupSpan<'a>> RocketFmt<S> {
    fn new(cli_colors: CliColors, level: impl Into<LevelFilter>) -> Self {
        let mut this = Self {
            depth: AtomicU8::new(0),
            filter: filter::Targets::new(),
            default_style: Style::new(),
            _subscriber: PhantomData,
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
        INDENT.get(self.depth()).copied().unwrap_or("--")
    }

    fn marker(&self) -> &'static str {
        static MARKER: &[&str] = &["", ">> ", ":: "];
        MARKER.get(self.depth()).copied().unwrap_or("-- ")
    }

    fn depth(&self) -> usize {
        self.depth.load(Ordering::Acquire) as usize
    }

    // fn increase_depth(&self) {
    //     self.depth.fetch_add(1, Ordering::AcqRel);
    // }
    //
    // fn decrease_depth(&self) {
    //     self.depth.fetch_sub(1, Ordering::AcqRel);
    // }

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
            Level::DEBUG => print!("{i}{m}[{} {}] ", "debug".paint(s).bold(), meta.target()),
            Level::TRACE => match (meta.file(), meta.line()) {
                (Some(file), Some(line)) => print!(
                    "{i}{m}[{level} {target} {path}:{line}] ",
                    level = "trace".paint(s).bold(),
                    target = meta.target().paint(s).dim(),
                    path = RelPath(file.into()).underline(),
                ),
                _ => print!("{i}{m}[{} {}] ", "trace".paint(s).bold(), meta.target())
            }
            _ => print!("{i}{m}"),
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
            self.print_compact_fields(metadata, data)
        } else if message.is_none() && !fields.is_empty() {
            self.print_compact_fields(metadata, data);
        }
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

        println!();
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

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for RocketFmt<S> {
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

    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("new_span: span does not exist");
        let data = Data::new(attrs);
        match span.metadata().name() {
            "config" => println!("configured for {}", &data["profile"]),
            name => {
                self.print_prefix(span.metadata());
                print!("{name} ");
                self.print_compact_fields(span.metadata(), attrs);
            }
        }

        span.extensions_mut().replace(data);
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctxt: Context<'_, S>) {
        let metadata = ctxt.span(id).unwrap().metadata();
        self.print_prefix(metadata);
        self.print_compact_fields(metadata, values);
    }

    fn on_enter(&self, _: &Id, _: Context<'_, S>) {
        self.depth.fetch_add(1, Ordering::AcqRel);
    }

    fn on_exit(&self, _: &Id, _: Context<'_, S>) {
        self.depth.fetch_sub(1, Ordering::AcqRel);
    }
}
