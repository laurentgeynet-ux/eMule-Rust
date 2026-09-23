use mule_daemon::{create_router, AppState, DaemonConfig, DownloadService};
use mule_mcp::{execute_tool, list_tools, DaemonClient};
use serde_json::json;
use std::net::SocketAddr;
use tempfile::tempdir;
use tokio::net::TcpListener;

async fn spawn_test_daemon() -> (SocketAddr, String, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token).with_bind_addr(addr);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    let app = create_router(state);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (addr, token, dir)
}

#[test]
fn test_tools_schema_integrity() {
    let tools = list_tools();
    assert!(tools.len() >= 15, "Expected at least 15 tools defined");

    let mut names = std::collections::HashSet::new();
    for tool in &tools {
        assert!(!tool.name.is_empty());
        assert!(!tool.description.is_empty());
        assert!(tool.name.starts_with("emule_"));
        assert!(
            names.insert(tool.name.clone()),
            "Duplicate tool name: {}",
            tool.name
        );

        let schema = &tool.input_schema;
        assert_eq!(schema.get("type").and_then(|t| t.as_str()), Some("object"));
    }
}

#[tokio::test]
async fn test_mcp_client_e2e_with_daemon() {
    let (addr, token, _dir) = spawn_test_daemon().await;
    let url = format!("ws://{addr}/rpc");

    // 1. Connect and perform hello handshake
    let client = DaemonClient::connect(Some(url), Some(token), None, None)
        .await
        .expect("DaemonClient connection should succeed");

    // 2. Test emule_status
    let status_res = execute_tool(&client, "emule_status", json!({})).await;
    assert!(
        !status_res.is_error,
        "Status tool call failed: {:?}",
        status_res
    );
    assert!(!status_res.content.is_empty());
    let status_json: serde_json::Value = serde_json::from_str(&status_res.content[0].text).unwrap();
    assert_eq!(
        status_json.get("downloadsCount").and_then(|v| v.as_u64()),
        Some(0)
    );

    // 3. Test emule_list_downloads (initially empty)
    let list_res = execute_tool(&client, "emule_list_downloads", json!({})).await;
    assert!(!list_res.is_error);
    let list_json: serde_json::Value = serde_json::from_str(&list_res.content[0].text).unwrap();
    assert!(list_json
        .get("downloads")
        .and_then(|d| d.as_array())
        .is_some());

    // 4. Test emule_get_settings
    let settings_res = execute_tool(&client, "emule_get_settings", json!({})).await;
    assert!(!settings_res.is_error);

    // 5. Test emule_set_limits
    let limits_res = execute_tool(
        &client,
        "emule_set_limits",
        json!({
            "download_limit_kbps": 500,
            "upload_limit_kbps": 100
        }),
    )
    .await;
    assert!(!limits_res.is_error);

    // 6. Test emule_add_link
    let test_link = "ed2k://|file|test_file.bin|1048576|31D6CFE0D16AE931B73C59D7E0C089C0|/";
    let add_res = execute_tool(&client, "emule_add_link", json!({ "link": test_link })).await;
    assert!(!add_res.is_error, "Add link failed: {:?}", add_res);
    let add_json: serde_json::Value = serde_json::from_str(&add_res.content[0].text).unwrap();
    let download_id = add_json
        .get("downloadId")
        .and_then(|id| id.as_str())
        .unwrap();

    // 7. Test emule_get_download_details by hash (resolves automatically)
    let details_res = execute_tool(
        &client,
        "emule_get_download_details",
        json!({
            "id_or_hash": "31D6CFE0D16AE931B73C59D7E0C089C0"
        }),
    )
    .await;
    assert!(
        !details_res.is_error,
        "get_download_details failed: {:?}",
        details_res
    );

    // 8. Test emule_pause_download by UUID
    let pause_res = execute_tool(
        &client,
        "emule_pause_download",
        json!({ "id_or_hash": download_id }),
    )
    .await;
    assert!(!pause_res.is_error, "pause failed: {:?}", pause_res);

    // 9. Test emule_resume_download by hash
    let resume_res = execute_tool(
        &client,
        "emule_resume_download",
        json!({
            "id_or_hash": "31D6CFE0D16AE931B73C59D7E0C089C0"
        }),
    )
    .await;
    assert!(!resume_res.is_error, "resume failed: {:?}", resume_res);

    // 10. Test emule_remove_download
    let remove_res = execute_tool(
        &client,
        "emule_remove_download",
        json!({
            "id_or_hash": download_id,
            "delete_files": false
        }),
    )
    .await;
    assert!(!remove_res.is_error, "remove failed: {:?}", remove_res);

    // 11. Test Categories
    let create_cat_res = execute_tool(
        &client,
        "emule_create_category",
        json!({
            "name": "ISO Images",
            "color": "#FF5722"
        }),
    )
    .await;
    assert!(
        !create_cat_res.is_error,
        "create_category failed: {:?}",
        create_cat_res
    );
    let cat_json: serde_json::Value =
        serde_json::from_str(&create_cat_res.content[0].text).unwrap();
    let cat_id = cat_json.get("id").and_then(|i| i.as_str()).unwrap();

    let list_cat_res = execute_tool(&client, "emule_list_categories", json!({})).await;
    assert!(!list_cat_res.is_error);

    let del_cat_res = execute_tool(&client, "emule_delete_category", json!({ "id": cat_id })).await;
    assert!(!del_cat_res.is_error);

    // 12. Test Network Policy
    let net_policy_res = execute_tool(&client, "emule_get_network_policy", json!({})).await;
    assert!(
        !net_policy_res.is_error,
        "get_network_policy failed: {:?}",
        net_policy_res
    );

    // 13. Test IP Filter
    let filter_res = execute_tool(&client, "emule_get_filter_status", json!({})).await;
    assert!(
        !filter_res.is_error,
        "get_filter_status failed: {:?}",
        filter_res
    );

    // 14. Test Shares
    let shares_res = execute_tool(&client, "emule_list_shares", json!({})).await;
    assert!(!shares_res.is_error, "list_shares failed: {:?}", shares_res);

    // 15. Test History Stats
    let hist_stats_res = execute_tool(&client, "emule_history_stats", json!({})).await;
    assert!(
        !hist_stats_res.is_error,
        "history_stats failed: {:?}",
        hist_stats_res
    );

    // 16. Test Identity
    let identity_res = execute_tool(&client, "emule_get_identity", json!({})).await;
    assert!(
        !identity_res.is_error,
        "get_identity failed: {:?}",
        identity_res
    );

    // 17. Test UPnP & Listener Status
    let upnp_res = execute_tool(&client, "emule_upnp_status", json!({})).await;
    assert!(!upnp_res.is_error, "upnp_status failed: {:?}", upnp_res);

    let listener_res = execute_tool(&client, "emule_listener_status", json!({})).await;
    assert!(
        !listener_res.is_error,
        "listener_status failed: {:?}",
        listener_res
    );
}

#[tokio::test]
async fn test_unreachable_daemon_error_reporting() {
    let result = DaemonClient::connect(
        Some("ws://127.0.0.1:19999/rpc".to_string()),
        None,
        None,
        None,
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Is EMule-Rust running"));
}
