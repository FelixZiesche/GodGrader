//! Prompt-Vorlagen als bearbeitbare Markdown-Dateien im App-Ordner
//! (`prompts/*.md`). Fehlt eine Datei, gilt die eingebaute Startfassung.

use serde::{Deserialize, Serialize};

use crate::paths::AppPaths;
use crate::Result;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PromptArt {
    Abschreiben,
    Bewerten,
    Feedback,
    EwhHilfe,
}

impl PromptArt {
    pub const ALLE: [PromptArt; 4] = [
        PromptArt::Abschreiben,
        PromptArt::Bewerten,
        PromptArt::Feedback,
        PromptArt::EwhHilfe,
    ];

    pub fn datei(self) -> &'static str {
        match self {
            PromptArt::Abschreiben => "abschreiben.md",
            PromptArt::Bewerten => "bewerten.md",
            PromptArt::Feedback => "feedback.md",
            PromptArt::EwhHilfe => "ewh_hilfe.md",
        }
    }

    pub fn titel(self) -> &'static str {
        match self {
            PromptArt::Abschreiben => "Abschreiben",
            PromptArt::Bewerten => "Bewerten und Zweitmeinung",
            PromptArt::Feedback => "Feedback",
            PromptArt::EwhHilfe => "KI-Hilfe beim Import",
        }
    }

    /// Platzhalter, die beim Senden ersetzt werden.
    pub fn platzhalter(self) -> &'static [&'static str] {
        match self {
            PromptArt::Abschreiben => &["{AUFGABENLISTE}"],
            PromptArt::Bewerten => &["{STUFE}", "{BE_MODUS}"],
            PromptArt::Feedback => &["{ANREDE}"],
            PromptArt::EwhHilfe => &["{BE_MODUS}"],
        }
    }

    pub fn standard(self) -> &'static str {
        match self {
            PromptArt::Abschreiben => include_str!("../../prompts/abschreiben.md"),
            PromptArt::Bewerten => include_str!("../../prompts/bewerten.md"),
            PromptArt::Feedback => include_str!("../../prompts/feedback.md"),
            PromptArt::EwhHilfe => include_str!("../../prompts/ewh_hilfe.md"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptVorlage {
    pub art: PromptArt,
    pub titel: String,
    pub text: String,
    /// Entspricht der eingebauten Startfassung.
    pub ist_standard: bool,
    pub platzhalter: Vec<String>,
}

/// Aktueller Text einer Vorlage.
pub fn laden(paths: &AppPaths, art: PromptArt) -> Result<String> {
    let p = paths.prompts_dir().join(art.datei());
    match std::fs::read_to_string(&p) {
        Ok(t) if !t.trim().is_empty() => Ok(t),
        Ok(_) => Ok(art.standard().to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(art.standard().to_string()),
        Err(e) => Err(e.into()),
    }
}

pub fn alle(paths: &AppPaths) -> Result<Vec<PromptVorlage>> {
    PromptArt::ALLE
        .iter()
        .map(|&art| {
            let text = laden(paths, art)?;
            Ok(PromptVorlage {
                art,
                titel: art.titel().into(),
                ist_standard: text.trim() == art.standard().trim(),
                text,
                platzhalter: art.platzhalter().iter().map(|s| s.to_string()).collect(),
            })
        })
        .collect()
}

pub fn speichern(paths: &AppPaths, art: PromptArt, text: &str) -> Result<()> {
    std::fs::create_dir_all(paths.prompts_dir())?;
    crate::storage::write_atomic(&paths.prompts_dir().join(art.datei()), text.as_bytes())
}

/// „Vorlage zurücksetzen": eigene Datei löschen, Startfassung gilt wieder.
pub fn zuruecksetzen(paths: &AppPaths, art: PromptArt) -> Result<()> {
    match std::fs::remove_file(paths.prompts_dir().join(art.datei())) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Ersetzt Platzhalter `{NAME}`.
pub fn fuellen(vorlage: &str, werte: &[(&str, &str)]) -> String {
    let mut s = vorlage.to_string();
    for (k, v) in werte {
        s = s.replace(k, v);
    }
    s
}

/// Alle Vorlagen für einen Lauf, einmal geladen.
#[derive(Debug, Clone)]
pub struct Vorlagen {
    pub abschreiben: String,
    pub bewerten: String,
    pub feedback: String,
    pub ewh_hilfe: String,
}

impl Vorlagen {
    pub fn laden(paths: &AppPaths) -> Result<Self> {
        Ok(Vorlagen {
            abschreiben: laden(paths, PromptArt::Abschreiben)?,
            bewerten: laden(paths, PromptArt::Bewerten)?,
            feedback: laden(paths, PromptArt::Feedback)?,
            ewh_hilfe: laden(paths, PromptArt::EwhHilfe)?,
        })
    }

    pub fn standard() -> Self {
        Vorlagen {
            abschreiben: PromptArt::Abschreiben.standard().into(),
            bewerten: PromptArt::Bewerten.standard().into(),
            feedback: PromptArt::Feedback.standard().into(),
            ewh_hilfe: PromptArt::EwhHilfe.standard().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speichern_und_zuruecksetzen() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path());
        paths.ensure().unwrap();
        assert!(alle(&paths).unwrap().iter().all(|v| v.ist_standard));
        speichern(&paths, PromptArt::Feedback, "Eigener Text {ANREDE}").unwrap();
        let v = alle(&paths).unwrap();
        let f = v.iter().find(|v| v.art == PromptArt::Feedback).unwrap();
        assert!(!f.ist_standard);
        assert_eq!(f.text, "Eigener Text {ANREDE}");
        zuruecksetzen(&paths, PromptArt::Feedback).unwrap();
        assert!(laden(&paths, PromptArt::Feedback)
            .unwrap()
            .contains("120 Wörter"));
        assert_eq!(fuellen("A {X} B", &[("{X}", "1")]), "A 1 B");
    }
}
