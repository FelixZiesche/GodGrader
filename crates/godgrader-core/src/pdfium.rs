//! Zugriff auf die PDFium-Bibliothek (wird mit der App ausgeliefert).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use pdfium_render::prelude::*;

use crate::{Error, Result};

static PDFIUM: OnceLock<Pdfium> = OnceLock::new();
static LIB_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Legt fest, in welchem Ordner `pdfium.dll` bzw. `libpdfium.so` liegt.
/// Muss vor dem ersten PDF-Zugriff aufgerufen werden.
pub fn set_library_dir(dir: impl AsRef<Path>) {
    *LIB_DIR.lock().expect("Sperre") = Some(dir.as_ref().to_path_buf());
}

fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = LIB_DIR.lock().expect("Sperre").clone() {
        v.push(d);
    }
    if let Ok(d) = std::env::var("GODGRADER_PDFIUM_DIR") {
        v.push(PathBuf::from(d));
    }
    // Entwicklung: vendor/pdfium/<plattform>/lib im Repo
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for up in [
        manifest.join("../../vendor/pdfium"),
        manifest.join("vendor/pdfium"),
    ] {
        v.push(up.join("lib"));
        v.push(up.join("bin"));
    }
    v
}

pub fn pdfium() -> Result<&'static Pdfium> {
    if let Some(p) = PDFIUM.get() {
        return Ok(p);
    }
    let mut last_err = String::from("nicht gefunden");
    let mut bindings = None;
    for dir in candidates() {
        let path = Pdfium::pdfium_platform_library_name_at_path(&dir);
        if !path.exists() {
            continue;
        }
        match Pdfium::bind_to_library(&path) {
            Ok(b) => {
                bindings = Some(b);
                break;
            }
            Err(e) => last_err = e.to_string(),
        }
    }
    let bindings = match bindings {
        Some(b) => b,
        None => Pdfium::bind_to_system_library().map_err(|e| {
            Error::Ungueltig(format!(
                "PDF-Bibliothek (PDFium) konnte nicht geladen werden: {last_err} / {e}"
            ))
        })?,
    };
    Ok(PDFIUM.get_or_init(|| Pdfium::new(bindings)))
}
