//! Notenschlüssel nach Felix' Tabellen.
//!
//! * Oberstufe („Punkte_Noten_11_12"): ab BE = kaufmännisch gerundet (Gesamt × Anteil).
//! * Sek I („Notentabelle Sek I"): ab BE = aufgerundet (Gesamt × Anteil).

use serde::{Deserialize, Serialize};

use super::Stufe;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Notenstufe {
    /// Notenpunkte (Oberstufe) bzw. `None` in der Sek I.
    pub punkte: Option<u8>,
    /// „1+“, „2-“ … bzw. „1“ … „6“.
    pub note: String,
    pub ab_be: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Notenschluessel {
    /// Von der besten zur schlechtesten Stufe.
    pub stufen: Vec<Notenstufe>,
    /// Aus dem Dokument übernommen (sonst berechnet).
    #[serde(default)]
    pub aus_dokument: bool,
}

/// (Notenpunkte, Note, Anteil in Prozent)
pub const OBERSTUFE: [(u8, &str, u32); 16] = [
    (15, "1+", 95),
    (14, "1", 90),
    (13, "1-", 85),
    (12, "2+", 80),
    (11, "2", 75),
    (10, "2-", 70),
    (9, "3+", 65),
    (8, "3", 60),
    (7, "3-", 55),
    (6, "4+", 50),
    (5, "4", 45),
    (4, "4-", 40),
    (3, "5+", 33),
    (2, "5", 27),
    (1, "5-", 20),
    (0, "6", 0),
];

/// (Note, Anteil in Prozent)
pub const SEK1: [(&str, u32); 6] = [
    ("1", 96),
    ("2", 80),
    ("3", 65),
    ("4", 50),
    ("5", 25),
    ("6", 0),
];

/// Kaufmännisch gerundet (x,5 → aufrunden).
pub fn ab_be_oberstufe(gesamt: f64, prozent: u32) -> f64 {
    let h = (gesamt * 100.0).round() as i64 * prozent as i64; // in 1/10000 BE
    ((h + 5000) / 10000) as f64
}

/// Aufgerundet.
pub fn ab_be_sek1(gesamt: f64, prozent: u32) -> f64 {
    let h = (gesamt * 100.0).round() as i64 * prozent as i64; // in 1/10000 BE
    ((h + 9999) / 10000) as f64
}

impl Notenschluessel {
    pub fn berechnen(stufe: Stufe, gesamt: f64) -> Self {
        let stufen = match stufe {
            Stufe::Oberstufe => OBERSTUFE
                .iter()
                .map(|&(np, note, p)| Notenstufe {
                    punkte: Some(np),
                    note: note.into(),
                    ab_be: ab_be_oberstufe(gesamt, p),
                })
                .collect(),
            Stufe::Sek1 => SEK1
                .iter()
                .map(|&(note, p)| Notenstufe {
                    punkte: None,
                    note: note.into(),
                    ab_be: ab_be_sek1(gesamt, p),
                })
                .collect(),
        };
        Notenschluessel {
            stufen,
            aus_dokument: false,
        }
    }

    /// Stufe für eine erreichte BE-Zahl.
    pub fn bewerte(&self, be: f64) -> Option<&Notenstufe> {
        self.stufen.iter().find(|s| be + 1e-9 >= s.ab_be)
    }

    /// Unterschiede zum berechneten Schlüssel (für Warnungen).
    pub fn abweichungen(&self, stufe: Stufe, gesamt: f64) -> Vec<String> {
        let soll = Self::berechnen(stufe, gesamt);
        if soll.stufen.len() != self.stufen.len() {
            return vec![format!(
                "Notenschlüssel hat {} Stufen, erwartet {}.",
                self.stufen.len(),
                soll.stufen.len()
            )];
        }
        self.stufen
            .iter()
            .zip(&soll.stufen)
            .filter(|(ist, soll)| (ist.ab_be - soll.ab_be).abs() > 1e-9)
            .map(|(ist, soll)| {
                let name = match ist.punkte {
                    Some(p) => format!("{p} NP"),
                    None => format!("Note {}", ist.note),
                };
                format!(
                    "{name}: ab {} BE im Dokument, berechnet {} BE.",
                    super::be_text(ist.ab_be),
                    super::be_text(soll.ab_be)
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ab(stufe: Stufe, gesamt: f64) -> Vec<f64> {
        Notenschluessel::berechnen(stufe, gesamt)
            .stufen
            .iter()
            .map(|s| s.ab_be)
            .collect()
    }

    #[test]
    fn oberstufe_50() {
        assert_eq!(
            ab(Stufe::Oberstufe, 50.0),
            [48., 45., 43., 40., 38., 35., 33., 30., 28., 25., 23., 20., 17., 14., 10., 0.]
        );
    }

    #[test]
    fn oberstufe_30() {
        assert_eq!(
            ab(Stufe::Oberstufe, 30.0),
            [29., 27., 26., 24., 23., 21., 20., 18., 17., 15., 14., 12., 10., 8., 6., 0.]
        );
    }

    #[test]
    fn oberstufe_52_wie_klausur_11_1() {
        assert_eq!(
            ab(Stufe::Oberstufe, 52.0),
            [49., 47., 44., 42., 39., 36., 34., 31., 29., 26., 23., 21., 17., 14., 10., 0.]
        );
    }

    #[test]
    fn sek1_50() {
        assert_eq!(ab(Stufe::Sek1, 50.0), [48., 40., 33., 25., 13., 0.]);
    }

    #[test]
    fn sek1_30() {
        assert_eq!(ab(Stufe::Sek1, 30.0), [29., 24., 20., 15., 8., 0.]);
    }

    #[test]
    fn noten_zuordnung() {
        let n = Notenschluessel::berechnen(Stufe::Oberstufe, 50.0);
        assert_eq!(n.bewerte(50.0).unwrap().punkte, Some(15));
        assert_eq!(n.bewerte(47.5).unwrap().punkte, Some(14));
        assert_eq!(n.bewerte(9.0).unwrap().punkte, Some(0));
        let s = Notenschluessel::berechnen(Stufe::Sek1, 50.0);
        assert_eq!(s.bewerte(39.0).unwrap().note, "3");
    }

    #[test]
    fn abweichung_wird_gemeldet() {
        let mut n = Notenschluessel::berechnen(Stufe::Oberstufe, 50.0);
        n.stufen[0].ab_be = 47.0;
        let a = n.abweichungen(Stufe::Oberstufe, 50.0);
        assert_eq!(a.len(), 1);
        assert!(a[0].contains("15 NP"));
    }
}
