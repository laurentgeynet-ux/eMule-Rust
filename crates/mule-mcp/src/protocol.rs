// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Type d'erreur JSON-RPC canonique — défini dans `mule-daemon` (source de
/// vérité unique pour les codes et constructeurs du workspace).
pub use mule_daemon::error::JsonRpcError;

pub const MCP_PROTOCOL_VERSION: &str = "2024-11-05";
pub const SERVER_NAME: &str = "emule-rust";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
}

pub fn make_response(id: Value, result: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

pub fn make_error(id: Value, error: JsonRpcError) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": error
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolCallContent>,
    #[serde(rename = "isError", skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
}

impl ToolCallResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolCallContent {
                content_type: "text".to_string(),
                text: text.into(),
            }],
            is_error: false,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            content: vec![ToolCallContent {
                content_type: "text".to_string(),
                text: msg.into(),
            }],
            is_error: true,
        }
    }

    pub fn json(val: &Value) -> Self {
        let text = serde_json::to_string_pretty(val).unwrap_or_else(|_| val.to_string());
        Self::text(text)
    }
}
