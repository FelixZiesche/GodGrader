//! Die vier Modell-Slots und die Prüfung der Standard-Slugs.

use serde::{Deserialize, Serialize};

use crate::models::{provider_of, ModelInfo};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum SlotKind {
    Abschreiben,
    Bewerten,
    Feedback,
    Zweitmeinung,
}

impl SlotKind {
    pub const ALLE: [SlotKind; 4] = [
        SlotKind::Abschreiben,
        SlotKind::Bewerten,
        SlotKind::Feedback,
        SlotKind::Zweitmeinung,
    ];

    pub fn default_model(self) -> &'static str {
        match self {
            SlotKind::Abschreiben => "google/gemini-3.1-pro-preview",
            SlotKind::Bewerten => "anthropic/claude-opus-5.5",
            SlotKind::Feedback => "anthropic/claude-sonnet-5.5",
            SlotKind::Zweitmeinung => "google/gemini-3.1-pro-preview",
        }
    }

    pub fn needs_images(self) -> bool {
        matches!(self, SlotKind::Abschreiben)
    }

    pub fn label(self) -> &'static str {
        match self {
            SlotKind::Abschreiben => "Abschreiben",
            SlotKind::Bewerten => "Bewerten",
            SlotKind::Feedback => "Feedback",
            SlotKind::Zweitmeinung => "Zweitmeinung",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    /// Modellstandard, kein Feld in der Anfrage.
    #[default]
    Standard,
    Niedrig,
    Mittel,
    Hoch,
}

