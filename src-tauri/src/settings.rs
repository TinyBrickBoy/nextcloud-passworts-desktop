//! Einstellungen der App, gespeichert als `settings.json` im Konfigurationsordner.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "auto", "de" oder "en"
    pub language: String,
    /// "system", "light" oder "dark"
    pub theme: String,
    /// Automatisch sperren nach so vielen Minuten ohne Aktivität, 0 = nie
    pub lock_minutes: u32,
    /// Zwischenablage nach so vielen Sekunden leeren, 0 = nie
    pub clipboard_seconds: u32,
    /// Favicons der Webseiten über den Nextcloud Server laden
    pub favicons: bool,
    /// Sortierung der Liste: "name" oder "edited"
    pub sort: String,
    /// Verschlüsselte Kopie des Tresors lokal vorhalten
    #[serde(default)]
    pub offline_cache: bool,
    /// Schnellentsperren: "off", "biometric" oder "pin"
    #[serde(default = "default_quick_unlock")]
    pub quick_unlock: String,
    /// Einrichtungsassistent abgeschlossen (Bestandsnutzer überspringen ihn)
    #[serde(default = "default_true")]
    pub setup_done: bool,
}

fn default_true() -> bool {
    true
}

fn default_quick_unlock() -> String {
    "off".into()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "auto".into(),
            theme: "system".into(),
            lock_minutes: 5,
            clipboard_seconds: 30,
            favicons: true,
            sort: "name".into(),
            offline_cache: false,
            quick_unlock: "off".into(),
            setup_done: true,
        }
    }
}

impl Settings {
    pub fn load(dir: &Path) -> Self {
        std::fs::read(dir.join("settings.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("settings.json"), json).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_settings_get_new_defaults() {
        let s: Settings = serde_json::from_str(r#"{"language":"de"}"#).unwrap();
        assert_eq!(s.quick_unlock, "off");
        assert!(s.setup_done);
        assert!(!s.offline_cache);
    }
}
