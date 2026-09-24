//! Entschlüsselter Tresor im Speicher und Umwandlung zwischen API und Oberfläche.
//!
//! Passwörter und geheime Zusatzfelder verlassen das Backend nur auf ausdrücklichen Wunsch
//! (Anzeigen), zum Kopieren gehen sie direkt in die Zwischenablage.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::api::{Api, RawFolder, RawPassword, RawTag, BASE_FOLDER};
use crate::crypto::{self, Keychain, CSE_TYPE};
use crate::i18n::t;

/// Laut API darf ein Passwort höchstens 20 Zusatzfelder haben.
const MAX_CUSTOM_FIELDS: usize = 20;

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
    pub tags: Vec<String>,
    pub favorite: bool,
    pub editable: bool,
    pub shared: bool,
    pub status: i64,
    pub edited: i64,
    pub created: i64,
}

#[derive(Serialize, Clone)]
pub struct Folder {
    pub id: String,
    #[serde(skip)]
    pub revision: String,
    pub label: String,
    pub parent: String,
}

#[derive(Serialize, Clone)]
pub struct Tag {
    pub id: String,
    #[serde(skip)]
    pub revision: String,
    pub label: String,
    pub color: String,
}

pub struct Vault {
    pub api: Arc<Api>,
    pub keychain: Option<Keychain>,
    /// Sperren ist nur sinnvoll, wenn zum Entsperren eine Eingabe nötig ist.
    pub lockable: bool,
    pub entries: Vec<Entry>,
    pub trash: Vec<Entry>,
    pub folders: Vec<Folder>,
    pub tags: Vec<Tag>,
    /// Anzahl der Einträge, die nicht entschlüsselt werden konnten.
    pub broken: usize,
}

#[derive(Deserialize, Serialize, Clone)]
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
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub id: String,
    pub label: String,
    pub username: String,
    pub url: String,
    pub notes: String,
    pub folder: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub editable: bool,
    pub shared: bool,
    pub status: i64,
    pub edited: i64,
    pub created: i64,
    pub has_password: bool,
    pub fields: Vec<CustomFieldView>,
}

#[derive(Serialize)]
pub struct VaultView {
    pub entries: Vec<EntryView>,
    pub trash: Vec<EntryView>,
    pub folders: Vec<Folder>,
    pub tags: Vec<Tag>,
    pub broken: usize,
    pub encrypted: bool,
    pub lockable: bool,
}

#[derive(Deserialize)]
pub struct FieldInput {
    pub label: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// `None` bei unveränderten geheimen Feldern, dann gilt `index`.
    pub value: Option<String>,
    pub index: Option<usize>,
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
    #[serde(default)]
    pub tags: Vec<String>,
    /// `None` lässt die Zusatzfelder unverändert.
    pub fields: Option<Vec<FieldInput>>,
}

fn custom_fields(raw: &str) -> Vec<RawCustomField> {
    serde_json::from_str(raw).unwrap_or_default()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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
            tags: self.tags.clone(),
            favorite: self.favorite,
            editable: self.editable,
            shared: self.shared,
            status: self.status,
            edited: self.edited,
            created: self.created,
            has_password: !self.password.is_empty(),
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
        tags: raw.tags,
        favorite: raw.favorite,
        editable: raw.editable,
        shared: raw.shared,
        status: raw.status,
        edited: raw.edited,
        created: raw.created,
    })
}

fn decrypt_folder(raw: RawFolder, keychain: Option<&Keychain>) -> Result<Folder, crypto::CryptoError> {
    let label = if raw.cse_type == CSE_TYPE {
        let keychain = keychain.ok_or_else(|| crypto::CryptoError::MissingKey(raw.cse_key.clone()))?;
        keychain.decrypt_field(&raw.cse_key, &raw.label)?
    } else {
        raw.label
    };
    Ok(Folder { id: raw.id, revision: raw.revision, label, parent: raw.parent })
}

