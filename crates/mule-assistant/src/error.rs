// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Erreurs typées de la boucle assistant.

/// Erreur fatale de l'assistant (configuration, backend, daemon ou
/// transport). Les refus de policy ne sont pas des erreurs : ils sont
/// renvoyés au modèle comme résultats d'outil.
#[derive(Debug, thiserror::Error)]
pub enum AssistantError {
    /// Configuration invalide (endpoint non loopback sans override…).
    #[error("configuration invalide : {0}")]
    Config(String),

    /// Échec côté backend LLM (HTTP, parsing, stream).
    #[error("backend LLM : {0}")]
    Backend(String),

    /// Échec de connexion ou d'appel au daemon.
    #[error("daemon : {0}")]
    Daemon(String),

    /// Erreur de transport (REPL, WebSocket).
    #[error("transport : {0}")]
    Transport(String),
}
