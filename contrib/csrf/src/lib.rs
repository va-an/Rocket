//! CSRF Security
//!
//! This library implements a variant of the "signed double-submit cookie"
//! anti-CSRF technique which ensures that attackers cannot forge requests to
//! the server as originating from innocent clients. It does so by checking that
//! every potentially unsafe request contains a [`Token`] corresponding to the
//! issuing client's [`Session`]. Both the [`Token`] and the [`Session`] are
//! created by the server and cannot be forged. As long as a client protects
//! their [`Session`] _long enough_, no attacker can act as that client.
//!
//! ## Design
//!
//! A [`Token`] is an unforgeable, verifiable value containing the following:
//!
//!   * At least 96-bits of entropy or unique data.
//!   * A context indicating what kind of request the token is valid for.
//!   * The session ID the token is associated with.
//!   * A crypographic signature using rotating key `T` of the above.
//!
//! A [`Session`] is an unforgeable, verifiable value containing the following:
//!
//!   * An ID consisting of least 64-bits of entropy.
//!   * A timestamp.
//!
//! Unsafe requests must include both a session and token, provided by the
//! server from an earlier request. The server checks their authenticity as well
//! as whether the session and request context matches the submitted token. If
//! so, the request is allowed. If not, the request is denied.
//!
//! Sessions are AEAD encrypted with a private server-side key `S` where `S !=
//! T` and are thus private, verifiable, and unforgeable. Every request is
//! associated with a session, and the encrypted session is sent as a cookie to
//! the client so as to re-identify the client's session in future requests.
//!
//! A `Token` can be created given a `Session`, to which it is cryptographically
//! tied. Many new tokens may be created in response to a request and are
//! returned in plaintext to the client for use in later requests.
//!
//! The tokenizer utilizes automatically rotating 256-bit server-side keys to
//! cryptographically sign unique tokens, which include client session
//! identifiers, which themselves are cryptographically signed, rotating 64-bit
//! identifiers maintained by the client.
//!
//! ## Automatic Rotation
//!
//! Tokens and session gracefully expire. Tokens expire by virtue of a rotating
//! set of keys while session expire via a timestamp.
//!
//! ### Token Expiration
//!
//! Tokens are signed with a single key `T` (the primary key) but verified with
//! either of two keys `T` (the primary key) and `T!` (the secondary key), to
//! avoid old tokens immediately becoming unusable. On rotation, `T` becomes
//! `T!` and a new `T` is generated.
//!
//! Rotation is configured via two parameters:
//!
//!   * `period`: the total time a key is used as either `T` or `T!`
//!   * `window`: how long `T!` _at least_ exists before being rotated out
//!
//! `period` must be greater than `window`. Additionally, `epoch` is a computed
//! parameter equal to `period - window`.
//!
//! Keys are rotated twice per period: once after `epoch` seconds and again
//! after `window` seconds. The following illustrates this timeline, where `e`
//! is `epoch`, `p` is `period`, and `w` is `window`:
//!
//! | Time | T | T! | Note                                            |
//! |------|---|----|-------------------------------------------------|
//! | 0    | A | -- |                                                 |
//! | e    | B | A  |                                                 |
//! | + w  | C | B  | A was `T` or `T!` for `e + w = p`, `T!` for `w` |
//! | + e  | D | C  | B was `T` or `T!` for `w + e = p`, `T!` for `e` |
//! | + w  | E | D  | C was `T` or `T!` for `e + w = p`, `T!` for `w` |
//!
//! Rotation occurs every `R` seconds at which point `T` becomes `T!`
//! and a new `T` is generated. After `R + E` seconds,
//!
//! to sign tokens is rotated every `R_t1` seconds. The old key
//! `T` becomes `T*` and a new, randomly generated key `T` takes its place.
//! After `R_t2` seconds, `T`
//!
//! ## Threat Model and Guarantees
//!
//! The design assumes the following:
//!
//!  * The server-side keys used by `Tokenizer` are secret to the server.
//!  * Client-side session information, provided by the server, is known only
//!    to the intended client, though that client may be coerced into revealing
//!    their session.
//!  * Attackers are unable to mount man-in-the-middle attacks. Attackers may be
//!    able to inspect traffic historically given sufficient time.
//!
//! An attacker can forge a request only when it is able to create or obtain a
//! `Token` _and_ a `Session`. There are several scenarios:
//!
//!   * If the attacker steals a `Token` `t` associated with id `id`, then
//!
//!    - The token `t` is only valid for session id `id`. Thus the attacker must
//!      either steal an existing session with id `id` or have the ability to
//!      create arbitrary sessions.
//!
//!      In the former case, because sessions are encrypted, a stolen `Session`
//!      does not itself reveal the session ID. The attacker is thus forced to
//!      target a specifc token/session pair or try tokens at random. Alternati
//!
//!      In the latter case, the attack must steal key `S`, which is held in
//!      in-memory sever-side and is presumed to be secure.
//!
//!   * If the attacker steals a `Session` `s`:
//!
//!     The attack must steal a `Token` `t` associated with `s` (difficult for
//!     the same reasons as above) or key `T`.
//!
//!    Stole a `Token` `t` or key `T` _and_ steals the session associated with
//!    the token `t` or key `S`.
//!
//! In return, it guarantees that in order to forge a request, an attacker must
//! steal the server-side secret key.
//!
//! This ensures that an attacker cannot create and inject their own, known,
//! CSRF token into the victim's authenticated session.

#![doc(html_root_url = "https://api.rocket.rs/master/rocket_csrf")]
#![doc(html_favicon_url = "https://rocket.rs/images/favicon.ico")]
#![doc(html_logo_url = "https://rocket.rs/images/logo-boxed.png")]

#[macro_use]
extern crate rocket;

#[cfg(test)]
mod tests;
mod key;
mod tokenizer;
mod config;
mod fairing;
mod session;

pub use config::Config;
pub use tokenizer::{Tokenizer, Token};
pub use session::{Session, SessionId};

pub const fn base64_len<T>() -> usize {
    (std::mem::size_of::<T>() * 4).div_ceil(3)
}