impl ReasoningEffort {
    /// Wert für `reasoning.effort` bei OpenRouter.
    pub fn api_value(self) -> Option<&'static str> {
        match self {
            ReasoningEffort::Standard => None,
            ReasoningEffort::Niedrig => Some("low"),
            ReasoningEffort::Mittel => Some("medium"),
            ReasoningEffort::Hoch => Some("high"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlotConfig {
    pub model: String,
    pub enabled: bool,
    pub temperature: f32,
    pub max_tokens: Option<u32>,
    pub reasoning: ReasoningEffort,
}

impl SlotConfig {
    pub fn default_for(kind: SlotKind) -> Self {
        SlotConfig {
            model: kind.default_model().to_string(),
            enabled: kind != SlotKind::Zweitmeinung,
            temperature: if kind == SlotKind::Feedback { 0.4 } else { 0.0 },
            max_tokens: Some(match kind {
                SlotKind::Abschreiben => 16_000,
                SlotKind::Bewerten | SlotKind::Zweitmeinung => 8_000,
                SlotKind::Feedback => 2_000,
            }),
            reasoning: ReasoningEffort::Standard,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlotSet {
    pub abschreiben: SlotConfig,
    pub bewerten: SlotConfig,
    pub feedback: SlotConfig,
    pub zweitmeinung: SlotConfig,
}

impl Default for SlotSet {
    fn default() -> Self {
        SlotSet {
            abschreiben: SlotConfig::default_for(SlotKind::Abschreiben),
            bewerten: SlotConfig::default_for(SlotKind::Bewerten),
            feedback: SlotConfig::default_for(SlotKind::Feedback),
            zweitmeinung: SlotConfig::default_for(SlotKind::Zweitmeinung),
        }
    }
}

impl SlotSet {
    pub fn get(&self, kind: SlotKind) -> &SlotConfig {
        match kind {
            SlotKind::Abschreiben => &self.abschreiben,
            SlotKind::Bewerten => &self.bewerten,
            SlotKind::Feedback => &self.feedback,
            SlotKind::Zweitmeinung => &self.zweitmeinung,
        }
    }

    pub fn get_mut(&mut self, kind: SlotKind) -> &mut SlotConfig {
        match kind {
            SlotKind::Abschreiben => &mut self.abschreiben,
            SlotKind::Bewerten => &mut self.bewerten,
            SlotKind::Feedback => &mut self.feedback,
            SlotKind::Zweitmeinung => &mut self.zweitmeinung,
        }
    }
}

/// Prüft, ob ein Modell in einen Slot gezogen werden darf.
pub fn validate_drop(kind: SlotKind, model: &ModelInfo) -> Result<(), String> {
    if kind.needs_images() && !model.reads_images {
        return Err(format!(
            "„{}“ kann keine Bilder lesen und eignet sich deshalb nicht zum Abschreiben.",
            model.name
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SlotCheck {
    pub kind: SlotKind,
    pub model: String,
    pub available: bool,
    /// Problem in Worten, z. B. „Modell nicht mehr verfügbar".
    pub problem: Option<String>,
    /// Naheliegendes verfügbares Modell, falls das eingestellte fehlt.
    pub suggestion: Option<String>,
}

/// Prüft alle Slots gegen die aktuelle Modellliste.
pub fn check_slots(slots: &SlotSet, models: &[ModelInfo]) -> Vec<SlotCheck> {
    SlotKind::ALLE
        .iter()
        .map(|&kind| {
            let cfg = slots.get(kind);
            let found = models.iter().find(|m| m.id == cfg.model);
            let (available, problem) = match found {
                None => (
                    false,
                    Some("Modell ist bei OpenRouter nicht (mehr) verfügbar.".into()),
                ),
                Some(m) => match validate_drop(kind, m) {
                    Ok(()) => (true, None),
                    Err(e) => (false, Some(e)),
                },
            };
            let suggestion = if available {
                None
            } else {
                suggest_replacement(&cfg.model, models, kind.needs_images())
            };
            SlotCheck {
                kind,
                model: cfg.model.clone(),
                available,
                problem,
                suggestion,
            }
        })
        .collect()
}

const FUELLWOERTER: &[&str] = &["preview", "latest", "exp", "experimental", "beta", "free"];

/// Familien-Wörter eines Slugs, z. B. `google/gemini-3.1-pro-preview` → `gemini`, `pro`.
fn family_tokens(slug: &str) -> Vec<String> {
    let name = slug
        .trim_start_matches('~')
        .split('/')
        .nth(1)
        .unwrap_or(slug);
    name.split(['-', ':', '_'])
        .map(|t| t.to_ascii_lowercase())
        .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_alphabetic()))
        .filter(|t| !FUELLWOERTER.contains(&t.as_str()))
        .collect()
}

/// Schlägt das neueste verfügbare Modell derselben Familie vor
/// (gleicher Anbieter, gleiche Familien-Wörter, z. B. neuestes „Claude Opus").
pub fn suggest_replacement(slug: &str, models: &[ModelInfo], needs_images: bool) -> Option<String> {
    let provider = provider_of(slug);
    let tokens = family_tokens(slug);
    let eligible = |m: &&ModelInfo| {
        m.provider == provider && m.id != slug && (!needs_images || m.reads_images)
    };
    let pick = |require: &[String]| {
        let mut c: Vec<&ModelInfo> = models
            .iter()
            .filter(eligible)
            .filter(|m| {
                let t = family_tokens(&m.id);
                require.iter().all(|r| t.contains(r))
            })
            .collect();
        // feste Slugs vor „~"-Aliasen, dann das neueste
        c.sort_by_key(|m| (!m.id.starts_with('~'), m.created));
        c.last().map(|m| m.id.clone())
    };
    pick(&tokens).or_else(|| pick(&tokens[..tokens.len().min(1)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: &str, created: i64, images: bool) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            name: id.into(),
            provider: provider_of(id),
            created,
            context_length: None,
            price_in_per_m: None,
            price_out_per_m: None,
            reads_images: images,
            supports_reasoning: false,
            supports_structured: false,
            supports_temperature: true,
            max_completion_tokens: None,
        }
    }

    fn liste() -> Vec<ModelInfo> {
        vec![
            m("google/gemini-3.5-pro", 300, true),
            m("google/gemini-3.5-flash", 310, true),
            m("google/gemini-2.5-pro", 100, true),
            m("~google/gemini-pro-latest", 320, true),
            m("anthropic/claude-opus-5.5", 200, true),
            m("anthropic/claude-opus-6", 400, true),
            m("anthropic/claude-sonnet-5.5", 210, true),
            m("deepseek/deepseek-v4", 220, false),
        ]
    }

    #[test]
    fn standard_slots() {
        let s = SlotSet::default();
        assert_eq!(s.abschreiben.temperature, 0.0);
        assert_eq!(s.bewerten.temperature, 0.0);
        assert_eq!(s.feedback.temperature, 0.4);
        assert!(!s.zweitmeinung.enabled);
        assert_eq!(s.bewerten.model, "anthropic/claude-opus-5.5");
    }

    #[test]
    fn abschreiben_braucht_bilder() {
        let l = liste();
        let ds = l.iter().find(|x| x.id == "deepseek/deepseek-v4").unwrap();
        assert!(validate_drop(SlotKind::Abschreiben, ds).is_err());
        assert!(validate_drop(SlotKind::Bewerten, ds).is_ok());
    }

    #[test]
    fn fehlender_gemini_slug_bekommt_neuestes_pro() {
        let s = suggest_replacement("google/gemini-3.1-pro-preview", &liste(), true);
        assert_eq!(s.as_deref(), Some("google/gemini-3.5-pro"));
    }

    #[test]
    fn fehlender_opus_slug_bekommt_neuestes_opus() {
        let s = suggest_replacement("anthropic/claude-opus-4", &liste(), false);
        assert_eq!(s.as_deref(), Some("anthropic/claude-opus-6"));
    }

    #[test]
    fn slotpruefung_markiert_fehlende() {
        let checks = check_slots(&SlotSet::default(), &liste());
        let a = &checks[0];
        assert_eq!(a.kind, SlotKind::Abschreiben);
        assert!(!a.available);
        assert_eq!(a.suggestion.as_deref(), Some("google/gemini-3.5-pro"));
        let b = &checks[1];
        assert!(b.available);
        assert!(b.suggestion.is_none());
    }
}
