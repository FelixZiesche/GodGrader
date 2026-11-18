//! Modellbibliothek: Liste von `GET /api/v1/models`, auf das reduziert,
//! was die Oberfläche braucht, und lokal zwischengespeichert.

use serde::{Deserialize, Serialize};

use crate::paths::AppPaths;
use crate::{storage, Result};

/// Ein Modell, wie die App es anzeigt.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    /// Anbieter aus dem Slug, z. B. `google` bei `google/gemini-…`.
    pub provider: String,
    /// Unix-Zeitstempel der Veröffentlichung, für „neuestes Modell".
    pub created: i64,
    pub context_length: Option<u64>,
    /// US-Dollar pro 1 Mio. Eingabe-Tokens.
    pub price_in_per_m: Option<f64>,
    /// US-Dollar pro 1 Mio. Ausgabe-Tokens.
    pub price_out_per_m: Option<f64>,
    pub reads_images: bool,
    pub supports_reasoning: bool,
    pub supports_structured: bool,
    pub supports_temperature: bool,
    pub max_completion_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelCache {
    /// Unix-Zeitstempel des Abrufs.
    pub fetched_at: i64,
    pub models: Vec<ModelInfo>,
}

impl ModelCache {
    pub fn load(paths: &AppPaths) -> Result<Option<Self>> {
        storage::read_json(&paths.models_cache())
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        storage::write_json(&paths.models_cache(), self)
    }

    pub fn get(&self, id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == id)
    }
}

// ---- Rohformat von OpenRouter ----

#[derive(Debug, Deserialize)]
pub(crate) struct RawModelList {
    pub data: Vec<RawModel>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawModel {
    id: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    created: Option<i64>,
    #[serde(default)]
    context_length: Option<u64>,
    #[serde(default)]
    architecture: Option<RawArchitecture>,
    #[serde(default)]
    pricing: Option<RawPricing>,
    #[serde(default)]
    top_provider: Option<RawTopProvider>,
    #[serde(default)]
    supported_parameters: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
struct RawArchitecture {
    #[serde(default)]
    input_modalities: Vec<String>,
    #[serde(default)]
    modality: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawPricing {
    #[serde(default)]
    prompt: Option<serde_json::Value>,
    #[serde(default)]
    completion: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
struct RawTopProvider {
    #[serde(default)]
    max_completion_tokens: Option<u64>,
}

/// OpenRouter liefert Preise pro Token als String („0.000003").
fn per_million(v: &Option<serde_json::Value>) -> Option<f64> {
    let x = match v.as_ref()? {
        serde_json::Value::String(s) => s.parse::<f64>().ok()?,
        serde_json::Value::Number(n) => n.as_f64()?,
        _ => return None,
    };
    if x < 0.0 {
        // -1 steht bei OpenRouter für „variabel" (z. B. Router-Modelle)
        return None;
    }
    Some((x * 1_000_000.0 * 1000.0).round() / 1000.0)
}

pub fn provider_of(id: &str) -> String {
    id.trim_start_matches('~')
        .split('/')
        .next()
        .unwrap_or_default()
        .to_string()
}

impl From<RawModel> for ModelInfo {
    fn from(r: RawModel) -> Self {
        let arch = r.architecture.unwrap_or_default();
        let reads_images = arch.input_modalities.iter().any(|m| m == "image")
            || arch
                .modality
                .as_deref()
                .map(|m| m.split("->").next().unwrap_or("").contains("image"))
                .unwrap_or(false);
        let params = r.supported_parameters.unwrap_or_default();
        let has = |p: &str| params.iter().any(|x| x == p);
        let pricing = r.pricing.unwrap_or_default();
        ModelInfo {
            provider: provider_of(&r.id),
            name: r.name.unwrap_or_else(|| r.id.clone()),
            created: r.created.unwrap_or(0),
            context_length: r.context_length,
            price_in_per_m: per_million(&pricing.prompt),
            price_out_per_m: per_million(&pricing.completion),
            reads_images,
            supports_reasoning: has("reasoning") || has("include_reasoning"),
            supports_structured: has("structured_outputs") || has("response_format"),
            supports_temperature: params.is_empty() || has("temperature"),
            max_completion_tokens: r.top_provider.and_then(|t| t.max_completion_tokens),
            id: r.id,
        }
    }
}

pub(crate) fn parse_model_list(json: &str) -> Result<Vec<ModelInfo>> {
    let raw: RawModelList = serde_json::from_str(json)?;
    Ok(raw.data.into_iter().map(ModelInfo::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEISPIEL: &str = r#"{"data":[
      {"id":"google/gemini-3.1-pro-preview","name":"Google: Gemini 3.1 Pro Preview","created":1771000000,
       "context_length":1048576,
       "architecture":{"modality":"text+image->text","input_modalities":["text","image","file"],"output_modalities":["text"]},
       "pricing":{"prompt":"0.000002","completion":"0.000012","image":"0"},
       "top_provider":{"max_completion_tokens":65536},
       "supported_parameters":["reasoning","response_format","structured_outputs","temperature","max_tokens"]},
      {"id":"openrouter/auto","name":"Auto Router","pricing":{"prompt":"-1","completion":"-1"},
       "architecture":{"input_modalities":["text"]}}
    ]}"#;

    #[test]
    fn liest_modellliste() {
        let m = parse_model_list(BEISPIEL).unwrap();
        assert_eq!(m.len(), 2);
        let g = &m[0];
        assert_eq!(g.provider, "google");
        assert!(g.reads_images);
        assert!(g.supports_reasoning);
        assert!(g.supports_structured);
        assert_eq!(g.price_in_per_m, Some(2.0));
        assert_eq!(g.price_out_per_m, Some(12.0));
        assert_eq!(g.max_completion_tokens, Some(65536));
        let a = &m[1];
        assert!(!a.reads_images);
        assert_eq!(a.price_in_per_m, None);
        assert!(a.supports_temperature);
    }

    #[test]
    fn anbieter_aus_slug() {
        assert_eq!(provider_of("~google/gemini-pro-latest"), "google");
        assert_eq!(provider_of("anthropic/claude-opus-5.5"), "anthropic");
    }
}
