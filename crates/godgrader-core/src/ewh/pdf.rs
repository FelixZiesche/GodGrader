//! PDF → Blöcke. Rekonstruiert Absätze und Tabellen aus Zeichenpositionen
//! und den Tabellenlinien (Word zeichnet sie als dünne, gefüllte Rechtecke).

use pdfium_render::prelude::*;

use super::blocks::{Block, Cell, Para, Run, Table};
use crate::{Error, Result};

/// Zeichen mit Lage; y wächst nach unten (0 = oberer Seitenrand).
#[derive(Debug, Clone, PartialEq)]
pub struct RawChar {
    pub ch: char,
    pub x0: f32,
    pub x1: f32,
    /// Grundlinie
    pub y: f32,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    /// Im Textfluss stand direkt davor ein Leerzeichen.
    pub space_before: bool,
}

/// Gefülltes Rechteck; `gray` 0 = schwarz, 1 = weiß.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawRect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub gray: f32,
}

#[derive(Debug, Clone, Default)]
pub struct RawPage {
    pub width: f32,
    pub height: f32,
    pub chars: Vec<RawChar>,
    pub rects: Vec<RawRect>,
}

/// Liest Zeichen und Rechtecke aller Seiten mit PDFium.
pub fn extract(bytes: &[u8]) -> Result<Vec<RawPage>> {
    let pdfium = crate::pdfium::pdfium()?;
    let doc = pdfium
        .load_pdf_from_byte_slice(bytes, None)
        .map_err(|e| Error::Ungueltig(format!("PDF nicht lesbar: {e}")))?;
    let mut pages = Vec::new();
    for page in doc.pages().iter() {
        let h = page.height().value;
        let mut raw = RawPage {
            width: page.width().value,
            height: h,
            ..Default::default()
        };
        if let Ok(text) = page.text() {
            let mut space = false;
            for c in text.chars().iter() {
                let Some(ch) = c.unicode_char() else { continue };
                // PDFium meldet Trennstriche am Zeilenende teils als Steuerzeichen
                let ch = if ch == '\u{2}' || ch == '\u{ad}' {
                    '-'
                } else {
                    ch
                };
                if ch.is_whitespace() || ch.is_control() {
                    space = true;
                    continue;
                }
                let space_before = std::mem::take(&mut space);
                let Ok(b) = c.loose_bounds() else { continue };
                let y = c.origin_y().map(|v| v.value).unwrap_or(b.bottom().value);
                let name = c.font_name().to_ascii_lowercase();
                let weight_bold = matches!(
                    c.font_weight(),
                    Some(PdfFontWeight::Weight600)
                        | Some(PdfFontWeight::Weight700Bold)
                        | Some(PdfFontWeight::Weight800)
                        | Some(PdfFontWeight::Weight900)
                );
                raw.chars.push(RawChar {
                    ch: match ch {
                        '\u{f0b7}' | '\u{f0a7}' | '▪' | '●' => '•',
                        other => other,
                    },
                    x0: b.left().value,
                    x1: b.right().value,
                    y: h - y,
                    size: c.scaled_font_size().value,
                    bold: name.contains("bold") || name.contains("black") || weight_bold,
                    italic: name.contains("italic")
                        || name.contains("oblique")
                        || c.font_is_italic(),
                    space_before,
                });
            }
        }
        for obj in page.objects().iter() {
            if obj.object_type() != PdfPageObjectType::Path {
                continue;
            }
            let Ok(q) = obj.bounds() else { continue };
            let r = q.to_rect();
            let filled = obj
                .as_path_object()
                .and_then(|p| p.fill_mode().ok())
                .map(|m| m != PdfPathFillMode::None)
                .unwrap_or(false);
            let stroked = obj
                .as_path_object()
                .and_then(|p| p.is_stroked().ok())
                .unwrap_or(false);
            let color = if filled {
                obj.fill_color().ok()
            } else if stroked {
                obj.stroke_color().ok()
            } else {
                None
            };
            let Some(color) = color else { continue };
            if color.alpha() == 0 {
                continue;
            }
            let gray =
                (color.red() as f32 + color.green() as f32 + color.blue() as f32) / (3.0 * 255.0);
            raw.rects.push(RawRect {
                x0: r.left().value,
                x1: r.right().value,
                y0: h - r.top().value,
                y1: h - r.bottom().value,
                gray,
            });
        }
        pages.push(raw);
    }
    Ok(pages)
}

