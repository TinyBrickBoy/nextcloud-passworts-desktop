//! Prüft die Krypto gegen Vektoren, die mit libsodium-wrappers-sumo erzeugt wurden.
//! Die Werte stammen aus Zufallsdaten und einem Testpasswort, es sind keine echten Zugangsdaten.

use ncpw_lib::crypto::{solve_pwdv1, Keychain};
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!("fixtures/libsodium_vectors.json")).unwrap()
}

#[test]
fn pwdv1_challenge_matches_libsodium() {
    let v = vectors();
    let salts: Vec<String> = serde_json::from_value(v["salts"].clone()).unwrap();
    let secret = solve_pwdv1(v["masterInput"].as_str().unwrap(), &salts).unwrap();
    assert_eq!(secret, v["challengeResult"].as_str().unwrap());
}

#[test]
fn keychain_and_field_decrypt() {
    let v = vectors();
    let keychain = Keychain::decrypt(v["keychain"].as_str().unwrap(), v["masterInput"].as_str().unwrap()).unwrap();
    let key_id = keychain.current_id().to_string();
    let plain = keychain.decrypt_field(&key_id, v["field"].as_str().unwrap()).unwrap();
    assert_eq!(plain, v["fieldPlain"].as_str().unwrap());
}

#[test]
fn field_roundtrip() {
    let v = vectors();
    let keychain = Keychain::decrypt(v["keychain"].as_str().unwrap(), v["masterInput"].as_str().unwrap()).unwrap();
    let key_id = keychain.current_id().to_string();
    let encrypted = keychain.encrypt_field("geheim 123").unwrap();
    assert_eq!(keychain.decrypt_field(&key_id, &encrypted).unwrap(), "geheim 123");
    assert_eq!(keychain.encrypt_field("").unwrap(), "");
}

#[test]
fn wrong_password_fails() {
    let v = vectors();
    assert!(Keychain::decrypt(v["keychain"].as_str().unwrap(), "falsches passwort 123").is_err());
    assert!(solve_pwdv1("zu kurz", &[]).is_err());
}