fn decrypt_tag(raw: RawTag, keychain: Option<&Keychain>) -> Result<Tag, crypto::CryptoError> {
    let (label, color) = if raw.cse_type == CSE_TYPE {
        let keychain = keychain.ok_or_else(|| crypto::CryptoError::MissingKey(raw.cse_key.clone()))?;
        (keychain.decrypt_field(&raw.cse_key, &raw.label)?, keychain.decrypt_field(&raw.cse_key, &raw.color)?)
    } else {
        (raw.label, raw.color)
    };
    Ok(Tag { id: raw.id, revision: raw.revision, label, color })
}

fn sort_entries(entries: &mut [Entry]) {
    entries.sort_by_key(|e| e.label.to_lowercase());
}

impl Vault {
    pub fn new(api: Arc<Api>, keychain: Option<Keychain>, lockable: bool) -> Self {
        Self {
            api,
            keychain,
            lockable,
            entries: Vec::new(),
            trash: Vec::new(),
            folders: Vec::new(),
            tags: Vec::new(),
            broken: 0,
        }
    }

    pub async fn refresh(&mut self) -> Result<(), String> {
        let (passwords, trashed, folders, tags) = tokio::try_join!(
            self.api.passwords(),
            self.api.trashed_passwords(),
            self.api.folders(),
            self.api.tags()
        )
        .map_err(|e| e.to_string())?;
        let keychain = self.keychain.as_ref();

        let mut broken = 0;
        let mut decrypt_all = |list: Vec<RawPassword>| -> Vec<Entry> {
            let mut entries: Vec<Entry> = list
                .into_iter()
                .filter_map(|raw| decrypt_password(raw, keychain).map_err(|_| broken += 1).ok())
                .collect();
            sort_entries(&mut entries);
            entries
        };
        let entries = decrypt_all(passwords);
        let trash = decrypt_all(trashed);

        let mut folders: Vec<Folder> = folders
            .into_iter()
            .filter(|f| f.id != BASE_FOLDER)
            .filter_map(|raw| decrypt_folder(raw, keychain).ok())
            .collect();
        folders.sort_by_key(|f| f.label.to_lowercase());

        let mut tags: Vec<Tag> = tags.into_iter().filter_map(|raw| decrypt_tag(raw, keychain).ok()).collect();
        tags.sort_by_key(|t| t.label.to_lowercase());

        self.entries = entries;
        self.trash = trash;
        self.folders = folders;
        self.tags = tags;
        self.broken = broken;
        Ok(())
    }

    pub fn view(&self) -> VaultView {
        VaultView {
            entries: self.entries.iter().map(Entry::view).collect(),
            trash: self.trash.iter().map(Entry::view).collect(),
            folders: self.folders.clone(),
            tags: self.tags.clone(),
            broken: self.broken,
            encrypted: self.keychain.is_some(),
            lockable: self.lockable,
        }
    }

    /// Sucht in den aktiven Einträgen und im Papierkorb.
    pub fn entry(&self, id: &str) -> Result<&Entry, String> {
        self.entries
            .iter()
            .chain(self.trash.iter())
            .find(|e| e.id == id)
            .ok_or_else(|| t("entry_not_found").to_string())
    }

    fn encrypt_body(&self, body: &mut Value, fields: &[&str]) -> Result<(), String> {
        match &self.keychain {
            Some(keychain) => {
                for field in fields {
                    let plain = body[*field].as_str().unwrap_or_default().to_string();
                    body[*field] = Value::String(keychain.encrypt_field(&plain).map_err(|e| e.to_string())?);
                }
                body["cseType"] = json!(CSE_TYPE);
                body["cseKey"] = json!(keychain.current_id());
            }
            None => body["cseType"] = json!("none"),
        }
        Ok(())
    }

