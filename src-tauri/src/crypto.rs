//! Kryptografie der Nextcloud Passwords App (PWDv1 Challenge, CSEv1 Keychain, CSEv1 Verschlüsselung).
//!
//! Die App nutzt libsodium. Hier werden die gleichen Primitive mit reinen Rust Crates nachgebildet:
//! - `crypto_generichash`  → BLAKE2b mit Schlüssel (64 Byte Ausgabe)
//! - `crypto_pwhash`       → Argon2id v1.3 mit den INTERACTIVE Parametern
//! - `crypto_secretbox`    → XSalsa20Poly1305 (Tag vor dem Ciphertext, wie bei NaCl)
//!
//! Die Kompatibilität wird in `tests/crypto_vectors.rs` gegen Vektoren aus libsodium geprüft.

use std::collections::HashMap;

use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use blake2::digest::{FixedOutput, Update};
use blake2::Blake2bMac512;
use crypto_secretbox::aead::{Aead, KeyInit};
use crypto_secretbox::XSalsa20Poly1305;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const PWHASH_SALTBYTES: usize = 16;
pub const SECRETBOX_NONCEBYTES: usize = 24;
pub const SECRETBOX_KEYBYTES: usize = 32;
pub const SECRETBOX_MACBYTES: usize = 16;
const BOX_SEEDBYTES: usize = 32;
const PWHASH_OPSLIMIT_INTERACTIVE: u32 = 2;
const PWHASH_MEMLIMIT_INTERACTIVE_KIB: u32 = 64 * 1024;

pub const CSE_TYPE: &str = "CSEv1r1";

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Das Verschlüsselungspasswort muss zwischen 12 und 128 Zeichen lang sein")]
    PasswordLength,
    #[error("Ungültige Kodierung")]
    Encoding,
    #[error("Entschlüsselung fehlgeschlagen")]
    Decrypt,
    #[error("Verschlüsselung fehlgeschlagen")]
    Encrypt,
    #[error("Schlüssel {0} fehlt im Schlüsselbund")]
    MissingKey(String),
    #[error("Ungültige Challenge")]
    Challenge,
}

fn check_password(password: &str) -> Result<(), CryptoError> {
    // libsodium-wrappers prüft `string.length`, also UTF-16 Codeeinheiten.
    let len = password.encode_utf16().count();
    if !(12..=128).contains(&len) {
        return Err(CryptoError::PasswordLength);
    }
    Ok(())
}

/// Hex dekodieren, bei Fehlschlag Base64 (Format vor Passwords 2020.2.0).
fn decode(value: &str) -> Result<Vec<u8>, CryptoError> {
    if let Ok(bytes) = hex::decode(value) {
        return Ok(bytes);
    }
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(value))
        .map_err(|_| CryptoError::Encoding)
}

fn pwhash(password: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; BOX_SEEDBYTES]>, CryptoError> {
    let params = Params::new(
        PWHASH_MEMLIMIT_INTERACTIVE_KIB,
        PWHASH_OPSLIMIT_INTERACTIVE,
        1,
        Some(BOX_SEEDBYTES),
    )
    .map_err(|_| CryptoError::Challenge)?;
    let mut out = Zeroizing::new([0u8; BOX_SEEDBYTES]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password, salt, out.as_mut())
        .map_err(|_| CryptoError::Challenge)?;
    Ok(out)
}

fn secretbox_open(data: &[u8], key: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    if data.len() < SECRETBOX_NONCEBYTES + SECRETBOX_MACBYTES || key.len() != SECRETBOX_KEYBYTES {
        return Err(CryptoError::Decrypt);
    }
    let (nonce, cipher) = data.split_at(SECRETBOX_NONCEBYTES);
    XSalsa20Poly1305::new(key.into())
        .decrypt(nonce.into(), cipher)
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Decrypt)
}

