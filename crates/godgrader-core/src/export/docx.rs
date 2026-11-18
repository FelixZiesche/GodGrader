//! Kleiner Word-Schreiber für die Ausgaben im Stil von Felix' Materialien:
//! Google Sans 11 pt, Titel 14 pt fett, Überschriften 11 pt fett,
//! schwarz-weiß, Tabellenköpfe E6E6E6, Linien A6A6A6, A4 mit Rändern
//! oben 1 cm, unten 1 cm, links 2 cm, rechts 1,5 cm.

use std::io::Write;

use zip::write::SimpleFileOptions;

use crate::Result;

/// Nutzbare Breite in Twips (A4 minus Ränder).
pub const BREITE: u32 = 11906 - 1134 - 851;

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            // Steuerzeichen sind in XML verboten
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

/// Ein Textstück mit Auszeichnung.
#[derive(Debug, Clone, Default)]
pub struct Run {
    pub text: String,
    pub fett: bool,
    pub kursiv: bool,
    /// Halbpunkte; `None` = 11 pt.
    pub groesse: Option<u32>,
}

impl Run {
    pub fn t(s: impl Into<String>) -> Self {
        Run {
            text: s.into(),
            ..Default::default()
        }
    }
    pub fn f(s: impl Into<String>) -> Self {
        Run {
            text: s.into(),
            fett: true,
            ..Default::default()
        }
    }
    pub fn k(s: impl Into<String>) -> Self {
        Run {
            text: s.into(),
            kursiv: true,
            ..Default::default()
        }
    }

