use rocket::form::Form;
use rocket::fairing::{AdHoc, Fairing, Info, Kind};
use rocket::figment::providers::Serialized;
use rocket::futures::Race;
use rocket::{Data, Orbit, Request, Rocket};
use rocket::tokio::{spawn, time::sleep};
use rocket::yansi::Paint;

use crate::{Config, Session, Token, Tokenizer};

struct TokenizerFairing {
    config: Config,
    tokenizer: Tokenizer,
}

impl TokenizerFairing {
    const FORM_FIELD: &'static str = "_authenticity_token";

    const HEADER: &'static str = "X-CSRF-Token";

    fn new(config: Config) -> Option<Self> {
        Some(Self { config, tokenizer: Tokenizer::new() })
    }
}

impl Tokenizer {
    pub fn fairing() -> impl Fairing {
        AdHoc::try_on_ignite("CSRF Protection Configuration", |rocket| async {
            let config = rocket.figment()
                .clone()
                .join(Serialized::default("csrf", Config::default()))
                .extract_inner::<Config>("csrf");

            match config {
                Ok(config) if config.enable => match TokenizerFairing::new(config) {
                    Some(fairing) => Ok(rocket.attach(fairing)),
                    None => {
                        error!("{}CSRF protection failed to initialize.", "🔐 ".mask());
                        Err(rocket)
                    }
                },
                Ok(_) => Ok(rocket),
                Err(e) => {
                    let kind = rocket::error::ErrorKind::Config(e);
                    rocket::Error::from(kind).pretty_print();
                    Err(rocket)
                },
            }
        })
    }
}

#[rocket::async_trait]
impl Fairing for TokenizerFairing {
    fn info(&self) -> Info {
        Info {
            name: "Tokenizer",
            kind: Kind::Singleton | Kind::Liftoff | Kind::Request | Kind::Response
        }
    }

    async fn on_liftoff(&self, rocket: &Rocket<Orbit>) {
        let rotate = self.config.rotate;
        info!("{}{}", "🔐 ".mask(), "CSRF Protection:".magenta());
        info_!("status: {}", "enabled".green());
        info_!("rotation: {}/{}", rotate.period, rotate.window);

        let tokenizer = self.tokenizer.clone();
        spawn(rocket.shutdown().race(async move {
            loop {
                sleep(rotate.epoch()).await;
                tokenizer.rotate();
                info!("{}{}", "🔐 ".mask(), "CSRF Protection: keys sliding.");

                sleep(rotate.window()).await;
                tokenizer.rotate();
                info!("{}{}", "🔐 ".mask(), "CSRF Protection: keys rotated.");
            }
        }));
    }

    async fn on_request(&self, req: &mut Request<'_>, data: &mut Data<'_>) {
        let session = Session::fetch(req);
        let gen_token = self.tokenizer.form_token(session.id());
        dbg!(&session, &gen_token, gen_token.to_string());

        if !req.method().supports_payload() {
            return;
        }

        let token = match req.content_type() {
            Some(mime) if mime.is_form() => {
                std::str::from_utf8(data.peek(192).await).ok()
                    .into_iter()
                    .flat_map(Form::values)
                    .find(|field| field.name == Self::FORM_FIELD)
                    .and_then(|field| field.value.parse::<Token>().ok())
            },
            // TODO: Fix _method resolution for form data in Rocket proper.
            Some(mime) if mime.is_form_data() => {
                let token = async {
                    let data = data.peek(512).await;
                    let boundary = mime.param("boundary")?;
                    let mut form = multer::Multipart::with_reader(data, boundary);
                    while let Ok(Some(field)) = form.next_field().await {
                        if field.name() == Some(Self::FORM_FIELD) {
                            return field.text().await.ok()?.parse().ok();
                        }
                    }

                    None
                };

                token.await
            },
            _ => req.headers().get_one(Self::HEADER).and_then(|s| s.parse().ok()),
        };

        // FIXME: Check token context matches the expectation too.
        if !dbg!(token.as_ref()).map_or(false, |token| self.tokenizer.validate(token, &session)) {
            match token {
                Some(_) => error_!("{}{}", "🔐 ".mask(), "CSRF Protection: invalid token."),
                None => error_!("{}{}", "🔐 ".mask(), "CSRF Protection: missing token."),
            }

            req.set_uri(uri!("/__rocket/csrf/denied"));
        }
    }
}
