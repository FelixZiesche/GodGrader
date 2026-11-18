//! Klausur-Projekte. Jedes Projekt liegt in einem eigenen Ordner mit einer
//! SQLite-Datenbank (`projekt.sqlite`). Löschen entfernt den ganzen Ordner.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::ewh::{Erwartungshorizont, Stufe};
use crate::paths::AppPaths;
use crate::{Error, Result};

const SCHEMA_VERSION: i32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ProjektMeta {
    pub id: String,
    pub titel: String,
    pub lerngruppe: String,
    /// ISO-Datum `JJJJ-MM-TT`.
    pub datum: String,
    pub stufe: Stufe,
    pub halbe_be: bool,
    /// Bereiche vor dem Senden schwärzen (optional, Standard an).
    pub schwaerzen: bool,
    /// Kostenlimit in US-Dollar; bei Erreichen wird angehalten.
    pub budget_usd: Option<f64>,
    /// Warnschwelle für den Anteil schlechter Noten in Prozent.
    pub warnschwelle_prozent: Option<f64>,
    pub erstellt: String,
    pub geaendert: String,
}

impl Default for ProjektMeta {
    fn default() -> Self {
        ProjektMeta {
            id: String::new(),
            titel: String::new(),
            lerngruppe: String::new(),
            datum: String::new(),
            stufe: Stufe::Oberstufe,
            halbe_be: false,
            schwaerzen: true,
            budget_usd: None,
            warnschwelle_prozent: None,
            erstellt: String::new(),
            geaendert: String::new(),
        }
    }
}

fn now() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub struct Projekt {
    pub dir: PathBuf,
    conn: Connection,
}

fn valid_id(id: &str) -> Result<()> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(Error::Ungueltig("Ungültige Projekt-ID.".into()));
    }
    Ok(())
}

impl Projekt {
    fn open_dir(dir: &Path) -> Result<Self> {
        let conn = Connection::open(dir.join("projekt.sqlite")).map_err(db)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(db)?;
        conn.pragma_update(None, "foreign_keys", "ON").map_err(db)?;
        // Pipeline und Oberfläche greifen gleichzeitig zu
        conn.busy_timeout(std::time::Duration::from_secs(10))
            .map_err(db)?;
        let p = Projekt {
            dir: dir.to_path_buf(),
            conn,
        };
        p.migrate()?;
        Ok(p)
    }

