//! App-Einstellungen (ohne API-Schlüssel, der liegt im Schlüsselbund).

use serde::{Deserialize, Serialize};

use crate::paths::AppPaths;
use crate::slots::SlotSet;
use crate::{storage, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    pub slots: SlotSet,
    /// Gleichzeitige Anfragen an OpenRouter.
    pub parallelism: u8,
    /// Nur Anbieter mit Zero Data Retention (`provider.zdr = true`).
    pub zdr_only: bool,
    /// Wiederholungen bei 429/5xx.
    pub max_retries: u8,
    /// Hinweis beim ersten Start wurde bestätigt.
    pub hinweis_bestaetigt: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            slots: SlotSet::default(),
            parallelism: 3,
            zdr_only: false,
            max_retries: 4,
            hinweis_bestaetigt: false,
        }
    }
}

impl AppSettings {
    pub fn load(paths: &AppPaths) -> Result<Self> {
        Ok(storage::read_json(&paths.settings_file())?.unwrap_or_default())
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        let mut s = self.clone();
        s.parallelism = s.parallelism.clamp(1, 16);
        storage::write_json(&paths.settings_file(), &s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speichern_und_laden() {
        let dir = tempfile::tempdir().unwrap();
        let p = AppPaths::new(dir.path());
        let mut s = AppSettings::load(&p).unwrap();
        assert_eq!(s, AppSettings::default());
        s.zdr_only = true;
        s.slots.feedback.model = "x/y".into();
        s.save(&p).unwrap();
        assert_eq!(AppSettings::load(&p).unwrap(), s);
    }

    #[test]
    fn alte_datei_mit_fehlenden_feldern() {
        let dir = tempfile::tempdir().unwrap();
        let p = AppPaths::new(dir.path());
        std::fs::write(p.settings_file(), r#"{"zdr_only":true}"#).unwrap();
        let s = AppSettings::load(&p).unwrap();
        assert!(s.zdr_only);
        assert_eq!(s.parallelism, 3);
    }
}
