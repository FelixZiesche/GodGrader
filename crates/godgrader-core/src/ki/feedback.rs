//! Schritt 3: Feedback. Erst nach der Freigabe, auf Basis der finalen BE,
//! der Begründungen und der Abschrift. Nur Text, keine Bilder, keine
//! Bezeichnung der Klausur.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Semaphore;

use super::anfragen::ModellParameter;
use super::aufruf;
use super::pipeline::PipelineKonfig;
use super::prompts::fuellen;
use super::speicher::Aufruf;
use crate::ewh::{be_text, Erwartungshorizont, Stufe};
use crate::openrouter::{provider_prefs, OpenRouter};
use crate::projekt::Projekt;
use crate::scans::KlausurStatus;
use crate::slots::SlotKind;
use crate::{Error, Result};

pub(crate) const SCHEMA_V5: &str = "
CREATE TABLE IF NOT EXISTS feedback (
    klausur_id TEXT PRIMARY KEY REFERENCES klausuren(id) ON DELETE CASCADE,
    text TEXT NOT NULL,
    ki_text TEXT NOT NULL,
    aktualisiert TEXT NOT NULL
);
";

fn db(e: rusqlite::Error) -> Error {
    Error::Ungueltig(format!("Projektdatenbank: {e}"))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeedbackZeile {
    pub klausur_id: String,
    pub text: String,
    pub ki_text: String,
    pub aktualisiert: String,
}

impl Projekt {
    pub fn feedback(&self, kid: &str) -> Result<Option<FeedbackZeile>> {
        self.conn()
            .query_row(
                "SELECT klausur_id, text, ki_text, aktualisiert FROM feedback WHERE klausur_id = ?1",
                [kid],
                |r| {
                    Ok(FeedbackZeile {
                        klausur_id: r.get(0)?,
                        text: r.get(1)?,
                        ki_text: r.get(2)?,
                        aktualisiert: r.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(db)
    }

    /// Speichert Feedback. `von_ki`: neue KI-Fassung (überschreibt auch den Text).
    pub fn feedback_speichern(&self, kid: &str, text: &str, von_ki: bool) -> Result<()> {
        let jetzt = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        if von_ki {
            self.conn()
                .execute(
                    "INSERT INTO feedback (klausur_id, text, ki_text, aktualisiert) VALUES (?1, ?2, ?2, ?3)
                     ON CONFLICT(klausur_id) DO UPDATE SET text = excluded.text, ki_text = excluded.ki_text, aktualisiert = excluded.aktualisiert",
                    params![kid, text, jetzt],
                )
                .map_err(db)?;
        } else {
            self.conn()
                .execute(
                    "INSERT INTO feedback (klausur_id, text, ki_text, aktualisiert) VALUES (?1, ?2, '', ?3)
                     ON CONFLICT(klausur_id) DO UPDATE SET text = excluded.text, aktualisiert = excluded.aktualisiert",
                    params![kid, text, jetzt],
                )
                .map_err(db)?;
        }
        Ok(())
    }
}

pub fn anrede(s: Stufe) -> &'static str {
    match s {
        Stufe::Oberstufe => "Anrede mit „Sie“",
        Stufe::Sek1 => "Anrede mit „du“",
    }
}

/// Nutzertext für das Feedback-Modell: Aufgaben, finale BE, Begründungen
/// und Abschrift. Keine Bezeichnung der Klausur.
pub fn nutzertext(p: &Projekt, ewh: &Erwartungshorizont, kid: &str) -> Result<String> {
    let bew = p.bewertungen(kid)?;
    let abschriften = p.abschriften(kid)?;
    let mut t = String::from("Bewertete Klausur (finale Bewertung der Lehrkraft):\n");
    let mut summe = 0.0;
    for a in &ewh.aufgaben {
        let erreicht: f64 = a
            .elemente
            .iter()
            .map(|e| {
                bew.iter()
                    .find(|b| b.element_id == e.id)
                    .and_then(|b| b.final_be)
                    .unwrap_or(0.0)
            })
            .sum();
        summe += erreicht;
        t.push_str(&format!(
            "\n## Aufgabe {} ({} von {} BE)\n{}\n",
            a.nr,
            be_text(erreicht),
            be_text(a.max_be),
            a.text.trim()
        ));
        for e in &a.elemente {
            let b = bew.iter().find(|b| b.element_id == e.id);
            let f = b.and_then(|b| b.final_be).unwrap_or(0.0);
            t.push_str(&format!(
                "- {} ({} von {} BE)",
                e.titel.trim(),
                be_text(f),
                be_text(e.max_be)
            ));
            // Begründung der KI nur, wenn die Lehrkraft den Wert übernommen hat
            if let Some(b) = b.filter(|b| b.ki_be == Some(f) && !b.begruendung.trim().is_empty()) {
                t.push_str(&format!(": {}", b.begruendung.trim()));
            }
            t.push('\n');
        }
        if let Some(ab) = abschriften.iter().find(|x| x.aufgabe == a.nr) {
            if !ab.text.trim().is_empty() {
                t.push_str(&format!("Antwort (Abschrift):\n{}\n", ab.text.trim()));
            }
        }
    }
    t.push_str(&format!(
        "\nGesamt: {} von {} BE.\n",
        be_text(summe),
        be_text(ewh.gesamt_be())
    ));
    Ok(t)
}

pub fn body(
    vorlage: &str,
    stufe: Stufe,
    nutzer: String,
    p: &ModellParameter,
    zdr_only: bool,
) -> Value {
    let system = fuellen(vorlage, &[("{ANREDE}", anrede(stufe))]);
    let mut b = json!({
        "model": p.model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": nutzer },
        ],
        "provider": provider_prefs(zdr_only),
        "usage": { "include": true },
    });
    if let Some(t) = p.temperature {
        b["temperature"] = json!(t);
    }
    if let Some(m) = p.max_tokens {
        b["max_tokens"] = json!(m);
    }
    if let Some(r) = p.reasoning {
        b["reasoning"] = json!({ "effort": r });
    }
    b
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct FeedbackErgebnis {
    pub erzeugt: usize,
    /// „Bezeichnung: Fehler“
    pub fehler: Vec<String>,
    pub kosten: f64,
    pub budget_erreicht: bool,
}

/// Erzeugt Feedback für die gewählten (freigegebenen) Klausuren.
pub async fn erzeugen(
    projekt: Projekt,
    client: OpenRouter,
    konfig: PipelineKonfig,
    klausuren: Vec<String>,
) -> Result<FeedbackErgebnis> {
    let ewh = projekt
        .ewh()?
        .ok_or_else(|| Error::Ungueltig("Kein Erwartungshorizont im Projekt.".into()))?;
    let slot = konfig.slots.get(SlotKind::Feedback).clone();
    if slot.model.trim().is_empty() {
        return Err(Error::Ungueltig("Slot „Feedback“ hat kein Modell.".into()));
    }
    let info = konfig.modelle.iter().find(|m| m.id == slot.model).cloned();
    let params = ModellParameter::aus(&slot, info.as_ref());
    let budget = projekt.meta()?.budget_usd;
    let start_kosten = projekt.kosten_gesamt()?;
    let alle = projekt.klausuren()?;
    let p = Arc::new(Mutex::new(projekt));
    let sem = Arc::new(Semaphore::new(konfig.parallel.clamp(1, 16)));
    let erg = Arc::new(Mutex::new(FeedbackErgebnis::default()));
    let mut set = tokio::task::JoinSet::new();
    for kid in klausuren {
        let Some(k) = alle.iter().find(|k| k.id == kid).cloned() else {
            continue;
        };
        if k.status != KlausurStatus::Freigegeben {
            erg.lock()
                .expect("Sperre")
                .fehler
                .push(format!("{}: noch nicht freigegeben", k.bezeichnung));
            continue;
        }
        let nutzer = nutzertext(&p.lock().expect("Sperre"), &ewh, &kid)?;
        let b = body(
            &konfig.vorlagen.feedback,
            ewh.stufe,
            nutzer,
            &params,
            konfig.zdr_only,
        );
        let (p, sem, erg, client, info) = (
            p.clone(),
            sem.clone(),
            erg.clone(),
            client.clone(),
            info.clone(),
        );
        let modell = slot.model.clone();
        set.spawn(async move {
            let Ok(_permit) = sem.acquire().await else {
                return;
            };
            if let Some(b) = budget {
                let bisher = p.lock().expect("Sperre").kosten_gesamt().unwrap_or(0.0);
                if bisher >= b {
                    erg.lock().expect("Sperre").budget_erreicht = true;
                    return;
                }
            }
            let t0 = Instant::now();
            let res = aufruf::senden(&client, &b, info.as_ref()).await;
            let pr = p.lock().expect("Sperre");
            let mut a = Aufruf {
                klausur_id: Some(kid.clone()),
                art: "feedback".into(),
                modell,
                dauer_ms: t0.elapsed().as_millis() as u64,
                ..Default::default()
            };
            match res {
                Ok(antwort) => {
                    a.ok = true;
                    a.tokens_ein = antwort.nutzung.tokens_ein;
                    a.tokens_aus = antwort.nutzung.tokens_aus;
                    a.kosten = antwort.nutzung.kosten;
                    a.geschaetzt = antwort.nutzung.geschaetzt;
                    let _ = pr.aufruf_protokollieren(&a);
                    match pr.feedback_speichern(&kid, antwort.inhalt.trim(), true) {
                        Ok(()) => erg.lock().expect("Sperre").erzeugt += 1,
                        Err(e) => erg
                            .lock()
                            .expect("Sperre")
                            .fehler
                            .push(format!("{}: {e}", k.bezeichnung)),
                    }
                }
                Err(e) => {
                    if let Error::Abgeschnitten(c) = e {
                        a.kosten = c;
                    }
                    let _ = pr.aufruf_protokollieren(&a);
                    erg.lock()
                        .expect("Sperre")
                        .fehler
                        .push(format!("{}: {e}", k.bezeichnung));
                }
            }
        });
    }
    while set.join_next().await.is_some() {}
    let mut e = erg.lock().expect("Sperre").clone();
    e.kosten = p.lock().expect("Sperre").kosten_gesamt()? - start_kosten;
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ki::prompts::Vorlagen;
    use crate::settings::AppSettings;
    use std::sync::Mutex as StdMutex;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    #[tokio::test]
    async fn feedback_nur_freigegeben_ohne_bezeichnung() {
        let dir = tempfile::tempdir().unwrap();
        let p = crate::export::testdaten::projekt(dir.path());
        let server = MockServer::start().await;
        let bodies = Arc::new(StdMutex::new(Vec::<Value>::new()));
        let b2 = bodies.clone();
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(move |req: &Request| {
                b2.lock().unwrap().push(serde_json::from_slice(&req.body).unwrap());
                ResponseTemplate::new(200).set_body_json(json!({
                    "choices": [{ "message": { "content": "Sie haben die Ziele sicher genannt." }, "finish_reason": "stop" }],
                    "usage": { "prompt_tokens": 500, "completion_tokens": 80, "cost": 0.003 }
                }))
            })
            .mount(&server)
            .await;
        let client = OpenRouter::new(server.uri(), Some("sk".into())).unwrap();
        let konfig = PipelineKonfig {
            slots: AppSettings::default().slots,
            modelle: vec![],
            zdr_only: false,
            parallel: 2,
            vorlagen: Vorlagen::standard(),
        };
        let erg = erzeugen(
            p,
            client,
            konfig,
            vec!["k1".into(), "k2".into(), "k3".into()],
        )
        .await
        .unwrap();
        assert_eq!(erg.erzeugt, 2);
        assert_eq!(erg.fehler.len(), 1, "k3 ist nicht freigegeben");
        assert!((erg.kosten - 0.006).abs() < 1e-9);
        let b = bodies.lock().unwrap();
        assert_eq!(b.len(), 2);
        for body in b.iter() {
            let s = body.to_string();
            assert!(
                !s.contains("Edler Kranich") && !s.contains("Kluger Dachs"),
                "Bezeichnung darf nicht gesendet werden"
            );
            assert!(!s.contains("image_url"));
            assert!(s.contains("Anrede mit „Sie“"));
            assert_eq!(body["provider"]["data_collection"], "deny");
        }
        let paths = crate::paths::AppPaths::new(dir.path());
        let id = Projekt::list(&paths).unwrap()[0].id.clone();
        let p = Projekt::open(&paths, &id).unwrap();
        let f = p.feedback("k1").unwrap().unwrap();
        assert_eq!(f.text, f.ki_text);
        p.feedback_speichern("k1", "Geändert", false).unwrap();
        let f = p.feedback("k1").unwrap().unwrap();
        assert_eq!(
            (f.text.as_str(), f.ki_text.as_str()),
            ("Geändert", "Sie haben die Ziele sicher genannt.")
        );
    }
}