    fn migrate(&self) -> Result<()> {
        let v: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db)?;
        if v < 1 {
            self.conn
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
                )
                .map_err(db)?;
        }
        if v < 2 {
            self.conn
                .execute_batch(crate::scans::SCHEMA_V2)
                .map_err(db)?;
        }
        if v < 3 {
            self.conn
                .execute_batch(crate::ki::speicher::SCHEMA_V3)
                .map_err(db)?;
        }
        if v < 4 {
            self.conn
                .execute_batch(crate::pruefung::SCHEMA_V4)
                .map_err(db)?;
        }
        if v < 5 {
            self.conn
                .execute_batch(crate::ki::feedback::SCHEMA_V5)
                .map_err(db)?;
        }
        if v < SCHEMA_VERSION {
            self.conn
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(db)?;
        }
        Ok(())
    }

    pub fn create(paths: &AppPaths, mut meta: ProjektMeta) -> Result<Self> {
        if meta.titel.trim().is_empty() {
            return Err(Error::Ungueltig("Bitte einen Titel eingeben.".into()));
        }
        meta.id = uuid::Uuid::new_v4().to_string();
        meta.erstellt = now();
        meta.geaendert = meta.erstellt.clone();
        let dir = paths.projects_dir().join(&meta.id);
        std::fs::create_dir_all(&dir)?;
        let p = Self::open_dir(&dir)?;
        p.put("meta", &meta)?;
        Ok(p)
    }

    pub fn open(paths: &AppPaths, id: &str) -> Result<Self> {
        valid_id(id)?;
        let dir = paths.projects_dir().join(id);
        if !dir.join("projekt.sqlite").exists() {
            return Err(Error::Ungueltig("Projekt nicht gefunden.".into()));
        }
        Self::open_dir(&dir)
    }

    pub fn list(paths: &AppPaths) -> Result<Vec<ProjektMeta>> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(paths.projects_dir()) else {
            return Ok(out);
        };
        for entry in rd.flatten() {
            let dir = entry.path();
            if !dir.join("projekt.sqlite").exists() {
                continue;
            }
            match Self::open_dir(&dir).and_then(|p| p.meta()) {
                Ok(m) => out.push(m),
                Err(e) => tracing::warn!("Projekt in {:?} nicht lesbar: {e}", dir.file_name()),
            }
        }
        out.sort_by(|a, b| b.geaendert.cmp(&a.geaendert));
        Ok(out)
    }

    /// Entfernt das Projekt mit allen Dateien (Scans, Bilder, Abschriften, Bewertungen).
    pub fn delete(paths: &AppPaths, id: &str) -> Result<()> {
        valid_id(id)?;
        let dir = paths.projects_dir().join(id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let v: Option<String> = self
            .conn
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db)?;
        v.map(|s| serde_json::from_str(&s).map_err(Error::from))
            .transpose()
    }

    pub fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let s = serde_json::to_string(value)?;
        self.conn
            .execute(
                "INSERT INTO kv (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, s],
            )
            .map_err(db)?;
        Ok(())
    }

    pub fn meta(&self) -> Result<ProjektMeta> {
        self.get("meta")?
            .ok_or_else(|| Error::Ungueltig("Projektdaten fehlen.".into()))
    }

    pub fn save_meta(&self, meta: &ProjektMeta) -> Result<ProjektMeta> {
        let old = self.meta()?;
        let mut m = meta.clone();
        m.id = old.id;
        m.erstellt = old.erstellt;
        m.geaendert = now();
        if m.titel.trim().is_empty() {
            return Err(Error::Ungueltig("Bitte einen Titel eingeben.".into()));
        }
        self.put("meta", &m)?;
        Ok(m)
    }

    fn touch(&self) -> Result<()> {
        let mut m = self.meta()?;
        m.geaendert = now();
        self.put("meta", &m)
    }

    pub fn ewh(&self) -> Result<Option<Erwartungshorizont>> {
        self.get("ewh")
    }

    pub fn save_ewh(&self, e: &Erwartungshorizont) -> Result<()> {
        self.put("ewh", e)?;
        self.touch()
    }
}

fn db(e: rusqlite::Error) -> Error {
    Error::Ungueltig(format!("Projektdatenbank: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anlegen_lesen_loeschen() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path());
        paths.ensure().unwrap();
        let p = Projekt::create(
            &paths,
            ProjektMeta {
                titel: "Klausur 11.1".into(),
                lerngruppe: "WR_gA".into(),
                ..Default::default()
            },
        )
        .unwrap();
        let id = p.meta().unwrap().id;
        let mut e = Erwartungshorizont::default();
        e.kopf.titel = "Test".into();
        p.save_ewh(&e).unwrap();
        drop(p);
        let list = Projekt::list(&paths).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].schwaerzen);
        let p = Projekt::open(&paths, &id).unwrap();
        assert_eq!(p.ewh().unwrap().unwrap().kopf.titel, "Test");
        drop(p);
        Projekt::delete(&paths, &id).unwrap();
        assert!(Projekt::list(&paths).unwrap().is_empty());
        assert!(!paths.projects_dir().join(&id).exists());
    }

    #[test]
    fn ungueltige_id() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path());
        assert!(Projekt::open(&paths, "../x").is_err());
        assert!(Projekt::delete(&paths, "..").is_err());
    }

    #[test]
    fn titel_pflicht() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path());
        assert!(Projekt::create(&paths, ProjektMeta::default()).is_err());
    }
}
