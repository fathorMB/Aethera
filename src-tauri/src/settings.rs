//! Preferenze dell'app, fuori dalla radice dati: dove sta la radice e cosa fare all'uscita.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExitBehavior {
    #[default]
    Ask,
    Stop,
    Leave,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_root: Option<PathBuf>,
    #[serde(default)]
    pub exit_behavior: ExitBehavior,
    /// Il profilo che si seleziona da solo aprendo Avvio, e da cui la tray può accendere il
    /// motore. Facoltativo: se il nome non corrisponde più a un profilo (rinominato, cancellato)
    /// non si inventa niente e non si cancella da qui, lo dice chi legge questa impostazione.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<String>,
}

/// `AETHERA_CONFIG_DIR` sposta le preferenze altrove: prove senza toccare quelle dell'utente.
fn settings_path() -> Option<PathBuf> {
    std::env::var_os("AETHERA_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::config_dir().map(|d| d.join("Aethera")))
        .map(|d| d.join("settings.toml"))
}

impl AppSettings {
    pub fn load() -> Self {
        settings_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = settings_path().ok_or("cartella di configurazione dell'utente sconosciuta")?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_profile_is_absent_by_default_and_not_written() {
        let s = AppSettings::default();
        assert_eq!(s.default_profile, None);
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(!text.contains("default_profile"), "{text}");
    }

    #[test]
    fn default_profile_round_trips_through_toml() {
        let s = AppSettings { default_profile: Some("G1".into()), ..Default::default() };
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(text.contains(r#"default_profile = "G1""#), "{text}");
        let back: AppSettings = toml::from_str(&text).unwrap();
        assert_eq!(back.default_profile.as_deref(), Some("G1"));
    }

    #[test]
    fn settings_without_default_profile_still_load() {
        // Un settings.toml scritto prima di questa impostazione non la porta: deve restare `None`,
        // non far fallire la lettura.
        let s: AppSettings = toml::from_str("exit_behavior = \"ask\"\n").unwrap();
        assert_eq!(s.default_profile, None);
    }
}