/// PDF-Datei → Blöcke.
pub fn read_pdf(bytes: &[u8]) -> Result<Vec<Block>> {
    let pages = extract(bytes)?;
    if pages.iter().all(|p| p.chars.is_empty()) {
        return Err(Error::Ungueltig(
            "Das PDF enthält keinen Text (eingescannt?). Bitte das Original aus Word als PDF speichern oder die .docx verwenden.".into(),
        ));
    }
    Ok(layout(&pages))
}

// ------------------------------------------------------------------
// Layout-Analyse (ohne PDFium testbar)
// ------------------------------------------------------------------

const RULE: f32 = 2.2; // max. Dicke einer Tabellenlinie in pt

fn is_hrule(r: &RawRect) -> bool {
    (r.y1 - r.y0) <= RULE && (r.x1 - r.x0) > 4.0 && r.gray < 0.8
}
fn is_vrule(r: &RawRect) -> bool {
    (r.x1 - r.x0) <= RULE && (r.y1 - r.y0) > 2.5 && r.gray < 0.8
}
fn is_shade(r: &RawRect) -> bool {
    (0.75..0.98).contains(&r.gray) && (r.x1 - r.x0) > 4.0 && (r.y1 - r.y0) > 4.0
}

fn cluster(mut v: Vec<f32>, tol: f32) -> Vec<f32> {
    v.sort_by(f32::total_cmp);
    let mut out: Vec<(f32, usize)> = Vec::new();
    for x in v {
        match out.last_mut() {
            Some((m, n)) if (x - *m / *n as f32).abs() <= tol => {
                *m += x;
                *n += 1;
            }
            _ => out.push((x, 1)),
        }
    }
    out.into_iter().map(|(m, n)| m / n as f32).collect()
}

#[derive(Debug, Clone)]
struct Grid {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    ys: Vec<f32>,
    xs: Vec<f32>,
    vrules: Vec<RawRect>,
}

