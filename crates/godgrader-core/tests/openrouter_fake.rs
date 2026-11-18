//! Tests gegen einen Fake-OpenRouter (wiremock).

use std::time::Duration;

use godgrader_core::openrouter::{provider_prefs, OpenRouter};
use godgrader_core::Error;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(server: &MockServer, key: Option<&str>) -> OpenRouter {
    OpenRouter::new(server.uri(), key.map(String::from))
        .unwrap()
        .with_retries(3, Duration::from_millis(5))
}

#[tokio::test]
async fn modellliste_mit_headern() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("X-Title", "GodGrader"))
        .and(header(
            "HTTP-Referer",
            "https://github.com/FelixZiesche/godgrader",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"id": "google/gemini-3.1-pro-preview", "name": "Gemini",
                      "architecture": {"input_modalities": ["text", "image"]},
                      "pricing": {"prompt": "0.000002", "completion": "0.000012"}}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let models = client(&server, None).list_models().await.unwrap();
    assert_eq!(models.len(), 1);
    assert!(models[0].reads_images);
}

#[tokio::test]
async fn schluesselpruefung() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/key"))
        .and(header("Authorization", "Bearer sk-test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {"label": "sk-or-v1-abc", "usage": 1.25, "limit": null, "is_free_tier": false}
        })))
        .mount(&server)
        .await;
    let info = client(&server, Some("sk-test")).check_key().await.unwrap();
    assert_eq!(info.usage, Some(1.25));
}

#[tokio::test]
async fn ohne_schluessel_kein_aufruf() {
    let server = MockServer::start().await;
    let err = client(&server, None).check_key().await.unwrap_err();
    assert!(matches!(err, Error::KeinSchluessel));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn wiederholt_bei_429_und_5xx() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/key"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/key"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"data": {}})))
        .mount(&server)
        .await;
    client(&server, Some("k")).check_key().await.unwrap();
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn kein_wiederholen_bei_401() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/key"))
        .respond_with(
            ResponseTemplate::new(401).set_body_json(json!({"error": {"message": "bad key"}})),
        )
        .mount(&server)
        .await;
    let err = client(&server, Some("k")).check_key().await.unwrap_err();
    assert!(matches!(err, Error::Api { status: 401, .. }));
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn chat_ohne_datenschutz_wird_blockiert() {
    let server = MockServer::start().await;
    let c = client(&server, Some("k"));
    let err = c
        .chat_raw(&json!({"model": "x", "messages": []}))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Ungueltig(_)));
    assert!(server.received_requests().await.unwrap().is_empty());

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices": []})))
        .mount(&server)
        .await;
    c.chat_raw(&json!({"model": "x", "messages": [], "provider": provider_prefs(true)}))
        .await
        .unwrap();
    let reqs = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&reqs[0].body).unwrap();
    assert_eq!(body["provider"]["data_collection"], "deny");
    assert_eq!(body["provider"]["zdr"], true);
}
