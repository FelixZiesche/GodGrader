//! Absturzsicheres Schreiben: erst in eine temporäre Datei im selben Ordner,
//! dann atomar umbenennen. Bricht das Programm mittendrin ab, bleibt die alte
//! Datei vollständig erhalten.

use std::io::Write;
use std::path::Path;

use serde::{de::DeserializeOwned, Serialize};

use crate::Result;

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_atomic(path, &bytes)
}

/// Liest JSON. Fehlt die Datei, kommt `None` zurück.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schreibt_und_liest_json() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("unter/ordner/x.json");
        write_json(&p, &vec![1, 2, 3]).unwrap();
        let v: Option<Vec<i32>> = read_json(&p).unwrap();
        assert_eq!(v, Some(vec![1, 2, 3]));
        write_json(&p, &vec![4]).unwrap();
        let v: Option<Vec<i32>> = read_json(&p).unwrap();
        assert_eq!(v, Some(vec![4]));
    }

    #[test]
    fn fehlende_datei_ist_none() {
        let dir = tempfile::tempdir().unwrap();
        let v: Option<Vec<i32>> = read_json(&dir.path().join("nix.json")).unwrap();
        assert!(v.is_none());
    }
}
