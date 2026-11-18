//! JSON-Schemas der Modellantworten, Auslesen und Prüfen.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::anfragen::BewertAufgabe;
use crate::{Error, Result};

pub fn abschrift_schema(nummern: &[String]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["aufgaben", "nicht_zugeordnet"],
        "properties": {
            "aufgaben": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["aufgabe", "abschrift", "unsichere_stellen", "seiten"],
                    "properties": {
                        "aufgabe": { "type": "string", "enum": nummern },
                        "abschrift": { "type": "string" },
                        "unsichere_stellen": { "type": "array", "items": { "type": "string" } },
                        "seiten": { "type": "array", "items": { "type": "integer" } }
                    }
                }
            },
            "nicht_zugeordnet": { "type": "string" }
        }
    })
}

pub fn bewertung_schema(aufgabe: &str, element_ids: &[String]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["aufgabe", "elemente", "summe_vorschlag", "hinweis_fuer_lehrkraft"],
        "properties": {
            "aufgabe": { "type": "string", "enum": [aufgabe] },
            "elemente": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["element_id", "max_be", "vorschlag_be", "begruendung", "beleg", "sicherheit", "haengt_an_unsicherer_stelle"],
                    "properties": {
                        "element_id": { "type": "string", "enum": element_ids },
                        "max_be": { "type": "number" },
                        "vorschlag_be": { "type": "number" },
                        "begruendung": { "type": "string" },
                        "beleg": { "type": "string" },
                        "sicherheit": { "type": "string", "enum": ["hoch", "mittel", "niedrig"] },
                        "haengt_an_unsicherer_stelle": { "type": "boolean" }
                    }
                }
            },
            "summe_vorschlag": { "type": "number" },
            "hinweis_fuer_lehrkraft": { "type": "string" }
        }
    })
}

pub fn ewh_hilfe_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["elemente"],
        "properties": {
            "elemente": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["titel", "text", "max_be"],
                    "properties": {
                        "titel": { "type": "string" },
                        "text": { "type": "string" },
                        "max_be": { "type": "number" }
                    }
                }
            }
        }
    })
}

/// Holt das JSON-Objekt aus einer Modellantwort, auch wenn es in
/// ```json … ``` oder in erklärenden Text eingebettet ist.
pub fn json_aus_text(text: &str) -> Result<Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        return Ok(v);
    }
    let start = t.find('{');
    let ende = t.rfind('}');
    if let (Some(a), Some(b)) = (start, ende) {
        if a < b {
            if let Ok(v) = serde_json::from_str::<Value>(&t[a..=b]) {
                return Ok(v);
            }
        }
    }
    Err(Error::Ungueltig(
        "Die Antwort des Modells enthielt kein gültiges JSON.".into(),
    ))
}

// ------------------------------------------------------------------
// Abschrift
// ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AbschriftAufgabe {
    pub aufgabe: String,
    pub abschrift: String,
    #[serde(default)]
    pub unsichere_stellen: Vec<String>,
    #[serde(default)]
    pub seiten: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AbschriftErgebnis {
    pub aufgaben: Vec<AbschriftAufgabe>,
    #[serde(default)]
    pub nicht_zugeordnet: String,
}

fn norm_nr(s: &str) -> String {
    s.trim()
        .trim_start_matches("Aufgabe")
        .trim()
        .trim_end_matches(['.', ')'])
        .to_lowercase()
}