fn find_grids(rects: &[RawRect]) -> Vec<Grid> {
    let rules: Vec<RawRect> = rects
        .iter()
        .copied()
        .filter(|r| is_hrule(r) || is_vrule(r))
        .collect();
    let n = rules.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let touch = |a: &RawRect, b: &RawRect| {
        let e = 1.5;
        a.x0 - e <= b.x1 && b.x0 - e <= a.x1 && a.y0 - e <= b.y1 && b.y0 - e <= a.y1
    };
    for i in 0..n {
        for j in i + 1..n {
            if touch(&rules[i], &rules[j]) {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<RawRect>> = Default::default();
    for (i, r) in rules.iter().enumerate() {
        groups.entry(root(&mut parent, i)).or_default().push(*r);
    }
    let mut grids = Vec::new();
    for (_, g) in groups {
        let v: Vec<RawRect> = g.iter().copied().filter(is_vrule).collect();
        let h: Vec<RawRect> = g.iter().copied().filter(is_hrule).collect();
        let xs = cluster(v.iter().map(|r| (r.x0 + r.x1) / 2.0).collect(), 2.5);
        let ys = cluster(h.iter().map(|r| (r.y0 + r.y1) / 2.0).collect(), 2.5);
        if xs.len() < 2 || ys.len() < 2 {
            continue;
        }
        grids.push(Grid {
            x0: g.iter().map(|r| r.x0).fold(f32::MAX, f32::min),
            y0: g.iter().map(|r| r.y0).fold(f32::MAX, f32::min),
            x1: g.iter().map(|r| r.x1).fold(f32::MIN, f32::max),
            y1: g.iter().map(|r| r.y1).fold(f32::MIN, f32::max),
            ys,
            xs,
            vrules: v,
        });
    }
    grids.sort_by(|a, b| a.y0.total_cmp(&b.y0));
    grids
}

/// Eine Textzeile.
#[derive(Debug, Clone)]
struct Line {
    x0: f32,
    x1: f32,
    y: f32,
    size: f32,
    words: Vec<Word>,
}

#[derive(Debug, Clone)]
struct Word {
    text: String,
    x0: f32,
    x1: f32,
    bold: bool,
    italic: bool,
    /// großer Abstand davor (Tabulator)
    tab_before: bool,
}

fn build_lines(chars: &[&RawChar]) -> Vec<Line> {
    let mut sorted: Vec<&RawChar> = chars.to_vec();
    sorted.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x0.total_cmp(&b.x0)));
    let mut rows: Vec<Vec<&RawChar>> = Vec::new();
    for c in sorted {
        match rows.last_mut() {
            Some(r) if (r[0].y - c.y).abs() <= 0.35 * r[0].size.max(c.size) => r.push(c),
            _ => rows.push(vec![c]),
        }
    }
    let mut lines = Vec::new();
    for mut r in rows {
        r.sort_by(|a, b| a.x0.total_cmp(&b.x0));
        let size = r.iter().map(|c| c.size).fold(0.0, f32::max).max(1.0);
        let mut words: Vec<Word> = Vec::new();
        let mut prev_x1: Option<f32> = None;
        for c in &r {
            let gap = prev_x1.map(|p| c.x0 - p).unwrap_or(0.0);
            let new_word = prev_x1.is_none() || (c.space_before && gap > 0.0) || gap > 0.25 * size;
            if new_word {
                words.push(Word {
                    text: c.ch.to_string(),
                    x0: c.x0,
                    x1: c.x1,
                    bold: c.bold,
                    italic: c.italic,
                    tab_before: prev_x1.is_some() && gap > 1.4 * size,
                });
            } else if let Some(w) = words.last_mut() {
                w.text.push(c.ch);
                w.x1 = c.x1;
                w.bold &= c.bold;
                w.italic &= c.italic;
            }
            prev_x1 = Some(c.x1.max(prev_x1.unwrap_or(f32::MIN)));
        }
        let y = r.iter().map(|c| c.y).sum::<f32>() / r.len() as f32;
        lines.push(Line {
            x0: words.first().map(|w| w.x0).unwrap_or(0.0),
            x1: words.last().map(|w| w.x1).unwrap_or(0.0),
            y,
            size,
            words,
        });
    }
    lines
}

fn is_task_number(s: &str) -> bool {
    let b = s.as_bytes();
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    (1..=2).contains(&digits)
        && (b.len() == digits || (b.len() == digits + 1 && b[digits].is_ascii_lowercase()))
}

/// Zeilen → Absätze.
fn lines_to_paras(lines: &[Line], margin: f32, page_center: f32) -> Vec<Para> {
    let mut paras: Vec<(Para, f32, usize)> = Vec::new(); // (Absatz, letzte y, Zeilenzahl)
    let mut last: Option<&Line> = None;
    for l in lines {
        let first = l.words.first().map(|w| w.text.as_str()).unwrap_or("");
        let bullet = first.starts_with('•');
        let at_margin = (l.x0 - margin).abs() <= 3.0;
        let task_start = at_margin
            && is_task_number(first)
            && l.words
                .get(1)
                .map(|w| w.text.chars().next().is_some_and(char::is_uppercase))
                .unwrap_or(false);
        let new_para = match last {
            None => true,
            Some(p) => {
                let gap = l.y - p.y;
                bullet || task_start || gap > 1.55 * l.size.max(p.size) || gap < 0.0
            }
        };
        let mut runs: Vec<Run> = Vec::new();
        for (i, w) in l.words.iter().enumerate() {
            let sep = if i == 0 {
                ""
            } else if w.tab_before || (task_start && i == 1) {
                "\t"
            } else {
                " "
            };
            let mut text = w.text.clone();
            if i == 0 && bullet {
                text = text.trim_start_matches('•').to_string();
                if text.is_empty() {
                    continue;
                }
            }
            match runs.last_mut() {
                Some(r) if r.bold == w.bold && r.italic == w.italic => {
                    r.text.push_str(sep);
                    r.text.push_str(&text);
                }
                Some(r) => {
                    r.text.push_str(sep);
                    runs.push(Run {
                        text,
                        bold: w.bold,
                        italic: w.italic,
                    });
                }
                None => runs.push(Run {
                    text,
                    bold: w.bold,
                    italic: w.italic,
                }),
            }
        }
        let centered = l.x0 > margin + 20.0 && ((l.x0 + l.x1) / 2.0 - page_center).abs() < 12.0;
        if new_para {
            paras.push((
                Para {
                    runs,
                    bullet,
                    centered,
                    at_margin,
                },
                l.y,
                1,
            ));
        } else if let Some((p, y, n)) = paras.last_mut() {
            join_line(&mut p.runs, &runs);
            p.centered &= centered;
            *y = l.y;
            *n += 1;
        }
        last = Some(l);
    }
    paras.into_iter().map(|(p, _, _)| p).collect()
}

/// Hängt eine Folgezeile an. Word trennt nur an echten Bindestrichen,
/// daher bleibt „EU-“ + „weit“ → „EU-weit“, aber „Einkommens-“ + „und“
/// → „Einkommens- und“.
fn join_line(prev: &mut Vec<Run>, next: &[Run]) {
    if let Some(last) = prev.last_mut() {
        let first_word = next
            .first()
            .map(|r| r.text.split_whitespace().next().unwrap_or("").to_string())
            .unwrap_or_default();
        let keep_space = !last.text.ends_with('-')
            || matches!(
                first_word.as_str(),
                "und" | "oder" | "bzw." | "sowie" | "als" | "bis"
            );
        if keep_space {
            last.text.push(' ');
        }
    }
    prev.extend(next.iter().cloned());
}

fn grid_to_table(grid: &Grid, chars: &[&RawChar], shades: &[RawRect], margin: f32) -> Table {
    let mut rows = Vec::new();
    for band in grid.ys.windows(2) {
        let (top, bottom) = (band[0], band[1]);
        if bottom - top < 3.0 {
            continue;
        }
        let mid = (top + bottom) / 2.0;
        // vertikale Linien, die dieses Band durchqueren
        let mut cuts: Vec<f32> = grid
            .vrules
            .iter()
            .filter(|v| v.y0 <= mid && v.y1 >= mid)
            .map(|v| (v.x0 + v.x1) / 2.0)
            .collect();
        cuts = cluster(cuts, 2.5);
        if cuts.len() < 2 {
            continue;
        }
        let mut row: Vec<Cell> = Vec::new();
        for seg in cuts.windows(2) {
            let (l, r) = (seg[0], seg[1]);
            let inside: Vec<&RawChar> = chars
                .iter()
                .copied()
                .filter(|c| {
                    let cx = (c.x0 + c.x1) / 2.0;
                    cx > l && cx < r && c.y > top && c.y - 0.75 * c.size < bottom
                })
                .collect();
            let lines = build_lines(&inside);
            let cell_margin = lines.iter().map(|l| l.x0).fold(f32::MAX, f32::min);
            let paras = lines_to_paras(
                &lines,
                if cell_margin.is_finite() {
                    cell_margin
                } else {
                    margin
                },
                (l + r) / 2.0,
            );
            let shaded = shades.iter().any(|s| {
                s.x0 <= (l + r) / 2.0 && s.x1 >= (l + r) / 2.0 && s.y0 <= mid && s.y1 >= mid
            });
            let span = grid
                .xs
                .iter()
                .filter(|x| **x > l + 2.5 && **x < r - 2.5)
                .count()
                + 1;
            row.push(Cell {
                paras: paras
                    .into_iter()
                    .map(|mut p| {
                        p.at_margin = false;
                        p
                    })
                    .collect(),
                shaded,
            });
            for _ in 1..span {
                row.push(Cell::default());
            }
        }
        rows.push(row);
    }
    Table { rows }
}

fn same_row(a: &[Cell], b: &[Cell]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.text() == y.text())
}

