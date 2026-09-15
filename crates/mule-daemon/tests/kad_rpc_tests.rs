use std::net::Ipv4Addr;
use serde_json::json;
use tempfile::tempdir;

use mule_daemon::{DaemonConfig, WorkDirMode};
use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::services::downloads::DownloadService;
use mule_daemon::state::AppState;
use mule_format::kad::{KadContactEntry, NodesDatFile};

#[tokio::test]
async fn test_kad_rpc_lifecycle_inspect_import_list_clear() {
    let tmp = tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let dl_dir = tmp.path().join("downloads");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::create_dir_all(&dl_dir).unwrap();

    let db_path = data_dir.join("mule.db");
    let downloads = DownloadService::new(&db_path, data_dir.join("temp")).unwrap();
    let config = DaemonConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        auth_token: "test-token".to_string(),
        data_dir,
        default_download_dir: dl_dir,
        workdir_mode: WorkDirMode::ExplicitCli,
    };

    let state = AppState::new(config, downloads);
    let mut authenticated = false;

    // 1. Handshake (system.hello)
    let hello_req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(json!(1)),
        method: "system.hello".into(),
        params: Some(json!({
            "protocolVersion": 1,
            "token": "test-token",
        })),
    };
    let resp = handle_request(&state, hello_req, &mut authenticated).await.unwrap();
    let res = resp.result.unwrap();
    let caps = res["capabilities"].as_array().unwrap();
    assert!(caps.iter().any(|c| c.as_str() == Some("kad-contacts")));
    assert!(authenticated);

    // 2. kad.status initial
    let status_req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(json!(2)),
        method: "kad.status".into(),
        params: None,
    };
    let resp = handle_request(&state, status_req, &mut authenticated).await.unwrap();
    let status = resp.result.unwrap();
    assert_eq!(status["totalContacts"], 0);

    // 3. Create a synthetic nodes.dat file
    let contacts = vec![
        KadContactEntry::new([1u8; 16], Ipv4Addr::new(127, 0, 0, 1), 4672, 4662, 2),
        KadContactEntry::new([2u8; 16], Ipv4Addr::new(127, 0, 0, 2), 4672, 0, 1),
    ];
    let nodes_file = NodesDatFile::new(2, contacts);
    let mut nodes_dat_bytes = Vec::new();
    nodes_file.write(&mut nodes_dat_bytes).unwrap();
    let nodes_dat_path = tmp.path().join("nodes.dat");
    std::fs::write(&nodes_dat_path, &nodes_dat_bytes).unwrap();

    // 4. kad.inspectNodesDat
    let inspect_req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(json!(3)),
        method: "kad.inspectNodesDat".into(),
        params: Some(json!({
            "path": nodes_dat_path.to_str().unwrap(),
        })),
    };
    let resp = handle_request(&state, inspect_req, &mut authenticated).await.unwrap();
    let inspect = resp.result.unwrap();
    assert_eq!(inspect["declaredCount"], 2);
    // In LocalLab profile, 127.0.0.1 is allowed
    assert_eq!(inspect["validContactsCount"], 2);

    // 5. kad.importNodesDat
    let import_req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(json!(4)),
        method: "kad.importNodesDat".into(),
        params: Some(json!({
            "path": nodes_dat_path.to_str().unwrap(),
        })),
    };
    let resp = handle_request(&state, import_req, &mut authenticated).await.unwrap();
    let import_res = resp.result.unwrap();
    assert_eq!(import_res["importedCount"], 2);

    // 6. kad.listContacts
    let list_req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(json!(5)),
        method: "kad.listContacts".into(),
        params: Some(json!({ "limit": 10 })),
    };
    let resp = handle_request(&state, list_req.clone(), &mut authenticated).await.unwrap();
    let list_res = resp.result.unwrap();
    assert_eq!(list_res["total"], 2);

    // 7. kad.clearContacts
    let clear_req = JsonRpcRequest {
        jsonrpc: "2.0".into(),
        id: Some(json!(6)),
        method: "kad.clearContacts".into(),
        params: None,
    };
    let resp = handle_request(&state, clear_req, &mut authenticated).await.unwrap();
    let clear_res = resp.result.unwrap();
    assert_eq!(clear_res["cleared"], 2);

    // 8. Verify list is now empty
    let resp = handle_request(&state, list_req, &mut authenticated).await.unwrap();
    let list_res = resp.result.unwrap();
    assert_eq!(list_res["total"], 0);
}
