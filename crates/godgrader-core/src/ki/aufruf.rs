//! Ein Chat-Aufruf mit Auswertung von Inhalt, Tokens und Kosten.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::ModelInfo;
use crate::openrouter::OpenRouter;
use crate::{Error, Result};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Nutzung {
    pub tokens_ein: u64,
    pub tokens_aus: u64,
    /// US-Dollar laut OpenRouter, sonst aus den Modellpreisen berechnet.
    pub kosten: f64,
    /// Kosten stammen aus den Modellpreisen (Schätzung).
    pub geschaetzt: bool,
}

#[derive(Debug, Clone)]
pub struct Antwort {
    pub inhalt: String,
    pub nutzung: Nutzung,
    /// Tatsächlich verwendetes Modell.
    pub modell: String,
}

/// Kosten aus Tokens und Modellpreisen.
pub fn kosten_aus_preisen(info: Option<&ModelInfo>, ein: u64, aus: u64) -> f64 {
    let Some(m) = info else { return 0.0 };
    let pe = m.price_in_per_m.unwrap_or(0.0);
    let pa = m.price_out_per_m.unwrap_or(0.0);
    (ein as f64 * pe + aus as f64 * pa) / 1_000_000.0
}

fn inhalt_text(msg: &Value) -> String {
    match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        // Manche Anbieter liefern Teile als Liste
        Some(Value::Array(teile)) => teile
            .iter()
            .filter_map(|t| t.get("text").and_then(|x| x.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// Wertet eine Antwort von `chat/completions` aus.
pub fn auswerten(v: &Value, info: Option<&ModelInfo>) -> Result<Antwort> {
    if let Some(e) = v.get("error") {
        let m = e
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unbekannter Fehler");
        return Err(Error::Ungueltig(format!("Fehler vom Modell: {m}")));
    }
    let usage = v.get("usage");
    let ein = usage
        .and_then(|u| u.get("prompt_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let aus = usage
        .and_then(|u| u.get("completion_tokens"))
        .and_then(|x| x.as_u64())
        .unwrap_or(0);
    let (kosten, geschaetzt) = match usage.and_then(|u| u.get("cost")).and_then(|c| c.as_f64()) {
        Some(c) => (c, false),
        None => (kosten_aus_preisen(info, ein, aus), true),
    };
    let nutzung = Nutzung {
        tokens_ein: ein,
        tokens_aus: aus,
        kosten,
        geschaetzt,
    };
    let wahl = v.pointer("/choices/0");
    let inhalt = wahl
        .and_then(|c| c.get("message"))
        .map(inhalt_text)
        .unwrap_or_default();
    let ende = wahl
        .and_then(|c| c.get("finish_reason"))
        .and_then(|f| f.as_str())
        .unwrap_or("");
    let modell = v
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or_default()
        .to_string();
    let antwort = Antwort {
        inhalt,
        nutzung,
        modell,
    };
    if ende == "length" {
        return Err(Error::Abgeschnitten(antwort.nutzung.kosten));
    }
    if antwort.inhalt.trim().is_empty() {
        return Err(Error::Ungueltig(
            "Das Modell hat eine leere Antwort geliefert.".into(),
        ));
    }
    Ok(antwort)
}

/// Sendet die Anfrage und wertet sie aus.
pub async fn senden(
    client: &OpenRouter,
    body: &Value,
    info: Option<&ModelInfo>,
) -> Result<Antwort> {
    let v = client.chat_raw(body).await?;
    auswerten(&v, info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn kosten_aus_usage_oder_preisen() {
        let v = json!({
            "model": "x/y",
            "choices": [{ "message": { "content": "{}" }, "finish_reason": "stop" }],
            "usage": { "prompt_tokens": 1000, "completion_tokens": 500, "cost": 0.0123 }
        });
        let a = auswerten(&v, None).unwrap();
        assert_eq!(a.nutzung.kosten, 0.0123);
        assert!(!a.nutzung.geschaetzt);

        let v = json!({
            "choices": [{ "message": { "content": [{ "type": "text", "text": "{\"a\":1}" }] } }],
            "usage": { "prompt_tokens": 1_000_000, "completion_tokens": 100_000 }
        });
        let info = ModelInfo {
            id: "x/y".into(),
            name: "Y".into(),
            provider: "x".into(),
            created: 0,
            context_length: None,
            price_in_per_m: Some(2.0),
            price_out_per_m: Some(10.0),
            reads_images: true,
            supports_reasoning: true,
            supports_structured: true,
            supports_temperature: true,
            max_completion_tokens: None,
        };
        let a = auswerten(&v, Some(&info)).unwrap();
        assert!((a.nutzung.kosten - 3.0).abs() < 1e-9);
        assert!(a.nutzung.geschaetzt);
        assert_eq!(a.inhalt, "{\"a\":1}");
    }

    #[test]
    fn abgeschnitten() {
        let v = json!({
            "choices": [{ "message": { "content": "{\"a\":" }, "finish_reason": "length" }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 10, "cost": 0.5 }
        });
        assert!(matches!(auswerten(&v, None), Err(Error::Abgeschnitten(c)) if c == 0.5));
    }
}
