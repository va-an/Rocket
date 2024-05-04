use rocket::Config;

#[cfg(feature = "trace")]
pub mod subscriber;
pub mod level;

pub trait PaintExt: Sized {
    fn emoji(self) -> yansi::Painted<Self>;
}

impl PaintExt for &str {
    /// Paint::masked(), but hidden on Windows due to broken output. See #1122.
    fn emoji(self) -> yansi::Painted<Self> {
        #[cfg(windows)] { yansi::Paint::new("").mask() }
        #[cfg(not(windows))] { yansi::Paint::new(self).mask() }
    }
}

macro_rules! declare_macro {
    ($($name:ident $level:ident),* $(,)?) => (
        $(declare_macro!([$] $name $level);)*
    );

    ([$d:tt] $name:ident $level:ident) => (
        #[macro_export]
        macro_rules! $name {
            ($d ($t:tt)*) => ({
                #[allow(unused_imports)]
                use $crate::trace::PaintExt as _;

                $crate::tracing::event!($crate::tracing::Level::$level, $d ($t)*);
            })
        }
    );
}

declare_macro!(
    launch_info INFO, launch_info_ INFO,
    launch_meta INFO, launch_meta_ INFO,
    error ERROR, error_ ERROR,
    info INFO, info_ INFO,
    trace TRACE, trace_ TRACE,
    debug DEBUG, debug_ DEBUG,
    warn WARN, warn_ WARN,
);

pub fn init<'a, T: Into<Option<&'a Config>>>(_config: T) {
    #[cfg(feature = "trace")]
    subscriber::init(_config.into());
}
