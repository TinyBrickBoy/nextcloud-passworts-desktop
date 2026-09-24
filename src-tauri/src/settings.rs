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
