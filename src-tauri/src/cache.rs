//! Verschluesselter Offline-Cache: `salt(16) || nonce || secretbox(json)`.

use std::collections::HashMap;
use std::path::Path;

use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::api::{RawFolder, RawPassword, RawTag};
use crate::crypto::{pwhash, secretbox_open, secretbox_seal, PWHASH_SALTBYTES};

const CACHE_FILE: &str = "cache.bin";

#[derive(Serialize, Deserialize)]
pub struct CachePayload {
    pub version: u32,
    pub timestamp: i64,
    pub needs_password: bool,
    pub keychains: HashMap<String, String>,
    pub passwords: Vec<RawPassword>,
    pub trashed: Vec<RawPassword>,
    pub folders: Vec<RawFolder>,
    pub tags: Vec<RawTag>,
}

pub fn write_cache(dir: &Path, app_password: &str, payload: &CachePayload) -> Result<(), String> {
    let json = zeroize::Zeroizing::new(serde_json::to_vec(payload).map_err(|e| e.to_string())?);
    let mut salt = [0u8; PWHASH_SALTBYTES];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    let key = pwhash(app_password.as_bytes(), &salt).map_err(|e| e.to_string())?;
    let sealed = secretbox_seal(&json, key.as_ref()).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(salt.len() + sealed.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&sealed);
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(CACHE_FILE), out).map_err(|e| e.to_string())
}

pub fn read_cache(dir: &Path, app_password: &str) -> Result<CachePayload, String> {
    let raw = std::fs::read(dir.join(CACHE_FILE)).map_err(|e| e.to_string())?;
    if raw.len() < PWHASH_SALTBYTES {
        return Err("cache file too short".into());
    }
    let (salt, sealed) = raw.split_at(PWHASH_SALTBYTES);
    let key = pwhash(app_password.as_bytes(), salt).map_err(|e| e.to_string())?;
    let plain = secretbox_open(sealed, key.as_ref()).map_err(|e| e.to_string())?;
    serde_json::from_slice(&plain).map_err(|e| e.to_string())
}

pub fn delete_cache(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(CACHE_FILE));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty(version: u32, timestamp: i64, needs_password: bool) -> CachePayload {
        CachePayload {
            version,
            timestamp,
            needs_password,
            keychains: HashMap::new(),
            passwords: vec![],
            trashed: vec![],
            folders: vec![],
            tags: vec![],
        }
    }

    #[test]
    fn cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut payload = empty(1, 1700000000, true);
        payload.keychains.insert("CSEv1r1".into(), "abc123".into());
        write_cache(dir.path(), "test-app-password-1234", &payload).unwrap();
        let loaded = read_cache(dir.path(), "test-app-password-1234").unwrap();
        assert_eq!(loaded.version, 1);
        assert_eq!(loaded.timestamp, 1700000000);
        assert!(loaded.needs_password);
        assert_eq!(loaded.keychains["CSEv1r1"], "abc123");
    }

    #[test]
    fn cache_wrong_password_fails() {
        let dir = tempfile::tempdir().unwrap();
        write_cache(dir.path(), "correct-password-12345", &empty(1, 0, false)).unwrap();
        assert!(read_cache(dir.path(), "wrong-password-123456").is_err());
    }

    #[test]
    fn cache_corrupt_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("cache.bin"), b"garbage").unwrap();
        assert!(read_cache(dir.path(), "any-password-1234567").is_err());
    }

    #[test]
    fn cache_delete_removes_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.bin");
        std::fs::write(&path, b"data").unwrap();
        delete_cache(dir.path());
        assert!(!path.exists());
    }
}
