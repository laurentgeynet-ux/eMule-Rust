// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

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
