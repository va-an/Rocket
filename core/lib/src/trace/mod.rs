use rocket::Config;

#[macro_use]
pub mod macros;
#[cfg(feature = "trace")]
pub mod subscriber;
pub mod level;
pub mod traceable;

pub fn init<'a, T: Into<Option<&'a Config>>>(_config: T) {
    #[cfg(feature = "trace")]
    subscriber::init(_config.into());
}
