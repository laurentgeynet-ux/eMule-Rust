// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Session assistant : boucle « modèle → outils → modèle » bornée.
//!
//! Invariants :
//! - le message `Assistant` complet (avec ses `tool_calls`) est poussé
//!   dans l'historique AVANT les résultats — exigence des APIs natives ;
//! - les `tool_calls` streamés sont accumulés puis validés par
//!   [`crate::policy::check_call`] AVANT toute exécution ;
//! - les erreurs d'outils ne cassent pas la boucle : elles repartent au
//!   modèle comme messages `role: "tool"` ;
//! - l'UI reçoit le résultat complet via [`AssistantEvent::ToolResult`],
//!   le modèle la version compactée ([`crate::tools_bridge`]).

use std::path::PathBuf;

use mule_mcp::tools::execute_tool;
use mule_mcp::DaemonClient;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, warn};

use crate::config::{AssistantConfig, SYSTEM_PROMPT};
use crate::error::AssistantError;
use crate::llm::{ChatMessage, LlmBackend, Role, ToolCall, ToolSchema};
use crate::policy;
use crate::tools_bridge;

/// Events émis par la session vers le transport (REPL, WebSocket) —
/// sérialisés tels quels en notifications `assistant.event` côté WS.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AssistantEvent {
    /// Fragment de texte généré par le modèle (streaming).
    Token {
        /// Fragment reçu.
        content: String,
    },
    /// Début d'exécution d'un outil — émis immédiatement après
    /// validation, sans attendre la fin de l'appel RPC.
    ToolStart {
        /// Nom de l'outil.
        name: String,
        /// Arguments exacts transmis au daemon.
        args: Value,
    },
    /// Résultat complet d'un outil (le modèle reçoit la version tronquée).
    ToolResult {
        /// Nom de l'outil.
        name: String,
        /// Vrai si l'appel a échoué ou a été refusé/annulé.
        is_error: bool,
        /// Contenu intégral du résultat.
        content: String,
    },
    /// Réponse textuelle finale du tour.
    Final {
        /// Réponse complète (après assemblage des tokens).
        content: String,
    },
    /// Information de fin de tour sans réponse (limite de rounds…).
    Notice {
        /// Message explicatif.
        message: String,
    },
    /// Erreur fatale du tour (backend, daemon).
    Error {
        /// Message d'erreur.
        message: String,
    },
}

/// Demande de confirmation remontée au transport : l'utilisateur tranche
/// sur les arguments exacts de l'appel.
pub struct ConfirmRequest {
    /// Appel proposé par le modèle (nom + arguments affichés tels quels).
    pub call: ToolCall,
    /// Canal de réponse : `true` = exécuter, `false`/`closed` = refuser.
    pub respond: oneshot::Sender<bool>,
}

/// Session de conversation : historique, outils visibles et client
/// daemon. Une session = un historique ; le transport (REPL/WS) ne fait
/// qu'acheminer entrées, events et confirmations.
pub struct Session {
    daemon: DaemonClient,
    backend: Box<dyn LlmBackend>,
    config: AssistantConfig,
    capabilities: Vec<String>,
    tools: Vec<ToolSchema>,
    history: Vec<ChatMessage>,
}

impl Session {
    /// Connexion au daemon puis construction de la surface d'outils :
    /// les capacités sont lues via `system.status` (le `system.hello`
    /// interne de `DaemonClient` ne conserve pas sa réponse).
    pub async fn connect(
        backend: Box<dyn LlmBackend>,
        config: AssistantConfig,
        url: Option<String>,
        token: Option<String>,
        workdir: Option<PathBuf>,
        app: Option<PathBuf>,
    ) -> Result<Self, AssistantError> {
        config.validate()?;
        let daemon = DaemonClient::connect(url, token, workdir, app)
            .await
            .map_err(AssistantError::Daemon)?;

        let capabilities = fetch_capabilities(&daemon).await;
        let tools = policy::visible_tools(config.tool_profile, &capabilities)
            .iter()
            .map(tools_bridge::from_tool_definition)
            .collect();

        Ok(Self {
            daemon,
            backend,
            config,
            capabilities,
            tools,
            history: vec![ChatMessage::system(SYSTEM_PROMPT)],
        })
    }

    /// Nombre d'outils actuellement présentés au modèle (diagnostic).
    pub fn visible_tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Bouche les `tool_calls` restés sans résultat après annulation
    /// d'un tour : les APIs exigent un message `tool` par appel demandé,
    /// sinon l'historique garderait des appels pendants que le modèle
    /// pourrait rejouer ou rejeter.
    pub fn seal_cancelled_turn(&mut self) {
        let Some(idx) = self
            .history
            .iter()
            .rposition(|m| m.role == Role::Assistant && !m.tool_calls.is_empty())
        else {
            return;
        };
        // Exécution séquentielle : les messages Tool postérieurs comptent
        // les appels déjà répondus, le reste est pendant.
        let answered = self.history[idx + 1..]
            .iter()
            .filter(|m| m.role == Role::Tool)
            .count();
        let pending: Vec<String> = self.history[idx]
            .tool_calls
            .iter()
            .skip(answered)
            .map(|c| c.name.clone())
            .collect();
        for name in pending {
            self.history
                .push(ChatMessage::tool(name, "annulé par l'utilisateur"));
        }
    }

