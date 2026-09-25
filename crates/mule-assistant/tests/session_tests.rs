// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Tests e2e de la boucle agentique : `MockBackend` scripté + daemon de
//! test loopback (`mule-test-support`). Aucun trafic sortant.

use std::net::SocketAddr;
use std::sync::Arc;

use mule_assistant::config::AssistantConfig;
use mule_assistant::llm::mock::{text_turn, tool_call_turn, MockBackend};
use mule_assistant::llm::{AssistantTurn, Role};
use mule_assistant::session::{AssistantEvent, ConfirmRequest, Session};
use mule_mcp::tools::ToolProfile;
use mule_test_support::spawn_test_daemon;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::mpsc;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

struct Harness {
    session: Session,
    backend: Arc<MockBackend>,
    addr: SocketAddr,
    events_tx: mpsc::UnboundedSender<AssistantEvent>,
    events_rx: mpsc::UnboundedReceiver<AssistantEvent>,
    confirm_tx: mpsc::Sender<ConfirmRequest>,
    confirm_rx: mpsc::Receiver<ConfirmRequest>,
    _dir: TempDir,
}

async fn build_session(turns: Vec<AssistantTurn>) -> Harness {
    let (addr, token, dir) = spawn_test_daemon(TOKEN).await;
    let backend = Arc::new(MockBackend::new(turns));
    let session = connect(&addr, &token, backend.clone(), AssistantConfig::default()).await;
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    let (confirm_tx, confirm_rx) = mpsc::channel(8);
    Harness {
        session,
        backend,
        addr,
        events_tx,
        events_rx,
        confirm_tx,
        confirm_rx,
        _dir: dir,
    }
}

async fn connect(
    addr: &SocketAddr,
    token: &str,
    backend: Arc<MockBackend>,
    config: AssistantConfig,
) -> Session {
    Session::connect(
        Box::new(backend),
        config,
        Some(format!("ws://{addr}/rpc")),
        Some(token.to_string()),
        None,
        None,
    )
    .await
    .expect("connexion session de test")
}

/// Draine tous les events émis pendant le tour.
fn drain_events(rx: &mut mpsc::UnboundedReceiver<AssistantEvent>) -> Vec<AssistantEvent> {
    let mut out = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        out.push(ev);
    }
    out
}

/// Tour texte simple : le modèle répond sans outil → `Final`.
#[tokio::test]
async fn test_simple_text_turn() {
    let mut h = build_session(vec![text_turn("Réponse du modèle")]).await;
    assert!(h.session.visible_tool_count() > 0);

    h.session
        .user_turn("bonjour", &h.events_tx, &h.confirm_tx)
        .await
        .unwrap();

    let events = drain_events(&mut h.events_rx);
    assert!(events.iter().any(|e| matches!(
        e,
        AssistantEvent::Final { content } if content == "Réponse du modèle"
    )));
    assert!(events
        .iter()
        .any(|e| matches!(e, AssistantEvent::Token { content } if content == "Réponse du modèle")));
}

/// Appel d'outil : le modèle demande `emule_status`, le daemon répond,
/// puis le modèle conclut — l'historique renvoyé au backend contient
/// `assistant(tool_calls)` puis `tool(tool_name)` dans l'ordre.
#[tokio::test]
async fn test_tool_call_executed_then_final() {
    let mut h = build_session(vec![
        tool_call_turn("emule_status", json!({})),
        text_turn("Voici l'état du daemon"),
    ])
    .await;

    h.session
        .user_turn("état ?", &h.events_tx, &h.confirm_tx)
        .await
        .unwrap();

    let events = drain_events(&mut h.events_rx);
    assert!(events
        .iter()
        .any(|e| matches!(e, AssistantEvent::ToolStart { name, .. } if name == "emule_status")));
    assert!(events.iter().any(|e| matches!(
        e,
        AssistantEvent::ToolResult { name, is_error, .. } if name == "emule_status" && !is_error
    )));
    assert!(events
        .iter()
        .any(|e| matches!(e, AssistantEvent::Final { content } if content.contains("Voici"))));

    // Historique vu par le backend au 2e appel : …, assistant(tool_calls),
    // tool(emule_status) — dans cet ordre.
    let history = h.backend.last_history();
    let assistant_idx = history
        .iter()
        .position(|m| m.role == Role::Assistant && !m.tool_calls.is_empty())
        .expect("message assistant avec tool_calls présent");
    let tool_idx = history
        .iter()
        .position(|m| m.role == Role::Tool && m.tool_name.as_deref() == Some("emule_status"))
        .expect("message tool emule_status présent");
    assert!(assistant_idx < tool_idx, "ordre assistant→tool respecté");
}

