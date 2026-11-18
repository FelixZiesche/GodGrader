//! Stand der Verarbeitung je Klausur, für die Seite „Verarbeitung“.

use serde::{Deserialize, Serialize};

use super::speicher::{SchrittArt, SchrittStatus};
use crate::ewh::Erwartungshorizont;
use crate::projekt::Projekt;
use crate::scans::KlausurStatus;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KlausurFortschritt {
    pub id: String,
    pub bezeichnung: String,
    pub status: KlausurStatus,
    pub seiten: usize,
    pub abschrift: SchrittStatus,
    /// z. B. „3 unsichere Stellen“.
    pub abschrift_hinweis: Option<String>,
    pub aufgaben: usize,
    pub bewertet: usize,
    pub zweit: usize,
    /// Läuft gerade ein Schritt dieser Klausur?
    pub laeuft: bool,
    /// Fehlermeldungen mit Schritt, z. B. „Bewerten 2: …“.
    pub fehler: Vec<String>,
    pub kosten: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Uebersicht {
    pub klausuren: Vec<KlausurFortschritt>,
    pub kosten_gesamt: f64,
    pub budget: Option<f64>,
    pub zweitmeinung: bool,
    pub hat_ewh: bool,
    /// Wird von der App gesetzt.
    pub laeuft: bool,
}

pub fn uebersicht(
    p: &Projekt,
    ewh: Option<&Erwartungshorizont>,
    zweitmeinung: bool,
) -> Result<Uebersicht> {
    let kosten = p.kosten_je_klausur()?;
    let schritte = p.alle_schritte()?;
    let aufgaben = ewh.map(|e| e.aufgaben.len()).unwrap_or(0);
    let nummern: Vec<String> = ewh.map(|e| e.aufgabennummern()).unwrap_or_default();
    let mut klausuren = Vec::new();
    for k in p.klausuren()? {
        let eigene: Vec<_> = schritte.iter().filter(|s| s.klausur_id == k.id).collect();
        let ab = eigene.iter().find(|s| s.art == SchrittArt::Abschreiben);
        let zaehle = |art: SchrittArt| {
            eigene
                .iter()
                .filter(|s| {
                    s.art == art
                        && s.status == SchrittStatus::Fertig
                        && nummern.contains(&s.aufgabe)
                })
                .count()
        };
        let fehler = eigene
            .iter()
            .filter(|s| s.status == SchrittStatus::Fehler)
            .map(|s| {
                let name = match s.art {
                    SchrittArt::Abschreiben => "Abschreiben".to_string(),
                    SchrittArt::Bewerten => format!("Bewerten {}", s.aufgabe),
                    SchrittArt::Zweitmeinung => format!("Zweitmeinung {}", s.aufgabe),
                };
                format!("{name}: {}", s.fehler.clone().unwrap_or_default())
            })
            .collect();
        klausuren.push(KlausurFortschritt {
            seiten: k.seiten.len(),
            abschrift: ab.map(|s| s.status).unwrap_or(SchrittStatus::Offen),
            abschrift_hinweis: ab.and_then(|s| s.hinweis.clone()),
            aufgaben,
            bewertet: zaehle(SchrittArt::Bewerten),
            zweit: zaehle(SchrittArt::Zweitmeinung),
            laeuft: eigene.iter().any(|s| s.status == SchrittStatus::Laeuft),
            fehler,
            kosten: kosten.get(&k.id).copied().unwrap_or(0.0),
            id: k.id,
            bezeichnung: k.bezeichnung,
            status: k.status,
        });
    }
    Ok(Uebersicht {
        klausuren,
        kosten_gesamt: p.kosten_gesamt()?,
        budget: p.meta()?.budget_usd,
        zweitmeinung,
        hat_ewh: ewh.is_some(),
        laeuft: false,
    })
}
