//! Exportdateien: Notenliste (xlsx, csv), Korrekturbögen, Feedback und
//! Statistik (docx). Nur freigegebene Klausuren.

use std::path::{Path, PathBuf};

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};
use serde::{Deserialize, Serialize};

use super::docx::{Doc, Run, Tabelle, Zelle, BREITE};
use super::statistik::{self, KlausurZeile, Quote, Statistik};
use crate::ewh::{be_text, ElementArt, Erwartungshorizont, Stufe};
use crate::projekt::{Projekt, ProjektMeta};
use crate::scans::KlausurStatus;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ExportOptionen {
    pub notenliste: bool,
    pub korrekturboegen: bool,
    pub sammeldatei: bool,
    pub feedback: bool,
    pub statistik: bool,
}

impl Default for ExportOptionen {
    fn default() -> Self {
        ExportOptionen {
            notenliste: true,
            korrekturboegen: true,
            sammeldatei: true,
            feedback: false,
            statistik: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ExportBericht {
    pub ordner: String,
    pub dateien: Vec<String>,
    /// Noch nicht freigegeben, daher nicht exportiert.
    pub fehlend: Vec<String>,
    /// Freigegeben, aber ohne Feedback.
    pub ohne_feedback: Vec<String>,
}

/// Dateinamentauglich für Windows: verbotene Zeichen und Leerzeichen → `_`.
pub fn dateiname(s: &str) -> String {
    let mut o = String::new();
    for c in s.trim().chars() {
        let ersatz = matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            || c.is_control()
            || c.is_whitespace();
        if ersatz {
            if !o.ends_with('_') {
                o.push('_');
            }
        } else {
            o.push(c);
        }
    }
    let o = o.trim_matches(|c| c == '_' || c == '.').to_string();
    if o.is_empty() {
        "Projekt".into()
    } else {
        o.chars().take(80).collect()
    }
}

fn zahl(v: f64) -> String {
    be_text((v * 10.0).round() / 10.0)
}

fn prozent(v: f64) -> String {
    format!("{} %", zahl(v))
}

/// „Pseudonym“, wenn eine Pseudonymliste genutzt wird, sonst „Name“.
pub fn bezeichnungs_label(p: &Projekt) -> Result<&'static str> {
    Ok(if p.pseudonyme()?.is_empty() {
        "Name"
    } else {
        "Pseudonym"
    })
}

fn titel_mit(ewh: &Erwartungshorizont, zusatz: &str) -> String {
    let t = ewh.kopf.titel.trim();
    let basis = t
        .trim_end_matches("Erwartungshorizont")
        .trim_end()
        .trim_end_matches(['–', '-', ':'])
        .trim_end();
    let basis = if basis.is_empty() { t } else { basis };
    format!("{basis} – {zusatz}")
}

/// Kopf wie im Erwartungshorizont, aber „Pseudonym:“ statt „Name:“.
fn kopf(
    d: &mut Doc,
    ewh: &Erwartungshorizont,
    meta: &ProjektMeta,
    titel: &str,
    label: &str,
    bezeichnung: Option<&str>,
) {
    d.titel(titel);
    let k = &ewh.kopf;
    let schule = k.schule.clone().unwrap_or_default();
    let sj = k.schuljahr.clone().unwrap_or_default();
    if !schule.is_empty() || !sj.is_empty() {
        d.tabelle(&Tabelle {
            breiten: vec![BREITE * 2 / 3, BREITE - BREITE * 2 / 3],
            zeilen: vec![vec![Zelle::t(schule), Zelle::t(sj).rechts()]],
            ohne_linien: true,
            ..Default::default()
        });
    }
    if let Some(th) = k.thema.as_deref().filter(|t| !t.trim().is_empty()) {
        d.absatz_mit(
            &[Run::f("Thema: "), Run::k(th)],
            super::docx::Ausrichtung::Mitte,
            0,
            Some(80),
        );
    }
    let gruppe_label = match ewh.stufe {
        Stufe::Oberstufe => "Kurs",
        Stufe::Sek1 => "Klasse",
    };
    let gruppe = k
        .lerngruppe
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| meta.lerngruppe.clone());
    let datum = k.datum.clone().unwrap_or_else(|| meta.datum.clone());
    let mut z = vec![];
    if let Some(b) = bezeichnung {
        z.push(Zelle {
            runs: vec![Run::f(format!("{label}: ")), Run::t(b)],
            ..Default::default()
        });
    } else {
        z.push(Zelle::t(""));
    }
    z.push(
        Zelle {
            runs: vec![Run::f(format!("{gruppe_label}: ")), Run::t(gruppe)],
            ..Default::default()
        }
        .mitte(),
    );
    z.push(
        Zelle {
            runs: vec![Run::f("Datum: "), Run::t(datum)],
            ..Default::default()
        }
        .rechts(),
    );
    d.tabelle(&Tabelle {
        breiten: vec![BREITE / 2, BREITE / 4, BREITE - BREITE / 2 - BREITE / 4],
        zeilen: vec![z],
        ohne_linien: true,
        ..Default::default()
    });
}

fn ergebnis_text(ewh: &Erwartungshorizont, z: &KlausurZeile) -> Vec<Run> {
    let mut r = vec![
        Run::f("Gesamt: "),
        Run::t(format!(
            "{} von {} BE ({})",
            be_text(z.summe),
            be_text(ewh.gesamt_be()),
            prozent(z.prozent)
        )),
        Run::t("\t"),
    ];
    match ewh.stufe {
        Stufe::Oberstufe => {
            r.push(Run::f("Notenpunkte: "));
            r.push(Run::t(format!(
                "{}{}",
                z.punkte
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "–".into()),
                z.note
                    .as_ref()
                    .map(|n| format!(" ({n})"))
                    .unwrap_or_default()
            )));
        }
        Stufe::Sek1 => {
            r.push(Run::f("Note: "));
            r.push(Run::t(z.note.clone().unwrap_or_else(|| "–".into())));
        }
    }
    r
}

