//! API-Schlüssel ausschließlich im Schlüsselbund des Betriebssystems
//! (Windows: Anmeldeinformationsverwaltung, Linux: Secret Service).
//! Der Schlüssel landet nie in Dateien oder Logs.

use std::sync::Mutex;

use crate::{Error, Result, APP_NAME};

const ACCOUNT: &str = "openrouter-api-key";

pub trait SecretStore: Send + Sync {
    fn get(&self) -> Result<Option<String>>;
    fn set(&self, value: &str) -> Result<()>;
    fn delete(&self) -> Result<()>;
}

/// Schlüsselbund des Betriebssystems.
pub struct KeyringStore;

impl KeyringStore {
    fn entry() -> Result<keyring::Entry> {
        keyring::Entry::new(APP_NAME, ACCOUNT).map_err(|e| Error::Schluesselbund(e.to_string()))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self) -> Result<Option<String>> {
        match Self::entry()?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Schluesselbund(e.to_string())),
        }
    }

    fn set(&self, value: &str) -> Result<()> {
        let value = value.trim();
        if value.is_empty() {
            return Err(Error::Ungueltig("Der API-Schlüssel ist leer.".into()));
        }
        Self::entry()?
            .set_password(value)
            .map_err(|e| Error::Schluesselbund(e.to_string()))
    }

    fn delete(&self) -> Result<()> {
        match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Schluesselbund(e.to_string())),
        }
    }
}

/// Nur für Tests und als Notlösung, wenn kein Schlüsselbund verfügbar ist.
#[derive(Default)]
pub struct MemoryStore(Mutex<Option<String>>);

impl SecretStore for MemoryStore {
    fn get(&self) -> Result<Option<String>> {
        Ok(self.0.lock().expect("Sperre").clone())
    }
    fn set(&self, value: &str) -> Result<()> {
        *self.0.lock().expect("Sperre") = Some(value.trim().to_string());
        Ok(())
    }
    fn delete(&self) -> Result<()> {
        *self.0.lock().expect("Sperre") = None;
        Ok(())
    }
}

/// Zeigt nur Anfang und Ende, z. B. für die Anzeige „sk-or-…a1b2".
pub fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 10 {
        return "•".repeat(chars.len());
    }
    let start: String = chars[..6].iter().collect();
    let end: String = chars[chars.len() - 4..].iter().collect();
    format!("{start}…{end}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maskiert() {
        assert_eq!(mask("sk-or-v1-1234567890abcd"), "sk-or-…abcd");
        assert_eq!(mask("kurz"), "••••");
    }

    #[test]
    fn memory_store() {
        let s = MemoryStore::default();
        assert!(s.get().unwrap().is_none());
        s.set(" abc ").unwrap();
        assert_eq!(s.get().unwrap().as_deref(), Some("abc"));
        s.delete().unwrap();
        assert!(s.get().unwrap().is_none());
    }
}
