#[macro_use]
extern crate rocket;

use rocket::{Config, State};
use rocket::fairing::AdHoc;

#[cfg(test)] mod tests;

#[get("/encrypt/<msg>")]
fn encrypt_endpoint(msg: &str, config: &State<Config>) -> String{
    let secret_key = config.secret_key.clone();
    let encrypted = secret_key.encrypt(msg).unwrap();

    info!("received message for encrypt: '{}'", msg);
    info!("encrypted msg: '{}'", encrypted);

    encrypted
}

#[get("/decrypt/<msg>")]
    fn decrypt_endpoint(msg: &str, config: &State<Config>) -> String {
        let secret_key = config.secret_key.clone();
        let decrypted = secret_key.decrypt(msg).unwrap();

        info!("received message for decrypt: '{}'", msg);
        info!("decrypted msg: '{}'", decrypted);

    decrypted
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![encrypt_endpoint, decrypt_endpoint])
        .attach(AdHoc::config::<Config>())
}