/// Korrekturbogen einer Klausur.
pub fn korrekturbogen(
    p: &Projekt,
    ewh: &Erwartungshorizont,
    meta: &ProjektMeta,
    kid: &str,
    label: &str,
) -> Result<Doc> {
    let k = p.klausur(kid)?;
    let bew = p.bewertungen(kid)?;
    let z = statistik::zeile(ewh, kid, &k.bezeichnung, &bew);
    let mut d = Doc::neu();
    kopf(
        &mut d,
        ewh,
        meta,
        &titel_mit(ewh, "Korrekturbogen"),
        label,
        Some(&k.bezeichnung),
    );

    d.ueberschrift("Bewertung");
    let b_auf = 1150;
    let b_be = 1350;
    let mut zeilen = Vec::new();
    for (ai, a) in ewh.aufgaben.iter().enumerate() {
        let mut gruende = Vec::new();
        for e in &a.elemente {
            let b = bew.iter().find(|b| b.element_id == e.id);
            let f = b.and_then(|b| b.final_be).unwrap_or(0.0);
            if f + 1e-9 < e.max_be {
                let mut s = format!("{} ({}/{})", e.titel.trim(), be_text(f), be_text(e.max_be));
                // KI-Begründung nur, wenn die Lehrkraft den Vorschlag übernommen hat
                if let Some(b) =
                    b.filter(|b| b.ki_be == Some(f) && !b.begruendung.trim().is_empty())
                {
                    s.push_str(&format!(": {}", b.begruendung.trim()));
                }
                gruende.push(s);
            }
        }
        let text = if gruende.is_empty() {
            "vollständig".to_string()
        } else {
            gruende.join("\n")
        };
        zeilen.push(vec![
            Zelle::t(a.nr.clone()).mitte(),
            Zelle::t(be_text(a.max_be)).mitte(),
            Zelle::f(be_text(z.je_aufgabe[ai])).mitte(),
            Zelle::t(text),
        ]);
    }
    zeilen.push(vec![
        Zelle::f("Σ").mitte(),
        Zelle::f(be_text(ewh.gesamt_be())).mitte(),
        Zelle::f(be_text(z.summe)).mitte(),
        Zelle::t(""),
    ]);
    d.tabelle(&Tabelle {
        breiten: vec![b_auf, b_be, b_be, BREITE - b_auf - 2 * b_be],
        kopf: vec![
            Zelle::t("Aufgabe"),
            Zelle::t("max. BE"),
            Zelle::t("erreichte BE"),
            Zelle::t("Kurzbegründung"),
        ],
        zeilen,
        ..Default::default()
    });

    // Ausgefülltes Bewertungsraster der AFB-III-Aufgabe(n)
    for a in ewh
        .aufgaben
        .iter()
        .filter(|a| a.hat_raster || a.elemente.iter().any(|e| e.art == ElementArt::Raster))
    {
        d.ueberschrift(&format!("Bewertungsraster Aufgabe {}", a.nr));
        let zeilen = a
            .elemente
            .iter()
            .map(|e| {
                let f = bew
                    .iter()
                    .find(|b| b.element_id == e.id)
                    .and_then(|b| b.final_be)
                    .unwrap_or(0.0);
                vec![
                    Zelle::t(e.titel.clone()),
                    Zelle::t(be_text(e.max_be)).mitte(),
                    Zelle::f(be_text(f)).mitte(),
                ]
            })
            .collect();
        d.tabelle(&Tabelle {
            breiten: vec![BREITE - 2 * b_be, b_be, b_be],
            kopf: vec![
                Zelle::t("Kriterium"),
                Zelle::t("max. BE"),
                Zelle::t("erreicht"),
            ],
            zeilen,
            ..Default::default()
        });
    }

    d.absatz_mit(
        &ergebnis_text(ewh, &z),
        super::docx::Ausrichtung::Links,
        160,
        None,
    );

    if let Some(f) = p.feedback(kid)?.filter(|f| !f.text.trim().is_empty()) {
        d.ueberschrift("Rückmeldung");
        d.text(f.text.trim());
    }
    Ok(d)
}

