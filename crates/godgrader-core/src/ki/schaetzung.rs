//! Kostenschätzung vor einem Lauf. Grundlage sind die Modellpreise und
//! grobe Tokenzahlen; sobald es genug echte Aufrufe im Projekt gibt, deren
//! Durchschnitt.

use serde::{Deserialize, Serialize};

use super::aufruf::kosten_aus_preisen;
use super::speicher::{SchrittArt, SchrittStatus};
use crate::ewh::{Aufgabe, Erwartungshorizont};
use crate::models::ModelInfo;
use crate::projekt::Projekt;
use crate::scans::KlausurStatus;
use crate::slots::{ReasoningEffort, SlotConfig, SlotKind, SlotSet};
use crate::Result;

/// Tokens pro Seitenbild (300 dpi, je nach Modell 1 100–2 300).
const TOKENS_BILD: u64 = 1_800;
/// Ausgabe pro Seite (Abschrift einer dicht beschriebenen Seite als JSON).
const TOKENS_ABSCHRIFT_SEITE: u64 = 650;
const TOKENS_PROMPT: u64 = 800;

fn denk_faktor(r: ReasoningEffort) -> f64 {
    match r {
        ReasoningEffort::Standard | ReasoningEffort::Niedrig => 1.5,
        ReasoningEffort::Mittel => 3.0,
        ReasoningEffort::Hoch => 5.0,
    }
}

fn tokens_text(s: &str) -> u64 {
    (s.chars().count() as f64 / 3.2).ceil() as u64
}

/// Geschätzte Kosten eines Abschreib-Aufrufs.
pub fn kosten_abschreiben(seitenbilder: usize, slot: &SlotConfig, info: Option<&ModelInfo>) -> f64 {
    let ein = TOKENS_PROMPT + TOKENS_BILD * seitenbilder as u64;
    let aus =
        (TOKENS_ABSCHRIFT_SEITE as f64 * seitenbilder as f64 * denk_faktor(slot.reasoning)) as u64;
    kosten_aus_preisen(info, ein, aus)
}

