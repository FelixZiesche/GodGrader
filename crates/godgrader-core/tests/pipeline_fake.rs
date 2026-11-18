//! Pipeline gegen einen Fake-OpenRouter (wiremock): strikte Trennung,
//! Kosten, Fortsetzen, Budget und Abbrechen. Braucht PDFium für die
//! Seitenbilder (sonst übersprungen).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use godgrader_core::ewh::{Aufgabe, Element, ElementArt, Erwartungshorizont, Kopf, Stufe};
use godgrader_core::ki::pipeline::{self, Ende, Ereignis, PipelineKonfig, Steuerung};
use godgrader_core::ki::prompts::Vorlagen;
use godgrader_core::ki::speicher::{SchrittArt, SchrittStatus};
use godgrader_core::openrouter::OpenRouter;
use godgrader_core::paths::AppPaths;
use godgrader_core::projekt::{Projekt, ProjektMeta};
use godgrader_core::scans::{ImportAuftrag, KlausurStatus, ScanOptionen};
use godgrader_core::settings::AppSettings;
use regex::Regex;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const GEHEIM: [&str; 4] = [
    "GEHEIMLÖSUNG",
    "AUFGABENGEHEIMNIS",
    "ELEMENTTITEL",
    "ERWARTETERINHALT",
];

fn sample(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn ewh() -> Erwartungshorizont {
    let mut e = Erwartungshorizont {
        kopf: Kopf {
            titel: "Test".into(),
            ..Default::default()
        },
        stufe: Stufe::Oberstufe,
        ..Default::default()
    };
    for nr in ["1a", "2"] {
        e.aufgaben.push(Aufgabe {
            nr: nr.into(),
            text: format!("AUFGABENGEHEIMNIS {nr}"),
            max_be: 4.0,
            loesung: "GEHEIMLÖSUNG".into(),
            elemente: (1..=2)
                .map(|i| Element {
                    id: format!("{nr}-{i}"),
                    titel: format!("ELEMENTTITEL {i}"),
                    text: "ERWARTETERINHALT".into(),
                    max_be: 2.0,
                    art: ElementArt::Element,
                    afb: None,
                })
                .collect(),
            afb: vec![],
            hat_raster: false,
        });
    }
    e
}

/// Projekt mit zwei Klausuren zu je zwei Seiten. `None`, wenn PDFium fehlt.
fn projekt(dir: &std::path::Path, budget: Option<f64>) -> Option<(AppPaths, String, Vec<String>)> {
    if godgrader_core::pdfium::pdfium().is_err() {
        eprintln!("PDFium nicht verfügbar, Test übersprungen");
        return None;
    }
    let paths = AppPaths::new(dir);
    paths.ensure().unwrap();
    let p = Projekt::create(
        &paths,
        ProjektMeta {
            titel: "Fake".into(),
            budget_usd: budget,
            schwaerzen: false,
            ..Default::default()
        },
    )
    .unwrap();
    p.save_ewh(&ewh()).unwrap();
    let pdf = sample("1_Klausur_WR_11.1.pdf");
    let ks = p
        .scans_importieren(
            &[
                ImportAuftrag {
                    bezeichnung: "Edler Kranich".into(),
                    pdf: pdf.clone(),
                    seiten: vec![0, 1],
                },
                ImportAuftrag {
                    bezeichnung: "Kluger Dachs".into(),
                    pdf,
                    seiten: vec![2, 3],
                },
            ],
            &ScanOptionen::default(),
            |_| {},
        )
        .unwrap();
    let id = p.meta().unwrap().id;
    Some((paths, id, ks.into_iter().map(|k| k.id).collect()))
}

fn antwort(inhalt: Value, kosten: f64) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(json!({
        "model": "fake/modell",
        "choices": [{ "message": { "content": inhalt.to_string() }, "finish_reason": "stop" }],
        "usage": { "prompt_tokens": 1000, "completion_tokens": 200, "cost": kosten }
    }))
}