/// Feedback als eigene kurze Datei zum Beilegen.
pub fn feedback_doc(
    p: &Projekt,
    ewh: &Erwartungshorizont,
    meta: &ProjektMeta,
    kid: &str,
    label: &str,
) -> Result<Option<Doc>> {
    let Some(f) = p.feedback(kid)?.filter(|f| !f.text.trim().is_empty()) else {
        return Ok(None);
    };
    let k = p.klausur(kid)?;
    let mut d = Doc::neu();
    kopf(
        &mut d,
        ewh,
        meta,
        &titel_mit(ewh, "Rückmeldung"),
        label,
        Some(&k.bezeichnung),
    );
    d.leer();
    d.text(f.text.trim());
    Ok(Some(d))
}

fn quoten_tabelle(d: &mut Doc, titel: &str, q: &[Quote]) {
    let zeilen = q
        .iter()
        .map(|q| {
            vec![
                Zelle::t(match &q.zusatz {
                    Some(z) => format!("{} ({z})", q.name),
                    None => q.name.clone(),
                }),
                Zelle::t(zahl(q.erreicht)).mitte(),
                Zelle::t(zahl(q.moeglich)).mitte(),
                Zelle::f(prozent(q.quote)).mitte(),
            ]
        })
        .collect();
    d.tabelle(&Tabelle {
        breiten: vec![BREITE - 3 * 1300, 1300, 1300, 1300],
        kopf: vec![
            Zelle::t(titel),
            Zelle::t("erreicht"),
            Zelle::t("möglich"),
            Zelle::t("Quote"),
        ],
        zeilen,
        ..Default::default()
    });
}

fn wert_text(st: &Statistik, v: Option<f64>) -> String {
    match v {
        None => "–".into(),
        Some(v) => match st.stufe {
            Stufe::Oberstufe => format!("{} NP", zahl(v)),
            Stufe::Sek1 => zahl(v),
        },
    }
}

