//! Prüfungen des Erwartungshorizonts. Liefert Warnungen, nichts wird blockiert.

use serde::{Deserialize, Serialize};

use super::{be_text, ElementArt, Erwartungshorizont};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Warnung {
    /// Betroffene Aufgabe (falls zuordenbar).
    pub aufgabe: Option<String>,
    pub text: String,
}

fn w(aufgabe: Option<&str>, text: String) -> Warnung {
    Warnung {
        aufgabe: aufgabe.map(str::to_string),
        text,
    }
}

fn eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

pub fn validate(e: &Erwartungshorizont, halbe_be: bool) -> Vec<Warnung> {
    let mut out = Vec::new();
    if e.aufgaben.is_empty() {
        out.push(w(None, "Keine Aufgaben erkannt.".into()));
        return out;
    }
    let mut seen = std::collections::HashSet::new();
    for a in &e.aufgaben {
        let nr = Some(a.nr.as_str());
        if !seen.insert(a.nr.clone()) {
            out.push(w(
                nr,
                format!("Aufgabennummer {} kommt mehrfach vor.", a.nr),
            ));
        }
        if a.elemente.is_empty() {
            out.push(w(
                nr,
                format!("Aufgabe {} hat keine Bewertungselemente.", a.nr),
            ));
        }
        let sum = a.summe_elemente();
        if !eq(sum, a.max_be) {
            out.push(w(
                nr,
                format!(
                    "Aufgabe {}: Elemente ergeben {} BE, die Aufgabe hat {} BE.",
                    a.nr,
                    be_text(sum),
                    be_text(a.max_be)
                ),
            ));
        }
        if a.hat_raster {
            let r: f64 = a
                .elemente
                .iter()
                .filter(|x| x.art == ElementArt::Raster)
                .map(|x| x.max_be)
                .sum();
            if !eq(r, a.max_be) {
                out.push(w(
                    nr,
                    format!(
                        "Aufgabe {}: Bewertungsraster ergibt {} BE, die Aufgabe hat {} BE.",
                        a.nr,
                        be_text(r),
                        be_text(a.max_be)
                    ),
                ));
            }
        }
        for el in &a.elemente {
            if el.max_be <= 0.0 {
                out.push(w(nr, format!("Element „{}“ hat keine BE.", el.titel)));
            }
            if !halbe_be && !eq(el.max_be, el.max_be.round()) {
                out.push(w(
                    nr,
                    format!(
                        "Element „{}“ hat halbe BE, halbe BE sind aber nicht erlaubt.",
                        el.titel
                    ),
                ));
            }
        }
        if a.afb.is_empty() {
            out.push(w(
                nr,
                format!("Aufgabe {}: kein Anforderungsbereich zugeordnet.", a.nr),
            ));
        } else {
            let s: f64 = a.afb.iter().map(|x| x.be).sum();
            if !eq(s, a.max_be) {
                out.push(w(
                    nr,
                    format!(
                        "Aufgabe {}: AFB-Übersicht nennt {} BE, die Aufgabe hat {} BE.",
                        a.nr,
                        be_text(s),
                        be_text(a.max_be)
                    ),
                ));
            }
        }
    }
    let gesamt = e.gesamt_be();
    if let Some(d) = e.gesamt_be_dokument {
        if !eq(d, gesamt) {
            out.push(w(
                None,
                format!(
                    "Summe aller Aufgaben ist {} BE, die AFB-Übersicht nennt {} BE.",
                    be_text(gesamt),
                    be_text(d)
                ),
            ));
        }
    }
    match &e.notenschluessel {
        Some(n) => {
            for a in n.abweichungen(e.stufe, gesamt) {
                out.push(w(
                    None,
                    format!("Notenschlüssel passt nicht zu {} BE: {a}", be_text(gesamt)),
                ));
            }
        }
        None => out.push(w(
            None,
            format!(
                "Kein Notenschlüssel im Dokument – er wird für {} BE berechnet.",
                be_text(gesamt)
            ),
        )),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ewh::{Afb, AfbAnteil, Aufgabe, Element, Notenschluessel, Stufe};

    fn el(be: f64) -> Element {
        Element {
            id: String::new(),
            titel: "x".into(),
            text: String::new(),
            max_be: be,
            art: Default::default(),
            afb: None,
        }
    }

    #[test]
    fn findet_summenfehler() {
        let mut e = Erwartungshorizont::default();
        e.aufgaben.push(Aufgabe {
            nr: "1".into(),
            text: String::new(),
            max_be: 4.0,
            loesung: String::new(),
            elemente: vec![el(1.0), el(2.5)],
            afb: vec![AfbAnteil {
                afb: Afb::I,
                be: 4.0,
                zusatz: None,
            }],
            hat_raster: false,
        });
        e.gesamt_be_dokument = Some(5.0);
        e.notenschluessel = Some(Notenschluessel::berechnen(Stufe::Oberstufe, 4.0));
        let ws: Vec<String> = validate(&e, false).into_iter().map(|w| w.text).collect();
        assert!(ws.iter().any(|w| w.contains("ergeben 3,5 BE")), "{ws:?}");
        assert!(ws.iter().any(|w| w.contains("halbe BE")));
        assert!(ws.iter().any(|w| w.contains("AFB-Übersicht nennt 5 BE")));
        assert_eq!(validate(&e, true).len(), ws.len() - 1);
    }
}
