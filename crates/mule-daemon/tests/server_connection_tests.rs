use std::time::Duration;
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};
use mule_network_policy::NetworkProfile;
use mule_protocol::ed2k::{OP_IDCHANGE, OP_LOGINREQUEST, OP_SERVERMESSAGE, OP_SERVERSTATUS};
use mule_protocol::PR_ED2K;

fn setup_test_state() -> (AppState, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "test_token_conn".to_string();

    let config =
        DaemonConfig::new(dir.path().to_path_buf(), &token).with_workdir_mode(WorkDirMode::Portable);
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
async fn test_server_connect_refused_under_local_lab_profile() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // Profile defaults to LocalLab. Trying to connect to public IP should be rejected.
    let req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": "91.121.162.155:4242" })),
    );
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    assert!(resp.error.is_some(), "Must be rejected under LocalLab profile");
    let err = resp.error.unwrap();
    assert!(err.message.contains("rejected by network policy"));
}

#[tokio::test]
async fn test_server_connect_refused_when_network_disabled() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // Switch to PublicEd2kPreview
    state.network_policy.set_profile(NetworkProfile::PublicEd2kPreview);
    // Disconnect all / network is disabled by default until explicitly enabled
    state.network_policy.disconnect_all();

    let req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": "91.121.162.155:4242" })),
    );
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    assert!(resp.error.is_some(), "Must be rejected when network is disabled");
    let err = resp.error.unwrap();
    assert!(err.message.contains("rejected by network policy"));
}

#[tokio::test]
async fn test_server_connect_refused_when_ip_filtered() {
    let (state, dir) = setup_test_state();
    let mut authenticated = true;

    // Load filter blocking 91.121.0.0 - 91.121.255.255
    let filter_file = dir.path().join("ipfilter.dat");
    std::fs::write(&filter_file, "091.121.000.000 - 091.121.255.255 , 100 , Blocked Server Range\n").unwrap();
    state.network_policy.ip_filter().load_from_file(&filter_file).unwrap();

    state.network_policy.set_profile(NetworkProfile::PublicEd2kPreview);
    state.network_policy.enable();

    let req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": "91.121.162.155:4242" })),
    );
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    assert!(resp.error.is_some(), "Must be rejected by IP filter");
    let err = resp.error.unwrap();
    assert!(err.message.contains("rejected by network policy") || err.message.contains("blocked by IP filter"));
}

#[tokio::test]
async fn test_server_connect_refused_when_disabled_in_db() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // Insert server as disabled in db
    let server = mule_core::model::Ed2kServer {
        address: "91.121.162.155:4242".to_string(),
        ip: "91.121.162.155".to_string(),
        port: 4242,
        name: "Test Server".to_string(),
        description: "".to_string(),
        ping: 0,
        preference: 0,
        failed_count: 0,
        dyn_ip: None,
        version: None,
        filter_status: mule_core::model::ServerFilterStatus::Disabled,
        last_seen_unix_ms: None,
        created_at_unix_ms: 0,
        updated_at_unix_ms: 0,
    };
    state.downloads.lock().await.upsert_servers(&[server]).unwrap();

    state.network_policy.set_profile(NetworkProfile::PublicEd2kPreview);
    state.network_policy.enable();

    let req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": "91.121.162.155:4242" })),
    );
    let resp = handle_request(&state, req, &mut authenticated).await.unwrap();
    assert!(resp.error.is_some(), "Must be rejected when server is disabled in DB");
    let err = resp.error.unwrap();
    assert!(err.message.contains("server is disabled"));
}

