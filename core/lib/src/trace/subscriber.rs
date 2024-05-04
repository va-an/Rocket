use std::sync::OnceLock;

use tracing::{Level, Subscriber};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{reload, filter, Layer, Registry};
use yansi::{Condition, Paint, Painted};

use crate::config::CliColors;
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

pub fn filter_layer(level: impl Into<LevelFilter>) -> filter::Targets {
    filter::Targets::new()
        .with_default(level.into())
        .with_target("rustls", LevelFilter::OFF)
        .with_target("hyper", LevelFilter::OFF)
}

pub fn fmt_layer<S: Subscriber + for<'span> LookupSpan<'span>>() -> impl Layer<S> {
    tracing_subscriber::fmt::layer().with_test_writer()

    // #[cfg(not(test))] { layer }
    // #[cfg(test)] { layer.with_test_writer() }
}

pub(crate) fn init(config: Option<&Config>) {
    static HANDLE: OnceLock<reload::Handle<filter::Targets, Registry>> = OnceLock::new();

    // Do nothing if there's no config and we've already initialized.
    if config.is_none() && HANDLE.get().is_some() {
        return;
    }

    // Always disable colors if requested or if the stdout/err aren't TTYs.
    let cli_colors = config.map(|c| c.cli_colors).unwrap_or(CliColors::Auto);
    let should_color = match cli_colors {
        CliColors::Always => Condition::ALWAYS,
        CliColors::Auto => Condition::DEFAULT,
        CliColors::Never => Condition::NEVER,
    };

    let log_level = config.map(|c| c.log_level).unwrap_or(Some(Level::INFO));
    let (filter, reload_handle) = reload::Layer::new(filter_layer(log_level));
    let result = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer())
        .try_init();

    if result.is_ok() {
        assert!(HANDLE.set(reload_handle).is_ok());
        yansi::whenever(should_color);
    } if let Some(handle) = HANDLE.get() {
        assert!(handle.modify(|filter| *filter = filter_layer(log_level)).is_ok());
        yansi::whenever(should_color);
    } else {
        yansi::disable()
    }
}
