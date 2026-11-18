//! Entwicklerwerkzeug: Erwartungshorizont einlesen und zusammenfassen.
use godgrader_core::ewh;

fn main() {
    let path = std::env::args().nth(1).expect("Datei angeben");
    let e = ewh::import_file(std::path::Path::new(&path)).expect("Import");
    if std::env::args().any(|a| a == "--json") {
        println!("{}", serde_json::to_string(&e).unwrap());
        return;
    }
    println!(
        "{} | {:?} | {:?} | {:?} | {:?}",
        e.kopf.titel, e.stufe, e.kopf.lerngruppe, e.kopf.datum, e.kopf.thema
    );
    for a in &e.aufgaben {
        println!(
            "\nAufgabe {} [{} BE] AFB {:?} raster={}",
            a.nr,
            a.max_be,
            a.afb.iter().map(|x| (x.afb, x.be)).collect::<Vec<_>>(),
            a.hat_raster
        );
        for el in &a.elemente {
            println!("  {:6} {:>4} {:?} {}", el.id, el.max_be, el.afb, el.titel);
        }
    }
    println!(
        "\nGesamt Dokument: {:?}, berechnet {}",
        e.gesamt_be_dokument,
        e.gesamt_be()
    );
    if let Some(n) = &e.notenschluessel {
        println!(
            "Notenschlüssel: {:?}",
            n.stufen.iter().map(|s| s.ab_be).collect::<Vec<_>>()
        );
    }
    println!("Hinweise: {:#?}", e.import_hinweise);
    for w in ewh::validate::validate(&e, false) {
        println!("WARNUNG: {}", w.text);
    }
}
