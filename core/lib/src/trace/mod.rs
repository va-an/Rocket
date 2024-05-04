use rocket::Config;

#[cfg(feature = "trace")]
pub mod subscriber;

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
    ($($name:ident),*) => (
        $(declare_macro!([$] $name);)*
    );

    ([$d:tt] $name:ident) => (
        #[macro_export]
        macro_rules! $name {
            ($d ($t:tt)*) => ({
                #[allow(unused_imports)]
                use $crate::trace::PaintExt as _;

                $crate::tracing::event!($crate::tracing::Level::INFO, $d ($t)*);
            })
        }
    );
}

declare_macro!(log, log_, launch_info, launch_info_, launch_meta, launch_meta_,
    error, error_, info, info_, trace, trace_, debug, debug_, warn, warn_);

pub fn init(_config: &Config) {
    #[cfg(feature = "trace")]
    subscriber::init(_config);
}
