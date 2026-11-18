//! Anfragen an die Modelle, **strikt getrennt**:
//!
//! - [`AbschreibAnfrage`] kann nur aus Seitenbildern und der Liste der
//!   Aufgabennummern entstehen. Sie hat kein Feld, in das Aufgabentexte,
//!   Lösungen oder Bewertungselemente passen würden.
//! - [`BewertAnfrage`] enthält nur Text. Der Nachrichteninhalt ist ein
//!   einfacher String, Bilder sind damit technisch ausgeschlossen.
//!
//! Tests prüfen beides am fertigen JSON (siehe unten und
//! `tests/pipeline_fake.rs`).

use serde_json::{json, Value};

use super::prompts::fuellen;
use super::schema;
use crate::ewh::{be_text, Aufgabe, ElementArt, Erwartungshorizont, Stufe};
use crate::models::ModelInfo;
use crate::slots::SlotConfig;

/// Modellparameter für einen Aufruf, aus Slot und Modellbibliothek.
#[derive(Debug, Clone)]
pub struct ModellParameter {
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub reasoning: Option<&'static str>,
    pub structured: bool,
}

impl ModellParameter {
    /// Unbekannte Modelle (nicht in der Bibliothek) bekommen alle Felder;
    /// OpenRouter ignoriert, was ein Modell nicht kann.
    pub fn aus(slot: &SlotConfig, info: Option<&ModelInfo>) -> Self {
        let temp_ok = info.map(|m| m.supports_temperature).unwrap_or(true);
        let reas_ok = info.map(|m| m.supports_reasoning).unwrap_or(true);
        ModellParameter {
            model: slot.model.clone(),
            temperature: temp_ok.then_some(slot.temperature),
            max_tokens: slot.max_tokens,
            reasoning: if reas_ok {
                slot.reasoning.api_value()
            } else {
                None
            },
            structured: info.map(|m| m.supports_structured).unwrap_or(false),
        }
    }

    fn body(&self, messages: Value, schema: Option<(&str, Value)>, provider: &Value) -> Value {
        let mut b = json!({
            "model": self.model,
            "messages": messages,
            "provider": provider,
            "usage": { "include": true },
        });
        if let Some(t) = self.temperature {
            b["temperature"] = json!(t);
        }
        if let Some(m) = self.max_tokens {
            b["max_tokens"] = json!(m);
        }
        if let Some(r) = self.reasoning {
            b["reasoning"] = json!({ "effort": r });
        }
        if let (true, Some((name, s))) = (self.structured, schema) {
            b["response_format"] = json!({
                "type": "json_schema",
                "json_schema": { "name": name, "strict": true, "schema": s }
            });
        }
        b
    }
}

// ------------------------------------------------------------------
// Schritt 1: Abschreiben
// ------------------------------------------------------------------

/// Ein aufbereitetes Seitenbild (JPEG, schon geschwärzt).
#[derive(Debug, Clone)]
pub struct Seitenbild {
    /// z. B. „Seite 2“ oder „Seite 2, obere Hälfte“.
    pub bezeichnung: String,
    pub jpeg: Vec<u8>,
}

/// Anfrage an das Abschreiben-Modell. Felder privat: nur über [`AbschreibAnfrage::neu`].
#[derive(Debug, Clone)]
pub struct AbschreibAnfrage {
    aufgabennummern: Vec<String>,
    seiten: Vec<Seitenbild>,
}

impl AbschreibAnfrage {
    /// Übernimmt aus dem Erwartungshorizont **nur die Aufgabennummern**.
    pub fn neu(ewh: &Erwartungshorizont, seiten: Vec<Seitenbild>) -> Self {
        AbschreibAnfrage {
            aufgabennummern: ewh.aufgabennummern(),
            seiten,
        }
    }

    pub fn aufgabennummern(&self) -> &[String] {
        &self.aufgabennummern
    }

    pub fn seitenzahl(&self) -> usize {
        self.seiten.len()
    }

