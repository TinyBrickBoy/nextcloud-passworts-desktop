//! HTTP Client für die Passwords API (`/index.php/apps/passwords/api/1.0`).

use std::sync::Mutex;
use std::time::Duration;

use reqwest::{header, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use zeroize::Zeroizing;

use crate::i18n::t;

pub const BASE_FOLDER: &str = "00000000-0000-0000-0000-000000000000";
const SESSION_HEADER: &str = "X-API-SESSION";

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{msg} ({detail})", msg = t("network"), detail = .0)]
    Network(String),
    #[error("{}", t("unauthorized"))]
    Unauthorized,
    #[error("{0}")]
    Server(String),
    #[error("{}", t("invalid_response"))]
    Invalid,
}

impl From<reqwest::Error> for ApiError {
    /// reqwest nennt die eigentliche Ursache (DNS, Zertifikat, Timeout …) nur in der
    /// `source()` Kette, deshalb werden alle Glieder in die Meldung übernommen.
    fn from(error: reqwest::Error) -> Self {
        let mut parts = vec![error.to_string()];
        let mut source = std::error::Error::source(&error);
        while let Some(cause) = source {
            let text = cause.to_string();
            if parts.last() != Some(&text) {
                parts.push(text);
            }
            source = cause.source();
        }
        ApiError::Network(parts.join(": "))
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    message: Option<String>,
}

pub struct Api {
    http: reqwest::Client,
    base: String,
    user: String,
    app_password: Zeroizing<String>,
    session: Mutex<Option<String>>,
}

impl Api {
    pub fn new(server: &str, user: &str, app_password: &str) -> Result<Self, ApiError> {
        Ok(Self {
            http: http_client()?,
            base: format!("{}/index.php/apps/passwords/api/1.0", server.trim_end_matches('/')),
            user: user.to_string(),
            app_password: Zeroizing::new(app_password.to_string()),
            session: Mutex::new(None),
        })
    }

