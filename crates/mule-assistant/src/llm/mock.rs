// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Backend de test scripté — aucune I/O, conforme à la règle
//! « tests hors-ligne uniquement » d'AGENTS.md.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use crate::error::AssistantError;
use crate::llm::{
    AssistantTurn, ChatMessage, LlmBackend, LlmMetrics, TokenSink, ToolSchema,
};

/// Backend LLM dépilataire : chaque appel à [`LlmBackend::chat`]
/// consomme le prochain [`AssistantTurn`] scripté et pousse son contenu
/// via le sink. File vide = tour vide (réponse finale sans texte).
pub struct MockBackend {
    turns: Mutex<VecDeque<AssistantTurn>>,
    /// Dernier historique reçu — pour assertions de test.
    last_history: Mutex<Vec<ChatMessage>>,
}

impl MockBackend {
    /// Construit le backend avec la séquence de tours à jouer.
    pub fn new(turns: Vec<AssistantTurn>) -> Self {
        Self {
            turns: Mutex::new(turns.into()),
            last_history: Mutex::new(Vec::new()),
        }
    }

    /// Copie du dernier historique reçu (assertions de test).
    pub fn last_history(&self) -> Vec<ChatMessage> {
        self.last_history.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl LlmBackend for MockBackend {
    fn chat<'a>(
        &'a self,
        history: &'a [ChatMessage],
        _tools: &'a [ToolSchema],
        sink: &'a TokenSink<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<AssistantTurn, AssistantError>> + Send + 'a>> {
        Box::pin(async move {
            *self.last_history.lock().unwrap_or_else(|e| e.into_inner()) = history.to_vec();
            let turn = self
                .turns
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .pop_front()
                .unwrap_or_default();
            if !turn.content.is_empty() {
                let _ = sink(&turn.content);
            }
            Ok(turn)
        })
    }
}

/// Fabrique un tour « texte simple » pour les tests.
pub fn text_turn(content: &str) -> AssistantTurn {
    AssistantTurn {
        content: content.to_string(),
        ..Default::default()
    }
}

/// Fabrique un tour « appel d'outil » pour les tests.
pub fn tool_call_turn(name: &str, args: serde_json::Value) -> AssistantTurn {
    AssistantTurn {
        tool_calls: vec![crate::llm::ToolCall {
            id: None,
            name: name.to_string(),
            args,
        }],
        ..Default::default()
    }
}

/// Métriques factices pour les tests.
pub fn fake_metrics() -> Option<LlmMetrics> {
    Some(LlmMetrics {
        load_duration_ms: Some(10),
        prompt_eval_duration_ms: Some(20),
        eval_duration_ms: Some(30),
        eval_count: Some(5),
    })
}