    pub fn body(&self, vorlage: &str, p: &ModellParameter, provider: &Value) -> Value {
        let liste = self.aufgabennummern.join(", ");
        let system = fuellen(vorlage, &[("{AUFGABENLISTE}", &liste)]);
        let mut inhalt = vec![json!({
            "type": "text",
            "text": format!(
                "Hier sind die {} Seitenbilder einer Klausur in der richtigen Reihenfolge. Mögliche Aufgabennummern: {liste}.",
                self.seiten.len()
            )
        })];
        for s in &self.seiten {
            inhalt.push(json!({ "type": "text", "text": s.bezeichnung }));
            inhalt.push(json!({
                "type": "image_url",
                "image_url": { "url": crate::scans::data_url_jpeg(&s.jpeg) }
            }));
        }
        let messages = json!([
            { "role": "system", "content": system },
            { "role": "user", "content": inhalt },
        ]);
        p.body(
            messages,
            Some(("abschrift", schema::abschrift_schema(&self.aufgabennummern))),
            provider,
        )
    }
}

// ------------------------------------------------------------------
// Schritt 2: Bewerten (auch Zweitmeinung)
// ------------------------------------------------------------------

/// Ein Bewertungselement, wie es das Bewerten-Modell sieht.
#[derive(Debug, Clone, PartialEq)]
pub struct BewertElement {
    pub id: String,
    pub titel: String,
    pub text: String,
    pub max_be: f64,
    pub raster: bool,
}

/// Aufgabe aus dem Erwartungshorizont für die Bewertung.
#[derive(Debug, Clone, PartialEq)]
pub struct BewertAufgabe {
    pub nr: String,
    pub text: String,
    pub max_be: f64,
    pub loesung: String,
    pub elemente: Vec<BewertElement>,
}

impl BewertAufgabe {
    pub fn aus(a: &Aufgabe) -> Self {
        BewertAufgabe {
            nr: a.nr.clone(),
            text: a.text.clone(),
            max_be: a.max_be,
            loesung: a.loesung.clone(),
            elemente: a
                .elemente
                .iter()
                .map(|e| BewertElement {
                    id: e.id.clone(),
                    titel: e.titel.clone(),
                    text: e.text.clone(),
                    max_be: e.max_be,
                    raster: e.art == ElementArt::Raster,
                })
                .collect(),
        }
    }
}

/// Anfrage an das Bewerten-Modell: nur Text, keine Bilder.
#[derive(Debug, Clone)]
pub struct BewertAnfrage {
    pub aufgabe: BewertAufgabe,
    /// Abschrift dieser Aufgabe (ggf. von der Lehrkraft korrigiert).
    pub abschrift: String,
    pub stufe: Stufe,
    pub halbe_be: bool,
}

pub fn stufe_text(s: Stufe) -> &'static str {
    match s {
        Stufe::Oberstufe => "Oberstufe, Notenpunkte 0–15",
        Stufe::Sek1 => "Sekundarstufe I, Noten 1–6",
    }
}

pub fn be_modus(halbe_be: bool) -> &'static str {
    if halbe_be {
        "ganze oder halbe BE"
    } else {
        "nur ganze BE"
    }
}

impl BewertAnfrage {
    pub fn nutzertext(&self) -> String {
        let a = &self.aufgabe;
        let mut t = format!(
            "## Aufgabe {}  ({} BE)\n\n{}\n\n## Bewertungselemente\n\n",
            a.nr,
            be_text(a.max_be),
            a.text.trim()
        );
        for e in &a.elemente {
            t.push_str(&format!(
                "- element_id `{}`{} – {} (max. {} BE): {}\n",
                e.id,
                if e.raster { " [Rasterkriterium]" } else { "" },
                e.titel.trim(),
                be_text(e.max_be),
                e.text.trim().replace('\n', " ")
            ));
        }
        if !a.loesung.trim().is_empty() {
            t.push_str(&format!(
                "\n## Erwartungshorizont (vollständig)\n\n{}\n",
                a.loesung.trim()
            ));
        }
        t.push_str("\nAndere sachlich richtige Lösungen sind gleichwertig.\n");
        t.push_str(&format!(
            "\n## Abschrift der Schülerantwort zu Aufgabe {}\n\n{}\n",
            a.nr,
            if self.abschrift.trim().is_empty() {
                "(keine Antwort)"
            } else {
                self.abschrift.trim()
            }
        ));
        t
    }

