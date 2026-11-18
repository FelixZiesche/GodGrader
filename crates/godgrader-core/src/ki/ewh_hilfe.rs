//! „KI-Hilfe beim Import“: das Bewerten-Modell gliedert eine unsauber
//! erkannte Aufgabe in Bewertungselemente. Der Erwartungshorizont enthält
//! keine personenbezogenen Daten. Das Ergebnis prüft die Lehrkraft im Editor.

use serde::Deserialize;
use serde_json::json;

use super::anfragen::{be_modus, ModellParameter};
use super::aufruf::{self, Nutzung};
use super::prompts::fuellen;
use super::schema;
use crate::ewh::{be_text, Aufgabe, Element, ElementArt};
use crate::models::ModelInfo;
use crate::openrouter::{provider_prefs, OpenRouter};
use crate::slots::SlotConfig;
use crate::{Error, Result};

#[derive(Deserialize)]
struct Antwort {
    elemente: Vec<Roh>,
}

#[derive(Deserialize)]
struct Roh {
    titel: String,
    #[serde(default)]
    text: String,
    max_be: f64,
}

pub async fn gliedern(
    client: &OpenRouter,
    slot: &SlotConfig,
    info: Option<&ModelInfo>,
    vorlage: &str,
    aufgabe: &Aufgabe,
    halbe_be: bool,
    zdr_only: bool,
) -> Result<(Vec<Element>, Nutzung)> {
    let system = fuellen(vorlage, &[("{BE_MODUS}", be_modus(halbe_be))]);
    let nutzer = format!(
        "## Aufgabe {} ({} BE)\n\n{}\n\n## Musterlösung\n\n{}",
        aufgabe.nr,
        be_text(aufgabe.max_be),
        aufgabe.text.trim(),
        if aufgabe.loesung.trim().is_empty() {
            aufgabe
                .elemente
                .iter()
                .map(|e| format!("- {} ({} BE): {}", e.titel, be_text(e.max_be), e.text))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            aufgabe.loesung.trim().to_string()
        }
    );
    let p = ModellParameter::aus(slot, info);
    let mut body = json!({
        "model": p.model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": nutzer },
        ],
        "provider": provider_prefs(zdr_only),
        "usage": { "include": true },
    });
    if let Some(m) = p.max_tokens {
        body["max_tokens"] = json!(m);
    }
    if let Some(t) = p.temperature {
        body["temperature"] = json!(t);
    }
    if p.structured {
        body["response_format"] = json!({
            "type": "json_schema",
            "json_schema": { "name": "elemente", "strict": true, "schema": schema::ewh_hilfe_schema() }
        });
    }
    let a = aufruf::senden(client, &body, info).await?;
    let roh: Antwort = serde_json::from_value(schema::json_aus_text(&a.inhalt)?)
        .map_err(|e| Error::Ungueltig(format!("Antwort hat ein unerwartetes Format: {e}")))?;
    let elemente = roh
        .elemente
        .into_iter()
        .filter(|r| r.max_be.is_finite() && r.max_be > 0.0)
        .enumerate()
        .map(|(i, r)| Element {
            id: format!("{}-{}", aufgabe.nr, i + 1),
            titel: r.titel.trim().to_string(),
            text: r.text.trim().to_string(),
            max_be: r.max_be,
            art: ElementArt::Element,
            afb: aufgabe.haupt_afb(),
        })
        .collect::<Vec<_>>();
    if elemente.is_empty() {
        return Err(Error::Ungueltig(
            "Das Modell hat keine Elemente geliefert.".into(),
        ));
    }
    Ok((elemente, a.nutzung))
}
