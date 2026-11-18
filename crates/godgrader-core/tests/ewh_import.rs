//! Import der Beispiel-Erwartungshorizonte aus `samples/`.

use std::path::PathBuf;

use godgrader_core::ewh::{self, validate::validate, Afb, ElementArt, Stufe};

fn sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../samples")
        .join(name)
}

fn summen_stimmen(e: &ewh::Erwartungshorizont) {
    for a in &e.aufgaben {
        assert!(
            (a.summe_elemente() - a.max_be).abs() < 1e-9,
            "Aufgabe {}: {} vs {}",
            a.nr,
            a.summe_elemente(),
            a.max_be
        );
    }
}

#[test]
fn word_oberstufe_11_1() {
    let e = ewh::import_file(&sample("2_Erwartungshorizont_WR_11.1_Nachbau.docx")).unwrap();
    assert_eq!(e.stufe, Stufe::Oberstufe);
    assert_eq!(e.aufgabennummern(), ["1a", "1b", "2", "3", "4", "5", "6"]);
    assert_eq!(e.gesamt_be(), 52.0);
    summen_stimmen(&e);
    let a6 = e.aufgaben.iter().find(|a| a.nr == "6").unwrap();
    assert!(a6.hat_raster);
    assert!(a6
        .text
        .contains("Den KI-Wandel sollte der Staat dem Markt überlassen."));
    assert_eq!(
        a6.elemente
            .iter()
            .filter(|x| x.art == ElementArt::Raster)
            .count(),
        6
    );
    assert_eq!(a6.elemente[0].afb, Some(Afb::II));
    let a1b = e.aufgaben.iter().find(|a| a.nr == "1b").unwrap();
    assert!(
        a1b.text.contains("Situation"),
        "Material gehört zum Aufgabentext"
    );
    assert!(validate(&e, false).is_empty(), "{:?}", validate(&e, false));
}

#[test]
fn word_sek1_mit_und_ohne_notenschluessel() {
    let e = ewh::import_file(&sample("2_Erwartungshorizont_Klasse9_Geld.docx")).unwrap();
    assert_eq!(e.stufe, Stufe::Sek1);
    assert_eq!(e.gesamt_be(), 50.0);
    summen_stimmen(&e);
    assert!(validate(&e, false).is_empty(), "{:?}", validate(&e, false));

    let o = ewh::import_file(&sample(
        "2_Erwartungshorizont_Klasse9_Geld_ohne_Notenschluessel.docx",
    ))
    .unwrap();
    assert!(o.notenschluessel.is_none());
    let n = o.notenschluessel_oder_berechnet();
    let ab: Vec<f64> = n.stufen.iter().map(|s| s.ab_be).collect();
    assert_eq!(ab, [48., 40., 33., 25., 13., 0.]);
}

#[test]
fn pdf_11_1_original() {
    let path = sample("2_Erwartungshorizont_WR_11.1.pdf");
    let e = match ewh::import_file(&path) {
        Ok(e) => e,
        Err(err) if err.to_string().contains("PDFium") => {
            eprintln!("PDFium nicht verfügbar, Test übersprungen: {err}");
            return;
        }
        Err(err) => panic!("{err}"),
    };
    assert_eq!(e.kopf.lerngruppe.as_deref(), Some("WR_gA"));
    assert_eq!(e.kopf.datum.as_deref(), Some("24.11.2026"));
    assert_eq!(e.aufgabennummern(), ["1a", "1b", "2", "3", "4", "5", "6"]);
    assert_eq!(e.gesamt_be(), 52.0);
    assert_eq!(e.gesamt_be_dokument, Some(52.0));
    summen_stimmen(&e);
    let anzahl: usize = e.aufgaben.iter().map(|a| a.elemente.len()).sum();
    assert_eq!(anzahl, 31);
    assert!(validate(&e, false).is_empty(), "{:?}", validate(&e, false));
}
