//! Übersetzte Meldungen des Backends. Die Oberfläche setzt die Sprache per `set_settings`.

use std::sync::atomic::{AtomicU8, Ordering};

static LANG: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En = 0,
    De = 1,
}

pub fn set_lang(code: &str) {
    let lang = if code.to_lowercase().starts_with("de") { Lang::De } else { Lang::En };
    LANG.store(lang as u8, Ordering::Relaxed);
}

fn lang() -> Lang {
    if LANG.load(Ordering::Relaxed) == Lang::De as u8 {
        Lang::De
    } else {
        Lang::En
    }
}

/// Liefert den Text zum Schlüssel in der aktuellen Sprache.
pub fn t(key: &str) -> &'static str {
    let (en, de) = match key {
        "network" => ("Cannot reach the server", "Keine Verbindung zum Server"),
        "unauthorized" => ("Login rejected. Please connect again.", "Anmeldung abgelehnt. Bitte neu verbinden."),
        "invalid_response" => ("Unexpected response from the server", "Unerwartete Antwort vom Server"),
        "server_error" => ("Server error", "Serverfehler"),
        "no_account" => ("No account set up", "Kein Konto eingerichtet"),
        "locked" => ("The vault is locked", "Der Tresor ist gesperrt"),
        "login_cancelled" => ("Login cancelled", "Anmeldung abgebrochen"),
        "login_timeout" => ("Login timed out", "Zeit für die Anmeldung abgelaufen"),
        "login_failed" => ("Login not possible. Is this a Nextcloud server?", "Login nicht möglich. Ist das ein Nextcloud Server?"),
        "invalid_server" => ("Invalid server address", "Ungültige Serveradresse"),
        "https_only" => ("The Passwords API only works over HTTPS", "Die Passwords API funktioniert nur über HTTPS"),
        "keyring_unavailable" => ("System keyring not available", "Schlüsselbund des Systems nicht verfügbar"),
        "keyring_missing" => ("App password not found in the keyring", "App Passwort nicht im Schlüsselbund gefunden"),
        "keyring_save" => ("Could not save to the keyring", "Speichern im Schlüsselbund fehlgeschlagen"),
        "password_required" => ("Encryption password required", "Verschlüsselungspasswort erforderlich"),
        "unlock_failed" => ("Unlock failed", "Entsperren fehlgeschlagen"),
        "challenge_unsupported" => ("This encryption type is not supported", "Dieser Verschlüsselungstyp wird nicht unterstützt"),
        "entry_not_found" => ("Entry not found", "Eintrag nicht gefunden"),
        "field_not_found" => ("Field not found", "Feld nicht gefunden"),
        "unknown_field" => ("Unknown field", "Unbekanntes Feld"),
        "password_missing" => ("Password is missing", "Passwort fehlt"),
        "name_missing" => ("Name is missing", "Name fehlt"),
        "folder_not_found" => ("Folder not found", "Ordner nicht gefunden"),
        "crypto_length" => (
            "The encryption password must be between 12 and 128 characters long",
            "Das Verschlüsselungspasswort muss zwischen 12 und 128 Zeichen lang sein",
        ),
        "crypto_encoding" => ("Invalid encoding", "Ungültige Kodierung"),
        "crypto_decrypt" => ("Decryption failed. Is the encryption password correct?", "Entschlüsselung fehlgeschlagen. Stimmt das Verschlüsselungspasswort?"),
        "crypto_encrypt" => ("Encryption failed", "Verschlüsselung fehlgeschlagen"),
        "crypto_missing_key" => ("Key missing from keychain", "Schlüssel fehlt im Schlüsselbund"),
        "crypto_challenge" => ("Invalid challenge", "Ungültige Challenge"),
        _ => ("Unknown error", "Unbekannter Fehler"),
    };
    match lang() {
        Lang::En => en,
        Lang::De => de,
    }
}
