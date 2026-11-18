//! Ablage der Pipeline-Ergebnisse in der Projektdatenbank. Jeder Schritt
//! wird einzeln gespeichert, damit ein Lauf nach Absturz oder Abbruch
//! dort weitermacht, wo er aufgehört hat.

use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::schema::{AbschriftErgebnis, BewertErgebnis};
use crate::projekt::Projekt;
use crate::scans::KlausurStatus;
use crate::{Error, Result};

pub(crate) const SCHEMA_V3: &str = "
CREATE TABLE IF NOT EXISTS schritte (
    klausur_id TEXT NOT NULL REFERENCES klausuren(id) ON DELETE CASCADE,
    art TEXT NOT NULL,
    aufgabe TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL,
    modell TEXT,
    fehler TEXT,
    hinweis TEXT,
    aktualisiert TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (klausur_id, art, aufgabe)
);
CREATE TABLE IF NOT EXISTS abschriften (
    klausur_id TEXT NOT NULL REFERENCES klausuren(id) ON DELETE CASCADE,
    aufgabe TEXT NOT NULL,
    text TEXT NOT NULL,
    ki_text TEXT NOT NULL,
    unsicher TEXT NOT NULL DEFAULT '[]',
    seiten TEXT NOT NULL DEFAULT '[]',
    reihenfolge INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (klausur_id, aufgabe)
);
CREATE TABLE IF NOT EXISTS bewertungen (
    klausur_id TEXT NOT NULL REFERENCES klausuren(id) ON DELETE CASCADE,
    element_id TEXT NOT NULL,
    aufgabe TEXT NOT NULL,
    max_be REAL NOT NULL,
    ki_be REAL,
    begruendung TEXT NOT NULL DEFAULT '',
    beleg TEXT NOT NULL DEFAULT '',
    sicherheit TEXT NOT NULL DEFAULT 'mittel',
    unsicher INTEGER NOT NULL DEFAULT 0,
    zweit_be REAL,
    zweit_begruendung TEXT,
    final_be REAL,
    bestaetigt INTEGER NOT NULL DEFAULT 0,
    reihenfolge INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (klausur_id, element_id)
);
-- Kostenprotokoll: bleibt auch nach dem Löschen einer Klausur erhalten.
CREATE TABLE IF NOT EXISTS aufrufe (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    klausur_id TEXT,
    art TEXT NOT NULL,
    aufgabe TEXT,
    modell TEXT NOT NULL,
    tokens_ein INTEGER NOT NULL DEFAULT 0,
    tokens_aus INTEGER NOT NULL DEFAULT 0,
    kosten REAL NOT NULL DEFAULT 0,
    geschaetzt INTEGER NOT NULL DEFAULT 0,
    dauer_ms INTEGER NOT NULL DEFAULT 0,
    ok INTEGER NOT NULL,
    zeit TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS aufrufe_klausur ON aufrufe(klausur_id);
";

fn db(e: rusqlite::Error) -> Error {
    Error::Ungueltig(format!("Projektdatenbank: {e}"))
}

fn jetzt() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SchrittArt {
    Abschreiben,
    Bewerten,
    Zweitmeinung,
}

impl SchrittArt {
    pub fn as_str(self) -> &'static str {
        match self {
            SchrittArt::Abschreiben => "abschreiben",
            SchrittArt::Bewerten => "bewerten",
            SchrittArt::Zweitmeinung => "zweitmeinung",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "abschreiben" => Some(SchrittArt::Abschreiben),
            "bewerten" => Some(SchrittArt::Bewerten),
            "zweitmeinung" => Some(SchrittArt::Zweitmeinung),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SchrittStatus {
    Offen,
    Laeuft,
    Fertig,
    Fehler,
}

impl SchrittStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            SchrittStatus::Offen => "offen",
            SchrittStatus::Laeuft => "laeuft",
            SchrittStatus::Fertig => "fertig",
            SchrittStatus::Fehler => "fehler",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "laeuft" => SchrittStatus::Laeuft,
            "fertig" => SchrittStatus::Fertig,
            "fehler" => SchrittStatus::Fehler,
            _ => SchrittStatus::Offen,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Schritt {
    pub klausur_id: String,
    pub art: SchrittArt,
    /// Leer beim Abschreiben (ganze Klausur).
    pub aufgabe: String,
    pub status: SchrittStatus,
    pub modell: Option<String>,
    pub fehler: Option<String>,
    pub hinweis: Option<String>,
    pub aktualisiert: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AbschriftZeile {
    /// Aufgabennummer; leer = „nicht zugeordnet“.
    pub aufgabe: String,
    pub text: String,
    /// Fassung des Modells (für das Änderungsprotokoll).
    pub ki_text: String,
    pub unsicher: Vec<String>,
    pub seiten: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BewertungZeile {
    pub element_id: String,
    pub aufgabe: String,
    pub max_be: f64,
    pub ki_be: Option<f64>,
    pub begruendung: String,
    pub beleg: String,
    pub sicherheit: String,
    pub unsicher: bool,
    pub zweit_be: Option<f64>,
    pub zweit_begruendung: Option<String>,
    pub final_be: Option<f64>,
    pub bestaetigt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Aufruf {
    pub klausur_id: Option<String>,
    pub art: String,
    pub aufgabe: Option<String>,
    pub modell: String,
    pub tokens_ein: u64,
    pub tokens_aus: u64,
    pub kosten: f64,
    pub geschaetzt: bool,
    pub dauer_ms: u64,
    pub ok: bool,
}

fn schritt_aus(r: &Row) -> rusqlite::Result<Schritt> {
    let art: String = r.get(1)?;
    let status: String = r.get(3)?;
    Ok(Schritt {
        klausur_id: r.get(0)?,
        art: SchrittArt::parse(&art).unwrap_or(SchrittArt::Bewerten),
        aufgabe: r.get(2)?,
        status: SchrittStatus::parse(&status),
        modell: r.get(4)?,
        fehler: r.get(5)?,
        hinweis: r.get(6)?,
        aktualisiert: r.get(7)?,
    })
}

const SCHRITT_SPALTEN: &str =
    "klausur_id, art, aufgabe, status, modell, fehler, hinweis, aktualisiert";

impl Projekt {
    /// Nach Absturz/Neustart: halb erledigte Schritte wieder auf „offen“.
    pub fn laufende_schritte_zuruecksetzen(&self) -> Result<usize> {
        self.conn()
            .execute(
                "UPDATE schritte SET status = 'offen' WHERE status = 'laeuft'",
                [],
            )
            .map_err(db)
    }

    pub fn schritt(
        &self,
        klausur: &str,
        art: SchrittArt,
        aufgabe: &str,
    ) -> Result<Option<Schritt>> {
        self.conn()
            .query_row(
                &format!("SELECT {SCHRITT_SPALTEN} FROM schritte WHERE klausur_id = ?1 AND art = ?2 AND aufgabe = ?3"),
                params![klausur, art.as_str(), aufgabe],
                schritt_aus,
            )
            .optional()
            .map_err(db)
    }

    pub fn schritte(&self, klausur: &str) -> Result<Vec<Schritt>> {
        let mut st = self
            .conn()
            .prepare(&format!(
                "SELECT {SCHRITT_SPALTEN} FROM schritte WHERE klausur_id = ?1 ORDER BY art, aufgabe"
            ))
            .map_err(db)?;
        let v = st
            .query_map([klausur], schritt_aus)
            .map_err(db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db)?;
        Ok(v)
    }

    pub fn alle_schritte(&self) -> Result<Vec<Schritt>> {
        let mut st = self
            .conn()
            .prepare(&format!("SELECT {SCHRITT_SPALTEN} FROM schritte"))
            .map_err(db)?;
        let v = st
            .query_map([], schritt_aus)
            .map_err(db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db)?;
        Ok(v)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn schritt_setzen(
        &self,
        klausur: &str,
        art: SchrittArt,
        aufgabe: &str,
        status: SchrittStatus,
        modell: Option<&str>,
        fehler: Option<&str>,
        hinweis: Option<&str>,
    ) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO schritte (klausur_id, art, aufgabe, status, modell, fehler, hinweis, aktualisiert)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(klausur_id, art, aufgabe) DO UPDATE SET
                   status = excluded.status,
                   modell = COALESCE(excluded.modell, schritte.modell),
                   fehler = excluded.fehler,
                   hinweis = COALESCE(excluded.hinweis, schritte.hinweis),
                   aktualisiert = excluded.aktualisiert",
                params![klausur, art.as_str(), aufgabe, status.as_str(), modell, fehler, hinweis, jetzt()],
            )
            .map_err(db)?;
        Ok(())
    }

    /// Setzt Schritte zurück, damit sie neu laufen (z. B. „neu bewerten“).
    pub fn schritte_loeschen(
        &self,
        klausur: &str,
        art: SchrittArt,
        aufgabe: Option<&str>,
    ) -> Result<()> {
        match aufgabe {
            Some(a) => self.conn().execute(
                "DELETE FROM schritte WHERE klausur_id = ?1 AND art = ?2 AND aufgabe = ?3",
                params![klausur, art.as_str(), a],
            ),
            None => self.conn().execute(
                "DELETE FROM schritte WHERE klausur_id = ?1 AND art = ?2",
                params![klausur, art.as_str()],
            ),
        }
        .map_err(db)?;
        Ok(())
    }

    /// Speichert eine neue Abschrift (ersetzt eine vorhandene vollständig).
    pub fn abschrift_speichern(
        &self,
        klausur: &str,
        a: &AbschriftErgebnis,
        nummern: &[String],
    ) -> Result<()> {
        let tx = self.conn().unchecked_transaction().map_err(db)?;
        tx.execute("DELETE FROM abschriften WHERE klausur_id = ?1", [klausur])
            .map_err(db)?;
        for (i, nr) in nummern.iter().enumerate() {
            let (text, unsicher, seiten) = match a.aufgaben.iter().find(|x| &x.aufgabe == nr) {
                Some(x) => (
                    x.abschrift.trim().to_string(),
                    serde_json::to_string(&x.unsichere_stellen)?,
                    serde_json::to_string(&x.seiten)?,
                ),
                None => (String::new(), "[]".into(), "[]".into()),
            };
            tx.execute(
                "INSERT INTO abschriften (klausur_id, aufgabe, text, ki_text, unsicher, seiten, reihenfolge)
                 VALUES (?1, ?2, ?3, ?3, ?4, ?5, ?6)",
                params![klausur, nr, text, unsicher, seiten, i as i64],
            )
            .map_err(db)?;
        }
        if !a.nicht_zugeordnet.trim().is_empty() {
            tx.execute(
                "INSERT INTO abschriften (klausur_id, aufgabe, text, ki_text, reihenfolge) VALUES (?1, '', ?2, ?2, ?3)",
                params![klausur, a.nicht_zugeordnet.trim(), nummern.len() as i64],
            )
            .map_err(db)?;
        }
        tx.commit().map_err(db)?;
        Ok(())
    }

    pub fn abschriften(&self, klausur: &str) -> Result<Vec<AbschriftZeile>> {
        let mut st = self
            .conn()
            .prepare(
                "SELECT aufgabe, text, ki_text, unsicher, seiten FROM abschriften WHERE klausur_id = ?1 ORDER BY reihenfolge",
            )
            .map_err(db)?;
        let rows = st
            .query_map([klausur], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db)?;
        rows.into_iter()
            .map(|(aufgabe, text, ki_text, u, s)| {
                Ok(AbschriftZeile {
                    aufgabe,
                    text,
                    ki_text,
                    unsicher: serde_json::from_str(&u)?,
                    seiten: serde_json::from_str(&s)?,
                })
            })
            .collect()
    }

    /// Text einer Aufgabe, wie er zur Bewertung geht.
    pub fn abschrift_text(&self, klausur: &str, aufgabe: &str) -> Result<Option<String>> {
        self.conn()
            .query_row(
                "SELECT text FROM abschriften WHERE klausur_id = ?1 AND aufgabe = ?2",
                params![klausur, aufgabe],
                |r| r.get(0),
            )
            .optional()
            .map_err(db)
    }

    /// Änderung der Abschrift durch die Lehrkraft.
    pub fn abschrift_aendern(&self, klausur: &str, aufgabe: &str, text: &str) -> Result<()> {
        let n = self
            .conn()
            .execute(
                "UPDATE abschriften SET text = ?3 WHERE klausur_id = ?1 AND aufgabe = ?2",
                params![klausur, aufgabe, text],
            )
            .map_err(db)?;
        if n == 0 {
            self.conn()
                .execute(
                    "INSERT INTO abschriften (klausur_id, aufgabe, text, ki_text, reihenfolge) VALUES (?1, ?2, ?3, '', 999)",
                    params![klausur, aufgabe, text],
                )
                .map_err(db)?;
        }
        Ok(())
    }

    /// Speichert die Bewertung einer Aufgabe (Erst- oder Zweitmeinung).
    /// Eine neue Erstbewertung setzt Bestätigungen der Lehrkraft zurück.
    pub fn bewertung_speichern(
        &self,
        klausur: &str,
        b: &BewertErgebnis,
        zweit: bool,
    ) -> Result<()> {
        let tx = self.conn().unchecked_transaction().map_err(db)?;
        for (i, e) in b.elemente.iter().enumerate() {
            if zweit {
                tx.execute(
                    "INSERT INTO bewertungen (klausur_id, element_id, aufgabe, max_be, zweit_be, zweit_begruendung, reihenfolge)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(klausur_id, element_id) DO UPDATE SET
                       zweit_be = excluded.zweit_be, zweit_begruendung = excluded.zweit_begruendung",
                    params![klausur, e.element_id, b.aufgabe, e.max_be, e.vorschlag_be, e.begruendung, i as i64],
                )
                .map_err(db)?;
            } else {
                tx.execute(
                    "INSERT INTO bewertungen (klausur_id, element_id, aufgabe, max_be, ki_be, begruendung, beleg, sicherheit, unsicher, reihenfolge)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                     ON CONFLICT(klausur_id, element_id) DO UPDATE SET
                       aufgabe = excluded.aufgabe, max_be = excluded.max_be, ki_be = excluded.ki_be,
                       begruendung = excluded.begruendung, beleg = excluded.beleg,
                       sicherheit = excluded.sicherheit, unsicher = excluded.unsicher,
                       reihenfolge = excluded.reihenfolge, final_be = NULL, bestaetigt = 0",
                    params![
                        klausur,
                        e.element_id,
                        b.aufgabe,
                        e.max_be,
                        e.vorschlag_be,
                        e.begruendung,
                        e.beleg,
                        e.sicherheit.as_str(),
                        e.haengt_an_unsicherer_stelle,
                        i as i64
                    ],
                )
                .map_err(db)?;
            }
        }
        tx.commit().map_err(db)?;
        Ok(())
    }

    pub fn bewertungen(&self, klausur: &str) -> Result<Vec<BewertungZeile>> {
        let mut st = self
            .conn()
            .prepare(
                "SELECT element_id, aufgabe, max_be, ki_be, begruendung, beleg, sicherheit, unsicher,
                        zweit_be, zweit_begruendung, final_be, bestaetigt
                 FROM bewertungen WHERE klausur_id = ?1 ORDER BY aufgabe, reihenfolge",
            )
            .map_err(db)?;
        let v = st
            .query_map([klausur], |r| {
                Ok(BewertungZeile {
                    element_id: r.get(0)?,
                    aufgabe: r.get(1)?,
                    max_be: r.get(2)?,
                    ki_be: r.get(3)?,
                    begruendung: r.get(4)?,
                    beleg: r.get(5)?,
                    sicherheit: r.get(6)?,
                    unsicher: r.get(7)?,
                    zweit_be: r.get(8)?,
                    zweit_begruendung: r.get(9)?,
                    final_be: r.get(10)?,
                    bestaetigt: r.get(11)?,
                })
            })
            .map_err(db)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(db)?;
        Ok(v)
    }

    pub fn aufruf_protokollieren(&self, a: &Aufruf) -> Result<()> {
        self.conn()
            .execute(
                "INSERT INTO aufrufe (klausur_id, art, aufgabe, modell, tokens_ein, tokens_aus, kosten, geschaetzt, dauer_ms, ok, zeit)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    a.klausur_id,
                    a.art,
                    a.aufgabe,
                    a.modell,
                    a.tokens_ein as i64,
                    a.tokens_aus as i64,
                    a.kosten,
                    a.geschaetzt,
                    a.dauer_ms as i64,
                    a.ok,
                    jetzt()
                ],
            )
            .map_err(db)?;
        Ok(())
    }

    /// Bisherige Kosten des Projekts in US-Dollar.
    pub fn kosten_gesamt(&self) -> Result<f64> {
        self.conn()
            .query_row("SELECT COALESCE(SUM(kosten), 0) FROM aufrufe", [], |r| {
                r.get(0)
            })
            .map_err(db)
    }

    /// Kosten je Klausur.
    pub fn kosten_je_klausur(&self) -> Result<std::collections::HashMap<String, f64>> {
        let mut st = self
            .conn()
            .prepare("SELECT klausur_id, SUM(kosten) FROM aufrufe WHERE klausur_id IS NOT NULL GROUP BY klausur_id")
            .map_err(db)?;
        let v = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)))
            .map_err(db)?
            .collect::<rusqlite::Result<_>>()
            .map_err(db)?;
        Ok(v)
    }

    pub fn aufrufe(&self, klausur: Option<&str>) -> Result<Vec<Aufruf>> {
        let sql = "SELECT klausur_id, art, aufgabe, modell, tokens_ein, tokens_aus, kosten, geschaetzt, dauer_ms, ok FROM aufrufe";
        let map = |r: &Row| {
            Ok(Aufruf {
                klausur_id: r.get(0)?,
                art: r.get(1)?,
                aufgabe: r.get(2)?,
                modell: r.get(3)?,
                tokens_ein: r.get::<_, i64>(4)? as u64,
                tokens_aus: r.get::<_, i64>(5)? as u64,
                kosten: r.get(6)?,
                geschaetzt: r.get(7)?,
                dauer_ms: r.get::<_, i64>(8)? as u64,
                ok: r.get(9)?,
            })
        };
        let v = match klausur {
            Some(k) => {
                let mut st = self
                    .conn()
                    .prepare(&format!("{sql} WHERE klausur_id = ?1 ORDER BY id"))
                    .map_err(db)?;
                let r = st
                    .query_map([k], map)
                    .map_err(db)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(db)?;
                r
            }
            None => {
                let mut st = self
                    .conn()
                    .prepare(&format!("{sql} ORDER BY id"))
                    .map_err(db)?;
                let r = st
                    .query_map([], map)
                    .map_err(db)?
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .map_err(db)?;
                r
            }
        };
        Ok(v)
    }

    pub fn klausur_status_setzen(&self, id: &str, status: KlausurStatus) -> Result<()> {
        self.conn()
            .execute(
                "UPDATE klausuren SET status = ?2 WHERE id = ?1",
                params![id, status.as_str()],
            )
            .map_err(db)?;
        Ok(())
    }

    pub fn klausur_status(&self, id: &str) -> Result<KlausurStatus> {
        let s: String = self
            .conn()
            .query_row("SELECT status FROM klausuren WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .map_err(db)?;
        Ok(KlausurStatus::parse(&s))
    }
}
