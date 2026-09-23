// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Share, source, upload, and listener MCP tool registrations and dispatchers.

pub mod handlers;
pub mod schemas;

use crate::client::DaemonClient;
use crate::protocol::{ToolCallResult, ToolDefinition};
use serde_json::Value;

/// Returns tool definitions for shares, sources, uploads, and listeners.
pub fn list_tools() -> Vec<ToolDefinition> {
    schemas::definitions()
}

/// Dispatches share/source tool calls.
pub async fn handle_tool(
    client: &DaemonClient,
    tool_name: &str,
    args: &Value,
) -> Option<ToolCallResult> {
    handlers::execute(client, tool_name, args).await
}
