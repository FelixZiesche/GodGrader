//! Optionale Pseudonymliste (CSV oder XLSX mit Spalte „Pseudonym“) und
//! Vorschläge aus Dateinamen.

use std::path::Path;

use calamine::Reader;

use crate::{Error, Result};

/// Liest Pseudonyme. Spalte „Pseudonym“ (Groß/Klein egal), sonst die erste Spalte.
pub fn import(path: &Path) -> Result<Vec<String>> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let rows: Vec<Vec<String>> = match ext.as_str() {
        "csv" | "txt" => read_csv(&std::fs::read(path)?)?,
        "xlsx" | "xls" | "ods" => read_sheet(path)?,
        _ => {
            return Err(Error::Ungueltig(
                "Bitte eine CSV- oder Excel-Datei wählen.".into(),
            ))
        }
    };
    Ok(extract(rows))
}

fn read_csv(bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or("");
    let delim = if first.contains(';') {
        b';'
    } else if first.contains('\t') {
        b'\t'
    } else {
        b','
    };
    let mut r = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for rec in r.records() {
        let rec = rec.map_err(|e| Error::Ungueltig(format!("CSV nicht lesbar: {e}")))?;
        rows.push(rec.iter().map(|s| s.trim().to_string()).collect());
    }
    Ok(rows)
}

fn read_sheet(path: &Path) -> Result<Vec<Vec<String>>> {
    let mut wb = calamine::open_workbook_auto(path)
        .map_err(|e| Error::Ungueltig(format!("Excel-Datei nicht lesbar: {e}")))?;
    let name = wb
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| Error::Ungueltig("Excel-Datei ohne Tabellenblatt.".into()))?;
    let range = wb
        .worksheet_range(&name)
        .map_err(|e| Error::Ungueltig(format!("Tabellenblatt nicht lesbar: {e}")))?;
    Ok(range
        .rows()
        .map(|r| r.iter().map(|c| c.to_string().trim().to_string()).collect())
        .collect())
}

fn extract(rows: Vec<Vec<String>>) -> Vec<String> {
    let col = rows
        .first()
        .and_then(|h| h.iter().position(|c| c.eq_ignore_ascii_case("pseudonym")));
    let (col, skip) = match col {
        Some(c) => (c, 1),
        None => (0, 0),
    };
    let mut out: Vec<String> = Vec::new();
    for r in rows.into_iter().skip(skip) {
        if let Some(v) = r.get(col) {
            let v = v.trim();
            if !v.is_empty() && !out.iter().any(|x| x == v) {
                out.push(v.to_string());
            }
        }
    }
    out
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .replace('ä', "ae")
        .replace('ö', "oe")
        .replace('ü', "ue")
        .replace('ß', "ss")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// Bezeichnung aus dem Dateinamen: passendes Pseudonym aus der Liste
/// (längster Treffer), sonst der Dateiname ohne Endung.
pub fn vorschlag(dateiname: &str, liste: &[String]) -> String {
    let stem = Path::new(dateiname)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(dateiname);
    let n = norm(stem);
    liste
        .iter()
        .filter(|p| {
            let np = norm(p);
            !np.is_empty() && n.contains(&np)
        })
        .max_by_key(|p| norm(p).len())
        .cloned()
        .unwrap_or_else(|| stem.replace(['_', '-'], " ").trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_mit_kopf_und_semikolon() {
        let rows = read_csv(
            "\u{feff}Nr;Pseudonym\n1;Edler Kranich\n2;Stiller Fuchs\n3;Edler Kranich\n".as_bytes(),
        )
        .unwrap();
        assert_eq!(extract(rows), ["Edler Kranich", "Stiller Fuchs"]);
    }

    #[test]
    fn csv_ohne_kopf() {
        let rows = read_csv(b"Edler Kranich\nStiller Fuchs\n").unwrap();
        assert_eq!(extract(rows), ["Edler Kranich", "Stiller Fuchs"]);
    }

    #[test]
    fn vorschlag_aus_dateiname() {
        let l = vec![
            "Edler Kranich".to_string(),
            "Stiller Fuchs".to_string(),
            "Fuchs".to_string(),
        ];
        assert_eq!(
            vorschlag("Scan_edler-kranich_2026.pdf", &l),
            "Edler Kranich"
        );
        assert_eq!(vorschlag("stiller_fuchs.pdf", &l), "Stiller Fuchs");
        assert_eq!(vorschlag("Klausur_07.pdf", &l), "Klausur 07");
    }
}
