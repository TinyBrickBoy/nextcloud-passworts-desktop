//! Schnellentsperrung: PIN (lokal verschluesselt) oder Biometrie (System Schluesselbund).

use std::fs;
use std::path::Path;

use rand::RngCore;
use zeroize::Zeroizing;

use crate::crypto::{pwhash, secretbox_open, secretbox_seal};

const PIN_FILE: &str = "pin.bin";
const ATTEMPTS_FILE: &str = "pin_attempts.json";
const MAX_ATTEMPTS: u32 = 5;
const SALT_LEN: usize = 16;
const BIOMETRIC_SERVICE: &str = "nextcloud-passwords-desktop:encryption";
#[cfg(target_os = "linux")]
const POLKIT_ACTION: &str = "de.tinybrickboy.ncpasswords.unlock";

fn read_attempts(dir: &Path) -> u32 {
    fs::read(dir.join(ATTEMPTS_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("count").and_then(|c| c.as_u64()))
        .map(|c| c.min(MAX_ATTEMPTS as u64) as u32)
        .unwrap_or(0)
}

fn write_attempts(dir: &Path, count: u32) {
    let _ = fs::write(dir.join(ATTEMPTS_FILE), serde_json::json!({ "count": count }).to_string());
}

pub fn store_pin(dir: &Path, password: &str, pin: &str) -> Result<(), String> {
    let mut salt = [0u8; SALT_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    let key = pwhash(pin.as_bytes(), &salt).map_err(|e| e.to_string())?;
    let sealed = secretbox_seal(password.as_bytes(), key.as_ref()).map_err(|e| e.to_string())?;
    let mut data = Vec::with_capacity(SALT_LEN + sealed.len());
    data.extend_from_slice(&salt);
    data.extend_from_slice(&sealed);
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    fs::write(dir.join(PIN_FILE), data).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(dir.join(ATTEMPTS_FILE));
    Ok(())
}

pub fn retrieve_pin(dir: &Path, pin: &str) -> Result<Zeroizing<String>, String> {
    let data = fs::read(dir.join(PIN_FILE)).map_err(|_| "PIN not set up".to_string())?;
    if data.len() <= SALT_LEN {
        return Err("PIN data corrupt".into());
    }
    let (salt, sealed) = data.split_at(SALT_LEN);
    let key = pwhash(pin.as_bytes(), salt).map_err(|e| e.to_string())?;
    match secretbox_open(sealed, key.as_ref()) {
        Ok(plain) => {
            let password = String::from_utf8(plain.to_vec()).map_err(|_| "PIN data corrupt".to_string())?;
            let _ = fs::remove_file(dir.join(ATTEMPTS_FILE));
            Ok(Zeroizing::new(password))
        }
        Err(_) => {
            let count = read_attempts(dir) + 1;
            if count >= MAX_ATTEMPTS {
                remove_pin(dir);
                Err("Too many wrong PIN attempts, PIN removed".into())
            } else {
                write_attempts(dir, count);
                Err(format!("Wrong PIN, {} attempts remaining", MAX_ATTEMPTS - count))
            }
        }
    }
}

pub fn remove_pin(dir: &Path) {
    let _ = fs::remove_file(dir.join(PIN_FILE));
    let _ = fs::remove_file(dir.join(ATTEMPTS_FILE));
}

pub fn pin_attempts_remaining(dir: &Path) -> u32 {
    MAX_ATTEMPTS - read_attempts(dir)
}

fn biometric_entry(server: &str, user: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(BIOMETRIC_SERVICE, &format!("{user}@{server}")).map_err(|e| e.to_string())
}

pub fn is_biometric_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        true
    }
    #[cfg(target_os = "linux")]
    {
        // Pruefe ob polkit und der Keyring erreichbar sind.
        let keyring_ok = match keyring::Entry::new(BIOMETRIC_SERVICE, "availability-check") {
            Ok(entry) => matches!(entry.get_password(), Ok(_) | Err(keyring::Error::NoEntry)),
            Err(_) => false,
        };
        if !keyring_ok {
            return false;
        }
        // Pruefe ob die polkit-Action registriert ist.
        std::process::Command::new("pkaction")
            .arg("--action-id")
            .arg(POLKIT_ACTION)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        false
    }
}

pub fn store_biometric(server: &str, user: &str, password: &str) -> Result<(), String> {
    biometric_entry(server, user)?.set_password(password).map_err(|e| e.to_string())
}

pub fn retrieve_biometric(server: &str, user: &str) -> Result<Zeroizing<String>, String> {
    #[cfg(target_os = "linux")]
    polkit_authenticate()?;
    biometric_entry(server, user)?.get_password().map(Zeroizing::new).map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn polkit_authenticate() -> Result<(), String> {
    let pid = std::process::id();
    let output = std::process::Command::new("pkcheck")
        .args([
            "--action-id", POLKIT_ACTION,
            "--process", &pid.to_string(),
            "--allow-user-interaction",
        ])
        .output()
        .map_err(|e| format!("polkit: {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err("Biometric authentication failed".into())
    }
}

pub fn remove_biometric(server: &str, user: &str) {
    if let Ok(entry) = biometric_entry(server, user) {
        let _ = entry.delete_credential();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        store_pin(dir.path(), "my-encryption-password", "123456").unwrap();
        let retrieved = retrieve_pin(dir.path(), "123456").unwrap();
        assert_eq!(retrieved.as_str(), "my-encryption-password");
    }

    #[test]
    fn pin_wrong_pin_fails() {
        let dir = tempfile::tempdir().unwrap();
        store_pin(dir.path(), "my-encryption-password", "123456").unwrap();
        assert!(retrieve_pin(dir.path(), "654321").is_err());
        assert_eq!(pin_attempts_remaining(dir.path()), 4);
    }

    #[test]
    fn pin_lockout_after_five_failures() {
        let dir = tempfile::tempdir().unwrap();
        store_pin(dir.path(), "secret", "111111").unwrap();
        for _ in 0..5 {
            let _ = retrieve_pin(dir.path(), "000000");
        }
        assert!(!dir.path().join("pin.bin").exists());
        assert!(!dir.path().join("pin_attempts.json").exists());
    }

    #[test]
    fn pin_resets_counter_on_success() {
        let dir = tempfile::tempdir().unwrap();
        store_pin(dir.path(), "secret", "111111").unwrap();
        let _ = retrieve_pin(dir.path(), "000000");
        let _ = retrieve_pin(dir.path(), "000000");
        assert_eq!(pin_attempts_remaining(dir.path()), 3);
        retrieve_pin(dir.path(), "111111").unwrap();
        assert_eq!(pin_attempts_remaining(dir.path()), 5);
    }
}
