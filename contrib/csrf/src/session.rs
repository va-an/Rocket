use std::str::FromStr;

use base64::DecodeError;
use rocket::http::{Cookie, CookieJar};
use rocket::request::{FromRequest, Outcome};
use rocket::time::{Duration, OffsetDateTime};
use rocket::Request;

use zerocopy::{FromBytes, IntoBytes, NoCell};
use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD as ENCODING};
use rand::distributions::{Distribution, Standard};
use rand::Rng;

#[repr(C)]
#[derive(Debug, Copy, Clone, IntoBytes, NoCell, FromBytes)]
pub struct SessionId {
    // A randomly generated ID.
    id: u64,
    // A unix timestamp (seconds since epoch).
    timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct Session {
    primary: SessionId,
    secondary: Option<SessionId>,
}

enum Error {
    /// An ID that has been expired for some duration.
    Expired(SessionId, Duration),
    Invalid(DecodeError),
    Missing,
}

impl Session {
    const PRIMARY_ID: &'static str = "__rocket_csrfsession_a";

    const SECONDARY_ID: &'static str = "__rocket_csrfsession_b";

    fn _fetch(jar: &CookieJar<'_>) -> Session {
        let max_age = Duration::hours(3);
        match SessionId::fetch(Self::PRIMARY_ID, jar, max_age) {
            Ok(primary) => {
                let secondary = SessionId::fetch(Self::SECONDARY_ID, jar, max_age);
                Session { primary, secondary: secondary.ok() }
            },
            Err(Error::Expired(id, elapsed)) if elapsed < max_age => {
                let primary = rand::random::<SessionId>();
                primary.insert_into(Self::PRIMARY_ID, jar, max_age);
                id.insert_into(Self::SECONDARY_ID, jar, max_age);
                Session { primary, secondary: Some(id) }
            },
            _ => {
                let primary = rand::random::<SessionId>();
                let secondary = SessionId::fetch(Self::SECONDARY_ID, jar, max_age);
                primary.insert_into(Self::PRIMARY_ID, jar, max_age);
                Session { primary, secondary: secondary.ok() }
            }
        }
    }

    pub fn id(&self) -> SessionId {
        self.primary
    }

    pub fn iter(&self) -> impl Iterator<Item = SessionId> {
        std::iter::once(self.primary).chain(self.secondary)
    }

    pub fn fetch(req: &Request<'_>) -> Session {
        req.local_cache(|| Self::_fetch(req.cookies())).clone()
    }
}

impl SessionId {
    /// Returns Ok(time remaining) or Err(time expired).
    fn validity(&self, max_age: Duration) -> Result<Duration, Duration> {
        let elapsed = OffsetDateTime::now_utc()
            .unix_timestamp()
            .checked_sub(self.timestamp)
            .map(Duration::seconds);

        match elapsed {
            Some(elasped) if elasped > max_age => Err(elasped - max_age),
            Some(elasped) => Ok(max_age - elasped),
            None => Err(Duration::MAX),
        }
    }

    fn fetch(name: &str, jar: &CookieJar<'_>, max_age: Duration) -> Result<SessionId, Error> {
        let cookie = jar.get_private(name).ok_or(Error::Missing)?;
        match cookie.value().parse::<SessionId>() {
            Ok(id) => match id.validity(max_age) {
                Ok(_) => Ok(id),
                Err(elapsed) => Err(Error::Expired(id, elapsed)),
            }
            Err(e) => Err(Error::Invalid(e))
        }
    }

    fn insert_into(self, name: &'static str, jar: &CookieJar<'_>, max_age: Duration) {
        let start = OffsetDateTime::from_unix_timestamp(self.timestamp)
            .unwrap_or_else(|_| OffsetDateTime::now_utc());

        let cookie = Cookie::build((name, self.to_string()))
            .http_only(false)
            .expires(start + max_age);

        jar.add_private(cookie);
    }

    pub fn value(&self) -> u64 {
        self.id
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Session {
    type Error = std::convert::Infallible;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        Outcome::Success(Self::fetch(req))
    }
}

impl Distribution<SessionId> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> SessionId {
        SessionId {
            id: rng.gen(),
            timestamp: OffsetDateTime::now_utc().unix_timestamp(),
        }
    }
}

impl ToString for SessionId {
    fn to_string(&self) -> String {
        ENCODING.encode(self.as_bytes())
    }
}

impl FromStr for SessionId {
    type Err = base64::DecodeError;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let bytes = ENCODING.decode(string)?;
        Self::read_from(&bytes)
            .ok_or(base64::DecodeError::InvalidLength(bytes.len()))
    }
}