/// Antwortet je nach Anfrage wie das Abschreiben- oder Bewerten-Modell und
/// merkt sich alle Anfragen.
fn fake(
    bodies: Arc<Mutex<Vec<Value>>>,
) -> impl Fn(&Request) -> ResponseTemplate + Send + Sync + 'static {
    let re = Regex::new(r"element_id `([^`]+)`").unwrap();
    move |req: &Request| {
        let body: Value = serde_json::from_slice(&req.body).unwrap();
        bodies.lock().unwrap().push(body.clone());
        let s = body.to_string();
        if s.contains("image_url") {
            antwort(
                json!({
                    "aufgaben": [
                        { "aufgabe": "1a", "abschrift": "Antwort [?Wort?]", "unsichere_stellen": ["[?Wort?]"], "seiten": [1] },
                        { "aufgabe": "2", "abschrift": "Zweite Antwort", "unsichere_stellen": [], "seiten": [2] }
                    ],
                    "nicht_zugeordnet": ""
                }),
                0.01,
            )
        } else {
            let user = body["messages"][1]["content"].as_str().unwrap_or_default();
            let elemente: Vec<Value> = re
                .captures_iter(user)
                .map(|c| {
                    json!({
                        "element_id": &c[1], "max_be": 2, "vorschlag_be": 1,
                        "begruendung": "teilweise", "beleg": "Antwort", "sicherheit": "hoch",
                        "haengt_an_unsicherer_stelle": false
                    })
                })
                .collect();
            antwort(
                json!({ "aufgabe": "x", "elemente": elemente, "summe_vorschlag": 2, "hinweis_fuer_lehrkraft": "" }),
                0.002,
            )
        }
    }
}

fn konfig(zweit: bool, parallel: usize) -> PipelineKonfig {
    let mut s = AppSettings::default();
    s.slots.zweitmeinung.enabled = zweit;
    s.slots.zweitmeinung.model = "fake/zweit".into();
    PipelineKonfig {
        slots: s.slots,
        modelle: vec![],
        zdr_only: false,
        parallel,
        vorlagen: Vorlagen::standard(),
    }
}

fn client(server: &MockServer) -> OpenRouter {
    OpenRouter::new(server.uri(), Some("sk-test".into()))
        .unwrap()
        .with_retries(1, Duration::from_millis(5))
}

#[tokio::test]
async fn lauf_trennung_kosten_fortsetzen() {
    let dir = tempfile::tempdir().unwrap();
    let Some((paths, id, ks)) = projekt(dir.path(), None) else {
        return;
    };
    let server = MockServer::start().await;
    let bodies = Arc::new(Mutex::new(Vec::new()));
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(fake(bodies.clone()))
        .mount(&server)
        .await;
    let ereignisse = Arc::new(Mutex::new(Vec::<Ereignis>::new()));
    let e2 = ereignisse.clone();
    let erg = pipeline::ausfuehren(
        Projekt::open(&paths, &id).unwrap(),
        client(&server),
        konfig(true, 3),
        ks.clone(),
        Steuerung::neu(),
        Arc::new(move |e: Ereignis| e2.lock().unwrap().push(e)),
    )
    .await
    .unwrap();
    assert_eq!(erg.ende, Ende::Fertig);
    assert_eq!(erg.fehler, 0);

    // 2 Abschriften + 2 Klausuren × 2 Aufgaben × (Bewertung + Zweitmeinung)
    let b = bodies.lock().unwrap().clone();
    assert_eq!(b.len(), 2 + 8);
    for body in &b {
        let s = body.to_string();
        assert_eq!(body["provider"]["data_collection"], "deny");
        if s.contains("image_url") {
            for g in GEHEIM {
                assert!(
                    !s.contains(g),
                    "Abschreib-Anfrage enthält Erwartungshorizont ({g})"
                );
            }
        } else {
            assert!(!s.contains("data:image"), "Bewerten-Anfrage enthält Bild");
            assert!(s.contains("GEHEIMLÖSUNG"));
        }
    }
    assert!(b.iter().any(|x| x["model"] == "fake/zweit"));

    let p = Projekt::open(&paths, &id).unwrap();
    assert!(
        (erg.kosten - (0.02 + 8.0 * 0.002)).abs() < 1e-9,
        "{}",
        erg.kosten
    );
    assert!((p.kosten_gesamt().unwrap() - erg.kosten).abs() < 1e-9);
    for k in &ks {
        assert_eq!(p.klausur_status(k).unwrap(), KlausurStatus::Vorkorrigiert);
        let a = p.abschriften(k).unwrap();
        assert_eq!(a[0].aufgabe, "1a");
        assert_eq!(a[0].unsicher, vec!["[?Wort?]".to_string()]);
        let bw = p.bewertungen(k).unwrap();
        assert_eq!(bw.len(), 4);
        assert!(bw
            .iter()
            .all(|x| x.ki_be == Some(1.0) && x.zweit_be == Some(1.0) && !x.bestaetigt));
    }
    assert!(ereignisse
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, Ereignis::Kosten { .. })));

    // Zweiter Lauf: alles fertig, keine neue Anfrage
    let erg2 = pipeline::ausfuehren(
        Projekt::open(&paths, &id).unwrap(),
        client(&server),
        konfig(true, 3),
        ks.clone(),
        Steuerung::neu(),
        Arc::new(|_e: Ereignis| {}),
    )
    .await
    .unwrap();
    assert_eq!(erg2.ende, Ende::Fertig);
    assert_eq!(bodies.lock().unwrap().len(), 10);

    // Abschrift ändern und eine Aufgabe neu bewerten: nur diese Anfrage
    p.abschrift_aendern(&ks[0], "2", "Korrigierte Antwort")
        .unwrap();
    p.schritte_loeschen(&ks[0], SchrittArt::Bewerten, Some("2"))
        .unwrap();
    pipeline::ausfuehren(
        Projekt::open(&paths, &id).unwrap(),
        client(&server),
        konfig(true, 3),
        vec![ks[0].clone()],
        Steuerung::neu(),
        Arc::new(|_e: Ereignis| {}),
    )
    .await
    .unwrap();
    let b = bodies.lock().unwrap();
    assert_eq!(b.len(), 11);
    assert!(b[10].to_string().contains("Korrigierte Antwort"));
}