/// Seiten → Blöcke in Lesereihenfolge.
pub fn layout(pages: &[RawPage]) -> Vec<Block> {
    // linker Rand = häufigster kleinster x-Wert außerhalb von Tabellen
    let mut blocks: Vec<Block> = Vec::new();
    let all_grids: Vec<Vec<Grid>> = pages.iter().map(|p| find_grids(&p.rects)).collect();
    let in_grid = |grids: &[Grid], c: &RawChar| {
        grids.iter().any(|g| {
            let cx = (c.x0 + c.x1) / 2.0;
            cx >= g.x0 && cx <= g.x1 && c.y >= g.y0 && c.y - 0.75 * c.size <= g.y1
        })
    };
    let margin = pages
        .iter()
        .zip(&all_grids)
        .flat_map(|(p, g)| {
            let body: Vec<&RawChar> = p.chars.iter().filter(|c| !in_grid(g, c)).collect();
            build_lines(&body)
                .into_iter()
                .map(|l| l.x0)
                .collect::<Vec<_>>()
        })
        .fold(f32::MAX, f32::min);
    let margin = if margin.is_finite() { margin } else { 0.0 };

    for (page, grids) in pages.iter().zip(&all_grids) {
        let shades: Vec<RawRect> = page.rects.iter().copied().filter(is_shade).collect();
        let body: Vec<&RawChar> = page
            .chars
            .iter()
            .filter(|c| !in_grid(grids, c))
            // Seitenzahlen o. Ä. am unteren Rand ignorieren
            .filter(|c| c.y < page.height - 28.0)
            .collect();
        let lines = build_lines(&body);
        // Abschnitte zwischen Tabellen in Lesereihenfolge
        let mut items: Vec<(f32, Block)> = Vec::new();
        let mut cursor = f32::MIN;
        let mut grid_iter = grids.iter().peekable();
        let mut pending: Vec<Line> = Vec::new();
        let flush = |pending: &mut Vec<Line>, items: &mut Vec<(f32, Block)>| {
            if pending.is_empty() {
                return;
            }
            let y = pending[0].y;
            for p in lines_to_paras(pending, margin, page.width / 2.0) {
                items.push((y, Block::Para(p)));
            }
            pending.clear();
        };
        for l in lines {
            while let Some(g) = grid_iter.peek() {
                if g.y0 < l.y - 0.5 * l.size && g.y0 >= cursor {
                    flush(&mut pending, &mut items);
                    let chars: Vec<&RawChar> = page.chars.iter().collect();
                    items.push((
                        g.y0,
                        Block::Table(grid_to_table(g, &chars, &shades, margin)),
                    ));
                    cursor = g.y1;
                    grid_iter.next();
                } else {
                    break;
                }
            }
            pending.push(l);
        }
        flush(&mut pending, &mut items);
        for g in grid_iter {
            let chars: Vec<&RawChar> = page.chars.iter().collect();
            items.push((
                g.y0,
                Block::Table(grid_to_table(g, &chars, &shades, margin)),
            ));
        }
        let mut first = true;
        for (_, b) in items {
            // Absatz über Seitenumbruch fortsetzen
            if first {
                if let (Block::Para(next), Some(Block::Para(prev))) = (&b, blocks.last_mut()) {
                    let pt = prev.text();
                    let t = pt.trim_end();
                    let open = !t.ends_with(['.', ':', '!', '?', ']', '“']) && !t.is_empty();
                    let nt = next.text();
                    let cont = nt.chars().next().is_some_and(|c| c.is_lowercase()) || open;
                    if open && cont && !next.bullet && !next.at_margin && !next.centered {
                        join_line(&mut prev.runs, &next.runs);
                        first = false;
                        continue;
                    }
                }
            }
            // Tabelle über Seitenumbruch zusammenführen
            if first {
                if let (Block::Table(next), Some(Block::Table(prev))) = (&b, blocks.last_mut()) {
                    if next.cols() == prev.cols() && !next.rows.is_empty() {
                        let skip = usize::from(
                            !prev.rows.is_empty() && same_row(&prev.rows[0], &next.rows[0]),
                        );
                        prev.rows.extend(next.rows.iter().skip(skip).cloned());
                        first = false;
                        continue;
                    }
                }
            }
            first = false;
            blocks.push(b);
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(s: &str, x: f32, y: f32, bold: bool) -> Vec<RawChar> {
        s.chars()
            .enumerate()
            .filter(|(_, c)| *c != ' ')
            .map(|(i, ch)| RawChar {
                ch,
                x0: x + i as f32 * 5.5,
                x1: x + i as f32 * 5.5 + 5.0,
                y,
                size: 11.0,
                bold,
                italic: false,
                space_before: i > 0 && s.chars().nth(i - 1) == Some(' '),
            })
            .collect()
    }

    #[test]
    fn absaetze_und_aufgabennummer() {
        let mut chars = Vec::new();
        chars.extend(word("1a", 56.7, 100.0, false));
        chars.extend(word("Nennen Sie die Ziele.", 76.5, 100.0, false));
        chars.extend(word("[6 BE]", 520.0, 100.0, false));
        chars.extend(word("Ziele (6 BE):", 76.5, 118.0, true));
        chars.extend(word("weiter im Absatz", 76.5, 132.0, false));
        let page = RawPage {
            width: 595.0,
            height: 842.0,
            chars,
            rects: vec![],
        };
        let blocks = layout(&[page]);
        let texts: Vec<String> = blocks
            .iter()
            .map(|b| match b {
                Block::Para(p) => p.text(),
                Block::Table(_) => "TABELLE".into(),
            })
            .collect();
        assert_eq!(texts.len(), 2, "{texts:?}");
        assert!(
            texts[0].starts_with("1a\tNennen Sie die Ziele."),
            "{:?}",
            texts[0]
        );
        assert!(texts[0].ends_with("\t[6 BE]"), "{:?}", texts[0]);
        assert_eq!(texts[1], "Ziele (6 BE): weiter im Absatz");
    }

    #[test]
    fn tabelle_aus_linien() {
        let line = |x0, y0, x1, y1| RawRect {
            x0,
            y0,
            x1,
            y1,
            gray: 0.65,
        };
        let rects = vec![
            line(50.0, 200.0, 300.0, 200.5),
            line(50.0, 220.0, 300.0, 220.5),
            line(50.0, 240.0, 300.0, 240.5),
            line(50.0, 200.0, 50.5, 240.5),
            line(150.0, 200.0, 150.5, 240.5),
            line(300.0, 200.0, 300.5, 240.5),
            RawRect {
                x0: 50.5,
                y0: 200.5,
                x1: 299.5,
                y1: 220.0,
                gray: 0.9,
            },
        ];
        let mut chars = word("Kopf", 55.0, 214.0, true);
        chars.extend(word("Art (1 BE)", 155.0, 214.0, true));
        chars.extend(word("A", 55.0, 234.0, false));
        chars.extend(word("erhaltend", 155.0, 234.0, false));
        let page = RawPage {
            width: 595.0,
            height: 842.0,
            chars,
            rects,
        };
        let blocks = layout(&[page]);
        assert_eq!(blocks.len(), 1);
        let Block::Table(t) = &blocks[0] else {
            panic!()
        };
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.cell_text(0, 1), "Art (1 BE)");
        assert_eq!(t.cell_text(1, 1), "erhaltend");
        assert!(t.rows[0][0].shaded);
        assert!(!t.rows[1][0].shaded);
    }
}
