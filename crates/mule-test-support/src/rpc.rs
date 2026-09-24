// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Construction de requêtes JSON-RPC de test.

use mule_daemon::rpc::types::JsonRpcRequest;
use serde_json::{json, Value};

/// Construit une `JsonRpcRequest` d'id numérique `id` pour `method`.
pub fn make_req(id: u64, method: &str, params: Option<Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(id)),
        method: method.to_string(),
        params,
    }
}