/// Klassenstatistik als Word-Datei.
pub fn statistik_doc(ewh: &Erwartungshorizont, meta: &ProjektMeta, st: &Statistik) -> Doc {
    let mut d = Doc::neu();
    kopf(&mut d, ewh, meta, &titel_mit(ewh, "Statistik"), "", None);
    if !st.nicht_freigegeben.is_empty() {
        let n = st.nicht_freigegeben.len();
        d.absatz(&[
            Run::f("Hinweis: "),
            Run::t(format!(
                "{n} {} noch nicht freigegeben und daher nicht ausgewertet.",
                if n == 1 {
                    "Klausur ist"
                } else {
                    "Klausuren sind"
                }
            )),
        ]);
    }

    d.ueberschrift("Überblick");
    let mut zeilen = vec![
        vec![
            Zelle::t("Ausgewertete Klausuren"),
            Zelle::f(st.anzahl.to_string()),
        ],
        vec![
            Zelle::t("Durchschnitt"),
            Zelle::f(wert_text(st, st.durchschnitt)),
        ],
        vec![Zelle::t("Median"), Zelle::f(wert_text(st, st.median))],
        vec![
            Zelle::t("Spannweite"),
            Zelle::f(format!(
                "{} bis {}",
                wert_text(st, st.minimum),
                wert_text(st, st.maximum)
            )),
        ],
        vec![
            Zelle::t("Durchschnitt in Prozent"),
            Zelle::f(
                st.durchschnitt_prozent
                    .map(prozent)
                    .unwrap_or_else(|| "–".into()),
            ),
        ],
    ];
    let schlecht_label = match st.stufe {
        Stufe::Oberstufe => "Anteil unter 5 NP",
        Stufe::Sek1 => "Anteil Noten 5 und 6",
    };
    zeilen.push(vec![
        Zelle::t(schlecht_label),
        Zelle::f(format!(
            "{} ({}){}",
            st.schlecht_prozent
                .map(prozent)
                .unwrap_or_else(|| "–".into()),
            st.schlecht_anzahl,
            match (st.warnschwelle, st.ueber_warnschwelle) {
                (Some(w), true) => format!(" – über der Warnschwelle von {}", prozent(w)),
                (Some(w), false) => format!(" – Warnschwelle {}", prozent(w)),
                _ => String::new(),
            }
        )),
    ]);
    zeilen.push(vec![
        Zelle::t("Von der Lehrkraft geänderte KI-Vorschläge"),
        Zelle::f(format!(
            "{} von {} ({})",
            st.geaendert,
            st.mit_vorschlag,
            st.geaendert_prozent
                .map(prozent)
                .unwrap_or_else(|| "–".into())
        )),
    ]);
    d.tabelle(&Tabelle {
        breiten: vec![BREITE / 2, BREITE - BREITE / 2],
        zeilen,
        ..Default::default()
    });

    // Notenspiegel als Graustufen-Balken aus Tabellenzellen
    d.ueberschrift("Notenspiegel");
    let max = st
        .notenspiegel
        .iter()
        .map(|n| n.anzahl)
        .max()
        .unwrap_or(0)
        .max(1);
    let spalten = max.clamp(1, 30);
    let label_b = 900u32;
    let zahl_b = 700u32;
    let zelle_b = (BREITE - label_b - zahl_b) / spalten as u32;
    let mut breiten = vec![label_b];
    breiten.extend(std::iter::repeat_n(zelle_b, spalten));
    breiten.push(zahl_b);
    let zeilen = st
        .notenspiegel
        .iter()
        .map(|n| {
            let balken = (n.anzahl * spalten).div_ceil(max);
            let mut z = vec![Zelle::f(n.label.clone()).mitte()];
            for i in 0..spalten {
                z.push(if i < balken {
                    Zelle::t("").farbe("595959")
                } else {
                    Zelle::t("")
                });
            }
            z.push(Zelle::t(n.anzahl.to_string()).mitte());
            z
        })
        .collect();
    d.tabelle(&Tabelle {
        breiten,
        kopf: vec![],
        zeilen,
        eng: true,
        ..Default::default()
    });

    d.ueberschrift("Lösungsquote je Aufgabe");
    quoten_tabelle(&mut d, "Aufgabe", &st.je_aufgabe);
    d.ueberschrift("Lösungsquote je Anforderungsbereich");
    quoten_tabelle(&mut d, "AFB", &st.je_afb);
    d.ueberschrift("Die drei schwächsten Bewertungselemente");
    quoten_tabelle(&mut d, "Element (Aufgabe)", &st.schwaechste);
    d.ueberschrift("Lösungsquote je Bewertungselement");
    quoten_tabelle(&mut d, "Element (Aufgabe)", &st.je_element);
    d
}

