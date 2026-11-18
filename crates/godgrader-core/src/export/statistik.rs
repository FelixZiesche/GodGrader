//! Ergebnisse und Klassenstatistik. Grundlage sind nur **freigegebene**
//! Klausuren und die finalen BE der Lehrkraft.

use serde::{Deserialize, Serialize};

use crate::ewh::{Afb, Erwartungshorizont, Stufe};
use crate::ki::speicher::BewertungZeile;
use crate::projekt::Projekt;
use crate::scans::KlausurStatus;
use crate::Result;

/// Eine Zeile der Notenliste.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KlausurZeile {
    pub id: String,
    pub bezeichnung: String,
    /// Finale BE je Aufgabe (Reihenfolge wie im Erwartungshorizont).
    pub je_aufgabe: Vec<f64>,
    pub summe: f64,
    pub prozent: f64,
    pub punkte: Option<u8>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quote {
    pub name: String,
    /// Zusatz, z. B. Aufgabe des Elements.
    pub zusatz: Option<String>,
    pub erreicht: f64,
    pub moeglich: f64,
    /// 0–100
    pub quote: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Notenstufe {
    pub label: String,
    pub anzahl: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Statistik {
    pub stufe: Stufe,
    pub anzahl: usize,
    /// Klausuren, die noch nicht freigegeben sind.
    pub nicht_freigegeben: Vec<String>,
    /// Von der besten zur schlechtesten Stufe.
    pub notenspiegel: Vec<Notenstufe>,
    /// Durchschnitt in NP (Oberstufe) bzw. Notenzahl (Sek I).
    pub durchschnitt: Option<f64>,
    pub median: Option<f64>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub durchschnitt_prozent: Option<f64>,
    pub je_aufgabe: Vec<Quote>,
    pub je_afb: Vec<Quote>,
    pub je_element: Vec<Quote>,
    pub schwaechste: Vec<Quote>,
    /// Elemente, deren finale BE vom KI-Vorschlag abweicht.
    pub geaendert: usize,
    /// Elemente mit KI-Vorschlag.
    pub mit_vorschlag: usize,
    pub geaendert_prozent: Option<f64>,
    /// Anteil schlechter Noten (Oberstufe unter 5 NP, Sek I Noten 5 und 6).
    pub schlecht_anzahl: usize,
    pub schlecht_prozent: Option<f64>,
    pub warnschwelle: Option<f64>,
    pub ueber_warnschwelle: bool,
}

fn quote(name: String, zusatz: Option<String>, erreicht: f64, moeglich: f64) -> Quote {
    Quote {
        name,
        zusatz,
        erreicht,
        moeglich,
        quote: if moeglich > 0.0 {
            erreicht / moeglich * 100.0
        } else {
            0.0
        },
    }
}

fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

/// Notenzahl einer Sek-I-Note („2“ → 2.0).
fn notenzahl(n: &str) -> Option<f64> {
    n.trim().chars().next()?.to_digit(10).map(|d| d as f64)
}

pub fn zeile(
    ewh: &Erwartungshorizont,
    id: &str,
    bezeichnung: &str,
    bew: &[BewertungZeile],
) -> KlausurZeile {
    let final_be = |eid: &str| {
        bew.iter()
            .find(|b| b.element_id == eid)
            .and_then(|b| b.final_be)
            .unwrap_or(0.0)
    };
    let je_aufgabe: Vec<f64> = ewh
        .aufgaben
        .iter()
        .map(|a| a.elemente.iter().map(|e| final_be(&e.id)).sum())
        .collect();
    let summe: f64 = je_aufgabe.iter().sum();
    let gesamt = ewh.gesamt_be();
    let stufe = ewh.notenschluessel_oder_berechnet();
    let s = stufe.bewerte(summe);
    KlausurZeile {
        id: id.into(),
        bezeichnung: bezeichnung.into(),
        je_aufgabe,
        summe,
        prozent: if gesamt > 0.0 {
            summe / gesamt * 100.0
        } else {
            0.0
        },
        punkte: s.and_then(|s| s.punkte),
        note: s.map(|s| s.note.clone()),
    }
}

/// Notenliste und Statistik der freigegebenen Klausuren.
pub fn berechnen(p: &Projekt, ewh: &Erwartungshorizont) -> Result<(Vec<KlausurZeile>, Statistik)> {
    let meta = p.meta()?;
    let mut zeilen = Vec::new();
    let mut alle_bew: Vec<Vec<BewertungZeile>> = Vec::new();
    let mut st = Statistik {
        stufe: ewh.stufe,
        warnschwelle: meta.warnschwelle_prozent,
        ..Default::default()
    };
    for k in p.klausuren()? {
        if k.status != KlausurStatus::Freigegeben {
            st.nicht_freigegeben.push(k.bezeichnung);
            continue;
        }
        let bew = p.bewertungen(&k.id)?;
        zeilen.push(zeile(ewh, &k.id, &k.bezeichnung, &bew));
        alle_bew.push(bew);
    }
    st.anzahl = zeilen.len();

    // Notenspiegel
    let schluessel = ewh.notenschluessel_oder_berechnet();
    st.notenspiegel = schluessel
        .stufen
        .iter()
        .map(|s| Notenstufe {
            label: match s.punkte {
                Some(np) => format!("{np:02}"),
                None => s.note.clone(),
            },
            anzahl: zeilen
                .iter()
                .filter(|z| z.punkte == s.punkte && z.note.as_deref() == Some(s.note.as_str()))
                .count(),
        })
        .collect();

    // Durchschnitt, Median, Spannweite
    let mut werte: Vec<f64> = zeilen
        .iter()
        .filter_map(|z| match ewh.stufe {
            Stufe::Oberstufe => z.punkte.map(|p| p as f64),
            Stufe::Sek1 => z.note.as_deref().and_then(notenzahl),
        })
        .collect();
    if !werte.is_empty() {
        st.durchschnitt = Some(werte.iter().sum::<f64>() / werte.len() as f64);
        st.minimum = werte.iter().cloned().reduce(f64::min);
        st.maximum = werte.iter().cloned().reduce(f64::max);
        st.median = median(&mut werte);
        st.durchschnitt_prozent =
            Some(zeilen.iter().map(|z| z.prozent).sum::<f64>() / zeilen.len() as f64);
    }
    st.schlecht_anzahl = werte
        .iter()
        .filter(|&&w| match ewh.stufe {
            Stufe::Oberstufe => w < 5.0,
            Stufe::Sek1 => w >= 5.0,
        })
        .count();
    if st.anzahl > 0 {
        let pz = st.schlecht_anzahl as f64 / st.anzahl as f64 * 100.0;
        st.schlecht_prozent = Some(pz);
        st.ueber_warnschwelle = st.warnschwelle.map(|w| pz > w + 1e-9).unwrap_or(false);
    }

    // Lösungsquoten
    let n = zeilen.len() as f64;
    let final_be = |eid: &str| -> f64 {
        alle_bew
            .iter()
            .map(|bew| {
                bew.iter()
                    .find(|b| b.element_id == eid)
                    .and_then(|b| b.final_be)
                    .unwrap_or(0.0)
            })
            .sum()
    };
    let mut afb: Vec<(Afb, f64, f64)> = Vec::new();
    for (ai, a) in ewh.aufgaben.iter().enumerate() {
        let erreicht: f64 = zeilen.iter().map(|z| z.je_aufgabe[ai]).sum();
        st.je_aufgabe.push(quote(
            format!("Aufgabe {}", a.nr),
            None,
            erreicht,
            a.max_be * n,
        ));
        for e in &a.elemente {
            let er = final_be(&e.id);
            st.je_element
                .push(quote(e.titel.clone(), Some(a.nr.clone()), er, e.max_be * n));
            if let Some(x) = e.afb.or_else(|| a.haupt_afb()) {
                match afb.iter_mut().find(|(f, _, _)| *f == x) {
                    Some(t) => {
                        t.1 += er;
                        t.2 += e.max_be * n;
                    }
                    None => afb.push((x, er, e.max_be * n)),
                }
            }
        }
    }
    afb.sort_by_key(|t| t.0);
    st.je_afb = afb
        .into_iter()
        .map(|(f, e, m)| quote(format!("AFB {f:?}"), None, e, m))
        .collect();
    if st.anzahl > 0 {
        let mut s = st.je_element.clone();
        s.sort_by(|a, b| a.quote.total_cmp(&b.quote));
        st.schwaechste = s.into_iter().take(3).collect();
    }

    // Wie oft hat die Lehrkraft den KI-Vorschlag geändert?
    for bew in &alle_bew {
        for b in bew {
            if let (Some(ki), Some(f)) = (b.ki_be, b.final_be) {
                st.mit_vorschlag += 1;
                if (ki - f).abs() > 1e-9 {
                    st.geaendert += 1;
                }
            }
        }
    }
    if st.mit_vorschlag > 0 {
        st.geaendert_prozent = Some(st.geaendert as f64 / st.mit_vorschlag as f64 * 100.0);
    }
    Ok((zeilen, st))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_gerade_und_ungerade() {
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&mut []), None);
        assert_eq!(notenzahl("2"), Some(2.0));
    }
}