/// Outil hors allowlist : refusé sans contact daemon, le refus repart au
/// modèle comme résultat `role: "tool"` (la boucle ne casse pas) et la
/// confirmation n'est jamais demandée.
#[tokio::test]
async fn test_out_of_allowlist_tool_refused() {
    let mut h = build_session(vec![
        tool_call_turn("emule_shutdown", json!({})),
        text_turn("refus expliqué"),
    ])
    .await;

    h.session
        .user_turn("arrête tout", &h.events_tx, &h.confirm_tx)
        .await
        .unwrap();

    let events = drain_events(&mut h.events_rx);
    assert!(events.iter().any(|e| matches!(
        e,
        AssistantEvent::ToolResult { name, is_error, content }
            if name == "emule_shutdown" && *is_error && content.contains("allowlist")
    )));
    // Aucun ToolStart n'a été émis : l'outil n'a pas été exécuté.
    assert!(!events
        .iter()
        .any(|e| matches!(e, AssistantEvent::ToolStart { name, .. } if name == "emule_shutdown")));
    // Pas de confirmation demandée : le channel n'a rien reçu.
    assert!(h.confirm_rx.try_recv().is_err());
    // La boucle a continué jusqu'au Final.
    assert!(events
        .iter()
        .any(|e| matches!(e, AssistantEvent::Final { .. })));
}

/// Confirmation acceptée : `emule_add_link` s'exécute après `o`.
#[tokio::test]
async fn test_confirmed_tool_executes() {
    let mut h = build_session(vec![
        tool_call_turn(
            "emule_add_link",
            json!({ "link": "ed2k://|file|test.bin|1048576|31D6CFE0D16AE931B73C59D7E0C089C0|/" }),
        ),
        text_turn("lien ajouté"),
    ])
    .await;

    let mut confirm_rx = std::mem::replace(&mut h.confirm_rx, mpsc::channel(1).1);
    let responder = tokio::spawn(async move {
        while let Some(req) = confirm_rx.recv().await {
            assert_eq!(req.call.name, "emule_add_link");
            let _ = req.respond.send(true);
        }
    });

    h.session
        .user_turn("ajoute ce lien", &h.events_tx, &h.confirm_tx)
        .await
        .unwrap();
    responder.abort();

    let events = drain_events(&mut h.events_rx);
    assert!(events
        .iter()
        .any(|e| matches!(e, AssistantEvent::ToolStart { name, .. } if name == "emule_add_link")));
    assert!(events.iter().any(|e| matches!(
        e,
        AssistantEvent::ToolResult { name, is_error, .. } if name == "emule_add_link" && !is_error
    )));
}

/// Confirmation refusée : l'outil n'est pas exécuté, le modèle reçoit
/// « annulé par l'utilisateur » comme résultat d'outil.
#[tokio::test]
async fn test_denied_tool_not_executed() {
    let mut h = build_session(vec![
        tool_call_turn(
            "emule_add_link",
            json!({ "link": "ed2k://|file|x.bin|1|31D6CFE0D16AE931B73C59D7E0C089C0|/" }),
        ),
        text_turn("compris, rien ajouté"),
    ])
    .await;

    let mut confirm_rx = std::mem::replace(&mut h.confirm_rx, mpsc::channel(1).1);
    let responder = tokio::spawn(async move {
        while let Some(req) = confirm_rx.recv().await {
            let _ = req.respond.send(false);
        }
    });

    h.session
        .user_turn("ajoute", &h.events_tx, &h.confirm_tx)
        .await
        .unwrap();
    responder.abort();

    let events = drain_events(&mut h.events_rx);
    assert!(!events
        .iter()
        .any(|e| matches!(e, AssistantEvent::ToolStart { .. })));
    assert!(events.iter().any(|e| matches!(
        e,
        AssistantEvent::ToolResult { name, is_error, content }
            if name == "emule_add_link" && *is_error && content.contains("annulé")
    )));
}

/// Sans répondeur de confirmation (canal fermé), un appel à confirmer
/// est refusé — défaut sûr en mode non interactif.
#[tokio::test]
async fn test_confirm_without_responder_refused() {
    let mut h = build_session(vec![
        tool_call_turn("emule_connect_server", json!({})),
        text_turn("impossible"),
    ])
    .await;
    drop(h.confirm_rx); // ferme le canal : plus de répondeur

    h.session
        .user_turn("connecte un serveur", &h.events_tx, &h.confirm_tx)
        .await
        .unwrap();

    let events = drain_events(&mut h.events_rx);
    assert!(events.iter().any(|e| matches!(
        e,
        AssistantEvent::ToolResult { name, is_error, content }
            if name == "emule_connect_server" && *is_error && content.contains("répondeur")
    )));
}

/// La boucle est bornée par `max_tool_rounds` — un modèle qui demande
/// toujours un outil est interrompu par un `Notice`.
#[tokio::test]
async fn test_tool_round_limit() {
    let turns = (0..6)
        .map(|_| tool_call_turn("emule_status", json!({})))
        .collect();
    let mut h = build_session(turns).await;

    // Reconnexion avec une borne serrée — même daemon, même backend.
    h.session = connect(
        &h.addr,
        TOKEN,
        h.backend.clone(),
        AssistantConfig {
            max_tool_rounds: 2,
            tool_profile: ToolProfile::Full,
            ..Default::default()
        },
    )
    .await;

    h.session
        .user_turn("encore", &h.events_tx, &h.confirm_tx)
        .await
        .unwrap();

    let events = drain_events(&mut h.events_rx);
    assert!(events
        .iter()
        .any(|e| matches!(e, AssistantEvent::Notice { .. })));
    // 2 rounds × 1 appel = 2 ToolResult, pas plus.
    let tool_results = events
        .iter()
        .filter(|e| matches!(e, AssistantEvent::ToolResult { .. }))
        .count();
    assert_eq!(tool_results, 2);
}
