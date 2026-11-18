//! Blöcke → Erwartungshorizont (godtest-Format).
//!
//! Erkennt Kopf, Aufgaben mit `[x BE]`, Bewertungselemente mit „(x BE)“,
//! Tabellen mit BE in den Spaltenköpfen, „je“-Angaben („drei Ziele, je Ziel
//! Indikator (1 BE) und Zielerreichung (1 BE)“), Bewertungsraster,
//! AFB-Übersicht und Notenschlüssel.

use std::sync::LazyLock;

use regex::Regex;

use super::blocks::{Block, Para, Table};
use super::noten::{Notenschluessel, Notenstufe};
use super::{be_text, Afb};
use super::{AfbAnteil, Aufgabe, Element, ElementArt, Erwartungshorizont, Kopf, Stufe};

static RE_TASK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(\d{1,2}(?:\.\d{1,2})?[a-z]?)[.)]?\t").unwrap());
static RE_TASK_BE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\s*(\d+(?:[.,]5)?)\s*BE\s*\]").unwrap());
/// „(1 BE)“, „(je 1 BE, 6 BE)“, „(2 BE)“
static RE_MARK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\((je\s+)?(\d+(?:[.,]5)?)\s*BE(?:\s*,\s*(?:insg(?:esamt|\.)\s*)?(\d+(?:[.,]5)?)\s*BE)?\)",
    )
    .unwrap()
});
static RE_JE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\bje\s+([A-ZÄÖÜ][\wäöüß-]*)").unwrap());
static RE_COUNT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(zwei|drei|vier|fünf|sechs|sieben|acht|2|3|4|5|6|7|8)\s+[A-ZÄÖÜ][\wäöüß-]*")
        .unwrap()
});
static RE_BE_NUM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+(?:[.,]5)?)\s*BE").unwrap());
static RE_AFB_MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\(AFB\s+(I{1,3})\)").unwrap());
static RE_DATUM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{1,2}\.\d{1,2}\.\d{4}").unwrap());
static RE_SJ: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"SJ\s*\d{4}/\d{2,4}").unwrap());

fn num(s: &str) -> f64 {
    s.replace(',', ".").parse().unwrap_or(0.0)
}

fn count_word(s: &str) -> Option<f64> {
    Some(match s.to_lowercase().as_str() {
        "zwei" | "2" => 2.0,
        "drei" | "3" => 3.0,
        "vier" | "4" => 4.0,
        "fünf" | "5" => 5.0,
        "sechs" | "6" => 6.0,
        "sieben" | "7" => 7.0,
        "acht" | "8" => 8.0,
        _ => return None,
    })
}

