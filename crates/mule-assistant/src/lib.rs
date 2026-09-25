// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Assistant IA local pour eMule-Rust.
//!
//! Pont fin entre un LLM local (Ollama en loopback) et le JSON-RPC du
//! daemon : le modèle propose des appels d'outils, [`session::Session`]
//! les valide ([`policy`]) puis les exécute via `mule_mcp::execute_tool` —
//! le daemon reste l'autorité de validation et de sécurité. Aucun nouveau
//! registre d'actions n'est créé : les schémas et le mapping RPC sont
//! réutilisés depuis `mule-mcp`.

pub mod config;
pub mod error;
pub mod llm;
pub mod policy;
pub mod repl;
pub mod session;
pub mod tools_bridge;
pub mod ws;

pub use config::AssistantConfig;
pub use error::AssistantError;
pub use llm::{AssistantTurn, ChatMessage, LlmBackend, LlmMetrics, Role, ToolCall, ToolSchema};
pub use session::{AssistantEvent, ConfirmRequest, Session};