#[tokio::test]
async fn budget_haelt_an_und_fortsetzen_geht_weiter() {
    let dir = tempfile::tempdir().unwrap();
    let Some((paths, id, ks)) = projekt(dir.path(), Some(0.005)) else {
        return;
    };
    let server = MockServer::start().await;
    let bodies = Arc::new(Mutex::new(Vec::new()));
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(fake(bodies.clone()))
        .mount(&server)
        .await;
    let erg = pipeline::ausfuehren(
        Projekt::open(&paths, &id).unwrap(),
        client(&server),
        konfig(false, 1),
        ks.clone(),
        Steuerung::neu(),
        Arc::new(|_e: Ereignis| {}),
    )
    .await
    .unwrap();
    assert_eq!(erg.ende, Ende::Budget);
    assert_eq!(bodies.lock().unwrap().len(), 1);
    let p = Projekt::open(&paths, &id).unwrap();
    let alle = p.alle_schritte().unwrap();
    assert!(alle.iter().all(|s| s.status != SchrittStatus::Laeuft));
    assert_eq!(
        alle.iter()
            .filter(|s| s.status == SchrittStatus::Fertig)
            .count(),
        1
    );

    // Budget erhöhen: Lauf macht weiter, ohne Fertiges zu wiederholen
    let mut m = p.meta().unwrap();
    m.budget_usd = Some(1.0);
    p.save_meta(&m).unwrap();
    let erg = pipeline::ausfuehren(
        Projekt::open(&paths, &id).unwrap(),
        client(&server),
        konfig(false, 2),
        ks.clone(),
        Steuerung::neu(),
        Arc::new(|_e: Ereignis| {}),
    )
    .await
    .unwrap();
    assert_eq!(erg.ende, Ende::Fertig);
    assert_eq!(bodies.lock().unwrap().len(), 2 + 4);
}

#[tokio::test]
async fn abbrechen_laesst_schritte_offen() {
    let dir = tempfile::tempdir().unwrap();
    let Some((paths, id, ks)) = projekt(dir.path(), None) else {
        return;
    };
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(antwort(json!({}), 0.01).set_delay(Duration::from_secs(5)))
        .mount(&server)
        .await;
    let st = Steuerung::neu();
    let st2 = st.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        st2.abbrechen();
    });
    let start = std::time::Instant::now();
    let erg = pipeline::ausfuehren(
        Projekt::open(&paths, &id).unwrap(),
        client(&server),
        konfig(false, 3),
        ks,
        st,
        Arc::new(|_e: Ereignis| {}),
    )
    .await
    .unwrap();
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(erg.ende, Ende::Abgebrochen);
    assert_eq!(erg.kosten, 0.0);
    let p = Projekt::open(&paths, &id).unwrap();
    assert!(p
        .alle_schritte()
        .unwrap()
        .iter()
        .all(|s| s.status == SchrittStatus::Offen));
}

#[tokio::test]
async fn ungueltige_antwort_wird_fehler() {
    let dir = tempfile::tempdir().unwrap();
    let Some((paths, id, ks)) = projekt(dir.path(), None) else {
        return;
    };
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(antwort(json!({ "falsch": true }), 0.01))
        .expect(4) // je Klausur zwei Versuche
        .mount(&server)
        .await;
    let erg = pipeline::ausfuehren(
        Projekt::open(&paths, &id).unwrap(),
        client(&server),
        konfig(false, 3),
        ks.clone(),
        Steuerung::neu(),
        Arc::new(|_e: Ereignis| {}),
    )
    .await
    .unwrap();
    assert_eq!(erg.fehler, 2);
    let p = Projekt::open(&paths, &id).unwrap();
    let s = p
        .schritt(&ks[0], SchrittArt::Abschreiben, "")
        .unwrap()
        .unwrap();
    assert_eq!(s.status, SchrittStatus::Fehler);
    assert!(s.fehler.unwrap().contains("Format"));
    // Kosten fehlgeschlagener Versuche zählen trotzdem
    assert!((p.kosten_gesamt().unwrap() - 0.04).abs() < 1e-9);
}