/// Geschätzte Kosten eines Bewerten-Aufrufs für eine Aufgabe.
pub fn kosten_bewerten(
    aufgabe: &Aufgabe,
    abschrift_tokens: u64,
    slot: &SlotConfig,
    info: Option<&ModelInfo>,
) -> f64 {
    let ewh_tokens = tokens_text(&aufgabe.text)
        + tokens_text(&aufgabe.loesung)
        + aufgabe
            .elemente
            .iter()
            .map(|e| tokens_text(&e.titel) + tokens_text(&e.text) + 20)
            .sum::<u64>();
    let ein = TOKENS_PROMPT + ewh_tokens + abschrift_tokens;
    let aus =
        ((150 + 120 * aufgabe.elemente.len() as u64) as f64 * denk_faktor(slot.reasoning)) as u64;
    kosten_aus_preisen(info, ein, aus)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Schaetzung {
    pub klausuren: usize,
    pub seitenbilder: usize,
    pub aufrufe: usize,
    /// US-Dollar, wahrscheinlichster Wert.
    pub kosten: f64,
    pub kosten_min: f64,
    pub kosten_max: f64,
    /// Durchschnitt bisheriger Aufrufe statt Preisliste.
    pub aus_erfahrung: bool,
    /// Modelle ohne bekannten Preis (Schätzung unvollständig).
    pub ohne_preis: Vec<String>,
    pub bisher: f64,
    pub budget: Option<f64>,
}

/// Schätzt die Kosten für die noch offenen Schritte der gewählten Klausuren.
pub fn schaetzen(
    p: &Projekt,
    ewh: &Erwartungshorizont,
    slots: &SlotSet,
    modelle: &[ModelInfo],
    klausuren: &[String],
) -> Result<Schaetzung> {
    let info = |k: SlotKind| modelle.iter().find(|m| m.id == slots.get(k).model);
    let mut s = Schaetzung {
        bisher: p.kosten_gesamt()?,
        budget: p.meta()?.budget_usd,
        ..Default::default()
    };
    for k in [
        SlotKind::Abschreiben,
        SlotKind::Bewerten,
        SlotKind::Zweitmeinung,
    ] {
        if k == SlotKind::Zweitmeinung && !slots.zweitmeinung.enabled {
            continue;
        }
        let i = info(k);
        if i.and_then(|m| m.price_in_per_m).is_none() {
            s.ohne_preis.push(slots.get(k).model.clone());
        }
    }
    // Erfahrungswerte: Durchschnitt erfolgreicher Aufrufe je Art
    let aufrufe = p.aufrufe(None)?;
    let schnitt = |art: &str| {
        let v: Vec<f64> = aufrufe
            .iter()
            .filter(|a| a.ok && a.art == art && a.kosten > 0.0)
            .map(|a| a.kosten)
            .collect();
        (v.len() >= 3).then(|| v.iter().sum::<f64>() / v.len() as f64)
    };
    let alle_klausuren = p.klausuren()?;
    let mut erfahrung = false;
    for kid in klausuren {
        let Some(k) = alle_klausuren.iter().find(|k| &k.id == kid) else {
            continue;
        };
        if k.status == KlausurStatus::Freigegeben {
            continue;
        }
        s.klausuren += 1;
        let seiten = k.seiten.len();
        let fertig = |art: SchrittArt, a: &str| -> Result<bool> {
            Ok(p.schritt(kid, art, a)?
                .map(|x| x.status == SchrittStatus::Fertig)
                == Some(true))
        };
        if !fertig(SchrittArt::Abschreiben, "")? {
            s.seitenbilder += seiten;
            s.aufrufe += 1;
            s.kosten += match schnitt("abschreiben") {
                Some(c) => {
                    erfahrung = true;
                    c
                }
                None => kosten_abschreiben(seiten, &slots.abschreiben, info(SlotKind::Abschreiben)),
            };
        }
        // Ohne Abschrift: Seiten gleichmäßig auf die Aufgaben verteilen
        let je_aufgabe =
            (seiten as u64 * TOKENS_ABSCHRIFT_SEITE) / ewh.aufgaben.len().max(1) as u64;
        for a in &ewh.aufgaben {
            let abschrift_tokens = p
                .abschrift_text(kid, &a.nr)?
                .map(|t| tokens_text(&t))
                .unwrap_or(je_aufgabe);
            for (art, kind) in [
                (SchrittArt::Bewerten, SlotKind::Bewerten),
                (SchrittArt::Zweitmeinung, SlotKind::Zweitmeinung),
            ] {
                if kind == SlotKind::Zweitmeinung && !slots.zweitmeinung.enabled {
                    continue;
                }
                if fertig(art, &a.nr)? {
                    continue;
                }
                s.aufrufe += 1;
                s.kosten += match schnitt(art.as_str()) {
                    Some(c) => {
                        erfahrung = true;
                        c
                    }
                    None => kosten_bewerten(a, abschrift_tokens, slots.get(kind), info(kind)),
                };
            }
        }
    }
    s.aus_erfahrung = erfahrung;
    let (lo, hi) = if erfahrung { (0.75, 1.4) } else { (0.5, 2.0) };
    s.kosten_min = s.kosten * lo;
    s.kosten_max = s.kosten * hi;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grobe_groessenordnung() {
        let info = ModelInfo {
            id: "g/p".into(),
            name: "P".into(),
            provider: "g".into(),
            created: 0,
            context_length: None,
            price_in_per_m: Some(2.0),
            price_out_per_m: Some(12.0),
            reads_images: true,
            supports_reasoning: true,
            supports_structured: true,
            supports_temperature: true,
            max_completion_tokens: None,
        };
        let slot = SlotConfig::default_for(SlotKind::Abschreiben);
        let k = kosten_abschreiben(4, &slot, Some(&info));
        // 4 Seiten: einige Cent, nicht Dollar
        assert!(k > 0.02 && k < 0.2, "{k}");
        assert_eq!(kosten_abschreiben(4, &slot, None), 0.0);
    }
}