/// CSV mit Semikolon, UTF-8 mit BOM, Dezimalkomma.
pub fn notenliste_csv(
    ewh: &Erwartungshorizont,
    zeilen: &[KlausurZeile],
    label: &str,
) -> Result<Vec<u8>> {
    let mut w = csv::WriterBuilder::new()
        .delimiter(b';')
        .from_writer(vec![0xEF, 0xBB, 0xBF]);
    let mut kopf = vec![label.to_string()];
    kopf.extend(
        ewh.aufgaben
            .iter()
            .map(|a| format!("Aufgabe {} ({} BE)", a.nr, be_text(a.max_be))),
    );
    kopf.push(format!("Summe ({} BE)", be_text(ewh.gesamt_be())));
    kopf.push("Prozent".into());
    kopf.push(match ewh.stufe {
        Stufe::Oberstufe => "NP".into(),
        Stufe::Sek1 => "Note".into(),
    });
    w.write_record(&kopf)
        .map_err(|e| Error::Ungueltig(e.to_string()))?;
    for z in zeilen {
        let mut r = vec![z.bezeichnung.clone()];
        r.extend(z.je_aufgabe.iter().map(|v| be_text(*v)));
        r.push(be_text(z.summe));
        r.push(zahl(z.prozent));
        r.push(match ewh.stufe {
            Stufe::Oberstufe => z.punkte.map(|p| p.to_string()).unwrap_or_default(),
            Stufe::Sek1 => z.note.clone().unwrap_or_default(),
        });
        w.write_record(&r)
            .map_err(|e| Error::Ungueltig(e.to_string()))?;
    }
    w.into_inner().map_err(|e| Error::Ungueltig(e.to_string()))
}

