//! Gemeinsame Zwischenform für Word und PDF: Absätze und Tabellen.

/// Textstück mit Auszeichnung.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Para {
    pub runs: Vec<Run>,
    /// Aufzählungspunkt (Word-Nummerierung oder „•“ im PDF).
    pub bullet: bool,
    pub centered: bool,
    /// Beginnt am linken Seitenrand (für Aufgabennummern im PDF).
    pub at_margin: bool,
}

impl Para {
    pub fn plain(text: impl Into<String>) -> Self {
        Para {
            runs: vec![Run {
                text: text.into(),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    /// Reiner Text, Tabulatoren als `\t`.
    pub fn text(&self) -> String {
        let t: String = self.runs.iter().map(|r| r.text.as_str()).collect();
        if self.bullet {
            format!("• {}", t.trim_start_matches(['•', ' ']))
        } else {
            t
        }
    }

    /// Text mit Markdown-Auszeichnung (**fett**, *kursiv*).
    pub fn markdown(&self) -> String {
        let mut out = String::new();
        if self.bullet {
            out.push_str("• ");
        }
        // benachbarte Runs gleicher Auszeichnung zusammenfassen
        let mut merged: Vec<Run> = Vec::new();
        for r in &self.runs {
            match merged.last_mut() {
                Some(l) if l.bold == r.bold && l.italic == r.italic => l.text.push_str(&r.text),
                _ => merged.push(r.clone()),
            }
        }
        for (i, r) in merged.iter().enumerate() {
            let mut t = r.text.replace('\t', " ");
            if self.bullet && i == 0 {
                t = t.trim_start_matches(['•', ' ']).to_string();
            }
            if t.trim().is_empty() || (!r.bold && !r.italic) {
                out.push_str(&t);
                continue;
            }
            let lead = t.len() - t.trim_start().len();
            let trail = t.len() - t.trim_end().len();
            let core = t.trim();
            let mark = match (r.bold, r.italic) {
                (true, true) => "***",
                (true, false) => "**",
                _ => "*",
            };
            out.push_str(&t[..lead]);
            out.push_str(mark);
            out.push_str(core);
            out.push_str(mark);
            out.push_str(&t[t.len() - trail..]);
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|r| r.text.trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cell {
    pub paras: Vec<Para>,
    /// Hellgrau hinterlegt (Tabellenkopf).
    pub shaded: bool,
}

impl Cell {
    pub fn text(&self) -> String {
        self.paras
            .iter()
            .map(|p| p.text())
            .filter(|t| !t.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn markdown(&self) -> String {
        self.paras
            .iter()
            .filter(|p| !p.is_empty())
            .map(|p| p.markdown())
            .collect::<Vec<_>>()
            .join("<br>")
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Table {
    pub rows: Vec<Vec<Cell>>,
}

impl Table {
    pub fn cols(&self) -> usize {
        self.rows.iter().map(|r| r.len()).max().unwrap_or(0)
    }

    pub fn cell_text(&self, r: usize, c: usize) -> String {
        self.rows
            .get(r)
            .and_then(|row| row.get(c))
            .map(|c| c.text())
            .unwrap_or_default()
    }

    /// Als Markdown-Tabelle (für Prompts und Anzeige).
    pub fn markdown(&self) -> String {
        let n = self.cols();
        if n == 0 {
            return String::new();
        }
        let mut out = String::new();
        for (i, row) in self.rows.iter().enumerate() {
            out.push('|');
            for c in 0..n {
                let t = row.get(c).map(|x| x.markdown()).unwrap_or_default();
                out.push(' ');
                out.push_str(&t.replace('|', "/").replace('\n', "<br>"));
                out.push_str(" |");
            }
            out.push('\n');
            if i == 0 {
                out.push('|');
                out.push_str(&" --- |".repeat(n));
                out.push('\n');
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Para(Para),
    Table(Table),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_auszeichnung() {
        let p = Para {
            runs: vec![
                Run {
                    text: "Formel: ".into(),
                    ..Default::default()
                },
                Run {
                    text: "Arbeitslosenquote ".into(),
                    bold: true,
                    italic: false,
                },
                Run {
                    text: "= …".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert_eq!(p.markdown(), "Formel: **Arbeitslosenquote** = …");
    }

    #[test]
    fn tabelle_als_markdown() {
        let t = Table {
            rows: vec![
                vec![
                    Cell {
                        paras: vec![Para::plain("A")],
                        shaded: true,
                    },
                    Cell {
                        paras: vec![Para::plain("B")],
                        shaded: true,
                    },
                ],
                vec![
                    Cell {
                        paras: vec![Para::plain("1")],
                        shaded: false,
                    },
                    Cell {
                        paras: vec![Para::plain("x|y")],
                        shaded: false,
                    },
                ],
            ],
        };
        assert_eq!(t.markdown(), "| A | B |\n| --- | --- |\n| 1 | x/y |\n");
    }
}
