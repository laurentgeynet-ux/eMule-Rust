// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Download and category MCP tool registrations and dispatchers.

pub mod handlers;
pub mod schemas;

use crate::client::DaemonClient;
use crate::protocol::{ToolCallResult, ToolDefinition};
pub use handlers::resolve_download_id;
use serde_json::Value;

/// Returns tool definitions for download and category management.
pub fn list_tools() -> Vec<ToolDefinition> {
    schemas::definitions()
}

/// Dispatches download/category tool calls.
pub async fn handle_tool(
    client: &DaemonClient,
    tool_name: &str,
    args: &Value,
) -> Option<ToolCallResult> {
    handlers::execute(client, tool_name, args).await
}
