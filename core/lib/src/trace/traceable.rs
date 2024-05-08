use crate::fairing::Fairing;
use crate::{Catcher, Config, Route};
use crate::util::Formatter;

use figment::Figment;
use rocket::http::Header;

pub trait Traceable {
    fn trace(&self);
}

pub trait TraceableCollection {
    fn trace_all(self);
}

impl<T: Traceable, I: IntoIterator<Item = T>> TraceableCollection for I {
    fn trace_all(self) {
        self.into_iter().for_each(|i| i.trace())
    }
}

impl<T: Traceable> Traceable for &T {
    fn trace(&self) {
        T::trace(self)
    }
}

impl Traceable for Figment {
    fn trace(&self) {
        for param in Config::PARAMETERS {
            if let Some(source) = self.find_metadata(param) {
                tracing::trace! {
                    param,
                    %source.name,
                    source.source = source.source.as_ref().map(|s| s.to_string()),
                }
            }
        }

        // Check for now deprecated config values.
        for (key, replacement) in Config::DEPRECATED_KEYS {
            if let Some(source) = self.find_metadata(key) {
                warn! {
                    name: "deprecated",
                    key,
                    replacement,
                    %source.name,
                    source.source = source.source.as_ref().map(|s| s.to_string()),
                    "config key `{key}` is deprecated and has no meaning"
                }
            }
        }
    }
}

impl Traceable for Config {
    fn trace(&self) {
        info! {
            name: "config",
            http2 = cfg!(feature = "http2"),
            log_level = self.log_level.map(|l| l.as_str()),
            cli_colors = %self.cli_colors,
            workers = self.workers,
            max_blocking = self.max_blocking,
            ident = %self.ident,
            ip_header = self.ip_header.as_ref().map(|s| s.as_str()),
            proxy_proto_header = self.proxy_proto_header.as_ref().map(|s| s.as_str()),
            limits = %Formatter(|f| f.debug_map()
                .entries(self.limits.limits.iter().map(|(k, v)| (k.as_str(), v.to_string())))
                .finish()),
            temp_dir = %self.temp_dir.relative().display(),
            keep_alive = (self.keep_alive != 0).then_some(self.keep_alive),
            shutdown.ctrlc = self.shutdown.ctrlc,
            shutdown.signals = %{
                #[cfg(not(unix))] {
                    "disabled (not unix)"
                }

                #[cfg(unix)] {
                    Formatter(|f| f.debug_set()
                        .entries(self.shutdown.signals.iter().map(|s| s.as_str()))
                        .finish())
                }
            },
                shutdown.grace = self.shutdown.grace,
                shutdown.mercy = self.shutdown.mercy,
                shutdown.force = self.shutdown.force,
        }

        #[cfg(feature = "secrets")] {
            if !self.secret_key.is_provided() {
                warn! {
                    name: "volatile_secret_key",
                    "secrets enabled without configuring a stable `secret_key`; \
                    private/signed cookies will become unreadable after restarting; \
                    disable the `secrets` feature or configure a `secret_key`; \
                    this becomes a hard error in non-debug profiles",
                }
            }

            let secret_key_is_known = Config::KNOWN_SECRET_KEYS.iter().any(|&key_str| {
                let value = figment::value::Value::from(key_str);
                self.secret_key == value.deserialize().expect("known key is valid")
            });

            if secret_key_is_known {
                warn! {
                    name: "insecure_secret_key",
                    "The configured `secret_key` is exposed and insecure. \
                    The configured key is publicly published and thus insecure. \
                    Try generating a new key with `head -c64 /dev/urandom | base64`."
                }
            }
        }
    }
}

impl Traceable for Route {
    fn trace(&self) {
        info! {
            name: "route",
            name = self.name.as_ref().map(|n| &**n),
            method = %self.method,
            rank = self.rank,
            uri = %self.uri,
            uri.base = %self.uri.base(),
            uri.unmounted = %self.uri.unmounted(),
            format = self.format.as_ref().map(display),
            sentinels = %Formatter(|f|{
                f.debug_set()
                    .entries(self.sentinels.iter().filter(|s| s.specialized).map(|s| s.type_name))
                    .finish()
            })
        }
    }
}

impl Traceable for Catcher {
    fn trace(&self) {
        info! {
            name: "catcher",
            name = self.name.as_ref().map(|n| &**n),
            code = self.code,
            rank = self.rank,
            uri.base = %self.base(),
        }
    }
}

impl Traceable for &dyn Fairing {
    fn trace(&self) {
        info!(name: "fairing", name = self.info().name, kind = %self.info().kind)
    }
}

impl Traceable for figment::error::Kind {
    fn trace(&self) {
        use figment::error::{OneOf as V, Kind::*};

        match self {
            Message(message) => error!(message),
            InvalidType(actual, expected) => error!(name: "invalid type", %actual, expected),
            InvalidValue(actual, expected) => error!(name: "invalid value", %actual, expected),
            InvalidLength(actual, expected) => error!(name: "invalid length", %actual, expected),
            UnknownVariant(actual, v) => error!(name: "unknown variant", actual, expected = %V(v)),
            UnknownField(actual, v) => error!(name: "unknown field", actual, expected = %V(v)),
            UnsupportedKey(actual, v) => error!(name: "unsupported key", %actual, expected = &**v),
            MissingField(value) => error!(name: "missing field", value = &**value),
            DuplicateField(value) => error!(name: "duplicate field", value),
            ISizeOutOfRange(value) => error!(name: "out of range signed integer", value),
            USizeOutOfRange(value) => error!(name: "out of range unsigned integer", value),
            Unsupported(value) => error!(name: "unsupported type", %value),
        }
    }
}

impl Traceable for figment::Error {
    fn trace(&self) {
        for e in self.clone() {
            let span = tracing::error_span! {
                "config",
                key = (!e.path.is_empty()).then_some(&e.path).and_then(|path| {
                    let (profile, metadata) = (e.profile.as_ref()?, e.metadata.as_ref()?);
                    Some(metadata.interpolate(profile, path))
                }),
                source.name = e.metadata.as_ref().map(|m| &*m.name),
                source.source = e.metadata.as_ref().and_then(|m| Some(m.source.as_ref()?.to_string())),
            };

            span.in_scope(|| e.kind.trace());
        }
    }
}

impl Traceable for Header<'_> {
    fn trace(&self) {
        info!(name: "header", name = self.name().as_str(), value = self.value());
    }
}