/// Excel-Datei mit den Blättern „Notenliste“ und „Statistik“.
pub fn notenliste_xlsx(
    ewh: &Erwartungshorizont,
    zeilen: &[KlausurZeile],
    st: &Statistik,
    label: &str,
) -> Result<Vec<u8>> {
    let x = |e: rust_xlsxwriter::XlsxError| Error::Ungueltig(format!("Excel: {e}"));
    let grau = Color::RGB(0xA6A6A6);
    let basis = Format::new().set_font_name("Google Sans").set_font_size(11);
    let kopf = basis
        .clone()
        .set_bold()
        .set_background_color(Color::RGB(0xE6E6E6))
        .set_border(FormatBorder::Thin)
        .set_border_color(grau)
        .set_text_wrap()
        .set_align(FormatAlign::Center);
    let zelle = basis
        .clone()
        .set_border(FormatBorder::Thin)
        .set_border_color(grau);
    let zahlf = zelle
        .clone()
        .set_num_format("0.0#")
        .set_align(FormatAlign::Center);
    let ganz = zelle.clone().set_align(FormatAlign::Center);
    let fett = basis.clone().set_bold();

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name("Notenliste").map_err(x)?;
    let mut spalte: u16 = 0;
    ws.write_with_format(0, spalte, label, &kopf).map_err(x)?;
    ws.set_column_width(0, 24).map_err(x)?;
    for a in &ewh.aufgaben {
        spalte += 1;
        ws.write_with_format(
            0,
            spalte,
            format!("Aufgabe {}\n({} BE)", a.nr, be_text(a.max_be)),
            &kopf,
        )
        .map_err(x)?;
        ws.set_column_width(spalte, 11).map_err(x)?;
    }
    let s_summe = spalte + 1;
    ws.write_with_format(
        0,
        s_summe,
        format!("Summe\n({} BE)", be_text(ewh.gesamt_be())),
        &kopf,
    )
    .map_err(x)?;
    ws.write_with_format(0, s_summe + 1, "Prozent", &kopf)
        .map_err(x)?;
    ws.write_with_format(
        0,
        s_summe + 2,
        if ewh.stufe == Stufe::Oberstufe {
            "NP"
        } else {
            "Note"
        },
        &kopf,
    )
    .map_err(x)?;
    for c in s_summe..=s_summe + 2 {
        ws.set_column_width(c, 11).map_err(x)?;
    }
    ws.set_row_height(0, 32).map_err(x)?;
    for (i, z) in zeilen.iter().enumerate() {
        let r = i as u32 + 1;
        ws.write_with_format(r, 0, &z.bezeichnung, &zelle)
            .map_err(x)?;
        for (j, v) in z.je_aufgabe.iter().enumerate() {
            ws.write_with_format(r, j as u16 + 1, *v, &zahlf)
                .map_err(x)?;
        }
        ws.write_with_format(r, s_summe, z.summe, &zahlf)
            .map_err(x)?;
        ws.write_with_format(r, s_summe + 1, (z.prozent * 10.0).round() / 10.0, &zahlf)
            .map_err(x)?;
        match ewh.stufe {
            Stufe::Oberstufe => match z.punkte {
                Some(p) => ws
                    .write_with_format(r, s_summe + 2, p as f64, &ganz)
                    .map_err(x)?,
                None => ws.write_with_format(r, s_summe + 2, "", &ganz).map_err(x)?,
            },
            Stufe::Sek1 => ws
                .write_with_format(r, s_summe + 2, z.note.clone().unwrap_or_default(), &ganz)
                .map_err(x)?,
        };
    }
    ws.set_freeze_panes(1, 1).map_err(x)?;

    let ws = wb.add_worksheet().set_name("Statistik").map_err(x)?;
    ws.set_column_width(0, 44).map_err(x)?;
    for c in 1..=3 {
        ws.set_column_width(c, 13).map_err(x)?;
    }
    let mut r = 0u32;
    let paar =
        |ws: &mut rust_xlsxwriter::Worksheet, r: &mut u32, a: &str, b: String| -> Result<()> {
            ws.write_with_format(*r, 0, a, &zelle).map_err(x)?;
            ws.write_with_format(*r, 1, b, &ganz).map_err(x)?;
            *r += 1;
            Ok(())
        };
    ws.write_with_format(r, 0, "Überblick", &fett).map_err(x)?;
    r += 1;
    paar(ws, &mut r, "Ausgewertete Klausuren", st.anzahl.to_string())?;
    paar(ws, &mut r, "Durchschnitt", wert_text(st, st.durchschnitt))?;
    paar(ws, &mut r, "Median", wert_text(st, st.median))?;
    paar(ws, &mut r, "Minimum", wert_text(st, st.minimum))?;
    paar(ws, &mut r, "Maximum", wert_text(st, st.maximum))?;
    paar(
        ws,
        &mut r,
        "Durchschnitt in Prozent",
        st.durchschnitt_prozent.map(prozent).unwrap_or_default(),
    )?;
    paar(
        ws,
        &mut r,
        if st.stufe == Stufe::Oberstufe {
            "Anteil unter 5 NP"
        } else {
            "Anteil Noten 5 und 6"
        },
        st.schlecht_prozent.map(prozent).unwrap_or_default(),
    )?;
    paar(
        ws,
        &mut r,
        "Geänderte KI-Vorschläge",
        st.geaendert_prozent.map(prozent).unwrap_or_default(),
    )?;
    r += 1;
    ws.write_with_format(r, 0, "Notenspiegel", &fett)
        .map_err(x)?;
    r += 1;
    ws.write_with_format(
        r,
        0,
        if st.stufe == Stufe::Oberstufe {
            "NP"
        } else {
            "Note"
        },
        &kopf,
    )
    .map_err(x)?;
    ws.write_with_format(r, 1, "Anzahl", &kopf).map_err(x)?;
    r += 1;
    for n in &st.notenspiegel {
        ws.write_with_format(r, 0, &n.label, &zelle).map_err(x)?;
        ws.write_with_format(r, 1, n.anzahl as f64, &ganz)
            .map_err(x)?;
        r += 1;
    }
    for (titel, q) in [
        ("Lösungsquote je Aufgabe", &st.je_aufgabe),
        ("Lösungsquote je AFB", &st.je_afb),
        ("Schwächste Bewertungselemente", &st.schwaechste),
        ("Lösungsquote je Bewertungselement", &st.je_element),
    ] {
        r += 1;
        ws.write_with_format(r, 0, titel, &fett).map_err(x)?;
        r += 1;
        for (c, h) in ["", "erreicht", "möglich", "Quote %"].iter().enumerate() {
            ws.write_with_format(r, c as u16, *h, &kopf).map_err(x)?;
        }
        r += 1;
        for q in q.iter() {
            let name = match &q.zusatz {
                Some(z) => format!("{} ({z})", q.name),
                None => q.name.clone(),
            };
            ws.write_with_format(r, 0, name, &zelle).map_err(x)?;
            ws.write_with_format(r, 1, q.erreicht, &zahlf).map_err(x)?;
            ws.write_with_format(r, 2, q.moeglich, &zahlf).map_err(x)?;
            ws.write_with_format(r, 3, (q.quote * 10.0).round() / 10.0, &zahlf)
                .map_err(x)?;
            r += 1;
        }
    }
    wb.save_to_buffer().map_err(x)
}

