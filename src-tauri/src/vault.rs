//! Entschlüsselter Tresor im Speicher und Umwandlung zwischen API und Oberfläche.
//!
//! Passwörter und geheime Zusatzfelder verlassen das Backend nur auf ausdrücklichen Wunsch
//! (Anzeigen), zum Kopieren gehen sie direkt in die Zwischenablage.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::api::{Api, RawFolder, RawPassword, BASE_FOLDER};
use crate::crypto::{self, Keychain, CSE_TYPE};

pub struct Entry {
    pub id: String,
    pub revision: String,
    pub label: String,
    pub username: String,
    pub password: Zeroizing<String>,
    pub url: String,
    pub notes: String,
    pub custom_fields: Zeroizing<String>,
    pub folder: String,
    pub favorite: bool,
    pub editable: bool,
    pub status: i64,
    pub edited: i64,
}

#[derive(Serialize, Clone)]
pub struct Folder {
    pub id: String,
    pub label: String,
    pub parent: String,
}

pub struct Vault {
    pub api: Arc<Api>,
    pub keychain: Option<Keychain>,
    pub entries: Vec<Entry>,
    pub folders: Vec<Folder>,
    /// Anzahl der Einträge, die nicht entschlüsselt werden konnten.
    pub broken: usize,
}

#[derive(Deserialize, Serialize)]
struct RawCustomField {
    #[serde(default)]
    label: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    value: String,
}

#[derive(Serialize)]
pub struct CustomFieldView {
    pub index: usize,
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// Leer bei geheimen Feldern.
    pub value: String,
}

#[derive(Serialize)]
pub struct EntryView {
    pub id: String,
    pub label: String,
    pub username: String,
    pub url: String,
    pub notes: String,
    pub folder: String,
    pub favorite: bool,
    pub editable: bool,
    pub status: i64,
    pub edited: i64,
    pub fields: Vec<CustomFieldView>,
}

#[derive(Serialize)]
pub struct VaultView {
    pub entries: Vec<EntryView>,
    pub folders: Vec<Folder>,
    pub broken: usize,
    pub encrypted: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveInput {
    pub id: Option<String>,
    pub label: String,
    pub username: String,
    /// `None` lässt das bestehende Passwort unverändert.
    pub password: Option<String>,
    pub url: String,
    pub notes: String,
    pub folder: Option<String>,
    pub favorite: bool,
}

fn custom_fields(raw: &str) -> Vec<RawCustomField> {
    serde_json::from_str(raw).unwrap_or_default()
}

impl Entry {
    fn view(&self) -> EntryView {
        let fields = custom_fields(&self.custom_fields)
            .into_iter()
            .enumerate()
            .filter(|(_, f)| f.kind != "data")
            .map(|(index, f)| CustomFieldView {
                index,
                value: if f.kind == "secret" { String::new() } else { f.value },
                label: f.label,
                kind: f.kind,
            })
            .collect();
        EntryView {
            id: self.id.clone(),
            label: self.label.clone(),
            username: self.username.clone(),
            url: self.url.clone(),
            notes: self.notes.clone(),
            folder: self.folder.clone(),
            favorite: self.favorite,
            editable: self.editable,
            status: self.status,
            edited: self.edited,
            fields,
        }
    }

    pub fn custom_value(&self, index: usize) -> Option<String> {
        custom_fields(&self.custom_fields).into_iter().nth(index).map(|f| f.value)
    }
}

fn decrypt_password(raw: RawPassword, keychain: Option<&Keychain>) -> Result<Entry, crypto::CryptoError> {
    let mut raw = raw;
    if raw.cse_type == CSE_TYPE {
        let keychain = keychain.ok_or_else(|| crypto::CryptoError::MissingKey(raw.cse_key.clone()))?;
        let key = raw.cse_key.as_str();
        raw.label = keychain.decrypt_field(key, &raw.label)?;
        raw.username = keychain.decrypt_field(key, &raw.username)?;
        raw.password = keychain.decrypt_field(key, &raw.password)?;
        raw.url = keychain.decrypt_field(key, &raw.url)?;
        raw.notes = keychain.decrypt_field(key, &raw.notes)?;
        raw.custom_fields = keychain.decrypt_field(key, &raw.custom_fields)?;
    }
    Ok(Entry {
        id: raw.id,
        revision: raw.revision,
        label: raw.label,
        username: raw.username,
        password: Zeroizing::new(raw.password),
        url: raw.url,
        notes: raw.notes,
        custom_fields: Zeroizing::new(raw.custom_fields),
        folder: raw.folder,
        favorite: raw.favorite,
        editable: raw.editable,
        status: raw.status,
        edited: raw.edited,
    })
}

fn decrypt_folder(raw: RawFolder, keychain: Option<&Keychain>) -> Result<Folder, crypto::CryptoError> {
    let label = if raw.cse_type == CSE_TYPE {
        let keychain = keychain.ok_or_else(|| crypto::CryptoError::MissingKey(raw.cse_key.clone()))?;
        keychain.decrypt_field(&raw.cse_key, &raw.label)?
    } else {
        raw.label
    };
    Ok(Folder { id: raw.id, label, parent: raw.parent })
}

impl Vault {
    pub fn new(api: Arc<Api>, keychain: Option<Keychain>) -> Self {
        Self { api, keychain, entries: Vec::new(), folders: Vec::new(), broken: 0 }
    }

