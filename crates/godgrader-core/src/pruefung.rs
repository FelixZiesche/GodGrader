//! Prüfansicht: BE der Lehrkraft, Bestätigungen, Freigabe und
//! Änderungsprotokoll (KI-Vorschlag gegenüber finaler BE).

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::ewh::{Element, Erwartungshorizont};
use crate::ki::speicher::{AbschriftZeile, BewertungZeile, Schritt};
use crate::projekt::Projekt;
use crate::scans::{KlausurInfo, KlausurStatus};
use crate::{Error, Result};

pub(crate) const SCHEMA_V4: &str = "
CREATE TABLE IF NOT EXISTS protokoll (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    klausur_id TEXT NOT NULL REFERENCES klausuren(id) ON DELETE CASCADE,
    element_id TEXT NOT NULL,
    ki_be REAL,
    vorher REAL,
    nachher REAL,
    bestaetigt INTEGER NOT NULL,
    zeit TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS protokoll_klausur ON protokoll(klausur_id);
";

fn db(e: rusqlite::Error) -> Error {
    Error::Ungueltig(format!("Projektdatenbank: {e}"))
}

/// Punkte, Prozent und Note einer Klausur.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Ergebnis {
    /// Finale BE, sonst KI-Vorschlag (für die Live-Anzeige).
    pub summe: f64,
    /// Nur bestätigte BE.
    pub summe_final: f64,
    pub gesamt: f64,
    pub prozent: f64,
    pub punkte: Option<u8>,
    pub note: Option<String>,
    pub elemente: usize,
    pub bestaetigt: usize,
    /// Elemente, deren finale BE vom KI-Vorschlag abweicht.
    pub geaendert: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PruefDaten {
    pub klausur: KlausurInfo,
    pub abschriften: Vec<AbschriftZeile>,
    pub bewertungen: Vec<BewertungZeile>,
    pub schritte: Vec<Schritt>,
    pub ergebnis: Ergebnis,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtokollEintrag {
    pub element_id: String,
    pub ki_be: Option<f64>,
    pub vorher: Option<f64>,
    pub nachher: Option<f64>,
    pub bestaetigt: bool,
    pub zeit: String,
}

fn element<'a>(ewh: &'a Erwartungshorizont, id: &str) -> Option<(&'a str, &'a Element)> {
    ewh.aufgaben.iter().find_map(|a| {
        a.elemente
            .iter()
            .find(|e| e.id == id)
            .map(|e| (a.nr.as_str(), e))
    })
}

pub fn ergebnis_aus(ewh: &Erwartungshorizont, bew: &[BewertungZeile]) -> Ergebnis {
    let gesamt = ewh.gesamt_be();
    let mut e = Ergebnis {
        gesamt,
        ..Default::default()
    };
    for a in &ewh.aufgaben {
        for el in &a.elemente {
            e.elemente += 1;
            let Some(b) = bew.iter().find(|b| b.element_id == el.id) else {
                continue;
            };
            e.summe += b.final_be.or(b.ki_be).unwrap_or(0.0);
            if b.bestaetigt {
                let f = b.final_be.unwrap_or(0.0);
                e.bestaetigt += 1;
                e.summe_final += f;
                if b.ki_be.map(|k| (k - f).abs() > 1e-9).unwrap_or(true) {
                    e.geaendert += 1;
                }
            }
        }
    }
    e.prozent = if gesamt > 0.0 {
        e.summe / gesamt * 100.0
    } else {
        0.0
    };
    if let Some(s) = ewh.notenschluessel_oder_berechnet().bewerte(e.summe) {
        e.punkte = s.punkte;
        e.note = Some(s.note.clone());
    }
    e
}

fn jetzt() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

impl Projekt {
    pub fn klausur(&self, id: &str) -> Result<KlausurInfo> {
        self.klausuren()?
            .into_iter()
            .find(|k| k.id == id)
            .ok_or_else(|| Error::Ungueltig("Klausur nicht gefunden.".into()))
    }

    pub fn pruefdaten(&self, kid: &str, ewh: &Erwartungshorizont) -> Result<PruefDaten> {
        let bewertungen = self.bewertungen(kid)?;
        Ok(PruefDaten {
            klausur: self.klausur(kid)?,
            abschriften: self.abschriften(kid)?,
            ergebnis: ergebnis_aus(ewh, &bewertungen),
            bewertungen,
            schritte: self.schritte(kid)?,
        })
    }

    pub fn ergebnis(&self, kid: &str, ewh: &Erwartungshorizont) -> Result<Ergebnis> {
        Ok(ergebnis_aus(ewh, &self.bewertungen(kid)?))
    }

    fn nicht_freigegeben(&self, kid: &str) -> Result<KlausurStatus> {
        let s = self.klausur_status(kid)?;
        if s == KlausurStatus::Freigegeben {
            return Err(Error::Ungueltig(
                "Diese Klausur ist freigegeben. Bitte zuerst die Freigabe zurücknehmen.".into(),
            ));
        }
        Ok(s)
    }

    /// BE eines Elements setzen und optional bestätigen. Jede Änderung wird
    /// protokolliert. Die Klausur geht dabei in den Status „in Prüfung“.
    pub fn be_setzen(
        &self,
        kid: &str,
        ewh: &Erwartungshorizont,
        element_id: &str,
        be: f64,
        bestaetigt: bool,
    ) -> Result<()> {
        let status = self.nicht_freigegeben(kid)?;
        let (nr, el) = element(ewh, element_id).ok_or_else(|| {
            Error::Ungueltig(format!(
                "Element {element_id} gibt es im Erwartungshorizont nicht."
            ))
        })?;
        let halbe = self.meta()?.halbe_be;
        if !be.is_finite() || be < 0.0 || be > el.max_be + 1e-9 {
            return Err(Error::Ungueltig(format!(
                "BE müssen zwischen 0 und {} liegen.",
                crate::ewh::be_text(el.max_be)
            )));
        }
        let schritt = if halbe { 2.0 } else { 1.0 };
        if ((be * schritt) - (be * schritt).round()).abs() > 1e-9 {
            return Err(Error::Ungueltig(if halbe {
                "Nur ganze oder halbe BE.".into()
            } else {
                "Nur ganze BE (halbe BE sind in diesem Projekt aus).".into()
            }));
        }
        let vorher: Option<(Option<f64>, Option<f64>, bool)> = self
            .conn()
            .query_row(
                "SELECT ki_be, final_be, bestaetigt FROM bewertungen WHERE klausur_id = ?1 AND element_id = ?2",
                params![kid, element_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(db)?;
        let tx = self.conn().unchecked_transaction().map_err(db)?;
        if vorher.is_some() {
            tx.execute(
                "UPDATE bewertungen SET final_be = ?3, bestaetigt = ?4 WHERE klausur_id = ?1 AND element_id = ?2",
                params![kid, element_id, be, bestaetigt],
            )
            .map_err(db)?;
        } else {
            // Ohne KI-Bewertung (z. B. nach einem Fehler) bewertet die Lehrkraft selbst
            let reihenfolge = ewh
                .aufgaben
                .iter()
                .find(|a| a.nr == nr)
                .and_then(|a| a.elemente.iter().position(|e| e.id == element_id))
                .unwrap_or(0) as i64;
            tx.execute(
                "INSERT INTO bewertungen (klausur_id, element_id, aufgabe, max_be, final_be, bestaetigt, reihenfolge)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![kid, element_id, nr, el.max_be, be, bestaetigt, reihenfolge],
            )
            .map_err(db)?;
        }
        let (ki, alt, alt_b) = vorher.unwrap_or((None, None, false));
        if alt != Some(be) || alt_b != bestaetigt {
            tx.execute(
                "INSERT INTO protokoll (klausur_id, element_id, ki_be, vorher, nachher, bestaetigt, zeit) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![kid, element_id, ki, alt, be, bestaetigt, jetzt()],
            )
            .map_err(db)?;
        }
        if matches!(status, KlausurStatus::Offen | KlausurStatus::Vorkorrigiert) {
            tx.execute(
                "UPDATE klausuren SET status = 'in_pruefung' WHERE id = ?1",
                [kid],
            )
            .map_err(db)?;
        }
        tx.commit().map_err(db)?;
        Ok(())
    }

    /// Bestätigung eines Elements zurücknehmen (BE bleiben stehen).
    pub fn bestaetigung_aufheben(&self, kid: &str, element_id: &str) -> Result<()> {
        self.nicht_freigegeben(kid)?;
        self.conn()
            .execute(
                "UPDATE bewertungen SET bestaetigt = 0 WHERE klausur_id = ?1 AND element_id = ?2",
                params![kid, element_id],
            )
            .map_err(db)?;
        Ok(())
    }

    /// Fehlende Bestätigungen (Element-IDs).
    pub fn unbestaetigt(&self, kid: &str, ewh: &Erwartungshorizont) -> Result<Vec<String>> {
        let bew = self.bewertungen(kid)?;
        Ok(ewh
            .aufgaben
            .iter()
            .flat_map(|a| a.elemente.iter())
            .filter(|el| {
                !bew.iter()
                    .any(|b| b.element_id == el.id && b.bestaetigt && b.final_be.is_some())
            })
            .map(|el| el.id.clone())
            .collect())
    }

    /// Freigabe: nur, wenn jedes Element bestätigt ist.
    pub fn freigeben(&self, kid: &str, ewh: &Erwartungshorizont) -> Result<()> {
        self.nicht_freigegeben(kid)?;
        let offen = self.unbestaetigt(kid, ewh)?;
        if !offen.is_empty() {
            return Err(Error::Ungueltig(format!(
                "Noch nicht bestätigt: {}.",
                offen.join(", ")
            )));
        }
        self.klausur_status_setzen(kid, KlausurStatus::Freigegeben)
    }

    pub fn freigabe_zuruecknehmen(&self, kid: &str) -> Result<()> {
        if self.klausur_status(kid)? == KlausurStatus::Freigegeben {
            self.klausur_status_setzen(kid, KlausurStatus::InPruefung)?;
        }
        Ok(())
    }

    pub fn protokoll(&self, kid: &str) -> Result<Vec<ProtokollEintrag>> {
        let mut st = self
            .conn()
            .prepare(
                "SELECT element_id, ki_be, vorher, nachher, bestaetigt, zeit FROM protokoll WHERE klausur_id = ?1 ORDER BY id",
            )
            .map_err(db)?;
        let v = st
            .query_map([kid], |r| {
                Ok(ProtokollEintrag {
                    element_id: r.get(0)?,
                    ki_be: r.get(1)?,
                    vorher: r.get(2)?,
                    nachher: r.get(3)?,
                    bestaetigt: r.get(4)?,
                    zeit: r.get(5)?,
                })
            })
            .map_err(db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db)?;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ewh::{Aufgabe, ElementArt, Kopf, Stufe};
    use crate::ki::schema::{BewertErgebnis, ElementBewertung, Sicherheit};
    use crate::paths::AppPaths;
    use crate::projekt::ProjektMeta;

    fn ewh() -> Erwartungshorizont {
        Erwartungshorizont {
            kopf: Kopf::default(),
            stufe: Stufe::Oberstufe,
            aufgaben: vec![Aufgabe {
                nr: "1".into(),
                text: String::new(),
                max_be: 4.0,
                loesung: String::new(),
                elemente: (1..=2)
                    .map(|i| Element {
                        id: format!("1-{i}"),
                        titel: String::new(),
                        text: String::new(),
                        max_be: 2.0,
                        art: ElementArt::Element,
                        afb: None,
                    })
                    .collect(),
                afb: vec![],
                hat_raster: false,
            }],
            ..Default::default()
        }
    }

    /// Projekt mit einer Klausur ohne Seiten (direkt in die Datenbank).
    fn projekt(dir: &std::path::Path) -> (Projekt, String) {
        let paths = AppPaths::new(dir);
        paths.ensure().unwrap();
        let p = Projekt::create(
            &paths,
            ProjektMeta {
                titel: "T".into(),
                ..Default::default()
            },
        )
        .unwrap();
        p.conn()
            .execute(
                "INSERT INTO klausuren (id, bezeichnung) VALUES ('k1', 'Edler Kranich')",
                [],
            )
            .unwrap();
        (p, "k1".into())
    }

    #[test]
    fn bestaetigen_freigeben_protokoll() {
        let dir = tempfile::tempdir().unwrap();
        let (p, k) = projekt(dir.path());
        let e = ewh();
        let b = BewertErgebnis {
            aufgabe: "1".into(),
            elemente: vec![
                ElementBewertung {
                    element_id: "1-1".into(),
                    max_be: 2.0,
                    vorschlag_be: 2.0,
                    begruendung: String::new(),
                    beleg: String::new(),
                    sicherheit: Sicherheit::Hoch,
                    haengt_an_unsicherer_stelle: false,
                },
                ElementBewertung {
                    element_id: "1-2".into(),
                    max_be: 2.0,
                    vorschlag_be: 1.0,
                    begruendung: String::new(),
                    beleg: String::new(),
                    sicherheit: Sicherheit::Niedrig,
                    haengt_an_unsicherer_stelle: true,
                },
            ],
            summe_vorschlag: 3.0,
            hinweis_fuer_lehrkraft: String::new(),
        };
        p.bewertung_speichern(&k, &b, false).unwrap();
        p.klausur_status_setzen(&k, KlausurStatus::Vorkorrigiert)
            .unwrap();
        let r = p.ergebnis(&k, &e).unwrap();
        assert_eq!((r.summe, r.bestaetigt), (3.0, 0));

        assert!(p.freigeben(&k, &e).is_err());
        p.be_setzen(&k, &e, "1-1", 2.0, true).unwrap();
        assert_eq!(p.klausur_status(&k).unwrap(), KlausurStatus::InPruefung);
        assert!(
            p.be_setzen(&k, &e, "1-2", 3.0, true).is_err(),
            "über Maximum"
        );
        assert!(
            p.be_setzen(&k, &e, "1-2", 0.5, true).is_err(),
            "halbe BE aus"
        );
        p.be_setzen(&k, &e, "1-2", 0.0, true).unwrap();
        let r = p.ergebnis(&k, &e).unwrap();
        assert_eq!(
            (r.summe, r.summe_final, r.bestaetigt, r.geaendert),
            (2.0, 2.0, 2, 1)
        );
        assert_eq!(r.prozent, 50.0);
        // Bei 4 BE fallen 55 %, 60 % und 50 % auf dieselbe Schwelle (2 BE): höchste Stufe gilt
        assert_eq!(r.punkte, Some(8));

        p.freigeben(&k, &e).unwrap();
        assert_eq!(p.klausur_status(&k).unwrap(), KlausurStatus::Freigegeben);
        assert!(
            p.be_setzen(&k, &e, "1-2", 1.0, true).is_err(),
            "freigegeben ist gesperrt"
        );
        p.freigabe_zuruecknehmen(&k).unwrap();
        assert_eq!(p.klausur_status(&k).unwrap(), KlausurStatus::InPruefung);

        let log = p.protokoll(&k).unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[1].ki_be, Some(1.0));
        assert_eq!(log[1].nachher, Some(0.0));
    }

    #[test]
    fn ohne_ki_bewertung_selbst_bewerten() {
        let dir = tempfile::tempdir().unwrap();
        let (p, k) = projekt(dir.path());
        let e = ewh();
        p.be_setzen(&k, &e, "1-1", 1.0, true).unwrap();
        p.be_setzen(&k, &e, "1-2", 2.0, true).unwrap();
        assert!(p.be_setzen(&k, &e, "9-9", 1.0, true).is_err());
        let r = p.ergebnis(&k, &e).unwrap();
        assert_eq!(r.summe_final, 3.0);
        assert_eq!(r.geaendert, 2);
        p.freigeben(&k, &e).unwrap();
    }
}