    /// Rafraîchit capacités et outils visibles — à appeler après une
    /// reconnexion ou un changement de flavor du daemon.
    pub async fn refresh_capabilities(&mut self) {
        self.capabilities = fetch_capabilities(&self.daemon).await;
        self.tools = policy::visible_tools(self.config.tool_profile, &self.capabilities)
            .iter()
            .map(tools_bridge::from_tool_definition)
            .collect();
    }

    /// Un tour utilisateur complet, borné par `config.max_tool_rounds`.
    ///
    /// `events` reçoit tokens, débuts/fins d'outils et réponse finale ;
    /// `confirms` reçoit les demandes de confirmation — sans répondeur
    /// (canal fermé), tout outil à confirmer est refusé (défaut sûr).
    pub async fn user_turn(
        &mut self,
        input: &str,
        events: &mpsc::UnboundedSender<AssistantEvent>,
        confirms: &mpsc::Sender<ConfirmRequest>,
    ) -> Result<(), AssistantError> {
        self.history.push(ChatMessage::user(input));

        let events_sink = events.clone();
        let sink = move |token: &str| -> bool {
            events_sink
                .send(AssistantEvent::Token {
                    content: token.to_string(),
                })
                .is_ok()
        };

        for _round in 0..self.config.max_tool_rounds {
            let turn = self
                .backend
                .chat(&self.history, &self.tools, &sink)
                .await?;

            if let Some(m) = &turn.metrics {
                debug!(
                    load_ms = m.load_duration_ms,
                    prompt_eval_ms = m.prompt_eval_duration_ms,
                    eval_ms = m.eval_duration_ms,
                    eval_count = m.eval_count,
                    "métriques backend LLM"
                );
            }

            // Historique : le message assistant complet AVANT les
            // résultats d'outils (exigence des APIs natives).
            self.history.push(ChatMessage::assistant(&turn));

            if turn.tool_calls.is_empty() {
                let _ = events.send(AssistantEvent::Final {
                    content: turn.content.clone(),
                });
                return Ok(());
            }

            for call in turn.tool_calls {
                self.execute_one_call(call, events, confirms).await;
            }
        }

        let _ = events.send(AssistantEvent::Notice {
            message: format!(
                "limite de {} tours outils atteinte — réponse interrompue",
                self.config.max_tool_rounds
            ),
        });
        Ok(())
    }

    /// Exécute un appel d'outil : validation policy, confirmation
    /// éventuelle, RPC daemon, puis empile le résultat dans l'historique.
    async fn execute_one_call(
        &mut self,
        call: ToolCall,
        events: &mpsc::UnboundedSender<AssistantEvent>,
        confirms: &mpsc::Sender<ConfirmRequest>,
    ) {
        // Validation revérifiée juste avant exécution.
        if let Err(e) = policy::check_call(self.config.tool_profile, &self.capabilities, &call.name)
        {
            self.push_tool_outcome(&call.name, true, &e.to_string(), events);
            return;
        }

        // Confirmation utilisateur sur les arguments exacts.
        if policy::requires_confirmation(&call.name) {
            let (tx, rx) = oneshot::channel();
            if confirms
                .send(ConfirmRequest {
                    call: call.clone(),
                    respond: tx,
                })
                .await
                .is_err()
            {
                self.push_tool_outcome(
                    &call.name,
                    true,
                    "refusé : aucun répondeur de confirmation actif",
                    events,
                );
                return;
            }
            match rx.await {
                Ok(true) => {}
                _ => {
                    self.push_tool_outcome(
                        &call.name,
                        true,
                        "annulé par l'utilisateur",
                        events,
                    );
                    return;
                }
            }
        }

        let _ = events.send(AssistantEvent::ToolStart {
            name: call.name.clone(),
            args: call.args.clone(),
        });

        let result = execute_tool(&self.daemon, &call.name, call.args.clone()).await;
        let full_text = result
            .content
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        let _ = events.send(AssistantEvent::ToolResult {
            name: call.name.clone(),
            is_error: result.is_error,
            content: full_text,
        });

        // Le modèle reçoit la version compactée/tronquée.
        let compacted = tools_bridge::compact_tool_result(&result, self.config.tool_result_max_chars);
        self.history.push(ChatMessage::tool(&call.name, compacted));
    }

    /// Empile un résultat d'outil refusé/annulé et émet l'event
    /// correspondant — le modèle voit le refus comme un résultat, la
    /// boucle ne casse pas.
    fn push_tool_outcome(
        &mut self,
        tool_name: &str,
        is_error: bool,
        message: &str,
        events: &mpsc::UnboundedSender<AssistantEvent>,
    ) {
        let _ = events.send(AssistantEvent::ToolResult {
            name: tool_name.to_string(),
            is_error,
            content: message.to_string(),
        });
        self.history.push(ChatMessage::tool(tool_name, message));
    }
}

/// Lit les capacités du daemon via `system.status` — en cas d'échec,
/// ensemble vide (les outils à capacité requise seront masqués, défaut
/// sûr) et warning tracé.
async fn fetch_capabilities(daemon: &DaemonClient) -> Vec<String> {
    match daemon.call("system.status", json!({})).await {
        Ok(res) => res
            .get("capabilities")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
        Err(e) => {
            warn!("lecture des capacités daemon impossible : {e}");
            Vec::new()
        }
    }
}
