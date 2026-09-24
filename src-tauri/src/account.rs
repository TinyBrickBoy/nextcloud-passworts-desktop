//! Anmeldung per Nextcloud Login Flow v2 und Ablage der Zugangsdaten.
//!
//! Server und Benutzername liegen in `account.json` im Konfigurationsordner,
//! das App Passwort ausschließlich im Schlüsselbund des Systems
//! (Secret Service unter Linux, Anmeldeinformationsverwaltung unter Windows).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::api::{http_client, ApiError};

const KEYRING_SERVICE: &str = "nextcloud-passwords-desktop";

#[derive(Serialize, Deserialize, Clone)]
pub struct Account {
    pub server: String,
    pub user: String,
}

impl Account {
    fn keyring_entry(&self) -> Result<keyring::Entry, String> {
        keyring::Entry::new(KEYRING_SERVICE, &format!("{}@{}", self.user, self.server))
            .map_err(|e| format!("Schlüsselbund nicht verfügbar: {e}"))
    }

    pub fn app_password(&self) -> Result<String, String> {
        self.keyring_entry()?
            .get_password()
            .map_err(|e| format!("App Passwort nicht im Schlüsselbund gefunden: {e}"))
    }

    pub fn save(&self, dir: &Path, app_password: &str) -> Result<(), String> {
        self.keyring_entry()?
            .set_password(app_password)
            .map_err(|e| format!("Speichern im Schlüsselbund fehlgeschlagen: {e}"))?;
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(file(dir), json).map_err(|e| e.to_string())
    }

    pub fn load(dir: &Path) -> Option<Self> {
        let bytes = std::fs::read(file(dir)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub fn remove(&self, dir: &Path) {
        if let Ok(entry) = self.keyring_entry() {
            let _ = entry.delete_credential();
        }
        let _ = std::fs::remove_file(file(dir));
    }
}

fn file(dir: &Path) -> PathBuf {
    dir.join("account.json")
}

/// Normalisiert die Serveradresse: https ergänzen, abschließenden Slash entfernen.
pub fn normalize_server(input: &str) -> Result<String, String> {
    let input = input.trim().trim_end_matches('/');
    let with_scheme = if input.contains("://") { input.to_string() } else { format!("https://{input}") };
    let mut url = url::Url::parse(&with_scheme).map_err(|_| "Ungültige Serveradresse".to_string())?;
    if url.scheme() != "https" && !is_local(&url) {
        return Err("Die Passwords API funktioniert nur über HTTPS".into());
    }
    // Wer eine kopierte Adresse wie …/index.php/login/v2 oder …/apps/passwords einträgt,
    // meint die Nextcloud davor. Ein Unterordner wie /nextcloud bleibt erhalten.
    let path = url.path().to_string();
    let base = ["/index.php", "/login", "/apps/", "/remote.php", "/ocs/"]
        .iter()
        .filter_map(|marker| path.find(marker))
        .min()
        .map(|cut| &path[..cut])
        .unwrap_or(&path);
    url.set_path(base.trim_end_matches('/'));
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::normalize_server;

    #[test]
    fn strips_copied_nextcloud_paths() {
        let expect = "https://cloud.example.com";
        assert_eq!(normalize_server("cloud.example.com").unwrap(), expect);
        assert_eq!(normalize_server("https://cloud.example.com/").unwrap(), expect);
        assert_eq!(normalize_server("https://cloud.example.com/index.php/login/v2").unwrap(), expect);
        assert_eq!(normalize_server("https://cloud.example.com/index.php/apps/passwords/#/all").unwrap(), expect);
        assert_eq!(normalize_server("cloud.example.com/login?redirect_url=x").unwrap(), expect);
        assert_eq!(
            normalize_server("https://example.com/nextcloud/index.php/apps/files").unwrap(),
            "https://example.com/nextcloud"
        );
        assert!(normalize_server("http://cloud.example.com").is_err());
    }
}

fn is_local(url: &url::Url) -> bool {
    matches!(url.host_str(), Some("localhost") | Some("127.0.0.1") | Some("::1"))
}

#[derive(Deserialize)]
pub struct FlowStart {
    pub poll: FlowPoll,
    pub login: String,
}

#[derive(Deserialize)]
pub struct FlowPoll {
    pub token: String,
    pub endpoint: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowResult {
    pub server: String,
    pub login_name: String,
    pub app_password: String,
}

pub async fn flow_start(server: &str) -> Result<FlowStart, ApiError> {
    let response = http_client()?
        .post(format!("{server}/index.php/login/v2"))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(ApiError::Server(format!(
            "Login nicht möglich ({}). Ist das ein Nextcloud Server?",
            response.status().as_u16()
        )));
    }
    response.json().await.map_err(|_| ApiError::Invalid)
}

/// Fragt den Server ab, bis der Benutzer den Login im Browser bestätigt hat (max. 20 Minuten).
pub async fn flow_poll(poll: &FlowPoll, cancelled: impl Fn() -> bool) -> Result<Option<FlowResult>, ApiError> {
    let client = http_client()?;
    for _ in 0..600 {
        if cancelled() {
            return Ok(None);
        }
        let response = client
            .post(&poll.endpoint)
            .form(&[("token", poll.token.as_str())])
            .send()
            .await?;
        if response.status().is_success() {
            return response.json().await.map(Some).map_err(|_| ApiError::Invalid);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Err(ApiError::Server("Zeit für die Anmeldung abgelaufen".into()))
}
