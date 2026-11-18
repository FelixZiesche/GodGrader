//! Erwartungshorizont: Datenmodell, Import (Word/PDF im godtest-Format),
//! Validierung und Notenschlüssel.

pub mod blocks;
pub mod docx;
pub mod noten;
pub mod parse;
pub mod pdf;
pub mod validate;

use serde::{Deserialize, Serialize};

pub use noten::{Notenschluessel, Notenstufe};

/// Stufe der Lerngruppe. Bestimmt Notenschlüssel und Anrede im Feedback.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Stufe {
    /// Notenpunkte 0–15, Anrede „Sie", Kopfzeile „Kurs:".
    #[default]
    Oberstufe,
    /// Noten 1–6, Anrede „du", Kopfzeile „Klasse:".
    Sek1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Afb {
    I,
    II,
    III,
}

impl Afb {
    pub fn parse(s: &str) -> Option<Afb> {
        match s.trim() {
            "I" | "1" => Some(Afb::I),
            "II" | "2" => Some(Afb::II),
            "III" | "3" => Some(Afb::III),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ElementArt {
    /// Normales Bewertungselement aus der Lösung.
    #[default]
    Element,
    /// Zeile des Bewertungsrasters (AFB-III-Aufgabe).
    Raster,
}

/// Ein BE-tragendes Element.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Element {
    /// Eindeutig im Erwartungshorizont, z. B. `2a-1`.
    pub id: String,
    /// Kurzbezeichnung, z. B. „Formel“ oder „Fall A – Begründung“.
    pub titel: String,
    /// Erwartete Lösung bzw. Kriterium (Markdown).
    pub text: String,
    pub max_be: f64,
    #[serde(default)]
    pub art: ElementArt,
    #[serde(default)]
    pub afb: Option<Afb>,
}

/// AFB-Anteil einer Aufgabe laut AFB-Übersicht.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AfbAnteil {
    pub afb: Afb,
    pub be: f64,
    /// z. B. „Fachbegriffe“ bei geteilten Aufgaben.
    #[serde(default)]
    pub zusatz: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Aufgabe {
    /// Aufgabennummer, z. B. `1a`, `2`.
    pub nr: String,
    /// Aufgabentext (Markdown, inkl. These/Zitat davor).
    pub text: String,
    /// BE laut `[x BE]` im Aufgabenkopf.
    pub max_be: f64,
    /// Vollständige Musterlösung (Markdown), auch Hinweise ohne BE.
    pub loesung: String,
    pub elemente: Vec<Element>,
    #[serde(default)]
    pub afb: Vec<AfbAnteil>,
    /// Aufgabe wird mit Bewertungsraster bewertet.
    #[serde(default)]
    pub hat_raster: bool,
}

impl Aufgabe {
    pub fn summe_elemente(&self) -> f64 {
        self.elemente.iter().map(|e| e.max_be).sum()
    }

    /// Haupt-AFB (größter Anteil).
    pub fn haupt_afb(&self) -> Option<Afb> {
        self.afb
            .iter()
            .max_by(|a, b| a.be.total_cmp(&b.be))
            .map(|a| a.afb)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Kopf {
    pub titel: String,
    #[serde(default)]
    pub schule: Option<String>,
    #[serde(default)]
    pub schuljahr: Option<String>,
    #[serde(default)]
    pub thema: Option<String>,
    /// Inhalt hinter „Kurs:“ bzw. „Klasse:“.
    #[serde(default)]
    pub lerngruppe: Option<String>,
    /// TT.MM.JJJJ wie im Dokument.
    #[serde(default)]
    pub datum: Option<String>,
    #[serde(default)]
    pub gleichwertig_hinweis: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Erwartungshorizont {
    pub kopf: Kopf,
    pub stufe: Stufe,
    pub aufgaben: Vec<Aufgabe>,
    /// Gesamt-BE laut AFB-Übersicht (falls vorhanden).
    #[serde(default)]
    pub gesamt_be_dokument: Option<f64>,
    #[serde(default)]
    pub notenschluessel: Option<Notenschluessel>,
    /// Hinweise des Importers (unsicher erkannte Stellen).
    #[serde(default)]
    pub import_hinweise: Vec<String>,
}

impl Erwartungshorizont {
    pub fn gesamt_be(&self) -> f64 {
        self.aufgaben.iter().map(|a| a.max_be).sum()
    }

    /// Nur die Aufgabennummern, z. B. für das Abschreiben-Modell.
    pub fn aufgabennummern(&self) -> Vec<String> {
        self.aufgaben.iter().map(|a| a.nr.clone()).collect()
    }

    /// Vergibt Element-IDs neu (`{nr}-{laufende Nummer}`).
    pub fn renumber(&mut self) {
        for a in &mut self.aufgaben {
            for (i, e) in a.elemente.iter_mut().enumerate() {
                e.id = format!("{}-{}", a.nr, i + 1);
            }
        }
    }

    /// Notenschlüssel aus dem Dokument oder berechnet.
    pub fn notenschluessel_oder_berechnet(&self) -> Notenschluessel {
        self.notenschluessel
            .clone()
            .unwrap_or_else(|| Notenschluessel::berechnen(self.stufe, self.gesamt_be()))
    }
}

/// Formatiert BE deutsch: `2`, `1,5`.
pub fn be_text(be: f64) -> String {
    if (be - be.round()).abs() < 1e-9 {
        format!("{}", be.round() as i64)
    } else {
        format!("{be:.1}").replace('.', ",")
    }
}

/// Liest einen Erwartungshorizont aus einer Word- oder PDF-Datei.
pub fn import_bytes(bytes: &[u8], dateiname: &str) -> crate::Result<Erwartungshorizont> {
    let lower = dateiname.to_lowercase();
    let blocks = if lower.ends_with(".pdf") || bytes.starts_with(b"%PDF") {
        pdf::read_pdf(bytes)?
    } else if lower.ends_with(".docx") || bytes.starts_with(b"PK") {
        docx::read_docx(bytes)?
    } else {
        return Err(crate::Error::Ungueltig(
            "Bitte eine Word-Datei (.docx) oder ein PDF wählen.".into(),
        ));
    };
    Ok(parse::parse(&blocks))
}

pub fn import_file(path: &std::path::Path) -> crate::Result<Erwartungshorizont> {
    let bytes = std::fs::read(path)?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    import_bytes(&bytes, name)
}
