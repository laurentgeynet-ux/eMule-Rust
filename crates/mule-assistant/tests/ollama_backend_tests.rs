// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Tests du backend Ollama : validation d'endpoint et parsing du stream
//! NDJSON contre un faux serveur loopback. Aucun trafic sortant.

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use mule_assistant::config::AssistantConfig;
use mule_assistant::llm::ollama::OllamaBackend;
use mule_assistant::llm::{ChatMessage, LlmBackend, ToolSchema};
use serde_json::{json, Value};
use tokio::net::TcpListener;

/// Validation de l'endpoint : http + IP littérale loopback exigés.
#[test]
fn test_endpoint_validation() {
    // OK : loopback littéral
    let ok = AssistantConfig {
        endpoint: "http://127.0.0.1:11434".to_string(),
        ..Default::default()
    };
    assert!(ok.validate().is_ok());

    // OK : IPv6 loopback
    let ok6 = AssistantConfig {
        endpoint: "http://[::1]:11434".to_string(),
        ..Default::default()
    };
    assert!(ok6.validate().is_ok());

    // Refus : nom d'hôte à résoudre
    let host = AssistantConfig {
        endpoint: "http://localhost:11434".to_string(),
        ..Default::default()
    };
    assert!(host.validate().is_err());

    // Refus : non loopback sans override
    let remote = AssistantConfig {
        endpoint: "http://8.8.8.8:11434".to_string(),
        ..Default::default()
    };
    assert!(remote.validate().is_err());

    // Accepté avec override explicite
    let remote_allowed = AssistantConfig {
        endpoint: "http://8.8.8.8:11434".to_string(),
        allow_remote_endpoint: true,
        ..Default::default()
    };
    assert!(remote_allowed.validate().is_ok());

    // Refus : scheme non-http
    let https = AssistantConfig {
        endpoint: "https://127.0.0.1:11434".to_string(),
        ..Default::default()
    };
    assert!(https.validate().is_err());
}

/// Corps NDJSON scripté du faux Ollama : deux fragments de contenu, un
/// tool_call, puis le chunk final `done` avec métriques.
fn scripted_ndjson() -> String {
    [
        json!({"model":"m","message":{"role":"assistant","content":"Bon"},"done":false}),
        json!({"model":"m","message":{"role":"assistant","content":"jour","tool_calls":[{"function":{"name":"emule_status","arguments":{}}}]},"done":false}),
        json!({"model":"m","done":true,"done_reason":"stop","load_duration":10_000_000_u64,"prompt_eval_duration":20_000_000_u64,"eval_duration":30_000_000_u64,"eval_count":7}),
    ]
    .iter()
    .map(|v| v.to_string())
    .collect::<Vec<_>>()
    .join("\n")
        + "\n"
}

/// Capture du corps de requête reçu par le faux serveur.
type CapturedBody = Arc<Mutex<Option<Value>>>;

async fn fake_chat(State(captured): State<CapturedBody>, Json(body): Json<Value>) -> impl IntoResponse {
    *captured.lock().unwrap() = Some(body);
    axum::response::Response::builder()
        .header("content-type", "application/x-ndjson")
        .body(scripted_ndjson())
        .unwrap()
}

/// Assemble le tour complet depuis le stream : contenu streamé au sink,
/// tool_calls accumulés, métriques du chunk `done`.
#[tokio::test]
async fn test_ollama_stream_parsing() {
    let captured: CapturedBody = Arc::new(Mutex::new(None));
    let app = Router::new()
        .route("/api/chat", post(fake_chat))
        .with_state(captured.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let config = AssistantConfig {
        endpoint: format!("http://{addr}"),
        model: "test-model".to_string(),
        ..Default::default()
    };
    let backend = OllamaBackend::new(&config).unwrap();

    // Historique incluant un message tool : vérifie la sérialisation.
    let history = vec![
        ChatMessage::system("sys"),
        ChatMessage::user("salut"),
        ChatMessage::tool("emule_status", "{\"ok\":true}"),
    ];
    let tools = vec![ToolSchema {
        name: "emule_status".to_string(),
        description: "status".to_string(),
        parameters: json!({"type":"object","properties":{}}),
    }];

    let tokens = Arc::new(Mutex::new(Vec::<String>::new()));
    let tokens2 = tokens.clone();
    let sink = move |t: &str| {
        tokens2.lock().unwrap().push(t.to_string());
        true
    };

    let turn = backend.chat(&history, &tools, &sink).await.unwrap();

    // Contenu concaténé + streamé en deux fragments.
    assert_eq!(turn.content, "Bonjour");
    assert_eq!(tokens.lock().unwrap().clone(), vec!["Bon", "jour"]);

    // Tool call accumulé et traduit au format neutre.
    assert_eq!(turn.tool_calls.len(), 1);
    assert_eq!(turn.tool_calls[0].name, "emule_status");
    assert!(turn.tool_calls[0].id.is_none());

    // Métriques converties ns → ms.
    let m = turn.metrics.unwrap();
    assert_eq!(m.load_duration_ms, Some(10));
    assert_eq!(m.prompt_eval_duration_ms, Some(20));
    assert_eq!(m.eval_duration_ms, Some(30));
    assert_eq!(m.eval_count, Some(7));

    // Corps de requête : tools au format function, tool_name sérialisé.
    let sent = captured.lock().unwrap().clone().unwrap();
    assert_eq!(sent["model"], "test-model");
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["tools"][0]["function"]["name"], "emule_status");
    assert_eq!(sent["messages"][0]["role"], "system");
    assert_eq!(sent["messages"][2]["role"], "tool");
    assert_eq!(sent["messages"][2]["tool_name"], "emule_status");
    assert_eq!(sent["messages"][2]["name"], "emule_status");
}

/// Réponse d'erreur HTTP → `AssistantError::Backend` avec le statut.
#[tokio::test]
async fn test_ollama_http_error() {
    async fn err_handler() -> impl IntoResponse {
        (axum::http::StatusCode::NOT_FOUND, "model not found")
    }
    let app = Router::new().route("/api/chat", post(err_handler));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let config = AssistantConfig {
        endpoint: format!("http://{addr}"),
        ..Default::default()
    };
    let backend = OllamaBackend::new(&config).unwrap();
    let res = backend.chat(&[], &[], &|_| true).await;
    let err = res.unwrap_err().to_string();
    assert!(err.contains("404"), "statut HTTP attendu dans l'erreur : {err}");
}
