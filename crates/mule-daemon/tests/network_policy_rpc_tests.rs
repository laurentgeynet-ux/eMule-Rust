// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use serde_json::{json, Value};
use tempfile::tempdir;

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};

fn setup_test_state() -> (AppState, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "policy_test_token_123".to_string();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    (state, dir)
}

fn make_req(id: u64, method: &str, params: Option<Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(id)),
        method: method.to_string(),
        params,
    }
}

#[tokio::test]
async fn test_network_policy_capabilities_in_hello_and_status() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = false;

    // 1. system.hello
    let hello_req = make_req(
        1,
        "system.hello",
        Some(json!({
            "clientName": "test-client",
            "clientVersion": "0.1.0",
            "protocolVersion": 1,
            "token": "policy_test_token_123"
        })),
    );
    let resp = handle_request(&state, hello_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    let caps = res["capabilities"].as_array().unwrap();
    assert!(caps.iter().any(|c| c.as_str() == Some("network-policy")));

    // 2. system.status
    let status_req = make_req(2, "system.status", None);
    let resp = handle_request(&state, status_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["networkState"], "preview-active");
    assert_eq!(res["networkProfile"], "public-ed2k-kad-preview");
    let caps = res["capabilities"].as_array().unwrap();
    assert!(caps.iter().any(|c| c.as_str() == Some("network-policy")));
}

#[tokio::test]
async fn test_network_get_and_set_policy_rpc() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // 1. network.getPolicy initial
    let req = make_req(10, "network.getPolicy", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["profile"], "public-ed2k-kad-preview");
    assert_eq!(res["state"], "preview-active");
    assert_eq!(res["activeConnections"]["peers"], 0);
    assert_eq!(res["activeConnections"]["servers"], 0);
    assert_eq!(res["activeConnections"]["listeners"], 0);
    assert_eq!(res["budgets"]["max_active_peer_connections"], 500);

    // 2. network.setProfile to public-ed2k-preview
    let set_req = make_req(
        11,
        "network.setProfile",
        Some(json!({ "profile": "public-ed2k-preview" })),
    );
    let resp = handle_request(&state, set_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["profile"], "public-ed2k-preview");

    // 3. network.setProfile with invalid name
    let invalid_req = make_req(
        12,
        "network.setProfile",
        Some(json!({ "profile": "unknown-profile" })),
    );
    let resp = handle_request(&state, invalid_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_some());

    // 4. network.setBudgets
    let mut new_budgets = state.network_policy.budgets();
    new_budgets.max_active_peer_connections = 35;
    let set_budgets_req = make_req(
        13,
        "network.setBudgets",
        Some(json!({ "budgets": new_budgets })),
    );
    let resp = handle_request(&state, set_budgets_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["budgets"]["max_active_peer_connections"], 35);
}

#[tokio::test]
async fn test_network_kill_switch_and_audit_log_rpc() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // 1. network.disconnectAll
    let kill_req = make_req(20, "network.disconnectAll", None);
    let resp = handle_request(&state, kill_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["success"], true);
    assert_eq!(res["state"], "disabled");

    // Verify policy query reflects disabled
    let get_req = make_req(21, "network.getPolicy", None);
    let resp = handle_request(&state, get_req, &mut authenticated)
        .await
        .unwrap();
    let res = resp.result.unwrap();
    assert_eq!(res["state"], "disabled");

    // 2. network.enable
    let enable_req = make_req(22, "network.enable", None);
    let resp = handle_request(&state, enable_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["state"], "preview-active");

    // 3. network.getAuditLog
    let audit_req = make_req(23, "network.getAuditLog", Some(json!({ "limit": 10 })));
    let resp = handle_request(&state, audit_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    let entries = res["entries"].as_array().unwrap();
    assert!(
        !entries.is_empty(),
        "Audit log should have entries from kill switch and enable"
    );
    assert!(entries
        .iter()
        .any(|e| e["event_type"] == "kill-switch-triggered"));
    assert!(entries.iter().any(|e| e["event_type"] == "network-enabled"));
}