/// Liest die Abschrift. Unbekannte Aufgabennummern landen in
/// „nicht zugeordnet“, doppelte werden zusammengeführt.
pub fn abschrift_lesen(v: Value, nummern: &[String]) -> Result<AbschriftErgebnis> {
    let roh: AbschriftErgebnis = serde_json::from_value(v)
        .map_err(|e| Error::Ungueltig(format!("Abschrift hat ein unerwartetes Format: {e}")))?;
    let mut aus = AbschriftErgebnis {
        aufgaben: vec![],
        nicht_zugeordnet: roh.nicht_zugeordnet.trim().to_string(),
    };
    for a in roh.aufgaben {
        let nr = nummern.iter().find(|n| norm_nr(n) == norm_nr(&a.aufgabe));
        match nr {
            Some(nr) => {
                if let Some(vorh) = aus.aufgaben.iter_mut().find(|x| &x.aufgabe == nr) {
                    vorh.abschrift = format!("{}\n\n{}", vorh.abschrift.trim_end(), a.abschrift);
                    vorh.unsichere_stellen.extend(a.unsichere_stellen);
                    for s in a.seiten {
                        if !vorh.seiten.contains(&s) {
                            vorh.seiten.push(s);
                        }
                    }
                } else {
                    aus.aufgaben.push(AbschriftAufgabe {
                        aufgabe: nr.clone(),
                        ..a
                    });
                }
            }
            None => {
                let zusatz = format!("[Aufgabe {}]\n{}", a.aufgabe.trim(), a.abschrift.trim());
                aus.nicht_zugeordnet = if aus.nicht_zugeordnet.is_empty() {
                    zusatz
                } else {
                    format!("{}\n\n{zusatz}", aus.nicht_zugeordnet)
                };
            }
        }
    }
    // Reihenfolge wie im Erwartungshorizont
    aus.aufgaben.sort_by_key(|a| {
        nummern
            .iter()
            .position(|n| n == &a.aufgabe)
            .unwrap_or(usize::MAX)
    });
    Ok(aus)
}

// ------------------------------------------------------------------
// Bewertung
// ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Sicherheit {
    Hoch,
    #[default]
    Mittel,
    Niedrig,
}