    pub fn body(&self, vorlage: &str, p: &ModellParameter, provider: &Value) -> Value {
        let system = fuellen(
            vorlage,
            &[
                ("{STUFE}", stufe_text(self.stufe)),
                ("{BE_MODUS}", be_modus(self.halbe_be)),
            ],
        );
        let ids: Vec<String> = self.aufgabe.elemente.iter().map(|e| e.id.clone()).collect();
        // Inhalt ist ein String: kein Platz für Bilder.
        let messages = json!([
            { "role": "system", "content": system },
            { "role": "user", "content": self.nutzertext() },
        ]);
        p.body(
            messages,
            Some((
                "bewertung",
                schema::bewertung_schema(&self.aufgabe.nr, &ids),
            )),
            provider,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ewh::{Element, Kopf};
    use crate::slots::SlotKind;

    pub(crate) fn ewh_mit_geheimnissen() -> Erwartungshorizont {
        let mut e = Erwartungshorizont {
            kopf: Kopf {
                titel: "Test".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        for (nr, wort) in [("1a", "GEHEIMLÖSUNG"), ("2", "MUSTERANTWORT")] {
            e.aufgaben.push(Aufgabe {
                nr: nr.into(),
                text: format!("Aufgabentext AUFGABENGEHEIMNIS {nr}"),
                max_be: 4.0,
                loesung: format!("Lösung {wort}"),
                elemente: vec![Element {
                    id: format!("{nr}-1"),
                    titel: format!("Titel ELEMENTTITEL {nr}"),
                    text: format!("Erwartet {wort}"),
                    max_be: 4.0,
                    art: ElementArt::Element,
                    afb: None,
                }],
                afb: vec![],
                hat_raster: false,
            });
        }
        e
    }

    #[test]
    fn abschreiben_ohne_erwartungshorizont() {
        let ewh = ewh_mit_geheimnissen();
        let a = AbschreibAnfrage::neu(
            &ewh,
            vec![Seitenbild {
                bezeichnung: "Seite 1".into(),
                jpeg: vec![0xFF, 0xD8, 0xFF],
            }],
        );
        let p = ModellParameter::aus(&SlotConfig::default_for(SlotKind::Abschreiben), None);
        let body = a.body(
            super::super::prompts::PromptArt::Abschreiben.standard(),
            &p,
            &crate::openrouter::provider_prefs(false),
        );
        let s = body.to_string();
        for verboten in [
            "GEHEIMLÖSUNG",
            "MUSTERANTWORT",
            "AUFGABENGEHEIMNIS",
            "ELEMENTTITEL",
            "Erwartet",
        ] {
            assert!(
                !s.contains(verboten),
                "Abschreib-Anfrage enthält {verboten}"
            );
        }
        assert!(s.contains("1a, 2"));
        assert!(s.contains("data:image/jpeg;base64,"));
        assert_eq!(body["provider"]["data_collection"], "deny");
    }

    #[test]
    fn bewerten_ohne_bilder() {
        let ewh = ewh_mit_geheimnissen();
        let b = BewertAnfrage {
            aufgabe: BewertAufgabe::aus(&ewh.aufgaben[0]),
            abschrift: "Die Antwort".into(),
            stufe: Stufe::Oberstufe,
            halbe_be: false,
        };
        let p = ModellParameter::aus(&SlotConfig::default_for(SlotKind::Bewerten), None);
        let body = b.body(
            super::super::prompts::PromptArt::Bewerten.standard(),
            &p,
            &crate::openrouter::provider_prefs(true),
        );
        let s = body.to_string();
        assert!(!s.contains("image_url"));
        assert!(!s.contains("data:image"));
        assert!(body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["content"].is_string()));
        assert!(s.contains("GEHEIMLÖSUNG") && s.contains("Die Antwort"));
        assert!(s.contains("nur ganze BE"));
        assert_eq!(body["provider"]["zdr"], true);
    }

    #[test]
    fn parameter_nach_faehigkeiten() {
        let mut slot = SlotConfig::default_for(SlotKind::Bewerten);
        slot.reasoning = crate::slots::ReasoningEffort::Hoch;
        let info = ModelInfo {
            id: "x/y".into(),
            name: "Y".into(),
            provider: "x".into(),
            created: 0,
            context_length: None,
            price_in_per_m: None,
            price_out_per_m: None,
            reads_images: false,
            supports_reasoning: false,
            supports_structured: true,
            supports_temperature: false,
            max_completion_tokens: None,
        };
        let p = ModellParameter::aus(&slot, Some(&info));
        assert!(p.temperature.is_none() && p.reasoning.is_none() && p.structured);
        let p2 = ModellParameter::aus(&slot, None);
        assert_eq!(p2.reasoning, Some("high"));
        assert!(!p2.structured);
    }
}
