// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Pont entre les `ToolDefinition` MCP et les schémas neutres du backend.
//!
//! Aucune redéfinition de schéma : les `inputSchema` JSON Schema déjà
//! construits par `mule-mcp` sont réutilisés tels quels — ils sont au
//! format attendu par le tool calling d'Ollama/OpenAI.

use mule_mcp::protocol::{ToolCallResult, ToolDefinition};
use serde_json::Value;

use crate::llm::ToolSchema;

/// Conversion `ToolDefinition` MCP → schéma neutre pour le modèle.
pub fn from_tool_definition(def: &ToolDefinition) -> ToolSchema {
    ToolSchema {
        name: def.name.clone(),
        description: def.description.clone(),
        parameters: def.input_schema.clone(),
    }
}

/// Rendu compact d'un résultat d'outil pour le contexte du modèle :
/// JSON minifié (le `ToolCallResult::json` MCP est pretty-printé, trop
/// coûteux en tokens) puis tronqué à `max_chars` avec le total réel
/// indiqué. L'UI reçoit la version complète via [`crate::AssistantEvent`].
pub fn compact_tool_result(result: &ToolCallResult, max_chars: usize) -> String {
    let raw = result
        .content
        .iter()
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // Minifie uniquement si le contenu est du JSON valide — sinon le
    // texte brut est renvoyé tel quel (messages d'erreur MCP).
    let compact = serde_json::from_str::<Value>(&raw)
        .map(|v| v.to_string())
        .unwrap_or(raw);

    let char_count = compact.chars().count();
    if char_count <= max_chars {
        compact
    } else {
        let truncated: String = compact.chars().take(max_chars).collect();
        format!("{truncated}… [tronqué pour le modèle — {char_count} caractères au total]")
    }
}
