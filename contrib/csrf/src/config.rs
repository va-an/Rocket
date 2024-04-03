use std::time::Duration;

use rocket::serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Config {
    pub enable: bool,
    pub rotate: Rotate,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct Rotate {
    pub period: u8,
    pub window: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self { enable: true, rotate: Rotate::default() }
    }
}

impl Default for Rotate {
    fn default() -> Self {
        Self {
            period: 24,
            window: 6,
        }
    }
}

impl Rotate {
    pub const fn period(&self) -> Duration {
        Duration::from_secs(self.period as u64 * 3600)
    }

    pub const fn window(&self) -> Duration {
        Duration::from_secs(self.window as u64 * 3600)
    }

    pub const fn epoch(&self) -> Duration {
        let wait = self.period.saturating_sub(self.window);
        Duration::from_secs(wait as u64 * 3600)
    }
}
