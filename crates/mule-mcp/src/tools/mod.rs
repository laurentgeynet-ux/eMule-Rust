// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

pub mod downloads;
pub mod kad;
pub mod servers;
pub mod shares;
pub mod system;

use crate::client::DaemonClient;
use crate::protocol::{ToolCallResult, ToolDefinition};
use serde_json::Value;
use tracing::info;

pub use downloads::resolve_download_id;

/// Propriété de schéma simple `{ "type": ty, "description": desc }`.
///
/// Source unique du boilerplate répété dans les ~87 définitions d'outils ;
/// les propriétés complexes (enum, items, default…) restent en `json!` brut.
pub(crate) fn prop(ty: &str, desc: &str) -> Value {
    serde_json::json!({ "type": ty, "description": desc })
}

/// Schéma d'entrée d'un outil : `{"type":"object","properties":…,"required":…}`.
/// `required` est omis quand la liste est vide.
pub(crate) fn obj_schema(properties: Value, required: &[&str]) -> Value {
    let mut schema = serde_json::json!({ "type": "object", "properties": properties });
    if !required.is_empty() {
        schema["required"] = serde_json::json!(required);
    }
    schema
}

pub fn list_tools() -> Vec<ToolDefinition> {
    let mut tools = Vec::with_capacity(64);
    tools.extend(system::list_tools());
    tools.extend(downloads::list_tools());
    tools.extend(servers::list_tools());
    tools.extend(shares::list_tools());
    tools.extend(kad::list_tools());
    tools
}

pub async fn execute_tool(client: &DaemonClient, tool_name: &str, args: Value) -> ToolCallResult {
    info!("Executing MCP tool: {tool_name} with args: {args}");

    if let Some(res) = system::handle_tool(client, tool_name, &args).await {
        return res;
    }
    if let Some(res) = downloads::handle_tool(client, tool_name, &args).await {
        return res;
    }
    if let Some(res) = servers::handle_tool(client, tool_name, &args).await {
        return res;
    }
    if let Some(res) = shares::handle_tool(client, tool_name, &args).await {
        return res;
    }
    if let Some(res) = kad::handle_tool(client, tool_name, &args).await {
        return res;
    }

    ToolCallResult::error(format!("Unknown tool: {tool_name}"))
}