    pub async fn call<T: DeserializeOwned>(&self, method: Method, path: &str, body: Option<Value>) -> Result<T, ApiError> {
        let mut request = self
            .http
            .request(method, format!("{}/{}", self.base, path))
            .basic_auth(&self.user, Some(self.app_password.as_str()));
        if let Some(session) = self.session.lock().unwrap().clone() {
            request = request.header(SESSION_HEADER, session);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await?;
        // Der Server kann die Session jederzeit erneuern, immer die neueste mitschicken.
        if let Some(session) = response.headers().get(SESSION_HEADER).and_then(|v| v.to_str().ok()) {
            *self.session.lock().unwrap() = Some(session.to_string());
        }

        let status = response.status();
        let bytes = response.bytes().await?;
        if status == StatusCode::UNAUTHORIZED {
            return Err(ApiError::Unauthorized);
        }
        if !status.is_success() {
            let message = serde_json::from_slice::<ErrorBody>(&bytes)
                .ok()
                .and_then(|e| e.message)
                .unwrap_or_else(|| format!("{} ({})", t("server_error"), status.as_u16()));
            return Err(ApiError::Server(message));
        }
        serde_json::from_slice(&bytes).map_err(|_| ApiError::Invalid)
    }

    pub fn clear_session(&self) {
        *self.session.lock().unwrap() = None;
    }

    pub async fn session_request(&self) -> Result<SessionRequirements, ApiError> {
        self.call(Method::GET, "session/request", None).await
    }

    pub async fn session_open(&self, challenge: Option<String>, token: Option<(String, String)>) -> Result<SessionOpen, ApiError> {
        let mut body = serde_json::Map::new();
        if let Some(challenge) = challenge {
            body.insert("challenge".into(), json!(challenge));
        }
        if let Some((id, code)) = token {
            let mut tokens = serde_json::Map::new();
            tokens.insert(id, json!(code));
            body.insert("token".into(), Value::Object(tokens));
        }
        self.call(Method::POST, "session/open", Some(Value::Object(body))).await
    }

    pub async fn session_close(&self) {
        let _ = self.call::<Value>(Method::GET, "session/close", None).await;
        self.clear_session();
    }

    pub async fn keepalive(&self) -> Result<(), ApiError> {
        self.call::<Value>(Method::GET, "session/keepalive", None).await.map(|_| ())
    }

    pub async fn passwords(&self) -> Result<Vec<RawPassword>, ApiError> {
        self.call(Method::POST, "password/list", Some(json!({ "details": "model+tag-ids" }))).await
    }

    pub async fn trashed_passwords(&self) -> Result<Vec<RawPassword>, ApiError> {
        let body = json!({ "criteria": { "trashed": true }, "details": "model+tag-ids" });
        self.call(Method::POST, "password/find", Some(body)).await
    }

    pub async fn restore_password(&self, id: &str) -> Result<Value, ApiError> {
        self.call(Method::PATCH, "password/restore", Some(json!({ "id": id }))).await
    }

    pub async fn folders(&self) -> Result<Vec<RawFolder>, ApiError> {
        self.call(Method::POST, "folder/list", Some(json!({ "details": "model" }))).await
    }

    pub async fn create_folder(&self, body: Value) -> Result<Saved, ApiError> {
        self.call(Method::POST, "folder/create", Some(body)).await
    }

    pub async fn update_folder(&self, body: Value) -> Result<Saved, ApiError> {
        self.call(Method::PATCH, "folder/update", Some(body)).await
    }

    pub async fn delete_folder(&self, id: &str, revision: &str) -> Result<Value, ApiError> {
        self.call(Method::DELETE, "folder/delete", Some(json!({ "id": id, "revision": revision }))).await
    }

    pub async fn tags(&self) -> Result<Vec<RawTag>, ApiError> {
        self.call(Method::POST, "tag/list", Some(json!({ "details": "model" }))).await
    }

    pub async fn create_tag(&self, body: Value) -> Result<Saved, ApiError> {
        self.call(Method::POST, "tag/create", Some(body)).await
    }

    pub async fn delete_tag(&self, id: &str, revision: &str) -> Result<Value, ApiError> {
        self.call(Method::DELETE, "tag/delete", Some(json!({ "id": id, "revision": revision }))).await
    }

    /// Favicon als PNG. Der Server begrenzt auf 15 Abrufe in 15 Sekunden.
    pub async fn favicon(&self, domain: &str, size: u32) -> Result<Vec<u8>, ApiError> {
        let domain: String = url::form_urlencoded::byte_serialize(domain.as_bytes()).collect();
        let response = self
            .http
            .get(format!("{}/service/favicon/{}/{}", self.base, domain, size))
            .basic_auth(&self.user, Some(self.app_password.as_str()))
            .header(header::ACCEPT, "image/png,image/*")
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(ApiError::Server(format!("{} ({})", t("server_error"), response.status().as_u16())));
        }
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn create_password(&self, body: Value) -> Result<Saved, ApiError> {
        self.call(Method::POST, "password/create", Some(body)).await
    }

    pub async fn update_password(&self, body: Value) -> Result<Saved, ApiError> {
        self.call(Method::PATCH, "password/update", Some(body)).await
    }

    pub async fn delete_password(&self, id: &str, revision: &str) -> Result<Value, ApiError> {
        self.call(Method::DELETE, "password/delete", Some(json!({ "id": id, "revision": revision }))).await
    }

    pub async fn generate_password(&self) -> Result<String, ApiError> {
        #[derive(Deserialize)]
        struct Generated {
            password: String,
        }
        let generated: Generated = self
            .call(Method::POST, "service/password", Some(json!({ "strength": 2, "numbers": true, "special": true })))
            .await?;
        Ok(generated.password)
    }
}

pub fn http_client() -> Result<reqwest::Client, ApiError> {
    let mut headers = header::HeaderMap::new();
    headers.insert(header::ACCEPT, header::HeaderValue::from_static("application/json"));
    Ok(reqwest::Client::builder()
        .user_agent(concat!("Nextcloud Passwords Desktop/", env!("CARGO_PKG_VERSION")))
        .default_headers(headers)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .build()?)
}

#[derive(Deserialize, Default)]
pub struct SessionRequirements {
    pub challenge: Option<Challenge>,
    #[serde(default)]
    pub token: Vec<TokenInfo>,
}

#[derive(Deserialize)]
pub struct Challenge {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub salts: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct TokenInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Deserialize)]
pub struct SessionOpen {
    #[serde(default)]
    pub success: bool,
    #[serde(default, deserialize_with = "map_or_empty")]
    pub keys: std::collections::HashMap<String, String>,
}

/// PHP kodiert ein leeres assoziatives Array als `[]`. Ohne Verschlüsselung liefert
/// `session/open` deshalb `"keys": []` statt eines Objekts.
fn map_or_empty<'de, D>(deserializer: D) -> Result<std::collections::HashMap<String, String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::Object(map) => serde_json::from_value(Value::Object(map)).map_err(serde::de::Error::custom),
        _ => Ok(Default::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_error_contains_cause() {
        let error = tauri::async_runtime::block_on(async {
            http_client().unwrap().get("https://ungueltig.invalid/").send().await.unwrap_err()
        });
        let text = ApiError::from(error).to_string();
        assert!(text.contains(" ("), "{text}");
        assert!(text.contains(':'), "Ursache fehlt: {text}");
    }

    #[test]
    fn session_open_accepts_empty_array_keys() {
        let open: SessionOpen = serde_json::from_str(r#"{"success":true,"keys":[]}"#).unwrap();
        assert!(open.success);
        assert!(open.keys.is_empty());
    }

    #[test]
    fn session_open_reads_keychains() {
        let open: SessionOpen = serde_json::from_str(r#"{"success":true,"keys":{"CSEv1r1":"abc"}}"#).unwrap();
        assert_eq!(open.keys["CSEv1r1"], "abc");
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawPassword {
    pub id: String,
    pub revision: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub custom_fields: String,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub cse_type: String,
    #[serde(default)]
    pub cse_key: String,
    #[serde(default)]
    pub status: i64,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub editable: bool,
    #[serde(default)]
    pub edited: i64,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub trashed: bool,
    #[serde(default)]
    pub shared: bool,
    #[serde(default, deserialize_with = "ids_or_empty")]
    pub tags: Vec<String>,
}

/// `+tag-ids` liefert eine Liste von IDs, ältere Server teils Objekte.
fn ids_or_empty<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    let items = match value {
        Value::Array(items) => items,
        Value::Object(map) => map.into_values().collect(),
        _ => Vec::new(),
    };
    Ok(items
        .into_iter()
        .filter_map(|item| match item {
            Value::String(id) => Some(id),
            Value::Object(obj) => obj.get("id").and_then(|v| v.as_str()).map(String::from),
            _ => None,
        })
        .collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTag {
    pub id: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub cse_type: String,
    #[serde(default)]
    pub cse_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawFolder {
    pub id: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub parent: String,
    #[serde(default)]
    pub cse_type: String,
    #[serde(default)]
    pub cse_key: String,
}

#[derive(Deserialize)]
pub struct Saved {
    pub id: String,
}