/// Liefert `nonce + secretbox(message)`.
fn secretbox_seal(message: &[u8], key: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if key.len() != SECRETBOX_KEYBYTES {
        return Err(CryptoError::Encrypt);
    }
    let mut nonce = [0u8; SECRETBOX_NONCEBYTES];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let cipher = XSalsa20Poly1305::new(key.into())
        .encrypt((&nonce).into(), message)
        .map_err(|_| CryptoError::Encrypt)?;
    let mut out = Vec::with_capacity(nonce.len() + cipher.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&cipher);
    Ok(out)
}

/// Löst die PWDv1 Challenge und gibt das Ergebnis hex kodiert zurück.
pub fn solve_pwdv1(password: &str, salts: &[String]) -> Result<String, CryptoError> {
    check_password(password)?;
    let [password_salt, hash_key, hash_salt] = salts else {
        return Err(CryptoError::Challenge);
    };
    let password_salt = hex::decode(password_salt).map_err(|_| CryptoError::Encoding)?;
    let hash_key = hex::decode(hash_key).map_err(|_| CryptoError::Encoding)?;
    let hash_salt = hex::decode(hash_salt).map_err(|_| CryptoError::Encoding)?;

    let mut mac = Blake2bMac512::new_with_salt_and_personal(&hash_key, &[], &[])
        .map_err(|_| CryptoError::Challenge)?;
    mac.update(password.as_bytes());
    mac.update(&password_salt);
    let generic_hash = Zeroizing::new(mac.finalize_fixed());

    let secret = pwhash(&generic_hash, &hash_salt)?;
    Ok(hex::encode(secret.as_ref()))
}

#[derive(Deserialize, Serialize)]
struct KeychainJson {
    keys: HashMap<String, String>,
    current: String,
}

/// Entschlüsselter CSEv1 Schlüsselbund.
pub struct Keychain {
    keys: HashMap<String, Zeroizing<Vec<u8>>>,
    current: String,
}

impl Keychain {
    pub fn decrypt(encrypted: &str, password: &str) -> Result<Self, CryptoError> {
        check_password(password)?;
        let raw = decode(encrypted)?;
        if raw.len() < PWHASH_SALTBYTES {
            return Err(CryptoError::Decrypt);
        }
        let (salt, payload) = raw.split_at(PWHASH_SALTBYTES);
        let key = pwhash(password.as_bytes(), salt)?;
        let json = secretbox_open(payload, key.as_ref())?;
        let parsed: KeychainJson = serde_json::from_slice(&json).map_err(|_| CryptoError::Decrypt)?;

        let mut keys = HashMap::new();
        for (id, value) in parsed.keys {
            let value = Zeroizing::new(value);
            keys.insert(id, Zeroizing::new(hex::decode(value.as_str()).map_err(|_| CryptoError::Encoding)?));
        }
        if !keys.contains_key(&parsed.current) {
            return Err(CryptoError::MissingKey(parsed.current));
        }
        Ok(Self { keys, current: parsed.current })
    }

    pub fn current_id(&self) -> &str {
        &self.current
    }

    pub fn decrypt_field(&self, key_id: &str, value: &str) -> Result<String, CryptoError> {
        if value.is_empty() {
            return Ok(String::new());
        }
        let key = self.keys.get(key_id).ok_or_else(|| CryptoError::MissingKey(key_id.to_string()))?;
        let plain = secretbox_open(&decode(value)?, key)?;
        String::from_utf8(plain.to_vec()).map_err(|_| CryptoError::Decrypt)
    }

    pub fn encrypt_field(&self, value: &str) -> Result<String, CryptoError> {
        if value.is_empty() {
            return Ok(String::new());
        }
        let key = &self.keys[&self.current];
        Ok(hex::encode(secretbox_seal(value.as_bytes(), key)?))
    }
}

/// SHA1 des Passworts, die App nutzt ihn für die Sicherheitsprüfung.
pub fn sha1_hex(value: &str) -> String {
    use sha1::{Digest, Sha1};
    hex::encode(Sha1::digest(value.as_bytes()))
}