#[tokio::test]
async fn test_server_connect_nominal_loopback_and_kill_switch() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // Profile LocalLab allows 127.0.0.1
    state.network_policy.set_profile(NetworkProfile::LocalLab);
    state.network_policy.enable();

    // Start a mock eD2K TCP server on loopback
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = listener.local_addr().unwrap();

    let mock_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        // Expect OP_LOGINREQUEST (PR_ED2K = 0xE3, length u32, OP_LOGINREQUEST = 0x01)
        let mut header = [0u8; 6];
        socket.read_exact(&mut header).await.unwrap();
        assert_eq!(header[0], PR_ED2K);
        assert_eq!(header[5], OP_LOGINREQUEST);
        let payload_len = u32::from_le_bytes([header[1], header[2], header[3], header[4]]) as usize;
        let mut payload = vec![0u8; payload_len - 1]; // -1 for opcode already read
        socket.read_exact(&mut payload).await.unwrap();

        // 1. Send OP_SERVERMESSAGE ("Welcome Loopback Server!")
        let msg = "Welcome Loopback Server!";
        let mut msg_wire = Vec::new();
        msg_wire.push(PR_ED2K);
        let msg_payload_len = 1 + 2 + msg.len();
        msg_wire.extend_from_slice(&(msg_payload_len as u32).to_le_bytes());
        msg_wire.push(OP_SERVERMESSAGE);
        msg_wire.extend_from_slice(&(msg.len() as u16).to_le_bytes());
        msg_wire.extend_from_slice(msg.as_bytes());
        socket.write_all(&msg_wire).await.unwrap();

        // 2. Send OP_IDCHANGE (HighID: 25_000_000, flags: 1)
        let mut idc_wire = Vec::new();
        idc_wire.push(PR_ED2K);
        let idc_payload_len = 1 + 8;
        idc_wire.extend_from_slice(&(idc_payload_len as u32).to_le_bytes());
        idc_wire.push(OP_IDCHANGE);
        idc_wire.extend_from_slice(&25_000_000u32.to_le_bytes());
        idc_wire.extend_from_slice(&1u32.to_le_bytes());
        socket.write_all(&idc_wire).await.unwrap();

        // 3. Send OP_SERVERSTATUS (users: 1200, files: 45000)
        let mut status_wire = Vec::new();
        status_wire.push(PR_ED2K);
        let status_payload_len = 1 + 8;
        status_wire.extend_from_slice(&(status_payload_len as u32).to_le_bytes());
        status_wire.push(OP_SERVERSTATUS);
        status_wire.extend_from_slice(&1200u32.to_le_bytes());
        status_wire.extend_from_slice(&45000u32.to_le_bytes());
        socket.write_all(&status_wire).await.unwrap();

        // Keep socket open until client disconnects
        let mut buf = [0u8; 128];
        let _ = socket.read(&mut buf).await;
    });

    // 1. Connect via servers.connect
    let connect_req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": mock_addr.to_string() })),
    );
    let connect_resp = handle_request(&state, connect_req, &mut authenticated).await.unwrap();
    assert!(connect_resp.error.is_none(), "Connect must succeed: {:?}", connect_resp.error);

    // 2. Poll servers.status until connected with HighID
    let mut status_ok = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let status_req = make_req(2, "servers.status", None);
        let status_resp = handle_request(&state, status_req, &mut authenticated).await.unwrap();
        let res = status_resp.result.unwrap();
        if res["status"] == "connected" {
            assert_eq!(res["clientId"], 25_000_000);
            assert_eq!(res["isHighId"], true);
            assert_eq!(res["welcomeMessage"], "Welcome Loopback Server!");
            assert_eq!(res["currentUsers"], 1200);
            assert_eq!(res["currentFiles"], 45000);
            status_ok = true;
            break;
        }
    }
    assert!(status_ok, "Session must reach connected state with HighID and status");

    // 3. Test concurrent connection attempt rejected (max 1 connection)
    let concurrent_req = make_req(
        3,
        "servers.connect",
        Some(json!({ "address": mock_addr.to_string() })),
    );
    let concurrent_resp = handle_request(&state, concurrent_req, &mut authenticated).await.unwrap();
    assert!(concurrent_resp.error.is_some(), "Concurrent connection must be rejected");
    assert!(concurrent_resp.error.unwrap().message.contains("already in progress"));

    // 4. Test Kill Switch via network.disconnectAll
    let disc_all_req = make_req(4, "network.disconnectAll", None);
    let disc_resp = handle_request(&state, disc_all_req, &mut authenticated).await.unwrap();
    assert!(disc_resp.error.is_none());

    // Verify session status is now disconnected
    let check_req = make_req(5, "servers.status", None);
    let check_resp = handle_request(&state, check_req, &mut authenticated).await.unwrap();
    assert_eq!(check_resp.result.unwrap()["status"], "disconnected");

    mock_task.abort();
}