fn schreiben(ordner: &Path, name: &str, bytes: &[u8], bericht: &mut ExportBericht) -> Result<()> {
    let ziel: PathBuf = ordner.join(name);
    crate::storage::write_atomic(&ziel, bytes)?;
    bericht.dateien.push(name.to_string());
    Ok(())
}

/// Schreibt alle gewählten Exporte in `ordner`.
pub fn exportieren(p: &Projekt, ordner: &Path, opt: &ExportOptionen) -> Result<ExportBericht> {
    let ewh = p
        .ewh()?
        .ok_or_else(|| Error::Ungueltig("Kein Erwartungshorizont im Projekt.".into()))?;
    let meta = p.meta()?;
    let label = bezeichnungs_label(p)?;
    let (zeilen, st) = statistik::berechnen(p, &ewh)?;
    if zeilen.is_empty() {
        return Err(Error::Ungueltig(
            "Noch keine Klausur freigegeben. Exportiert werden nur freigegebene Klausuren.".into(),
        ));
    }
    std::fs::create_dir_all(ordner)?;
    let projekt = dateiname(&meta.titel);
    let mut bericht = ExportBericht {
        ordner: ordner.display().to_string(),
        fehlend: st.nicht_freigegeben.clone(),
        ..Default::default()
    };
    if opt.notenliste {
        schreiben(
            ordner,
            &format!("Notenliste_{projekt}.xlsx"),
            &notenliste_xlsx(&ewh, &zeilen, &st, label)?,
            &mut bericht,
        )?;
        schreiben(
            ordner,
            &format!("Notenliste_{projekt}.csv"),
            &notenliste_csv(&ewh, &zeilen, label)?,
            &mut bericht,
        )?;
    }
    let freigegeben: Vec<_> = p
        .klausuren()?
        .into_iter()
        .filter(|k| k.status == KlausurStatus::Freigegeben)
        .collect();
    if opt.korrekturboegen || opt.sammeldatei {
        let mut sammel = Doc::neu();
        for (i, k) in freigegeben.iter().enumerate() {
            let d = korrekturbogen(p, &ewh, &meta, &k.id, label)?;
            if opt.korrekturboegen {
                schreiben(
                    ordner,
                    &format!(
                        "Korrekturbogen_{}_{projekt}.docx",
                        dateiname(&k.bezeichnung)
                    ),
                    &d.bytes()?,
                    &mut bericht,
                )?;
            }
            if i > 0 {
                sammel.seitenumbruch();
            }
            sammel.anhaengen(d);
        }
        if opt.sammeldatei {
            schreiben(
                ordner,
                &format!("Korrekturboegen_{projekt}.docx"),
                &sammel.bytes()?,
                &mut bericht,
            )?;
        }
    }
    for k in &freigegeben {
        match feedback_doc(p, &ewh, &meta, &k.id, label)? {
            Some(d) if opt.feedback => {
                schreiben(
                    ordner,
                    &format!("Feedback_{}_{projekt}.docx", dateiname(&k.bezeichnung)),
                    &d.bytes()?,
                    &mut bericht,
                )?;
            }
            Some(_) => {}
            None => bericht.ohne_feedback.push(k.bezeichnung.clone()),
        }
    }
    if opt.statistik {
        schreiben(
            ordner,
            &format!("Statistik_{projekt}.docx"),
            &statistik_doc(&ewh, &meta, &st).bytes()?,
            &mut bericht,
        )?;
    }
    Ok(bericht)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dateinamen_fuer_windows() {
        assert_eq!(
            dateiname("11.1 Klausur: Markt/Staat?"),
            "11.1_Klausur_Markt_Staat"
        );
        assert_eq!(dateiname("Edler Kranich"), "Edler_Kranich");
        assert_eq!(dateiname("  "), "Projekt");
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;
    use std::io::Read;

    fn docx_text(pfad: &Path) -> String {
        let mut z = zip::ZipArchive::new(std::fs::File::open(pfad).unwrap()).unwrap();
        let mut s = String::new();
        z.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        s
    }

    #[test]
    fn statistik_nur_freigegebene() {
        let dir = tempfile::tempdir().unwrap();
        let p = super::super::testdaten::projekt(dir.path());
        let e = super::super::testdaten::ewh();
        let (zeilen, st) = statistik::berechnen(&p, &e).unwrap();
        assert_eq!(zeilen.len(), 2);
        assert_eq!(st.nicht_freigegeben, vec!["Stiller Uhu".to_string()]);
        // k1: 9 von 10 BE = 90 % → 14 NP; k2: 2 von 10 = 20 % → 1 NP
        assert_eq!(zeilen[0].summe, 9.0);
        assert_eq!(zeilen[0].punkte, Some(14));
        assert_eq!(zeilen[1].punkte, Some(1));
        assert_eq!(st.durchschnitt, Some(7.5));
        assert_eq!(st.median, Some(7.5));
        assert_eq!(st.schlecht_anzahl, 1);
        assert!(st.ueber_warnschwelle);
        assert_eq!(st.notenspiegel.iter().map(|n| n.anzahl).sum::<usize>(), 2);
        let afb3 = st.je_afb.iter().find(|q| q.name == "AFB III").unwrap();
        assert_eq!((afb3.erreicht, afb3.moeglich), (7.0, 12.0));
        assert_eq!(st.schwaechste.len(), 3);
        assert_eq!(st.schwaechste[0].name, "Indikator");
    }

    #[test]
    fn exporte_schreiben() {
        let dir = tempfile::tempdir().unwrap();
        let p = super::super::testdaten::projekt(dir.path());
        p.feedback_speichern("k1", "Sie argumentieren sicher.", false)
            .unwrap();
        let ziel = dir.path().join("export");
        let opt = ExportOptionen {
            feedback: true,
            ..Default::default()
        };
        let b = exportieren(&p, &ziel, &opt).unwrap();
        assert_eq!(b.fehlend, vec!["Stiller Uhu".to_string()]);
        assert_eq!(b.ohne_feedback, vec!["Kluger Dachs".to_string()]);
        for f in [
            "Notenliste_11.1_Klausur.xlsx",
            "Notenliste_11.1_Klausur.csv",
            "Korrekturbogen_Edler_Kranich_11.1_Klausur.docx",
            "Korrekturbogen_Kluger_Dachs_11.1_Klausur.docx",
            "Korrekturboegen_11.1_Klausur.docx",
            "Feedback_Edler_Kranich_11.1_Klausur.docx",
            "Statistik_11.1_Klausur.docx",
        ] {
            assert!(ziel.join(f).exists(), "{f} fehlt");
        }
        assert!(!ziel
            .join("Korrekturbogen_Stiller_Uhu_11.1_Klausur.docx")
            .exists());
        let csv = std::fs::read(ziel.join("Notenliste_11.1_Klausur.csv")).unwrap();
        assert_eq!(&csv[..3], &[0xEF, 0xBB, 0xBF]);
        let csv = String::from_utf8(csv[3..].to_vec()).unwrap();
        assert!(csv.starts_with("Name;Aufgabe 1 (4 BE);Aufgabe 2 (6 BE);Summe (10 BE);Prozent;NP"));
        assert!(csv.contains("Edler Kranich;3;6;9;90;14"));
        let kb = docx_text(&ziel.join("Korrekturbogen_Edler_Kranich_11.1_Klausur.docx"));
        assert!(kb.contains("Korrekturbogen") && kb.contains("Edler Kranich"));
        assert!(kb.contains("Bewertungsraster Aufgabe 2"));
        assert!(kb.contains("Sie argumentieren sicher."));
        assert!(kb.contains("Indikator (1/2)"));
        let sammel = docx_text(&ziel.join("Korrekturboegen_11.1_Klausur.docx"));
        assert!(sammel.contains("Kluger Dachs") && sammel.contains("w:type=\"page\""));
        let st = docx_text(&ziel.join("Statistik_11.1_Klausur.docx"));
        assert!(st.contains("Notenspiegel") && st.contains("über der Warnschwelle"));
        // Mit Pseudonymliste heißt das Feld „Pseudonym“
        p.set_pseudonyme(&["Edler Kranich".into()]).unwrap();
        exportieren(&p, &ziel, &opt).unwrap();
        assert!(
            docx_text(&ziel.join("Korrekturbogen_Edler_Kranich_11.1_Klausur.docx"))
                .contains("Pseudonym: ")
        );
        // Zum Ansehen: GODGRADER_EXPORT_DIR=/pfad cargo test exporte_schreiben
        if let Ok(d) = std::env::var("GODGRADER_EXPORT_DIR") {
            exportieren(&p, Path::new(&d), &opt).unwrap();
        }
    }
}
