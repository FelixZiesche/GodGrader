//! Word (.docx) → Blöcke. Liest `word/document.xml` direkt (zip + quick-xml).

use std::io::Read;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::blocks::{Block, Cell, Para, Run, Table};
use crate::{Error, Result};

/// Minimaler XML-Baum.
#[derive(Debug, Default)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }
    fn find(&self, path: &[&str]) -> Option<&Node> {
        let mut n = self;
        for p in path {
            n = n.child(p)?;
        }
        Some(n)
    }
}

fn start_node(e: &BytesStart) -> Node {
    let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let attrs = e
        .attributes()
        .flatten()
        .map(|a| {
            (
                String::from_utf8_lossy(a.key.as_ref()).into_owned(),
                a.unescape_value()
                    .map(|v| v.into_owned())
                    .unwrap_or_default(),
            )
        })
        .collect();
    Node {
        name,
        attrs,
        ..Default::default()
    }
}

fn parse_xml(xml: &str) -> Result<Node> {
    let mut reader = Reader::from_str(xml);
    let mut stack: Vec<Node> = vec![Node::default()];
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(start_node(&e)),
            Ok(Event::Empty(e)) => {
                let n = start_node(&e);
                stack.last_mut().expect("Wurzel").children.push(n);
            }
            Ok(Event::End(_)) => {
                let n = stack.pop().expect("Element");
                stack
                    .last_mut()
                    .ok_or_else(|| Error::Ungueltig("Word-Datei ist beschädigt.".into()))?
                    .children
                    .push(n);
            }
            Ok(Event::Text(t)) => {
                if let Some(n) = stack.last_mut() {
                    let text = t.unescape().map_err(|e| Error::Ungueltig(e.to_string()))?;
                    n.text.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Ungueltig(format!("Word-Datei nicht lesbar: {e}"))),
            _ => {}
        }
    }
    stack
        .pop()
        .ok_or_else(|| Error::Ungueltig("Word-Datei ist leer.".into()))
}

fn on(n: Option<&Node>) -> bool {
    match n {
        None => false,
        Some(n) => !matches!(n.attr("w:val"), Some("0" | "false" | "none")),
    }
}

fn collect_runs(node: &Node, out: &mut Vec<Run>) {
    for c in &node.children {
        match c.name.as_str() {
            "w:r" => {
                let rpr = c.child("w:rPr");
                let bold = on(rpr.and_then(|r| r.child("w:b")));
                let italic = on(rpr.and_then(|r| r.child("w:i")));
                let mut text = String::new();
                for rc in &c.children {
                    match rc.name.as_str() {
                        "w:t" => text.push_str(&rc.text),
                        "w:tab" | "w:ptab" => text.push('\t'),
                        "w:br" | "w:cr" => text.push(' '),
                        "w:noBreakHyphen" => text.push('-'),
                        "w:softHyphen" => {}
                        "w:sym" => text.push('•'),
                        _ => {}
                    }
                }
                if !text.is_empty() {
                    out.push(Run { text, bold, italic });
                }
            }
            // Gelöschtes (Änderungsverfolgung) ignorieren
            "w:del" | "w:pPr" | "w:rPr" => {}
            // Hyperlinks, Einfügungen, Smart Tags, Felder: Inhalt übernehmen
            _ => collect_runs(c, out),
        }
    }
}

fn para(node: &Node) -> Para {
    let ppr = node.child("w:pPr");
    let bullet = ppr.and_then(|p| p.child("w:numPr")).is_some();
    let centered = matches!(
        ppr.and_then(|p| p.child("w:jc"))
            .and_then(|j| j.attr("w:val")),
        Some("center")
    );
    let indent = ppr
        .and_then(|p| p.child("w:ind"))
        .and_then(|i| i.attr("w:left").or(i.attr("w:start")))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    let hanging = ppr
        .and_then(|p| p.child("w:ind"))
        .and_then(|i| i.attr("w:hanging"))
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(0);
    let mut runs = Vec::new();
    collect_runs(node, &mut runs);
    Para {
        runs,
        bullet,
        centered,
        at_margin: indent - hanging <= 0,
    }
}

fn table(node: &Node) -> Table {
    let mut rows = Vec::new();
    for tr in node.children.iter().filter(|c| c.name == "w:tr") {
        let mut cells = Vec::new();
        for tc in tr.children.iter().filter(|c| c.name == "w:tc") {
            let tcpr = tc.child("w:tcPr");
            let fill = tcpr
                .and_then(|p| p.child("w:shd"))
                .and_then(|s| s.attr("w:fill"))
                .unwrap_or("auto");
            let shaded = !matches!(fill.to_ascii_lowercase().as_str(), "auto" | "ffffff" | "");
            // Vertikal verbundene Fortsetzungszellen bleiben leer
            let mut paras = Vec::new();
            for c in &tc.children {
                match c.name.as_str() {
                    "w:p" => paras.push(para(c)),
                    "w:tbl" => {
                        // verschachtelte Tabelle: als Textabsätze übernehmen
                        for row in table(c).rows {
                            let t: Vec<String> = row.iter().map(|x| x.text()).collect();
                            paras.push(Para::plain(t.join(" | ")));
                        }
                    }
                    _ => {}
                }
            }
            let cell = Cell { paras, shaded };
            let span = tcpr
                .and_then(|p| p.child("w:gridSpan"))
                .and_then(|g| g.attr("w:val"))
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1);
            cells.push(cell);
            // verbundene Spalten als leere Zellen auffüllen, damit Spalten passen
            for _ in 1..span {
                cells.push(Cell::default());
            }
        }
        rows.push(cells);
    }
    Table { rows }
}

fn body_blocks(node: &Node, out: &mut Vec<Block>) {
    for c in &node.children {
        match c.name.as_str() {
            "w:p" => out.push(Block::Para(para(c))),
            "w:tbl" => out.push(Block::Table(table(c))),
            "w:sdt" => {
                if let Some(content) = c.child("w:sdtContent") {
                    body_blocks(content, out);
                }
            }
            _ => {}
        }
    }
}

/// Liest eine .docx-Datei aus Bytes.
pub fn read_docx(bytes: &[u8]) -> Result<Vec<Block>> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| Error::Ungueltig("Keine gültige Word-Datei (.docx).".into()))?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .map_err(|_| Error::Ungueltig("Word-Datei ohne Dokumentinhalt.".into()))?
        .read_to_string(&mut xml)?;
    let root = parse_xml(&xml)?;
    let body = root
        .find(&["w:document", "w:body"])
        .ok_or_else(|| Error::Ungueltig("Word-Datei ohne Textkörper.".into()))?;
    let mut out = Vec::new();
    body_blocks(body, &mut out);
    Ok(out)
}
