//! Ausgaben: Notenliste, Korrekturbögen, Feedback, Statistik.

pub mod ausgabe;
pub mod docx;
pub mod statistik;

#[cfg(test)]
pub(crate) mod testdaten {
    use crate::ewh::{
        Afb, AfbAnteil, Aufgabe, Element, ElementArt, Erwartungshorizont, Kopf, Stufe,
    };
    use crate::paths::AppPaths;
    use crate::projekt::{Projekt, ProjektMeta};

    pub fn ewh() -> Erwartungshorizont {
        let el = |id: &str, t: &str, be: f64, art: ElementArt, afb: Option<Afb>| Element {
            id: id.into(),
            titel: t.into(),
            text: String::new(),
            max_be: be,
            art,
            afb,
        };
        Erwartungshorizont {
            kopf: Kopf {
                titel: "11.1 Klausur im Fach Wirtschaft und Recht – Erwartungshorizont".into(),
                schule: Some("Staatliches Gymnasium Bergschule Apolda".into()),
                schuljahr: Some("SJ 2026/27".into()),
                thema: Some("Soziale Marktwirtschaft".into()),
                lerngruppe: Some("WR_gA".into()),
                datum: Some("24.11.2026".into()),
                gleichwertig_hinweis: true,
            },
            stufe: Stufe::Oberstufe,
            aufgaben: vec![
                Aufgabe {
                    nr: "1".into(),
                    text: "Nennen Sie …".into(),
                    max_be: 4.0,
                    loesung: String::new(),
                    elemente: vec![
                        el("1-1", "Ziele", 2.0, ElementArt::Element, None),
                        el("1-2", "Indikator", 2.0, ElementArt::Element, None),
                    ],
                    afb: vec![AfbAnteil {
                        afb: Afb::I,
                        be: 4.0,
                        zusatz: None,
                    }],
                    hat_raster: false,
                },
                Aufgabe {
                    nr: "2".into(),
                    text: "Erörtern Sie …".into(),
                    max_be: 6.0,
                    loesung: String::new(),
                    elemente: vec![
                        el("2-1", "Argumente", 4.0, ElementArt::Raster, Some(Afb::III)),
                        el("2-2", "Fazit", 2.0, ElementArt::Raster, Some(Afb::III)),
                    ],
                    afb: vec![AfbAnteil {
                        afb: Afb::III,
                        be: 6.0,
                        zusatz: None,
                    }],
                    hat_raster: true,
                },
            ],
            ..Default::default()
        }
    }

    /// Projekt mit drei Klausuren, zwei davon freigegeben.
    pub fn projekt(dir: &std::path::Path) -> Projekt {
        let paths = AppPaths::new(dir);
        paths.ensure().unwrap();
        let p = Projekt::create(
            &paths,
            ProjektMeta {
                titel: "11.1 Klausur".into(),
                warnschwelle_prozent: Some(30.0),
                ..Default::default()
            },
        )
        .unwrap();
        let e = ewh();
        p.save_ewh(&e).unwrap();
        for (id, name) in [
            ("k1", "Edler Kranich"),
            ("k2", "Kluger Dachs"),
            ("k3", "Stiller Uhu"),
        ] {
            p.conn()
                .execute(
                    "INSERT INTO klausuren (id, bezeichnung) VALUES (?1, ?2)",
                    [id, name],
                )
                .unwrap();
        }
        for (k, werte) in [("k1", [2.0, 1.0, 4.0, 2.0]), ("k2", [1.0, 0.0, 1.0, 0.0])] {
            for (eid, v) in ["1-1", "1-2", "2-1", "2-2"].iter().zip(werte) {
                p.be_setzen(k, &e, eid, v, true).unwrap();
            }
            p.freigeben(k, &e).unwrap();
        }
        p.be_setzen("k3", &e, "1-1", 2.0, true).unwrap();
        p
    }
}