    fn build_custom_fields(existing: Option<&Entry>, fields: Option<Vec<FieldInput>>) -> Zeroizing<String> {
        let old = existing.map(|e| custom_fields(&e.custom_fields)).unwrap_or_default();
        let Some(fields) = fields else {
            return existing.map(|e| e.custom_fields.clone()).unwrap_or_default();
        };
        let mut out: Vec<RawCustomField> = fields
            .into_iter()
            .filter(|f| !f.label.trim().is_empty())
            .filter_map(|f| {
                let value = match f.value {
                    Some(value) => value,
                    None => old.get(f.index?)?.value.clone(),
                };
                let kind = match f.kind.as_str() {
                    "text" | "secret" | "email" | "url" | "file" => f.kind,
                    _ => "text".into(),
                };
                Some(RawCustomField { label: f.label.trim().chars().take(48).collect(), kind, value })
            })
            .collect();
        // Technische Felder anderer Clients bleiben erhalten.
        out.extend(old.into_iter().filter(|f| f.kind == "data"));
        out.truncate(MAX_CUSTOM_FIELDS);
        if out.is_empty() {
            return Zeroizing::new(String::new());
        }
        Zeroizing::new(serde_json::to_string(&out).unwrap_or_default())
    }

    pub async fn save(&mut self, input: SaveInput) -> Result<String, String> {
        let existing = match &input.id {
            Some(id) => Some(self.entry(id)?),
            None => None,
        };

        let password = match (&input.password, existing) {
            (Some(p), _) => Zeroizing::new(p.clone()),
            (None, Some(e)) => e.password.clone(),
            (None, None) => return Err(t("password_missing").into()),
        };
        if input.label.trim().is_empty() {
            return Err(t("name_missing").into());
        }
        let password_changed = existing.map(|e| *e.password != *password).unwrap_or(true);
        let edited = match existing {
            Some(e) if !password_changed => e.edited,
            _ => now(),
        };
        let custom = Self::build_custom_fields(existing, input.fields);

        // Eine leere Liste ändert serverseitig nichts, eine unbekannte ID lehnt der Server ab
        // ("Tag does not exist"). Eine Liste mit einem leeren Eintrag entfernt alle Tags.
        let tags = if input.tags.is_empty() && existing.map(|e| !e.tags.is_empty()).unwrap_or(false) {
            vec![String::new()]
        } else {
            input.tags.clone()
        };

        let mut body = json!({
            "label": input.label.trim(),
            "username": input.username,
            "password": password.as_str(),
            "url": input.url.trim(),
            "notes": input.notes,
            "customFields": custom.as_str(),
            "hash": crypto::sha1_hex(&password),
            "folder": input.folder.clone().filter(|f| !f.is_empty()).unwrap_or_else(|| BASE_FOLDER.to_string()),
            "favorite": input.favorite,
            "edited": edited,
            "tags": tags,
        });
        self.encrypt_body(&mut body, &["label", "username", "password", "url", "notes", "customFields"])?;

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

    /// Setzt oder entfernt den Favoriten Stern, ohne sonst etwas zu ändern.
    pub async fn set_favorite(&mut self, id: &str, favorite: bool) -> Result<(), String> {
        let e = self.entry(id)?;
        let input = SaveInput {
            id: Some(e.id.clone()),
            label: e.label.clone(),
            username: e.username.clone(),
            password: None,
            url: e.url.clone(),
            notes: e.notes.clone(),
            folder: Some(e.folder.clone()),
            favorite,
            tags: e.tags.clone(),
            fields: None,
        };
        self.save(input).await.map(|_| ())
    }

    /// Verschiebt in den Papierkorb, oder löscht endgültig, wenn der Eintrag schon dort liegt.
    pub async fn delete(&mut self, id: &str) -> Result<(), String> {
        let revision = self.entry(id)?.revision.clone();
        self.api.delete_password(id, &revision).await.map_err(|e| e.to_string())?;
        self.refresh().await
    }

    pub async fn restore(&mut self, id: &str) -> Result<(), String> {
        self.api.restore_password(id).await.map_err(|e| e.to_string())?;
        self.refresh().await
    }

    pub async fn empty_trash(&mut self) -> Result<(), String> {
        let items: Vec<(String, String)> = self.trash.iter().map(|e| (e.id.clone(), e.revision.clone())).collect();
        for (id, revision) in items {
            self.api.delete_password(&id, &revision).await.map_err(|e| e.to_string())?;
        }
        self.refresh().await
    }

    pub async fn create_folder(&mut self, label: &str, parent: Option<String>) -> Result<String, String> {
        if label.trim().is_empty() {
            return Err(t("name_missing").into());
        }
        let mut body = json!({
            "label": label.trim().chars().take(48).collect::<String>(),
            "parent": parent.filter(|p| !p.is_empty()).unwrap_or_else(|| BASE_FOLDER.to_string()),
        });
        self.encrypt_body(&mut body, &["label"])?;
        let saved = self.api.create_folder(body).await.map_err(|e| e.to_string())?;
        self.refresh().await?;
        Ok(saved.id)
    }

    pub async fn rename_folder(&mut self, id: &str, label: &str) -> Result<(), String> {
        if label.trim().is_empty() {
            return Err(t("name_missing").into());
        }
        let folder = self.folders.iter().find(|f| f.id == id).ok_or_else(|| t("folder_not_found").to_string())?;
        let mut body = json!({
            "id": folder.id,
            "revision": folder.revision,
            "label": label.trim().chars().take(48).collect::<String>(),
            "parent": if folder.parent.is_empty() { BASE_FOLDER.to_string() } else { folder.parent.clone() },
        });
        self.encrypt_body(&mut body, &["label"])?;
        self.api.update_folder(body).await.map_err(|e| e.to_string())?;
        self.refresh().await
    }

    pub async fn delete_folder(&mut self, id: &str) -> Result<(), String> {
        let folder = self.folders.iter().find(|f| f.id == id).ok_or_else(|| t("folder_not_found").to_string())?;
        let revision = folder.revision.clone();
        self.api.delete_folder(id, &revision).await.map_err(|e| e.to_string())?;
        self.refresh().await
    }

    pub async fn create_tag(&mut self, label: &str, color: &str) -> Result<Tag, String> {
        if label.trim().is_empty() {
            return Err(t("name_missing").into());
        }
        let label: String = label.trim().chars().take(48).collect();
        let mut body = json!({ "label": label, "color": color });
        self.encrypt_body(&mut body, &["label", "color"])?;
        let saved = self.api.create_tag(body).await.map_err(|e| e.to_string())?;
        self.refresh().await?;
        self.tags
            .iter()
            .find(|t| t.id == saved.id)
            .cloned()
            .ok_or_else(|| t("entry_not_found").to_string())
    }

    pub async fn delete_tag(&mut self, id: &str) -> Result<(), String> {
        let revision = self.tags.iter().find(|t| t.id == id).map(|t| t.revision.clone()).unwrap_or_default();
        self.api.delete_tag(id, &revision).await.map_err(|e| e.to_string())?;
        self.refresh().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(custom: &str) -> Entry {
        Entry {
            id: "1".into(),
            revision: "r".into(),
            label: "x".into(),
            username: String::new(),
            password: Zeroizing::new("pw".into()),
            url: String::new(),
            notes: String::new(),
            custom_fields: Zeroizing::new(custom.into()),
            folder: String::new(),
            tags: vec![],
            favorite: false,
            editable: true,
            shared: false,
            status: 0,
            edited: 0,
            created: 0,
        }
    }

    #[test]
    fn custom_fields_keep_secret_and_data_fields() {
        let old = entry(r#"[{"label":"PIN","type":"secret","value":"1234"},{"label":"x","type":"data","value":"intern"}]"#);
        let fields = vec![
            FieldInput { label: "PIN".into(), kind: "secret".into(), value: None, index: Some(0) },
            FieldInput { label: "Mail".into(), kind: "email".into(), value: Some("a@b.c".into()), index: None },
            FieldInput { label: "  ".into(), kind: "text".into(), value: Some("leer".into()), index: None },
        ];
        let json = Vault::build_custom_fields(Some(&old), Some(fields));
        let parsed = custom_fields(&json);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].value, "1234");
        assert_eq!(parsed[1].kind, "email");
        assert_eq!(parsed[2].kind, "data");
    }

    #[test]
    fn custom_fields_unchanged_when_none() {
        let old = entry(r#"[{"label":"a","type":"text","value":"b"}]"#);
        assert_eq!(Vault::build_custom_fields(Some(&old), None).as_str(), old.custom_fields.as_str());
        assert_eq!(Vault::build_custom_fields(None, Some(vec![])).as_str(), "");
    }
}