    fn xml(&self) -> String {
        let mut rpr = String::new();
        if self.fett {
            rpr.push_str("<w:b/>");
        }
        if self.kursiv {
            rpr.push_str("<w:i/>");
        }
        if let Some(g) = self.groesse {
            rpr.push_str(&format!("<w:sz w:val=\"{g}\"/><w:szCs w:val=\"{g}\"/>"));
        }
        let rpr = if rpr.is_empty() {
            String::new()
        } else {
            format!("<w:rPr>{rpr}</w:rPr>")
        };
        let mut o = String::new();
        for (i, zeile) in self.text.split('\n').enumerate() {
            if i > 0 {
                o.push_str(&format!("<w:r>{rpr}<w:br/></w:r>"));
            }
            for (j, teil) in zeile.split('\t').enumerate() {
                if j > 0 {
                    o.push_str(&format!("<w:r>{rpr}<w:tab/></w:r>"));
                }
                if !teil.is_empty() {
                    o.push_str(&format!(
                        "<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
                        esc(teil)
                    ));
                }
            }
        }
        o
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ausrichtung {
    #[default]
    Links,
    Mitte,
    Rechts,
}

impl Ausrichtung {
    fn xml(self) -> &'static str {
        match self {
            Ausrichtung::Links => "",
            Ausrichtung::Mitte => "<w:jc w:val=\"center\"/>",
            Ausrichtung::Rechts => "<w:jc w:val=\"right\"/>",
        }
    }
}

/// Tabellenzelle.
#[derive(Debug, Clone, Default)]
pub struct Zelle {
    pub runs: Vec<Run>,
    pub ausrichtung: Ausrichtung,
    /// Hintergrund als Hex, z. B. `E6E6E6`.
    pub farbe: Option<String>,
    pub spannen: u32,
}

impl Zelle {
    pub fn t(s: impl Into<String>) -> Self {
        Zelle {
            runs: vec![Run::t(s)],
            ..Default::default()
        }
    }
    pub fn f(s: impl Into<String>) -> Self {
        Zelle {
            runs: vec![Run::f(s)],
            ..Default::default()
        }
    }
    pub fn rechts(mut self) -> Self {
        self.ausrichtung = Ausrichtung::Rechts;
        self
    }
    pub fn mitte(mut self) -> Self {
        self.ausrichtung = Ausrichtung::Mitte;
        self
    }
    pub fn farbe(mut self, hex: &str) -> Self {
        self.farbe = Some(hex.into());
        self
    }
    pub fn spannen(mut self, n: u32) -> Self {
        self.spannen = n;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct Tabelle {
    /// Spaltenbreiten in Twips; Summe ≈ [`BREITE`].
    pub breiten: Vec<u32>,
    /// Kopfzeile (grau hinterlegt, fett, wird auf Folgeseiten wiederholt).
    pub kopf: Vec<Zelle>,
    pub zeilen: Vec<Vec<Zelle>>,
    /// Ohne Linien (z. B. für den Kopfbereich).
    pub ohne_linien: bool,
    /// Kleinere Zellabstände (für Diagramme aus Zellen).
    pub eng: bool,
}

/// Ein Word-Dokument im Aufbau.
#[derive(Debug, Default)]
pub struct Doc {
    body: String,
}

impl Doc {
    pub fn neu() -> Self {
        Self::default()
    }

    pub fn absatz_mit(
        &mut self,
        runs: &[Run],
        ausrichtung: Ausrichtung,
        abstand_vor: u32,
        abstand_nach: Option<u32>,
    ) {
        let nach = abstand_nach
            .map(|n| format!(" w:after=\"{n}\""))
            .unwrap_or_default();
        self.body.push_str(&format!(
            "<w:p><w:pPr><w:spacing w:before=\"{abstand_vor}\"{nach}/>{}</w:pPr>{}</w:p>",
            ausrichtung.xml(),
            runs.iter().map(Run::xml).collect::<String>()
        ));
    }

    pub fn absatz(&mut self, runs: &[Run]) {
        self.absatz_mit(runs, Ausrichtung::Links, 0, None);
    }

    pub fn text(&mut self, s: &str) {
        self.absatz(&[Run::t(s)]);
    }

    /// Titel: 14 pt fett, zentriert.
    pub fn titel(&mut self, s: &str) {
        self.absatz_mit(
            &[Run {
                text: s.into(),
                fett: true,
                groesse: Some(28),
                ..Default::default()
            }],
            Ausrichtung::Mitte,
            0,
            Some(60),
        );
    }

    /// Überschrift: 11 pt fett mit Abstand davor.
    pub fn ueberschrift(&mut self, s: &str) {
        self.body.push_str(&format!(
            "<w:p><w:pPr><w:keepNext/><w:spacing w:before=\"200\" w:after=\"80\"/></w:pPr>{}</w:p>",
            Run::f(s).xml()
        ));
    }

    pub fn leer(&mut self) {
        self.body.push_str("<w:p/>");
    }

    pub fn seitenumbruch(&mut self) {
        self.body
            .push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
    }

    pub fn tabelle(&mut self, t: &Tabelle) {
        let linie = |seite: &str| {
            format!("<w:{seite} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"A6A6A6\"/>")
        };
        let rahmen = if t.ohne_linien {
            "<w:tblBorders><w:top w:val=\"nil\"/><w:left w:val=\"nil\"/><w:bottom w:val=\"nil\"/><w:right w:val=\"nil\"/><w:insideH w:val=\"nil\"/><w:insideV w:val=\"nil\"/></w:tblBorders>".to_string()
        } else {
            format!(
                "<w:tblBorders>{}{}{}{}{}{}</w:tblBorders>",
                linie("top"),
                linie("left"),
                linie("bottom"),
                linie("right"),
                linie("insideH"),
                linie("insideV")
            )
        };
        let (rand_lr, rand_ob) = if t.eng { (40, 10) } else { (85, 30) };
        let gesamt: u32 = t.breiten.iter().sum();
        let mut x = format!(
            "<w:tbl><w:tblPr><w:tblW w:w=\"{gesamt}\" w:type=\"dxa\"/><w:tblLayout w:type=\"fixed\"/>{rahmen}<w:tblCellMar><w:top w:w=\"{rand_ob}\" w:type=\"dxa\"/><w:left w:w=\"{rand_lr}\" w:type=\"dxa\"/><w:bottom w:w=\"{rand_ob}\" w:type=\"dxa\"/><w:right w:w=\"{rand_lr}\" w:type=\"dxa\"/></w:tblCellMar></w:tblPr><w:tblGrid>"
        );
        for b in &t.breiten {
            x.push_str(&format!("<w:gridCol w:w=\"{b}\"/>"));
        }
        x.push_str("</w:tblGrid>");
        let zeile = |zellen: &[Zelle], kopf: bool, breiten: &[u32]| -> String {
            let mut z = String::from("<w:tr>");
            if kopf {
                z = String::from("<w:tr><w:trPr><w:tblHeader/><w:cantSplit/></w:trPr>");
            } else {
                z.push_str("<w:trPr><w:cantSplit/></w:trPr>");
            }
            let mut spalte = 0usize;
            for c in zellen {
                let span = c.spannen.max(1) as usize;
                let w: u32 = breiten.iter().skip(spalte).take(span).sum();
                spalte += span;
                let farbe = c
                    .farbe
                    .clone()
                    .or_else(|| kopf.then(|| "E6E6E6".to_string()))
                    .map(|f| format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{f}\"/>"))
                    .unwrap_or_default();
                let span_xml = if span > 1 {
                    format!("<w:gridSpan w:val=\"{span}\"/>")
                } else {
                    String::new()
                };
                let runs: Vec<Run> = c
                    .runs
                    .iter()
                    .cloned()
                    .map(|mut r| {
                        if kopf {
                            r.fett = true;
                        }
                        r
                    })
                    .collect();
                z.push_str(&format!(
                    "<w:tc><w:tcPr><w:tcW w:w=\"{w}\" w:type=\"dxa\"/>{span_xml}{farbe}</w:tcPr><w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\"/>{}</w:pPr>{}</w:p></w:tc>",
                    c.ausrichtung.xml(),
                    runs.iter().map(Run::xml).collect::<String>()
                ));
            }
            z.push_str("</w:tr>");
            z
        };
        if !t.kopf.is_empty() {
            x.push_str(&zeile(&t.kopf, true, &t.breiten));
        }
        for r in &t.zeilen {
            x.push_str(&zeile(r, false, &t.breiten));
        }
        x.push_str("</w:tbl>");
        self.body.push_str(&x);
        // Word braucht nach einer Tabelle einen Absatz, sonst kleben Tabellen zusammen
        self.body
            .push_str("<w:p><w:pPr><w:spacing w:before=\"0\" w:after=\"0\"/></w:pPr></w:p>");
    }

    /// Fügt ein weiteres Dokument an (für Sammeldateien).
    pub fn anhaengen(&mut self, other: Doc) {
        self.body.push_str(&other.body);
    }

    pub fn bytes(&self) -> Result<Vec<u8>> {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let o =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            let mut datei = |name: &str, inhalt: &str| -> Result<()> {
                z.start_file(name, o)
                    .map_err(|e| crate::Error::Ungueltig(e.to_string()))?;
                z.write_all(inhalt.as_bytes())?;
                Ok(())
            };
            datei("[Content_Types].xml", CONTENT_TYPES)?;
            datei("_rels/.rels", RELS)?;
            datei("word/_rels/document.xml.rels", DOC_RELS)?;
            datei("word/styles.xml", STYLES)?;
            datei("word/settings.xml", SETTINGS)?;
            datei(
                "word/document.xml",
                &format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><w:body>{}<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"567\" w:right=\"851\" w:bottom=\"567\" w:left=\"1134\" w:header=\"284\" w:footer=\"284\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>",
                    self.body
                ),
            )?;
            z.finish()
                .map_err(|e| crate::Error::Ungueltig(e.to_string()))?;
        }
        Ok(buf.into_inner())
    }
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

const DOC_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>"#;

const SETTINGS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:defaultTabStop w:val="709"/><w:characterSpacingControl w:val="doNotCompress"/><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>"#;

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Google Sans" w:hAnsi="Google Sans" w:eastAsia="Google Sans" w:cs="Google Sans"/><w:color w:val="000000"/><w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="de-DE"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="60" w:line="264" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="Standard"><w:name w:val="Normal"/><w:qFormat/></w:style><w:style w:type="table" w:default="1" w:styleId="NormaleTabelle"><w:name w:val="Normal Table"/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="85" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="85" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style></w:styles>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn dokument_ist_gueltiges_zip_mit_layout() {
        let mut d = Doc::neu();
        d.titel("Korrekturbogen");
        d.ueberschrift("Aufgaben");
        d.tabelle(&Tabelle {
            breiten: vec![2000, BREITE - 2000],
            kopf: vec![Zelle::t("A"), Zelle::t("B & <C>")],
            zeilen: vec![vec![Zelle::t("1"), Zelle::t("Zeile 1\nZeile 2")]],
            ..Default::default()
        });
        let bytes = d.bytes().unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut doc = String::new();
        zip.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc)
            .unwrap();
        assert!(doc.contains("B &amp; &lt;C&gt;"));
        assert!(doc.contains("w:fill=\"E6E6E6\""));
        assert!(doc.contains("w:color=\"A6A6A6\""));
        assert!(doc.contains("w:left=\"1134\"") && doc.contains("w:right=\"851\""));
        assert!(doc.contains("<w:sz w:val=\"28\"/>"));
        let mut st = String::new();
        zip.by_name("word/styles.xml")
            .unwrap()
            .read_to_string(&mut st)
            .unwrap();
        assert!(st.contains("Google Sans") && st.contains("<w:sz w:val=\"22\"/>"));
    }
}