fn clean(s: &str) -> String {
    s.replace('\t', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Kurzer Titel aus einem Textstück.
fn short_title(s: &str) -> String {
    let s = clean(s)
        .trim_matches(|c: char| c == ':' || c == ',' || c == '–' || c == '-' || c.is_whitespace())
        .trim_start_matches("• ")
        .to_string();
    let s = s.strip_prefix("und ").unwrap_or(&s).to_string();
    if s.chars().count() <= 70 {
        return s;
    }
    let mut out = String::new();
    for w in s.split(' ') {
        if out.chars().count() + w.chars().count() > 64 {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    format!("{out} …")
}

fn is_task_start(p: &Para) -> Option<String> {
    if p.centered {
        return None;
    }
    RE_TASK.captures(&p.text()).map(|c| c[1].to_string())
}

fn is_quote(p: &Para) -> bool {
    let t = p.text();
    let t = t.trim();
    (t.starts_with('„') || t.starts_with('"') || t.starts_with('»'))
        && (t.ends_with('“') || t.ends_with('"') || t.ends_with('«'))
        && t.chars().count() < 400
}

fn is_section_heading(p: &Para) -> bool {
    let t = clean(&p.text());
    matches!(
        t.as_str(),
        "AFB"
            | "AFB-Übersicht"
            | "Anforderungsbereiche"
            | "Notenschlüssel"
            | "Bewertung"
            | "Notentabelle"
    )
}

fn is_afb_table(t: &Table) -> bool {
    t.rows
        .first()
        .is_some_and(|r| r.iter().any(|c| clean(&c.text()) == "AFB I"))
}

fn is_noten_table(t: &Table) -> bool {
    t.rows.iter().any(|r| {
        let f = r.first().map(|c| clean(&c.text())).unwrap_or_default();
        f == "Notenpunkte" || f == "ab BE"
    })
}

fn is_raster_table(t: &Table) -> bool {
    t.rows.first().is_some_and(|r| {
        let f = r.first().map(|c| clean(&c.text())).unwrap_or_default();
        (f.starts_with("Anforderungsbereich")
            || f.starts_with("Bewertungsraster")
            || f.starts_with("Kriterien"))
            && r.iter().any(|c| c.text().contains("BE"))
    })
}

fn table_has_marks(t: &Table) -> bool {
    t.rows.iter().flatten().any(|c| RE_MARK.is_match(&c.text()))
}

fn block_has_marks(b: &Block) -> bool {
    match b {
        Block::Para(p) => RE_MARK.is_match(&p.text()),
        Block::Table(t) => table_has_marks(t) || is_raster_table(t),
    }
}

fn block_markdown(b: &Block) -> String {
    match b {
        Block::Para(p) => p.markdown(),
        Block::Table(t) => t.markdown(),
    }
}

#[derive(Debug)]
struct Cand {
    el: Element,
    /// Gruppenkopf wie „Kein vollkommener Wettbewerb (2 BE):“
    header: bool,
}

fn element(titel: String, text: String, max_be: f64) -> Element {
    let afb = RE_AFB_MARK.captures(&text).and_then(|c| Afb::parse(&c[1]));
    Element {
        id: String::new(),
        titel,
        text,
        max_be,
        art: ElementArt::Element,
        afb,
    }
}

/// Elemente aus einem Absatz.
fn para_elements(p: &Para, next_table: Option<&Table>, consumed_table: &mut bool) -> Vec<Cand> {
    let text = p.text();
    let md = p.markdown();
    let marks: Vec<regex::Captures> = RE_MARK.captures_iter(&text).collect();
    if marks.is_empty() {
        return vec![];
    }

    // „drei Ziele, je Ziel Indikator (1 BE) und Zielerreichung (1 BE)“
    if let Some(je) = RE_JE.captures(&text) {
        let je_pos = je.get(0).unwrap().end();
        let count = RE_COUNT
            .captures_iter(&text[..je.get(0).unwrap().start()])
            .last()
            .and_then(|c| count_word(&c[1]));
        let unit_marks: Vec<&regex::Captures> = marks
            .iter()
            .filter(|m| {
                m.get(0).unwrap().start() > je_pos && m.get(1).is_none() && m.get(3).is_none()
            })
            .collect();
        if let (Some(n), false) = (count, unit_marks.is_empty()) {
            let mut out = Vec::new();
            let mut start = je_pos;
            let pool = next_table.filter(|t| !is_raster_table(t));
            for m in unit_marks {
                let mm = m.get(0).unwrap();
                let label = short_title(&text[start..mm.start()]);
                start = mm.end();
                let mut body = format!(
                    "{} – {} × {} BE ({})",
                    label,
                    n,
                    be_text(num(&m[2])),
                    clean(&text)
                );
                if let Some(t) = pool {
                    body.push_str("\n\nMögliche Lösungen:\n");
                    body.push_str(&t.markdown());
                }
                out.push(Cand {
                    el: element(label, body, n * num(&m[2])),
                    header: false,
                });
            }
            if pool.is_some() {
                *consumed_table = true;
            }
            return out;
        }
    }

    // „(je 1 BE)“ ohne Anzahl und ohne Gesamtwert ist ein Hinweis zu
    // bestehenden Elementen, kein eigenes Element
    let marks: Vec<regex::Captures> = marks
        .into_iter()
        .filter(|m| !(m.get(1).is_some() && m.get(3).is_none()))
        .collect();
    if marks.is_empty() {
        return vec![];
    }

    // „(je 1 BE, 6 BE)“ → ein Element mit Gesamtwert
    if marks.len() == 1 {
        let m = &marks[0];
        let mm = m.get(0).unwrap();
        let max = match m.get(3) {
            Some(total) => num(total.as_str()),
            None => num(&m[2]),
        };
        let before = &text[..mm.start()];
        let after = text[mm.end()..].trim();
        // „**Formel:** …“ oder „**Vergleich** (1 BE): …“ → fetter Anfang als Titel
        let first_idx = p.runs.iter().position(|r| !r.text.trim().is_empty());
        let bold_lead = first_idx.and_then(|i| {
            let r = &p.runs[i];
            if !r.bold {
                return None;
            }
            let rest: String = p.runs[i + 1..].iter().map(|r| r.text.as_str()).collect();
            let rest = rest.trim_start();
            (r.text.trim_end().ends_with(':') || rest.starts_with('(') || rest.starts_with(':'))
                .then(|| r.text.clone())
        });
        let titel = match bold_lead {
            Some(b) if b.trim().chars().count() > 2 => short_title(&b),
            _ if !before.trim().is_empty() => short_title(before),
            _ => short_title(after),
        };
        let header = after == ":" || (after.is_empty() && before.trim_end().ends_with(':'));
        let header = header && m.get(1).is_none() && before.chars().count() < 80;
        let mut body = md.clone();
        if let (Some(t), true) = (
            next_table.filter(|t| !is_raster_table(t) && !table_has_marks(t)),
            text.trim_end().ends_with(':'),
        ) {
            body.push_str("\n\n");
            body.push_str(&t.markdown());
            *consumed_table = true;
        }
        return vec![Cand {
            el: element(titel, body, max),
            header,
        }];
    }

    // mehrere Marken in einem Absatz → aufteilen; eine erste Marke, die
    // genau der Summe der übrigen entspricht, ist eine Gesamtangabe
    let mut out = Vec::new();
    let mut start = 0;
    let rest_sum: f64 = marks.iter().skip(1).map(|m| num(&m[2])).sum();
    let skip_first = (num(&marks[0][2]) - rest_sum).abs() < 1e-9;
    let mut prefix = String::new();
    for (idx, m) in marks.iter().enumerate() {
        if idx == 0 && skip_first {
            let mm = m.get(0).unwrap();
            prefix = short_title(&text[..mm.start()]);
            start = mm.end();
            continue;
        }
        let mm = m.get(0).unwrap();
        let seg = &text[start..mm.start()];
        let max = match m.get(3) {
            Some(total) => num(total.as_str()),
            None => num(&m[2]),
        };
        let body = if prefix.is_empty() {
            clean(seg)
        } else {
            format!("{} (Teil von: {prefix})", clean(seg))
        };
        out.push(Cand {
            el: element(short_title(seg), body, max),
            header: false,
        });
        start = mm.end();
    }
    if let Some(last) = out.last_mut() {
        let rest = clean(&text[start..]);
        if !rest.is_empty() && rest != "." {
            last.el.text = format!("{} {}", last.el.text, rest);
        }
    }
    out
}

/// Elemente aus einer Tabelle mit BE in den Spaltenköpfen.
fn table_elements(t: &Table) -> Vec<Cand> {
    let mut out = Vec::new();
    let Some(head) = t.rows.first() else {
        return out;
    };
    let be_cols: Vec<(usize, f64, String)> = head
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let txt = c.text();
            RE_MARK.captures(&txt).map(|m| {
                let label = clean(&RE_MARK.replace_all(&txt, ""));
                (i, num(&m[2]), label)
            })
        })
        .collect();
    if !be_cols.is_empty() {
        for row in t.rows.iter().skip(1) {
            let label_col = (0..row.len()).find(|i| !be_cols.iter().any(|(c, _, _)| c == i));
            let row_label = label_col.map(|i| clean(&row[i].text())).unwrap_or_default();
            let mut context = String::new();
            for (i, c) in row.iter().enumerate() {
                if !be_cols.iter().any(|(bc, _, _)| *bc == i) && Some(i) != label_col {
                    let txt = clean(&c.text());
                    if !txt.is_empty() {
                        context.push_str(&format!(" ({}: {})", clean(&head[i].text()), txt));
                    }
                }
            }
            for (c, be, col_label) in &be_cols {
                let Some(cell) = row.get(*c) else { continue };
                if cell.text().trim().is_empty() {
                    continue;
                }
                let titel = short_title(&format!("{row_label} – {col_label}"));
                let text = format!("{row_label}{context} – {col_label}: {}", cell.markdown());
                out.push(Cand {
                    el: element(titel, text, *be),
                    header: false,
                });
            }
        }
        return out;
    }
    // sonst: Zellen mit eigenen Marken
    for row in &t.rows {
        let row_label = row.first().map(|c| clean(&c.text())).unwrap_or_default();
        for (i, c) in row.iter().enumerate() {
            let txt = c.text();
            for m in RE_MARK.captures_iter(&txt) {
                let max = match m.get(3) {
                    Some(total) => num(total.as_str()),
                    None => num(&m[2]),
                };
                let titel = if i == 0 {
                    short_title(&txt)
                } else {
                    short_title(&format!(
                        "{row_label} – {}",
                        clean(&RE_MARK.replace_all(&txt, ""))
                    ))
                };
                out.push(Cand {
                    el: element(titel, format!("{row_label}: {}", c.markdown()), max),
                    header: false,
                });
            }
        }
    }
    out
}

fn raster_elements(t: &Table) -> (Vec<Element>, Option<f64>) {
    let head = &t.rows[0];
    let max_col = head
        .iter()
        .position(|c| c.text().to_lowercase().contains("maximal"))
        .unwrap_or(head.len().saturating_sub(1));
    let mut els = Vec::new();
    let mut total = None;
    for row in t.rows.iter().skip(1) {
        let Some(first) = row.first() else { continue };
        let crit = first.text();
        let max_txt = row.get(max_col).map(|c| c.text()).unwrap_or_default();
        let max = RE_BE_NUM.captures(&max_txt).map(|c| num(&c[1]));
        if clean(&crit).starts_with("Gesamt") {
            total = max;
            continue;
        }
        if clean(&crit).is_empty() {
            continue;
        }
        let max = max.or_else(|| RE_MARK.captures(&crit).map(|m| num(&m[2])));
        let Some(max) = max else { continue };
        let first_line = first
            .paras
            .iter()
            .map(|p| p.text())
            .find(|t| !t.trim().is_empty())
            .unwrap_or_default();
        let titel = short_title(
            first_line
                .trim()
                .trim_end_matches(':')
                .trim_end_matches("z. B.")
                .trim_end_matches(", z. B.:")
                .trim_end_matches(','),
        );
        let mut e = element(titel, first.markdown(), max);
        e.art = ElementArt::Raster;
        els.push(e);
    }
    (els, total)
}

fn resolve_headers(cands: Vec<Cand>) -> Vec<Element> {
    let mut out: Vec<Element> = Vec::new();
    let mut i = 0;
    while i < cands.len() {
        if cands[i].header {
            let target = cands[i].el.max_be;
            let mut sum = 0.0;
            let mut j = i + 1;
            while j < cands.len() && !cands[j].header && sum + 1e-9 < target {
                sum += cands[j].el.max_be;
                j += 1;
            }
            if (sum - target).abs() < 1e-9 && j > i + 1 {
                let head = cands[i].el.titel.clone();
                for c in &cands[i + 1..j] {
                    let mut e = c.el.clone();
                    e.text = format!("{} (Teil von: {head})", e.text);
                    out.push(e);
                }
                i = j;
                continue;
            }
        }
        out.push(cands[i].el.clone());
        i += 1;
    }
    out
}

struct TaskRaw {
    nr: String,
    text_blocks: Vec<Block>,
    max_be: Option<f64>,
    sol: Vec<Block>,
}

fn parse_header(paras: &[&Para]) -> (Kopf, Option<Stufe>) {
    let mut k = Kopf::default();
    let mut stufe = None;
    for p in paras {
        let t = clean(&p.text());
        if t.is_empty() {
            continue;
        }
        if k.titel.is_empty() {
            k.titel = t.clone();
            if t.contains("Klassenarbeit")
                || t.contains("Leistungskontrolle")
                || t.contains("Klasse ")
            {
                stufe = Some(Stufe::Sek1);
            }
            continue;
        }
        if let Some(m) = RE_SJ.find(&t) {
            k.schuljahr = Some(m.as_str().to_string());
            let schule = clean(&t[..m.start()]);
            if !schule.is_empty() {
                k.schule = Some(schule);
            }
            continue;
        }
        if let Some(th) = t.strip_prefix("Thema:") {
            k.thema = Some(th.trim().to_string());
            continue;
        }
        if t.contains("Name:") || t.contains("Pseudonym:") {
            k.datum = RE_DATUM.find(&t).map(|m| m.as_str().to_string());
            for (label, st) in [("Kurs:", Stufe::Oberstufe), ("Klasse:", Stufe::Sek1)] {
                if let Some(pos) = t.find(label) {
                    stufe = Some(st);
                    let rest = &t[pos + label.len()..];
                    let end = rest.find("Datum:").unwrap_or(rest.len());
                    let v = rest[..end].trim();
                    if !v.is_empty() {
                        k.lerngruppe = Some(v.to_string());
                    }
                }
            }
            continue;
        }
        if t.contains("gleichwertig zu bewerten") {
            k.gleichwertig_hinweis = true;
        }
    }
    (k, stufe)
}

fn parse_afb(t: &Table, aufgaben: &mut [Aufgabe], hints: &mut Vec<String>) -> Option<f64> {
    let head: Vec<String> = t.rows[0].iter().map(|c| clean(&c.text())).collect();
    let col = |name: &str| head.iter().position(|h| h == name);
    let row = |name: &str| {
        t.rows.iter().find(|r| {
            r.first()
                .is_some_and(|c| clean(&c.text()).starts_with(name))
        })
    };
    let (Some(r_auf), Some(r_be)) = (row("Aufgaben"), row("BE")) else {
        hints.push("AFB-Übersicht: Zeilen „Aufgaben“ und „BE“ nicht gefunden.".into());
        return None;
    };
    for (name, afb) in [
        ("AFB I", Afb::I),
        ("AFB II", Afb::II),
        ("AFB III", Afb::III),
    ] {
        let Some(c) = col(name) else { continue };
        let tasks_txt = r_auf.get(c).map(|x| clean(&x.text())).unwrap_or_default();
        let be_txt = r_be.get(c).map(|x| clean(&x.text())).unwrap_or_default();
        if tasks_txt.is_empty() || tasks_txt == "–" || tasks_txt == "-" {
            continue;
        }
        let items: Vec<(String, Option<String>)> = tasks_txt
            .split([',', ';'])
            .map(|s| {
                let s = s.trim();
                match s.find('(') {
                    Some(p) => (
                        s[..p].trim().to_string(),
                        Some(s[p + 1..].trim_end_matches(')').trim().to_string()),
                    ),
                    None => (s.to_string(), None),
                }
            })
            .filter(|(n, _)| !n.is_empty())
            .collect();
        let bes: Vec<f64> = be_txt
            .split(['+', ','])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(num)
            .collect();
        for (i, (nr, zusatz)) in items.iter().enumerate() {
            let Some(a) = aufgaben.iter_mut().find(|a| &a.nr == nr) else {
                hints.push(format!(
                    "AFB-Übersicht nennt Aufgabe {nr}, die nicht gefunden wurde."
                ));
                continue;
            };
            let be = if bes.len() == items.len() {
                bes[i]
            } else if items.len() == 1 && bes.len() == 1 {
                bes[0]
            } else {
                a.max_be
            };
            a.afb.push(AfbAnteil {
                afb,
                be,
                zusatz: zusatz.clone(),
            });
        }
    }
    col("gesamt")
        .and_then(|c| r_be.get(c))
        .map(|c| clean(&c.text()))
        .filter(|s| !s.is_empty())
        .map(|s| num(&s))
}

fn parse_noten(t: &Table) -> Option<Notenschluessel> {
    let row = |name: &str| {
        t.rows
            .iter()
            .find(|r| r.first().is_some_and(|c| clean(&c.text()) == name))
            .map(|r| {
                r.iter()
                    .skip(1)
                    .map(|c| clean(&c.text()))
                    .collect::<Vec<_>>()
            })
    };
    let ab = row("ab BE")?;
    let noten = row("Note");
    let punkte = row("Notenpunkte");
    let mut stufen = Vec::new();
    for (i, v) in ab.iter().enumerate() {
        if v.is_empty() {
            continue;
        }
        let p = punkte
            .as_ref()
            .and_then(|p| p.get(i))
            .and_then(|s| s.parse::<u8>().ok());
        let note = noten
            .as_ref()
            .and_then(|n| n.get(i).cloned())
            .or_else(|| p.map(|x| x.to_string()))
            .unwrap_or_default();
        stufen.push(Notenstufe {
            punkte: p,
            note,
            ab_be: num(v),
        });
    }
    (!stufen.is_empty()).then_some(Notenschluessel {
        stufen,
        aus_dokument: true,
    })
}

/// Hauptfunktion.
pub fn parse(blocks: &[Block]) -> Erwartungshorizont {
    let mut ewh = Erwartungshorizont::default();
    let mut hints = Vec::new();

    // Kopf: alles vor der ersten Aufgabe
    let first_task = blocks
        .iter()
        .position(|b| matches!(b, Block::Para(p) if is_task_start(p).is_some()))
        .unwrap_or(blocks.len());
    let head_paras: Vec<&Para> = blocks[..first_task]
        .iter()
        .filter_map(|b| match b {
            Block::Para(p) => Some(p),
            _ => None,
        })
        .collect();
    let (kopf, stufe) = parse_header(&head_paras);
    ewh.kopf = kopf;
    ewh.stufe = stufe.unwrap_or_default();

    // Aufgaben einsammeln
    let mut tasks: Vec<TaskRaw> = Vec::new();
    let mut tail: Vec<Block> = Vec::new(); // AFB, Notenschlüssel
    let mut i = first_task;
    let mut in_tail = false;
    while i < blocks.len() {
        let b = &blocks[i];
        if let Block::Para(p) = b {
            if is_section_heading(p) {
                in_tail = true;
                i += 1;
                continue;
            }
            if let (Some(nr), false) = (is_task_start(p), in_tail) {
                // Aufgabentext bis „[x BE]“
                let mut text_blocks = vec![b.clone()];
                let mut max_be = RE_TASK_BE.captures(&p.text()).map(|c| num(&c[1]));
                let mut j = i + 1;
                while max_be.is_none() && j < blocks.len() && j < i + 8 {
                    match &blocks[j] {
                        Block::Para(q)
                            if is_task_start(q).is_none() && !RE_MARK.is_match(&q.text()) =>
                        {
                            max_be = RE_TASK_BE.captures(&q.text()).map(|c| num(&c[1]));
                            text_blocks.push(blocks[j].clone());
                            j += 1;
                        }
                        _ => break,
                    }
                }
                if max_be.is_none() {
                    // kein [x BE] gefunden: nur die erste Zeile ist Aufgabentext
                    text_blocks.truncate(1);
                    j = i + 1;
                    hints.push(format!("Aufgabe {nr}: keine Angabe „[x BE]“ gefunden."));
                }
                // These/Zitat am Ende der vorigen Lösung gehört zu dieser Aufgabe
                if let Some(prev) = tasks.last_mut() {
                    if let Some(Block::Para(q)) = prev.sol.last() {
                        if is_quote(q) {
                            let q = prev.sol.pop().unwrap();
                            text_blocks.insert(0, q);
                        }
                    }
                }
                tasks.push(TaskRaw {
                    nr,
                    text_blocks,
                    max_be,
                    sol: Vec::new(),
                });
                i = j;
                continue;
            }
        }
        if let Block::Table(t) = b {
            if is_afb_table(t) || is_noten_table(t) {
                in_tail = true;
            }
        }
        if in_tail {
            tail.push(b.clone());
        } else if let Some(t) = tasks.last_mut() {
            t.sol.push(b.clone());
        }
        i += 1;
    }

    // Aufgaben auswerten
    for t in tasks {
        let mut sol = t.sol;
        // Material direkt nach dem Aufgabentext (Tabelle ohne BE, „Quelle: …“)
        let mut text_blocks = t.text_blocks;
        while let Some(b) = sol.first() {
            let material = match b {
                Block::Table(tb) => !table_has_marks(tb) && !is_raster_table(tb),
                Block::Para(p) => clean(&p.text()).starts_with("Quelle:"),
            };
            let next_has_marks = sol.iter().any(block_has_marks);
            if material && next_has_marks {
                text_blocks.push(sol.remove(0));
            } else {
                break;
            }
        }
        let nr_prefix = Regex::new(&format!(
            r"^\s*(?:\*+)?{}(?:\*+)?[.)]?\s+",
            regex::escape(&t.nr)
        ))
        .unwrap();
        let text = text_blocks
            .iter()
            .map(|b| {
                let md = block_markdown(b);
                let md = RE_TASK_BE.replace_all(&md, "").to_string();
                nr_prefix.replace(&md, "").trim().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let loesung = sol
            .iter()
            .map(block_markdown)
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        let mut cands: Vec<Cand> = Vec::new();
        let mut raster: Vec<Element> = Vec::new();
        let mut raster_total = None;
        let mut k = 0;
        while k < sol.len() {
            match &sol[k] {
                Block::Para(p) => {
                    let next_table = match sol.get(k + 1) {
                        Some(Block::Table(tb)) => Some(tb),
                        _ => None,
                    };
                    let mut consumed = false;
                    cands.extend(para_elements(p, next_table, &mut consumed));
                    if consumed {
                        k += 1;
                    }
                }
                Block::Table(tb) if is_raster_table(tb) => {
                    let (els, total) = raster_elements(tb);
                    raster.extend(els);
                    raster_total = total.or(raster_total);
                }
                Block::Table(tb) => cands.extend(table_elements(tb)),
            }
            k += 1;
        }
        let mut elemente = resolve_headers(cands);
        let hat_raster = !raster.is_empty();
        elemente.extend(raster);
        let max_be = t
            .max_be
            .unwrap_or_else(|| elemente.iter().map(|e| e.max_be).sum());
        if let Some(total) = raster_total {
            let s: f64 = elemente
                .iter()
                .filter(|e| e.art == ElementArt::Raster)
                .map(|e| e.max_be)
                .sum();
            if (s - total).abs() > 1e-9 {
                hints.push(format!(
                    "Aufgabe {}: Rasterzeilen ergeben {} BE, „Gesamtpunktzahl“ nennt {} BE.",
                    t.nr,
                    be_text(s),
                    be_text(total)
                ));
            }
        }
        if elemente.is_empty() {
            hints.push(format!(
                "Aufgabe {}: keine Bewertungselemente erkannt – als ein Element übernommen. Bitte prüfen oder „KI-Hilfe beim Import“ nutzen.",
                t.nr
            ));
            elemente.push(element("Gesamte Aufgabe".into(), loesung.clone(), max_be));
        }
        ewh.aufgaben.push(Aufgabe {
            nr: t.nr,
            text,
            max_be,
            loesung,
            elemente,
            afb: Vec::new(),
            hat_raster,
        });
    }

    // AFB-Übersicht und Notenschlüssel
    for b in &tail {
        if let Block::Table(t) = b {
            if is_afb_table(t) {
                ewh.gesamt_be_dokument = parse_afb(t, &mut ewh.aufgaben, &mut hints);
            } else if is_noten_table(t) {
                ewh.notenschluessel = parse_noten(t);
                if ewh
                    .notenschluessel
                    .as_ref()
                    .is_some_and(|n| n.stufen.iter().all(|s| s.punkte.is_none()))
                {
                    ewh.stufe = Stufe::Sek1;
                }
            }
        }
    }

    // AFB je Element
    for a in &mut ewh.aufgaben {
        let main = a.haupt_afb();
        for e in &mut a.elemente {
            if e.afb.is_none() {
                e.afb = main;
            }
        }
    }

    ewh.renumber();
    ewh.import_hinweise = hints;
    ewh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ewh::blocks::{Cell, Run};

    fn p(t: &str) -> Block {
        Block::Para(Para::plain(t))
    }
    fn cell(t: &str) -> Cell {
        Cell {
            paras: vec![Para::plain(t)],
            shaded: false,
        }
    }
    fn table(rows: &[&[&str]]) -> Block {
        Block::Table(Table {
            rows: rows
                .iter()
                .map(|r| r.iter().map(|c| cell(c)).collect())
                .collect(),
        })
    }

    fn beispiel() -> Vec<Block> {
        vec![
            p("11.2 Klausur im Fach Wirtschaft und Recht – Erwartungshorizont"),
            p("\tStaatliches Gymnasium Bergschule Apolda\tSJ 2026/27"),
            p("Thema: Konjunktur"),
            p("Name:\tKurs: WR_eA\tDatum: 21.01.2027"),
            p("Andere sachlich richtige Lösungen sind gleichwertig zu bewerten."),
            p("1a\tNennen Sie drei Ziele.\t[3 BE]"),
            p("Ziele (je 1 BE, 3 BE): A, B, C"),
            p("2\tBerechnen Sie die Quote. (Formel, Rechenweg, Lösung)"),
            p("Vergleichen Sie.\t[3 BE]"),
            p("Formel: x = y (1 BE)"),
            p("Rechenweg und Lösung: 6,3 % (1 BE)"),
            p("Hinweis ohne BE."),
            p("Vergleich (1 BE): liegt darüber."),
            p("3\tOrdnen Sie zu.\t[4 BE]"),
            table(&[
                &["Maßnahme", "Art (1 BE)", "Begründung (1 BE)"],
                &["A", "erhaltend", "weil …"],
                &["B", "anpassend", "weil …"],
            ]),
            p("4\tErklären Sie an zwei Merkmalen.\t[4 BE]"),
            p("Zwei Merkmale, je Merkmal Erläuterung (1 BE) und Beleg (1 BE), z. B.:"),
            table(&[
                &["Merkmal", "Erläuterung (1 BE)", "Beleg (1 BE)"],
                &["X", "a", "b"],
                &["Y", "c", "d"],
                &["Z", "e", "f"],
            ]),
            p("5\tErklären Sie.\t[4 BE]"),
            p("Teil A (2 BE):"),
            p("• erstes (1 BE)"),
            p("• zweites (1 BE)"),
            p("Teil B (2 BE) – eins (1 BE), zwei (1 BE)"),
            p("„These“"),
            p("6\tErörtern Sie.\t[6 BE]"),
            table(&[
                &["Anforderungsbereich III", "erreichte BE", "maximale BE"],
                &["klärt die Fachbegriffe (AFB II)", "", "2 BE"],
                &["Fazit", "", "4 BE"],
                &["Gesamtpunktzahl", "", "6 BE"],
            ]),
            p("AFB"),
            table(&[
                &["", "AFB I", "AFB II", "AFB III", "gesamt"],
                &["Aufgaben", "1a", "2, 3, 4, 5, 6 (Fachbegriffe)", "6", ""],
                &["BE", "3", "3 + 4 + 4 + 4 + 2", "4", "24"],
                &["Anteil", "", "", "", ""],
            ]),
            p("Notenschlüssel"),
            table(&[
                &["Note", "1", "2", "3", "4", "5", "6"],
                &["ab BE", "24", "20", "16", "12", "6", "0"],
            ]),
        ]
    }

    #[test]
    fn kopf() {
        let e = parse(&beispiel());
        assert_eq!(
            e.kopf.titel,
            "11.2 Klausur im Fach Wirtschaft und Recht – Erwartungshorizont"
        );
        assert_eq!(e.kopf.schuljahr.as_deref(), Some("SJ 2026/27"));
        assert_eq!(
            e.kopf.schule.as_deref(),
            Some("Staatliches Gymnasium Bergschule Apolda")
        );
        assert_eq!(e.kopf.thema.as_deref(), Some("Konjunktur"));
        assert_eq!(e.kopf.lerngruppe.as_deref(), Some("WR_eA"));
        assert_eq!(e.kopf.datum.as_deref(), Some("21.01.2027"));
        assert!(e.kopf.gleichwertig_hinweis);
    }

    #[test]
    fn aufgaben_und_elemente() {
        let e = parse(&beispiel());
        let nrs: Vec<&str> = e.aufgaben.iter().map(|a| a.nr.as_str()).collect();
        assert_eq!(nrs, ["1a", "2", "3", "4", "5", "6"]);
        for a in &e.aufgaben {
            assert!(
                (a.summe_elemente() - a.max_be).abs() < 1e-9,
                "Aufgabe {}: {} vs {} {:#?}",
                a.nr,
                a.summe_elemente(),
                a.max_be,
                a.elemente
            );
        }
        let a1 = &e.aufgaben[0];
        assert_eq!(a1.elemente.len(), 1);
        assert_eq!(a1.elemente[0].titel, "Ziele");
        let a2 = &e.aufgaben[1];
        assert_eq!(a2.elemente.len(), 3);
        assert_eq!(a2.elemente[0].id, "2-1");
        assert_eq!(a2.elemente[0].titel, "Formel: x = y");
        assert!(a2.text.contains("Vergleichen Sie."));
        assert!(a2.text.starts_with("Berechnen"), "{}", a2.text);
        assert!(a2.loesung.contains("Hinweis ohne BE."));
        let a3 = &e.aufgaben[2];
        assert_eq!(a3.elemente.len(), 4);
        assert_eq!(a3.elemente[1].titel, "A – Begründung");
        let a4 = &e.aufgaben[3];
        assert_eq!(a4.elemente.len(), 2);
        assert_eq!(a4.elemente[0].max_be, 2.0);
        assert_eq!(a4.elemente[0].titel, "Erläuterung");
        assert!(a4.elemente[0].text.contains("Mögliche Lösungen"));
        let a5 = &e.aufgaben[4];
        assert_eq!(a5.elemente.len(), 4, "{:#?}", a5.elemente);
        let a6 = &e.aufgaben[5];
        assert!(a6.hat_raster);
        assert!(a6.text.starts_with("„These“"));
        assert_eq!(a6.elemente[0].afb, Some(Afb::II));
        assert_eq!(a6.elemente[1].afb, Some(Afb::III));
    }

    #[test]
    fn afb_und_noten() {
        let e = parse(&beispiel());
        assert_eq!(e.gesamt_be_dokument, Some(24.0));
        let a6 = &e.aufgaben[5];
        assert_eq!(a6.afb.len(), 2);
        assert_eq!(a6.haupt_afb(), Some(Afb::III));
        assert_eq!(e.aufgaben[0].afb[0].afb, Afb::I);
        let n = e.notenschluessel.as_ref().unwrap();
        assert_eq!(n.stufen.len(), 6);
        assert_eq!(e.stufe, Stufe::Sek1);
        assert!(e.import_hinweise.is_empty(), "{:?}", e.import_hinweise);
    }

    #[test]
    fn fett_als_titel() {
        let para = Para {
            runs: vec![
                Run {
                    text: "Formel:".into(),
                    bold: true,
                    italic: false,
                },
                Run {
                    text: " a = b (1 BE)".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut c = false;
        let els = para_elements(&para, None, &mut c);
        assert_eq!(els[0].el.titel, "Formel");
    }
}