    pub async fn refresh(&mut self) -> Result<(), String> {
        let (passwords, folders) = tokio::try_join!(self.api.passwords(), self.api.folders()).map_err(|e| e.to_string())?;
        let keychain = self.keychain.as_ref();

        let mut broken = 0;
        let mut entries: Vec<Entry> = passwords
            .into_iter()
            .filter_map(|raw| decrypt_password(raw, keychain).map_err(|_| broken += 1).ok())
            .collect();
        entries.sort_by_key(|e| e.label.to_lowercase());

        let mut folders: Vec<Folder> = folders
            .into_iter()
            .filter(|f| f.id != BASE_FOLDER)
            .filter_map(|raw| decrypt_folder(raw, keychain).ok())
            .collect();
        folders.sort_by_key(|f| f.label.to_lowercase());

        self.entries = entries;
        self.folders = folders;
        self.broken = broken;
        Ok(())
    }

    pub fn view(&self) -> VaultView {
        VaultView {
            entries: self.entries.iter().map(Entry::view).collect(),
            folders: self.folders.clone(),
            broken: self.broken,
            encrypted: self.keychain.is_some(),
        }
    }

    pub fn entry(&self, id: &str) -> Result<&Entry, String> {
        self.entries.iter().find(|e| e.id == id).ok_or_else(|| "Eintrag nicht gefunden".to_string())
    }

    pub async fn save(&mut self, input: SaveInput) -> Result<String, String> {
        let existing = match &input.id {
            Some(id) => Some(self.entry(id)?),
            None => None,
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let password = match (&input.password, existing) {
            (Some(p), _) => Zeroizing::new(p.clone()),
            (None, Some(e)) => e.password.clone(),
            (None, None) => return Err("Passwort fehlt".into()),
        };
        if input.label.trim().is_empty() {
            return Err("Name fehlt".into());
        }
        let password_changed = existing.map(|e| *e.password != *password).unwrap_or(true);
        let edited = match existing {
            Some(e) if !password_changed => e.edited,
            _ => now,
        };
        let custom = existing.map(|e| e.custom_fields.clone()).unwrap_or_default();

        let mut body = json!({
            "label": input.label.trim(),
            "username": input.username,
            "password": password.as_str(),
            "url": input.url.trim(),
            "notes": input.notes,
            "customFields": custom.as_str(),
            "hash": crypto::sha1_hex(&password),
            "folder": input.folder.clone().unwrap_or_else(|| BASE_FOLDER.to_string()),
            "favorite": input.favorite,
            "edited": edited,
            "cseType": "none",
        });

        if let Some(keychain) = &self.keychain {
            for field in ["label", "username", "password", "url", "notes", "customFields"] {
                let plain = body[field].as_str().unwrap_or_default().to_string();
                body[field] = Value::String(keychain.encrypt_field(&plain).map_err(|e| e.to_string())?);
            }
            body["cseType"] = json!(CSE_TYPE);
            body["cseKey"] = json!(keychain.current_id());
        }

        let saved = match existing {
            Some(e) => {
                body["id"] = json!(e.id);
                body["revision"] = json!(e.revision);
                self.api.update_password(body).await
            }
            None => self.api.create_password(body).await,
        }
        .map_err(|e| e.to_string())?;

        self.refresh().await?;
        Ok(saved.id)
    }

    pub async fn trash(&mut self, id: &str) -> Result<(), String> {
        let revision = self.entry(id)?.revision.clone();
        self.api.delete_password(id, &revision).await.map_err(|e| e.to_string())?;
        self.refresh().await
    }
}