impl Sicherheit {
    pub fn as_str(self) -> &'static str {
        match self {
            Sicherheit::Hoch => "hoch",
            Sicherheit::Mittel => "mittel",
            Sicherheit::Niedrig => "niedrig",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "hoch" => Sicherheit::Hoch,
            "niedrig" => Sicherheit::Niedrig,
            _ => Sicherheit::Mittel,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ElementBewertung {
    pub element_id: String,
    #[serde(default)]
    pub max_be: f64,
    pub vorschlag_be: f64,
    #[serde(default)]
    pub begruendung: String,
    #[serde(default)]
    pub beleg: String,
    #[serde(default)]
    pub sicherheit: Sicherheit,
    #[serde(default)]
    pub haengt_an_unsicherer_stelle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BewertErgebnis {
    #[serde(default)]
    pub aufgabe: String,
    pub elemente: Vec<ElementBewertung>,
    #[serde(default)]
    pub summe_vorschlag: f64,
    #[serde(default)]
    pub hinweis_fuer_lehrkraft: String,
}

/// Auf ganze bzw. halbe BE abrunden (nie aufrunden: im Zweifel prüft die Lehrkraft).
fn runden(be: f64, halbe: bool) -> f64 {
    if halbe {
        (be * 2.0 + 1e-9).floor() / 2.0
    } else {
        (be + 1e-9).floor()
    }
}

/// Liest und prüft eine Bewertung. Ergebnis: bereinigte Bewertung in der
/// Reihenfolge der Elemente plus Hinweise auf Korrekturen.
pub fn bewertung_lesen(
    v: Value,
    aufgabe: &BewertAufgabe,
    halbe_be: bool,
) -> Result<(BewertErgebnis, Vec<String>)> {
    let roh: BewertErgebnis = serde_json::from_value(v)
        .map_err(|e| Error::Ungueltig(format!("Bewertung hat ein unerwartetes Format: {e}")))?;
    let mut hinweise = Vec::new();
    let mut elemente = Vec::new();
    for el in &aufgabe.elemente {
        let Some(b) = roh.elemente.iter().find(|b| b.element_id.trim() == el.id) else {
            return Err(Error::Ungueltig(format!(
                "Bewertung unvollständig: Element {} fehlt.",
                el.id
            )));
        };
        let mut b = b.clone();
        b.max_be = el.max_be;
        if !b.vorschlag_be.is_finite() || b.vorschlag_be < 0.0 {
            hinweise.push(format!("{}: ungültiger Vorschlag, auf 0 gesetzt.", el.id));
            b.vorschlag_be = 0.0;
            b.sicherheit = Sicherheit::Niedrig;
        }
        if b.vorschlag_be > el.max_be {
            hinweise.push(format!(
                "{}: Vorschlag über dem Maximum, auf {} begrenzt.",
                el.id, el.max_be
            ));
            b.vorschlag_be = el.max_be;
        }
        let r = runden(b.vorschlag_be, halbe_be);
        if (r - b.vorschlag_be).abs() > 1e-9 {
            hinweise.push(format!(
                "{}: {} BE abgerundet auf {}.",
                el.id, b.vorschlag_be, r
            ));
            b.vorschlag_be = r;
        }
        if b.haengt_an_unsicherer_stelle {
            b.sicherheit = Sicherheit::Niedrig;
        }
        elemente.push(b);
    }
    let summe = elemente.iter().map(|e| e.vorschlag_be).sum();
    Ok((
        BewertErgebnis {
            aufgabe: aufgabe.nr.clone(),
            elemente,
            summe_vorschlag: summe,
            hinweis_fuer_lehrkraft: roh.hinweis_fuer_lehrkraft.trim().to_string(),
        },
        hinweise,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ki::anfragen::BewertElement;

    fn aufgabe() -> BewertAufgabe {
        BewertAufgabe {
            nr: "2".into(),
            text: String::new(),
            max_be: 4.0,
            loesung: String::new(),
            elemente: vec![
                BewertElement {
                    id: "2-1".into(),
                    titel: "A".into(),
                    text: String::new(),
                    max_be: 2.0,
                    raster: false,
                },
                BewertElement {
                    id: "2-2".into(),
                    titel: "B".into(),
                    text: String::new(),
                    max_be: 2.0,
                    raster: false,
                },
            ],
        }
    }

    #[test]
    fn json_in_text() {
        let v = json_aus_text("Hier:\n```json\n{\"a\": 1}\n```").unwrap();
        assert_eq!(v["a"], 1);
        assert!(json_aus_text("nichts").is_err());
    }

    #[test]
    fn bewertung_wird_bereinigt() {
        let v = json!({
            "aufgabe": "2",
            "elemente": [
                { "element_id": "2-2", "max_be": 2, "vorschlag_be": 3, "begruendung": "x", "beleg": "", "sicherheit": "hoch", "haengt_an_unsicherer_stelle": false },
                { "element_id": "2-1", "max_be": 2, "vorschlag_be": 1.5, "begruendung": "y", "beleg": "z", "sicherheit": "hoch", "haengt_an_unsicherer_stelle": true }
            ],
            "summe_vorschlag": 4.5,
            "hinweis_fuer_lehrkraft": ""
        });
        let (b, h) = bewertung_lesen(v.clone(), &aufgabe(), false).unwrap();
        assert_eq!(b.elemente[0].element_id, "2-1");
        assert_eq!(b.elemente[0].vorschlag_be, 1.0);
        assert_eq!(b.elemente[0].sicherheit, Sicherheit::Niedrig);
        assert_eq!(b.elemente[1].vorschlag_be, 2.0);
        assert_eq!(b.summe_vorschlag, 3.0);
        assert_eq!(h.len(), 2);
        let (b, _) = bewertung_lesen(v, &aufgabe(), true).unwrap();
        assert_eq!(b.elemente[0].vorschlag_be, 1.5);
    }

    #[test]
    fn fehlendes_element_ist_fehler() {
        let v = json!({ "elemente": [ { "element_id": "2-1", "vorschlag_be": 1 } ] });
        assert!(bewertung_lesen(v, &aufgabe(), false).is_err());
    }

    #[test]
    fn abschrift_zuordnung() {
        let n = vec!["1a".to_string(), "1b".to_string(), "2".to_string()];
        let v = json!({
            "aufgaben": [
                { "aufgabe": "2", "abschrift": "zwei", "unsichere_stellen": [], "seiten": [2] },
                { "aufgabe": "1A", "abschrift": "eins", "unsichere_stellen": ["[?Wort?]"], "seiten": [1] },
                { "aufgabe": "2", "abschrift": "weiter", "unsichere_stellen": [], "seiten": [3] },
                { "aufgabe": "7", "abschrift": "falsch", "unsichere_stellen": [], "seiten": [3] }
            ],
            "nicht_zugeordnet": ""
        });
        let a = abschrift_lesen(v, &n).unwrap();
        assert_eq!(a.aufgaben.len(), 2);
        assert_eq!(a.aufgaben[0].aufgabe, "1a");
        assert_eq!(a.aufgaben[1].abschrift, "zwei\n\nweiter");
        assert_eq!(a.aufgaben[1].seiten, vec![2, 3]);
        assert!(a.nicht_zugeordnet.contains("[Aufgabe 7]"));
    }
}
