use std::sync::OnceLock;

use tracing::{Level, Subscriber};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{reload, filter, Layer, Registry};
use yansi::{Condition, Paint, Painted};

use crate::Config;

pub trait PaintExt: Sized {
    fn emoji(self) -> Painted<Self>;
}

impl PaintExt for &str {
    /// Paint::masked(), but hidden on Windows due to broken output. See #1122.
    fn emoji(self) -> Painted<Self> {
        #[cfg(windows)] { Paint::new("").mask() }
        #[cfg(not(windows))] { Paint::new(self).mask() }
    }
}

pub fn filter_layer(level: Level) -> filter::Targets {
    filter::Targets::new()
        .with_default(level)
        .with_target("rustls", LevelFilter::OFF)
        .with_target("hyper", LevelFilter::OFF)
}

pub fn fmt_layer<S: Subscriber + for<'span> LookupSpan<'span>>() -> impl Layer<S> {
    let layer = tracing_subscriber::fmt::layer();

    #[cfg(not(test))] { layer }
    #[cfg(test)] { layer.with_test_writer() }
}

pub(crate) fn init(config: &Config) {
    static HANDLE: OnceLock<reload::Handle<filter::Targets, Registry>> = OnceLock::new();

    // FIXME: Read the true level from `config`.
    let level = Level::INFO;

    // Always disable colors if requested or if the stdout/err aren't TTYs.
    let should_color = match config.cli_colors {
        crate::config::CliColors::Always => Condition::ALWAYS,
        crate::config::CliColors::Auto => Condition::DEFAULT,
        crate::config::CliColors::Never => Condition::NEVER,
    };

    let (filter, reload_handle) = reload::Layer::new(filter_layer(level));
    let result = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer())
        .try_init();

    if result.is_ok() {
        assert!(HANDLE.set(reload_handle).is_ok());
        yansi::whenever(should_color);
    } else if let Some(handle) = HANDLE.get() {
        assert!(handle.modify(|filter| *filter = filter_layer(level)).is_ok());
        yansi::whenever(should_color);
    } else {
        yansi::disable()
    }
}
