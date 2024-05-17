#[macro_use]
mod macros;
mod traceable;

#[cfg(feature = "trace")]
#[cfg_attr(nightly, doc(cfg(feature = "trace")))]
pub mod subscriber;

pub(crate) mod level;

#[doc(inline)]
pub use traceable::{Traceable, TraceableCollection};

#[doc(inline)]
pub use macros::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(crate = "rocket::serde")]
pub enum TraceFormat {
    #[serde(rename = "pretty")]
    #[serde(alias = "PRETTY")]
    Pretty,
    #[serde(rename = "compact")]
    #[serde(alias = "COMPACT")]
    Compact
}

#[cfg_attr(nightly, doc(cfg(feature = "trace")))]
pub fn init<'a, T: Into<Option<&'a crate::Config>>>(config: T) {
    #[cfg(feature = "trace")]
    crate::trace::subscriber::RocketDynFmt::init(config.into())
}
