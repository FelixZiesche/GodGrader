//! Entwicklerwerkzeug: zeigt die erkannten Blöcke einer Datei.
//! `cargo run -p godgrader-core --example bloecke -- datei.pdf`

use godgrader_core::ewh::blocks::Block;

fn main() {
    let path = std::env::args().nth(1).expect("Datei angeben");
    let bytes = std::fs::read(&path).expect("lesbar");
    let blocks = if path.ends_with(".pdf") {
        godgrader_core::ewh::pdf::read_pdf(&bytes)
    } else {
        godgrader_core::ewh::docx::read_docx(&bytes)
    }
    .expect("Blöcke");
    for b in blocks {
        match b {
            Block::Para(p) => println!(
                "P{}{}{} {}",
                if p.at_margin { "m" } else { " " },
                if p.bullet { "•" } else { " " },
                if p.centered { "c" } else { " " },
                p.markdown().replace('\t', "⇥")
            ),
            Block::Table(t) => {
                println!("TABELLE {}x{}", t.rows.len(), t.cols());
                for r in &t.rows {
                    let cells: Vec<String> = r
                        .iter()
                        .map(|c| {
                            format!(
                                "{}{}",
                                if c.shaded { "▒" } else { "" },
                                c.text().replace('\n', " / ")
                            )
                        })
                        .collect();
                    println!("  | {}", cells.join(" | "));
                }
            }
        }
    }
}
