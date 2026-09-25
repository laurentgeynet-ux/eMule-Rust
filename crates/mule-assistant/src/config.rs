// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Configuration de l'assistant : endpoint LLM, modèle, budgets.
//!
//! Tous les réglages vivent ici — aucune valeur en dur dans la logique,
//! conformément à la règle « constantes et réglages » d'AGENTS.md.

use std::time::Duration;

use mule_mcp::tools::ToolProfile;

use crate::error::AssistantError;

/// Endpoint LLM par défaut : API native Ollama sur loopback.
pub const DEFAULT_ENDPOINT: &str = "http://127.0.0.1:11434";

/// Modèle par défaut — doit implémenter le tool calling Ollama.
pub const DEFAULT_MODEL: &str = "qwen3:8b";

/// Nombre maximal de tours « modèle → outils → modèle » par message
/// utilisateur : borne la boucle agentique.
pub const DEFAULT_MAX_TOOL_ROUNDS: u32 = 4;

/// Taille maximale (caractères) du résultat d'outil renvoyé au modèle —
/// l'UI reçoit toujours la version complète via [`crate::AssistantEvent`].
pub const DEFAULT_TOOL_RESULT_MAX_CHARS: usize = 4000;

/// Timeout global d'une requête vers le backend LLM : une génération
/// longue sur CPU modeste ne doit pas être tronquée prématurément.
pub const DEFAULT_LLM_TIMEOUT: Duration = Duration::from_secs(300);

/// Timeout d'établissement de la connexion HTTP au backend.
pub const LLM_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Port par défaut du serveur WebSocket de l'assistant.
pub const DEFAULT_WS_PORT: u16 = 4680;

/// Prompt système : contexte métier et consignes pour le modèle.
/// Rédigé en français, cohérent avec la langue du projet.
pub const SYSTEM_PROMPT: &str = "Tu es l'assistant intégré d'eMule-Rust, un client P2P eD2K/Kad. \
Tu pilotes l'application via les outils fournis : utilise-les pour obtenir l'état réel \
(téléchargements, Kad, serveurs) plutôt que de deviner. Enchaîne les appels si nécessaire. \
Quand une information suffit, réponds directement sans nouvel appel. \
Réponds en français, de façon concise. Si un outil échoue, explique l'erreur à l'utilisateur.";

/// Configuration validée d'une session assistant.
#[derive(Debug, Clone)]
pub struct AssistantConfig {
    /// URL de base du backend LLM (ex. `http://127.0.0.1:11434`).
    pub endpoint: String,
    /// Nom du modèle côté backend.
    pub model: String,
    /// Autorise explicitement un endpoint non loopback — désactivé par
    /// défaut pour coller à la posture d'isolation du projet.
    pub allow_remote_endpoint: bool,
    /// Borne de la boucle agentique (cf. [`DEFAULT_MAX_TOOL_ROUNDS`]).
    pub max_tool_rounds: u32,
    /// Troncation du résultat d'outil pour le modèle
    /// (cf. [`DEFAULT_TOOL_RESULT_MAX_CHARS`]).
    pub tool_result_max_chars: usize,
    /// Timeout HTTP global vers le backend.
    pub llm_timeout: Duration,
    /// Plafond de risque des outils exposés au modèle.
    pub tool_profile: ToolProfile,
}

impl Default for AssistantConfig {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_ENDPOINT.to_string(),
            model: DEFAULT_MODEL.to_string(),
            allow_remote_endpoint: false,
            max_tool_rounds: DEFAULT_MAX_TOOL_ROUNDS,
            tool_result_max_chars: DEFAULT_TOOL_RESULT_MAX_CHARS,
            llm_timeout: DEFAULT_LLM_TIMEOUT,
            tool_profile: ToolProfile::Full,
        }
    }
}

impl AssistantConfig {
    /// Valide l'endpoint : scheme `http` et hôte **IP littérale** loopback
    /// (`127.0.0.1`, `::1`) — les noms d'hôte sont refusés car résolubles
    /// vers l'extérieur. `--allow-remote-endpoint` est le seul override.
    pub fn validate(&self) -> Result<(), AssistantError> {
        let parsed = url::Url::parse(&self.endpoint).map_err(|e| {
            AssistantError::Config(format!("endpoint illisible '{}': {e}", self.endpoint))
        })?;
        if parsed.scheme() != "http" {
            return Err(AssistantError::Config(format!(
                "endpoint '{}' : seul le scheme http est accepté",
                self.endpoint
            )));
        }
        let is_loopback = match parsed.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            Some(url::Host::Domain(_)) => {
                return Err(AssistantError::Config(format!(
                    "endpoint '{}' : l'hôte doit être une IP littérale (127.0.0.1 ou ::1), pas un nom à résoudre",
                    self.endpoint
                )));
            }
            None => {
                return Err(AssistantError::Config(format!(
                    "endpoint '{}' sans hôte",
                    self.endpoint
                )));
            }
        };
        if !is_loopback && !self.allow_remote_endpoint {
            return Err(AssistantError::Config(format!(
                "endpoint '{}' non loopback : passer --allow-remote-endpoint pour l'autoriser explicitement",
                self.endpoint
            )));
        }
        Ok(())
    }
}
