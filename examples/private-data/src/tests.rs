use rocket::local::blocking::Client;

#[test]
fn encrypt_decrypt() {
    let client = Client::tracked(super::rocket()).unwrap();
    let msg = "some-secret-message";

    let encrypted = client.get(format!("/encrypt/{}", msg)).dispatch().into_string().unwrap();
    let decrypted = client.get(format!("/decrypt/{}", encrypted)).dispatch().into_string().unwrap();

    assert_eq!(msg, decrypted);
}
