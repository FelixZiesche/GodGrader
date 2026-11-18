//! OpenRouter-Client. Einziger Netzwerkpartner der App.

use std::time::Duration;

use reqwest::{header, Method, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::models::{parse_model_list, ModelInfo};
use crate::{Error, Result, APP_NAME};

pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const REFERER: &str = "https://github.com/FelixZiesche/godgrader";

#[derive(Clone)]
pub struct OpenRouter {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    max_retries: u8,
    /// Grundwartezeit für den Backoff; in Tests verkürzt.
    backoff_base: Duration,
}

impl std::fmt::Debug for OpenRouter {
    // Schlüssel nie ausgeben
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenRouter")
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "***"))
            .finish()
    }
}

/// Angaben zum Schlüssel aus `GET /key`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyInfo {
    #[serde(default)]
    pub label: Option<String>,
    /// Bisher verbrauchte Credits (US-Dollar).
    #[serde(default)]
    pub usage: Option<f64>,
    #[serde(default)]
    pub limit: Option<f64>,
    #[serde(default)]
    pub limit_remaining: Option<f64>,
    #[serde(default)]
    pub is_free_tier: Option<bool>,
}

/// Datenschutz-Vorgaben für jede Anfrage.
pub fn provider_prefs(zdr_only: bool) -> Value {
    let mut p = json!({ "data_collection": "deny" });
    if zdr_only {
        p["zdr"] = Value::Bool(true);
    }
    p
}

impl OpenRouter {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Result<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert("HTTP-Referer", header::HeaderValue::from_static(REFERER));
        headers.insert("X-Title", header::HeaderValue::from_static(APP_NAME));
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(concat!("GodGrader/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(600))
            .build()?;
        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            max_retries: 4,
            backoff_base: Duration::from_millis(1000),
        })
    }

    pub fn with_retries(mut self, max_retries: u8, backoff_base: Duration) -> Self {
        self.max_retries = max_retries;
        self.backoff_base = backoff_base;
        self
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url, path.trim_start_matches('/'))
    }

    /// Sendet eine Anfrage und wiederholt bei 429 und 5xx mit
    /// exponentiellem Backoff (berücksichtigt `Retry-After`).
    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        auth: bool,
    ) -> Result<String> {
        let mut attempt: u32 = 0;
        loop {
            let mut req = self.http.request(method.clone(), self.url(path));
            if let Some(k) = &self.api_key {
                req = req.bearer_auth(k);
            } else if auth {
                return Err(Error::KeinSchluessel);
            }
            if let Some(b) = body {
                req = req.json(b);
            }
            let outcome = req.send().await;
            let retry_after = match outcome {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        return Ok(resp.text().await?);
                    }
                    let retry_after = resp
                        .headers()
                        .get(header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .map(Duration::from_secs);
                    let text = resp.text().await.unwrap_or_default();
                    let retryable =
                        status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
                    if !retryable || attempt >= self.max_retries as u32 {
                        return Err(Error::Api {
                            status: status.as_u16(),
                            message: api_error_message(status, &text),
                        });
                    }
                    retry_after
                }
                Err(e) => {
                    let retryable = e.is_timeout() || e.is_connect() || e.is_request();
                    if !retryable || attempt >= self.max_retries as u32 {
                        return Err(e.into());
                    }
                    None
                }
            };
            let backoff = self.backoff_base * 2u32.pow(attempt.min(6));
            let wait = retry_after.unwrap_or(backoff).min(Duration::from_secs(120));
            tracing::warn!(
                versuch = attempt + 1,
                warte_ms = wait.as_millis() as u64,
                "OpenRouter: neuer Versuch"
            );
            tokio::time::sleep(wait).await;
            attempt += 1;
        }
    }

    /// Lädt die Modellliste (öffentlich, Schlüssel optional).
    pub async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let text = self.send(Method::GET, "models", None, false).await?;
        parse_model_list(&text)
    }

    /// „Verbindung prüfen": fragt die Schlüsseldaten ab, kostet nichts.
    pub async fn check_key(&self) -> Result<KeyInfo> {
        #[derive(Deserialize)]
        struct Wrap {
            data: KeyInfo,
        }
        let text = self.send(Method::GET, "key", None, true).await?;
        Ok(serde_json::from_str::<Wrap>(&text)?.data)
    }

    /// Roher Chat-Aufruf. `body` muss bereits `provider` enthalten
    /// (siehe [`provider_prefs`]); das prüft diese Funktion.
    pub async fn chat_raw(&self, body: &Value) -> Result<Value> {
        if body.get("provider").and_then(|p| p.get("data_collection")) != Some(&json!("deny")) {
            return Err(Error::Ungueltig(
                "Interner Fehler: Anfrage ohne Datenschutz-Vorgabe blockiert.".into(),
            ));
        }
        let text = self
            .send(Method::POST, "chat/completions", Some(body), true)
            .await?;
        Ok(serde_json::from_str(&text)?)
    }
}

/// Fehlermeldung aus der OpenRouter-Antwort, ohne Anfrageinhalte.
fn api_error_message(status: StatusCode, body: &str) -> String {
    let from_body = serde_json::from_str::<Value>(body).ok().and_then(|v| {
        v.pointer("/error/message")
            .and_then(|m| m.as_str())
            .map(str::to_string)
    });
    let hint = match status.as_u16() {
        401 => "API-Schlüssel ungültig oder widerrufen.",
        402 => "Guthaben bei OpenRouter aufgebraucht.",
        403 => "Zugriff verweigert.",
        404 => "Modell oder Endpunkt nicht gefunden.",
        429 => "Zu viele Anfragen, bitte später erneut versuchen.",
        _ => "",
    };
    match (from_body, hint.is_empty()) {
        (Some(m), true) => m,
        (Some(m), false) => format!("{hint} ({m})"),
        (None, false) => hint.to_string(),
        (None, true) => status
            .canonical_reason()
            .unwrap_or("Unbekannter Fehler")
            .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datenschutz_vorgaben() {
        assert_eq!(provider_prefs(false), json!({"data_collection": "deny"}));
        assert_eq!(
            provider_prefs(true),
            json!({"data_collection": "deny", "zdr": true})
        );
    }

    #[test]
    fn debug_zeigt_keinen_schluessel() {
        let c = OpenRouter::new(DEFAULT_BASE_URL, Some("sk-or-geheim".into())).unwrap();
        assert!(!format!("{c:?}").contains("geheim"));
    }

    #[test]
    fn fehlermeldung() {
        let m = api_error_message(
            StatusCode::UNAUTHORIZED,
            r#"{"error":{"message":"No auth"}}"#,
        );
        assert!(m.contains("ungültig"));
        assert!(m.contains("No auth"));
    }
}
