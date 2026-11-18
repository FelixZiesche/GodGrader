//! Scans: PDFs importieren, Seiten mit 300 dpi rendern, in Graustufen
//! umwandeln, Kontrast anheben, optional schwärzen und teilen.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::Engine;
use image::{DynamicImage, GrayImage, ImageFormat, Luma};
use pdfium_render::prelude::*;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::projekt::Projekt;
use crate::{Error, Result};

pub const DPI: f32 = 300.0;
const JPEG_QUALITAET: u8 = 90;

/// Status einer Klausur im Ablauf.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum KlausurStatus {
    #[default]
    Offen,
    Vorkorrigiert,
    InPruefung,
    Freigegeben,
}

impl KlausurStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            KlausurStatus::Offen => "offen",
            KlausurStatus::Vorkorrigiert => "vorkorrigiert",
            KlausurStatus::InPruefung => "in_pruefung",
            KlausurStatus::Freigegeben => "freigegeben",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "vorkorrigiert" => KlausurStatus::Vorkorrigiert,
            "in_pruefung" => KlausurStatus::InPruefung,
            "freigegeben" => KlausurStatus::Freigegeben,
            _ => KlausurStatus::Offen,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Halbteil {
    Oben,
    Unten,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SeiteInfo {
    pub id: String,
    /// Laufende Nummer innerhalb der Klausur (ab 1).
    pub nr: u32,
    pub halbteil: Option<Halbteil>,
    /// Seite im Original-PDF (ab 1).
    pub quelle_seite: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KlausurInfo {
    pub id: String,
    pub bezeichnung: String,
    pub status: KlausurStatus,
    pub quelle: String,
    pub seiten: Vec<SeiteInfo>,
}

/// Rechteck in Seitenanteilen (0..1), oben links = (0, 0).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Bereich {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "art", content = "liste", rename_all = "lowercase")]
pub enum SeitenAuswahl {
    Alle,
    /// Seitennummern innerhalb der Klausur (ab 1).
    Seiten(Vec<u32>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Schwaerzung {
    pub bereich: Bereich,
    pub seiten: SeitenAuswahl,
}

impl Schwaerzung {
    fn gilt_fuer(&self, seite: u32) -> bool {
        match &self.seiten {
            SeitenAuswahl::Alle => true,
            SeitenAuswahl::Seiten(l) => l.contains(&seite),
        }
    }
}

/// Ein Importauftrag: welche Seiten eines PDFs zu einer Klausur gehören.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportAuftrag {
    pub bezeichnung: String,
    pub pdf: String,
    /// Seiten im PDF (ab 0).
    pub seiten: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScanOptionen {
    pub teilen: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Fortschritt {
    pub fertig: usize,
    pub gesamt: usize,
    pub text: String,
}

pub(crate) const SCHEMA_V2: &str = "
CREATE TABLE IF NOT EXISTS klausuren (
    id TEXT PRIMARY KEY,
    bezeichnung TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'offen',
    quelle TEXT NOT NULL DEFAULT '',
    reihenfolge INTEGER NOT NULL DEFAULT 0,
    erstellt TEXT NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS seiten (
    id TEXT PRIMARY KEY,
    klausur_id TEXT NOT NULL REFERENCES klausuren(id) ON DELETE CASCADE,
    nr INTEGER NOT NULL,
    halbteil TEXT,
    original TEXT NOT NULL,
    quelle_seite INTEGER NOT NULL,
    datei TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS seiten_klausur ON seiten(klausur_id, nr);
";

fn db(e: rusqlite::Error) -> Error {
    Error::Ungueltig(format!("Projektdatenbank: {e}"))
}

// ------------------------------------------------------------------
// Bildverarbeitung (ohne PDFium testbar)
// ------------------------------------------------------------------

/// Graustufen, Kontrast leicht anheben, Schwärzung anwenden.
pub fn aufbereiten(img: DynamicImage, schwaerzung: &[Bereich]) -> GrayImage {
    let mut g = img.to_luma8();
    // Kontrast leicht anheben: Papier (90-%-Quantil) auf Weiß ziehen
    // (Faktor höchstens 1,4), Grautöne der Schrift etwas abdunkeln.
    // Kein Schwarzpunkt aus dem Histogramm, damit fast leere Seiten nicht kippen.
    let mut hist = [0u64; 256];
    for p in g.pixels() {
        hist[p.0[0] as usize] += 1;
    }
    let total: u64 = hist.iter().sum::<u64>().max(1);
    let mut acc = 0u64;
    let mut papier = 255.0;
    for (i, h) in hist.iter().enumerate() {
        acc += h;
        if acc as f64 >= 0.9 * total as f64 {
            papier = (i as f64).max(1.0);
            break;
        }
    }
    let factor = (255.0 / papier).clamp(1.0, 1.4);
    let lut: Vec<u8> = (0..256)
        .map(|v| {
            let x = (v as f64 * factor).min(255.0) / 255.0;
            (x.powf(1.2) * 255.0).round() as u8
        })
        .collect();
    for p in g.pixels_mut() {
        p.0[0] = lut[p.0[0] as usize];
    }
    let (w, h) = g.dimensions();
    for b in schwaerzung {
        let x0 = (b.x.clamp(0.0, 1.0) * w as f32) as u32;
        let y0 = (b.y.clamp(0.0, 1.0) * h as f32) as u32;
        let x1 = ((b.x + b.w).clamp(0.0, 1.0) * w as f32).ceil() as u32;
        let y1 = ((b.y + b.h).clamp(0.0, 1.0) * h as f32).ceil() as u32;
        for y in y0..y1.min(h) {
            for x in x0..x1.min(w) {
                g.put_pixel(x, y, Luma([0]));
            }
        }
    }
    g
}

/// Obere und untere Hälfte mit 3 % Überlappung, damit keine Zeile zerschnitten wird.
pub fn teilen(g: &GrayImage) -> (GrayImage, GrayImage) {
    let (w, h) = g.dimensions();
    let overlap = h * 3 / 100;
    let mitte = h / 2;
    let oben = image::imageops::crop_imm(g, 0, 0, w, (mitte + overlap).min(h)).to_image();
    let start = mitte.saturating_sub(overlap);
    let unten = image::imageops::crop_imm(g, 0, start, w, h - start).to_image();
    (oben, unten)
}

pub fn jpeg_bytes(g: &GrayImage) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITAET);
    g.write_with_encoder(enc)
        .map_err(|e| Error::Ungueltig(format!("Bild konnte nicht gespeichert werden: {e}")))?;
    Ok(out)
}

pub fn data_url_png(img: &DynamicImage) -> Result<String> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| Error::Ungueltig(e.to_string()))?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(buf.into_inner())
    ))
}

pub fn data_url_jpeg(bytes: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

// ------------------------------------------------------------------
// PDF
// ------------------------------------------------------------------

pub fn seitenzahl(pdf: &Path) -> Result<u32> {
    let pdfium = crate::pdfium::pdfium()?;
    let doc = pdfium
        .load_pdf_from_file(pdf, None)
        .map_err(|e| Error::Ungueltig(format!("PDF nicht lesbar: {e}")))?;
    Ok(doc.pages().len() as u32)
}

fn render(page: &PdfPage, breite_px: i32) -> Result<DynamicImage> {
    let cfg = PdfRenderConfig::new()
        .set_target_width(breite_px)
        .render_form_data(true);
    page.render_with_config(&cfg)
        .and_then(|b| b.as_image())
        .map_err(|e| Error::Ungueltig(format!("Seite konnte nicht gerendert werden: {e}")))
}

/// Miniaturen aller Seiten eines PDFs (Breite in Pixeln).
pub fn miniaturen(pdf: &Path, breite: i32) -> Result<Vec<String>> {
    let pdfium = crate::pdfium::pdfium()?;
    let doc = pdfium
        .load_pdf_from_file(pdf, None)
        .map_err(|e| Error::Ungueltig(format!("PDF nicht lesbar: {e}")))?;
    doc.pages()
        .iter()
        .map(|p| {
            render(&p, breite)
                .and_then(|img| data_url_png(&DynamicImage::ImageLuma8(img.to_luma8())))
        })
        .collect()
}

/// Eine Seite als Vorschau (für das Schwärzen-Werkzeug).
pub fn seitenvorschau(pdf: &Path, seite: u32, breite: i32) -> Result<String> {
    let pdfium = crate::pdfium::pdfium()?;
    let doc = pdfium
        .load_pdf_from_file(pdf, None)
        .map_err(|e| Error::Ungueltig(format!("PDF nicht lesbar: {e}")))?;
    let page = doc
        .pages()
        .get(seite as i32)
        .map_err(|_| Error::Ungueltig("Seite nicht vorhanden.".into()))?;
    let img = render(&page, breite)?;
    data_url_png(&DynamicImage::ImageLuma8(img.to_luma8()))
}

fn breite_300dpi(page: &PdfPage) -> i32 {
    (page.width().value / 72.0 * DPI).round() as i32
}

// ------------------------------------------------------------------
// Projekt-Datenbank
// ------------------------------------------------------------------

impl Projekt {
    fn scans_dir(&self) -> PathBuf {
        self.dir.join("scans")
    }

    pub fn schwaerzung(&self) -> Result<Vec<Schwaerzung>> {
        Ok(self.get("schwaerzung")?.unwrap_or_default())
    }

    pub fn set_schwaerzung(&self, s: &[Schwaerzung]) -> Result<()> {
        self.put("schwaerzung", &s)
    }

    pub fn pseudonyme(&self) -> Result<Vec<String>> {
        Ok(self.get("pseudonyme")?.unwrap_or_default())
    }

    pub fn set_pseudonyme(&self, l: &[String]) -> Result<()> {
        self.put("pseudonyme", &l)
    }

    pub fn scan_optionen(&self) -> Result<ScanOptionen> {
        Ok(self.get("scan_optionen")?.unwrap_or_default())
    }

    pub fn klausuren(&self) -> Result<Vec<KlausurInfo>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT id, bezeichnung, status, quelle FROM klausuren ORDER BY reihenfolge, erstellt")
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(KlausurInfo {
                    id: r.get(0)?,
                    bezeichnung: r.get(1)?,
                    status: KlausurStatus::parse(&r.get::<_, String>(2)?),
                    quelle: r.get(3)?,
                    seiten: vec![],
                })
            })
            .map_err(db)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db)?;
        let mut out = Vec::new();
        for mut k in rows {
            k.seiten = self.seiten(&k.id)?;
            out.push(k);
        }
        Ok(out)
    }

    pub fn seiten(&self, klausur_id: &str) -> Result<Vec<SeiteInfo>> {
        let mut stmt = self
            .conn()
            .prepare("SELECT id, nr, halbteil, quelle_seite FROM seiten WHERE klausur_id = ?1 ORDER BY nr")
            .map_err(db)?;
        let v = stmt
            .query_map(params![klausur_id], |r| {
                let h: Option<String> = r.get(2)?;
                Ok(SeiteInfo {
                    id: r.get(0)?,
                    nr: r.get(1)?,
                    halbteil: match h.as_deref() {
                        Some("oben") => Some(Halbteil::Oben),
                        Some("unten") => Some(Halbteil::Unten),
                        _ => None,
                    },
                    quelle_seite: r.get(3)?,
                })
            })
            .map_err(db)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db)?;
        Ok(v)
    }

    /// Pfad des aufbereiteten Seitenbilds.
    pub fn seitenbild(&self, seite_id: &str) -> Result<PathBuf> {
        let datei: String = self
            .conn()
            .query_row(
                "SELECT datei FROM seiten WHERE id = ?1",
                params![seite_id],
                |r| r.get(0),
            )
            .map_err(|_| Error::Ungueltig("Seite nicht gefunden.".into()))?;
        Ok(self.dir.join(datei))
    }

    pub fn klausur_umbenennen(&self, id: &str, bezeichnung: &str) -> Result<()> {
        let b = bezeichnung.trim();
        if b.is_empty() {
            return Err(Error::Ungueltig(
                "Die Bezeichnung darf nicht leer sein.".into(),
            ));
        }
        self.conn()
            .execute(
                "UPDATE klausuren SET bezeichnung = ?1 WHERE id = ?2",
                params![b, id],
            )
            .map_err(db)?;
        Ok(())
    }

    pub fn klausur_loeschen(&self, id: &str) -> Result<()> {
        for s in self.seiten(id)? {
            let _ = std::fs::remove_file(self.seitenbild(&s.id)?);
        }
        self.conn()
            .execute("DELETE FROM klausuren WHERE id = ?1", params![id])
            .map_err(db)?;
        self.originale_aufraeumen()
    }

    fn originale_aufraeumen(&self) -> Result<()> {
        let mut stmt = self
            .conn()
            .prepare("SELECT DISTINCT original FROM seiten")
            .map_err(db)?;
        let used: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .map_err(db)?
            .collect::<std::result::Result<_, _>>()
            .map_err(db)?;
        if let Ok(rd) = std::fs::read_dir(self.scans_dir().join("original")) {
            for e in rd.flatten() {
                let rel = format!("scans/original/{}", e.file_name().to_string_lossy());
                if !used.contains(&rel) {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
        Ok(())
    }

    /// Importiert Klausuren aus PDFs und erzeugt die Seitenbilder.
    pub fn scans_importieren(
        &self,
        auftraege: &[ImportAuftrag],
        optionen: &ScanOptionen,
        mut fortschritt: impl FnMut(Fortschritt),
    ) -> Result<Vec<KlausurInfo>> {
        if auftraege.iter().any(|a| a.bezeichnung.trim().is_empty()) {
            return Err(Error::Ungueltig(
                "Jede Klausur braucht eine Bezeichnung.".into(),
            ));
        }
        self.put("scan_optionen", optionen)?;
        let orig_dir = self.scans_dir().join("original");
        std::fs::create_dir_all(&orig_dir)?;
        std::fs::create_dir_all(self.scans_dir().join("seiten"))?;
        // Originale ins Projekt kopieren (einmal pro Quelldatei)
        let mut kopien: Vec<(String, String)> = Vec::new();
        for a in auftraege {
            if kopien.iter().any(|(q, _)| q == &a.pdf) {
                continue;
            }
            let rel = format!("scans/original/{}.pdf", uuid::Uuid::new_v4());
            std::fs::copy(&a.pdf, self.dir.join(&rel))?;
            kopien.push((a.pdf.clone(), rel));
        }
        let schwaerzung = if self.meta()?.schwaerzen {
            self.schwaerzung()?
        } else {
            vec![]
        };
        let gesamt: usize = auftraege.iter().map(|a| a.seiten.len()).sum();
        let mut fertig = 0;
        let start_order: i64 = self
            .conn()
            .query_row(
                "SELECT COALESCE(MAX(reihenfolge), 0) FROM klausuren",
                [],
                |r| r.get(0),
            )
            .map_err(db)?;
        let pdfium = crate::pdfium::pdfium()?;
        let mut ids = Vec::new();
        for (ki, a) in auftraege.iter().enumerate() {
            let rel_orig = &kopien.iter().find(|(q, _)| q == &a.pdf).expect("Kopie").1;
            let doc = pdfium
                .load_pdf_from_file(&self.dir.join(rel_orig), None)
                .map_err(|e| Error::Ungueltig(format!("PDF nicht lesbar: {e}")))?;
            let kid = uuid::Uuid::new_v4().to_string();
            let quelle = Path::new(&a.pdf)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.conn()
                .execute(
                    "INSERT INTO klausuren (id, bezeichnung, status, quelle, reihenfolge, erstellt) VALUES (?1, ?2, 'offen', ?3, ?4, datetime('now'))",
                    params![kid, a.bezeichnung.trim(), quelle, start_order + 1 + ki as i64],
                )
                .map_err(db)?;
            let mut nr = 1u32;
            for (i, &ps) in a.seiten.iter().enumerate() {
                fortschritt(Fortschritt {
                    fertig,
                    gesamt,
                    text: format!("{}: Seite {}", a.bezeichnung.trim(), i + 1),
                });
                let page = doc
                    .pages()
                    .get(ps as i32)
                    .map_err(|_| Error::Ungueltig(format!("Seite {} fehlt im PDF.", ps + 1)))?;
                let img = render(&page, breite_300dpi(&page))?;
                let bereiche: Vec<Bereich> = schwaerzung
                    .iter()
                    .filter(|s| s.gilt_fuer(i as u32 + 1))
                    .map(|s| s.bereich)
                    .collect();
                let g = aufbereiten(img, &bereiche);
                let teile: Vec<(Option<&str>, GrayImage)> = if optionen.teilen {
                    let (o, u) = teilen(&g);
                    vec![(Some("oben"), o), (Some("unten"), u)]
                } else {
                    vec![(None, g)]
                };
                for (halb, bild) in teile {
                    let sid = uuid::Uuid::new_v4().to_string();
                    let rel = format!("scans/seiten/{sid}.jpg");
                    crate::storage::write_atomic(&self.dir.join(&rel), &jpeg_bytes(&bild)?)?;
                    self.conn()
                        .execute(
                            "INSERT INTO seiten (id, klausur_id, nr, halbteil, original, quelle_seite, datei) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                            params![sid, kid, nr, halb, rel_orig, ps + 1, rel],
                        )
                        .map_err(db)?;
                    nr += 1;
                }
                fertig += 1;
            }
            ids.push(kid);
        }
        fortschritt(Fortschritt {
            fertig,
            gesamt,
            text: "fertig".into(),
        });
        let alle = self.klausuren()?;
        Ok(alle.into_iter().filter(|k| ids.contains(&k.id)).collect())
    }

    /// Erzeugt alle Seitenbilder neu (z. B. nach geänderter Schwärzung).
    pub fn seiten_neu_erzeugen(&self, mut fortschritt: impl FnMut(Fortschritt)) -> Result<()> {
        let schwaerzung = if self.meta()?.schwaerzen {
            self.schwaerzung()?
        } else {
            vec![]
        };
        let optionen = self.scan_optionen()?;
        let mut stmt = self
            .conn()
            .prepare("SELECT s.id, s.klausur_id, s.halbteil, s.original, s.quelle_seite, s.datei FROM seiten s ORDER BY s.klausur_id, s.nr")
            .map_err(db)?;
        type Row = (String, String, Option<String>, String, u32, String);
        let rows: Vec<Row> = stmt
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .map_err(db)?
            .collect::<std::result::Result<_, _>>()
            .map_err(db)?;
        drop(stmt);
        let pdfium = crate::pdfium::pdfium()?;
        // Seitennummer innerhalb der Klausur (Originalseite) für die Seitenauswahl
        let mut erste: std::collections::HashMap<String, u32> = Default::default();
        for (_, kid, _, _, qs, _) in &rows {
            let e = erste.entry(kid.clone()).or_insert(*qs);
            *e = (*e).min(*qs);
        }
        let gesamt = rows.len();
        let mut cache: Option<(String, PdfDocument)> = None;
        for (i, (_, kid, halb, orig, qs, datei)) in rows.iter().enumerate() {
            fortschritt(Fortschritt {
                fertig: i,
                gesamt,
                text: format!("Seite {} von {gesamt}", i + 1),
            });
            if cache.as_ref().map(|(p, _)| p != orig).unwrap_or(true) {
                let d = pdfium
                    .load_pdf_from_file(&self.dir.join(orig), None)
                    .map_err(|e| Error::Ungueltig(format!("PDF nicht lesbar: {e}")))?;
                cache = Some((orig.clone(), d));
            }
            let doc = &cache.as_ref().expect("geladen").1;
            let page = doc
                .pages()
                .get((*qs - 1) as i32)
                .map_err(|_| Error::Ungueltig("Seite fehlt im Original.".into()))?;
            let seite_in_klausur = qs - erste[kid] + 1;
            let bereiche: Vec<Bereich> = schwaerzung
                .iter()
                .filter(|s| s.gilt_fuer(seite_in_klausur))
                .map(|s| s.bereich)
                .collect();
            let g = aufbereiten(render(&page, breite_300dpi(&page))?, &bereiche);
            let bild = match (halb.as_deref(), optionen.teilen) {
                (Some("oben"), _) => teilen(&g).0,
                (Some("unten"), _) => teilen(&g).1,
                _ => g,
            };
            crate::storage::write_atomic(&self.dir.join(datei), &jpeg_bytes(&bild)?)?;
        }
        fortschritt(Fortschritt {
            fertig: gesamt,
            gesamt,
            text: "fertig".into(),
        });
        Ok(())
    }

    /// Erste Originalseite der ersten Klausur, ungeschwärzt, als Vorlage
    /// für das Schwärzen-Werkzeug. `None`, wenn noch nichts importiert ist.
    pub fn beispielseite(&self, breite: i32) -> Result<Option<String>> {
        let row: Option<(String, u32)> = self
            .conn()
            .query_row(
                "SELECT s.original, MIN(s.quelle_seite) FROM seiten s JOIN klausuren k ON k.id = s.klausur_id \
                 GROUP BY s.klausur_id ORDER BY k.reihenfolge, k.erstellt LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db)?;
        match row {
            Some((orig, qs)) => Ok(Some(seitenvorschau(&self.dir.join(orig), qs - 1, breite)?)),
            None => Ok(None),
        }
    }

    /// Verkleinerte Vorschau eines aufbereiteten Seitenbilds.
    pub fn seiten_miniatur(&self, seite_id: &str, breite: u32) -> Result<String> {
        let img = image::open(self.seitenbild(seite_id)?)
            .map_err(|e| Error::Ungueltig(format!("Seitenbild nicht lesbar: {e}")))?;
        let t = img.resize(breite, breite * 2, image::imageops::FilterType::Triangle);
        let g = t.to_luma8();
        Ok(data_url_jpeg(&jpeg_bytes(&g)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schwaerzen_und_kontrast() {
        let mut img = GrayImage::from_pixel(100, 200, Luma([200]));
        img.put_pixel(50, 150, Luma([60]));
        let g = aufbereiten(
            DynamicImage::ImageLuma8(img),
            &[Bereich {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 0.1,
            }],
        );
        // oberste 20 Zeilen schwarz
        assert_eq!(g.get_pixel(10, 5).0[0], 0);
        assert_eq!(g.get_pixel(99, 19).0[0], 0);
        assert!(g.get_pixel(10, 25).0[0] > 0);
        // Kontrast: Papier wird heller, Schrift bleibt dunkel
        assert!(g.get_pixel(10, 100).0[0] > 240);
        assert!(g.get_pixel(50, 150).0[0] < 80);
    }

    #[test]
    fn teilen_mit_ueberlappung() {
        let img = GrayImage::from_pixel(10, 100, Luma([255]));
        let (o, u) = teilen(&img);
        assert_eq!(o.height(), 53);
        assert_eq!(u.height(), 53);
    }

    #[test]
    fn seitenauswahl() {
        let s = Schwaerzung {
            bereich: Bereich {
                x: 0.0,
                y: 0.0,
                w: 0.1,
                h: 0.1,
            },
            seiten: SeitenAuswahl::Seiten(vec![1]),
        };
        assert!(s.gilt_fuer(1));
        assert!(!s.gilt_fuer(2));
        let json = serde_json::to_string(&s.seiten).unwrap();
        assert_eq!(json, r#"{"art":"seiten","liste":[1]}"#);
    }
}
