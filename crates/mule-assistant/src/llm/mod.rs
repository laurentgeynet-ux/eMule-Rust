// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Abstraction du backend LLM et types de la conversation.
//!
//! Le trait [`LlmBackend`] est volontairement minimal : il reçoit
//! l'historique et les schémas d'outils, pousse les tokens via `sink` au
//! fil de l'eau, et retourne le tour complet — `tool_calls` accumulés,
//! validables AVANT toute exécution. Chaque backend traduit le format
//! filaire de son API vers ces types neutres.

pub mod mock;
pub mod ollama;

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AssistantError;

/// Rôle d'un message de conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Consignes système initiales.
    System,
    /// Entrée utilisateur.
    User,
    /// Réponse du modèle (texte et/ou appels d'outils).
    Assistant,
    /// Résultat d'un outil renvoyé au modèle.
    Tool,
}

/// Appel d'outil émis par le modèle.
///
/// `id` est **optionnel** : l'API native Ollama n'exige pas
/// d'identifiant d'appel contrairement au format OpenAI — chaque
/// backend conserve son identifiant éventuel sans qu'on le suppose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Identifiant d'appel éventuel (OpenAI) — absent côté Ollama natif.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Nom de l'outil (`emule_*`).
    pub name: String,
    /// Arguments JSON proposés par le modèle.
    #[serde(default)]
    pub args: Value,
}

/// Message d'historique échangé avec le backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Rôle de l'émetteur.
    pub role: Role,
    /// Contenu textuel (vide pour un tour d'appels purs).
    #[serde(default)]
    pub content: String,
    /// Appels d'outils demandés — présents uniquement sur `Assistant`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// Nom de l'outil dont ce message porte le résultat — `Tool` uniquement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl ChatMessage {
    /// Message système (consignes).
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_name: None,
        }
    }

    /// Entrée utilisateur.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_name: None,
        }
    }

    /// Réponse du modèle — poussée dans l'historique AVANT les résultats
    /// d'outils, complète avec ses `tool_calls` (exigence Ollama/OpenAI).
    pub fn assistant(turn: &AssistantTurn) -> Self {
        Self {
            role: Role::Assistant,
            content: turn.content.clone(),
            tool_calls: turn.tool_calls.clone(),
            tool_name: None,
        }
    }

    /// Résultat d'outil — `tool_name` identifie l'outil côté Ollama.
    pub fn tool(tool_name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: content.into(),
            tool_calls: Vec::new(),
            tool_name: Some(tool_name.into()),
        }
    }
}

/// Tour de réponse complet du modèle.
#[derive(Debug, Clone, Default)]
pub struct AssistantTurn {
    /// Texte destiné à l'utilisateur.
    pub content: String,
    /// Appels d'outils demandés — vide = réponse finale.
    pub tool_calls: Vec<ToolCall>,
    /// Contenu « thinking » des modèles qui en émettent : collecté pour
    /// diagnostic mais jamais affiché à l'utilisateur par défaut.
    pub thinking: String,
    /// Métriques rapportées par le backend (durées Ollama…).
    pub metrics: Option<LlmMetrics>,
}

/// Métriques d'inférence rapportées par le backend (ms / tokens).
#[derive(Debug, Clone, Default)]
pub struct LlmMetrics {
    /// Durée de chargement du modèle.
    pub load_duration_ms: Option<u64>,
    /// Durée d'évaluation du prompt.
    pub prompt_eval_duration_ms: Option<u64>,
    /// Durée de génération.
    pub eval_duration_ms: Option<u64>,
    /// Nombre de tokens générés.
    pub eval_count: Option<u64>,
}

/// Schéma d'outil présenté au modèle — format neutre que chaque backend
/// sérialise à sa façon (Ollama `{type:"function", function:{…}}`).
#[derive(Debug, Clone)]
pub struct ToolSchema {
    /// Nom de l'outil.
    pub name: String,
    /// Description fonctionnelle.
    pub description: String,
    /// JSON Schema des paramètres.
    pub parameters: Value,
}

/// Sink de tokens : retourne `false` quand le consommateur est parti —
/// le backend doit alors interrompre le streaming au plus tôt.
pub type TokenSink<'a> = dyn Fn(&str) -> bool + Send + Sync + 'a;

/// Backend d'inférence interchangeable (Ollama, mock de test…).
///
/// Object-safety : la future est boxée pour permettre
/// `Box<dyn LlmBackend>` — le backend est choisi à l'exécution selon la
/// configuration.
pub trait LlmBackend: Send + Sync {
    /// Un tour de chat : streamer `content` vers `sink`, accumuler
    /// `tool_calls`, retourner le tour complet.
    fn chat<'a>(
        &'a self,
        history: &'a [ChatMessage],
        tools: &'a [ToolSchema],
        sink: &'a TokenSink<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<AssistantTurn, AssistantError>> + Send + 'a>>;
}

/// Délégation pour `Arc<T>` : permet aux tests de conserver une poignée
/// sur le backend après injection dans la session (`Box<dyn>`).
impl<T: LlmBackend + ?Sized> LlmBackend for std::sync::Arc<T> {
    fn chat<'a>(
        &'a self,
        history: &'a [ChatMessage],
        tools: &'a [ToolSchema],
        sink: &'a TokenSink<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<AssistantTurn, AssistantError>> + Send + 'a>> {
        (**self).chat(history, tools, sink)
    }
}
