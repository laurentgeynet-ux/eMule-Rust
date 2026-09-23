// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use serde_json::{json, Value};
use std::time::Duration;
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};
use mule_network_policy::NetworkProfile;
use mule_protocol::ed2k::{
    OP_FOUNDSOURCES, OP_GETSOURCES, OP_GETSOURCES_OBFU, OP_IDCHANGE, OP_LOGINREQUEST,
    OP_SEARCHREQUEST, OP_SEARCH_RESULT, OP_SERVERMESSAGE, OP_SERVERSTATUS,
};
use mule_protocol::PR_ED2K;

fn setup_test_state() -> (AppState, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "test_token_conn".to_string();

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
async fn test_server_connect_refused_under_local_lab_profile() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // Explicitly test LocalLab profile rejects public WAN address
    state.network_policy.set_profile(NetworkProfile::LocalLab);
    let req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": "91.121.162.155:4242" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_some(),
        "Must be rejected under LocalLab profile"
    );
    let err = resp.error.unwrap();
    assert!(err.message.contains("rejected by network policy"));
}

#[tokio::test]
async fn test_server_connect_refused_when_network_disabled() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // Switch to PublicEd2kPreview
    state
        .network_policy
        .set_profile(NetworkProfile::PublicEd2kPreview);
    // Disconnect all / network is disabled by default until explicitly enabled
    state.network_policy.disconnect_all();

    let req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": "91.121.162.155:4242" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_some(),
        "Must be rejected when network is disabled"
    );
    let err = resp.error.unwrap();
    assert!(err.message.contains("rejected by network policy"));
}

#[tokio::test]
async fn test_server_connect_refused_when_ip_filtered() {
    let (state, dir) = setup_test_state();
    let mut authenticated = true;

    // Load filter blocking 91.121.0.0 - 91.121.255.255
    let filter_file = dir.path().join("ipfilter.dat");
    std::fs::write(
        &filter_file,
        "091.121.000.000 - 091.121.255.255 , 100 , Blocked Server Range\n",
    )
    .unwrap();
    state
        .network_policy
        .ip_filter()
        .load_from_file(&filter_file)
        .unwrap();

    state
        .network_policy
        .set_profile(NetworkProfile::PublicEd2kPreview);
    state.network_policy.enable();

    let req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": "91.121.162.155:4242" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_some(), "Must be rejected by IP filter");
    let err = resp.error.unwrap();
    assert!(
        err.message.contains("rejected by network policy")
            || err.message.contains("blocked by IP filter")
    );
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
    state
        .downloads
        .lock()
        .await
        .upsert_servers(&[server])
        .unwrap();

    state
        .network_policy
        .set_profile(NetworkProfile::PublicEd2kPreview);
    state.network_policy.enable();

    let req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": "91.121.162.155:4242" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_some(),
        "Must be rejected when server is disabled in DB"
    );
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
    // The mock server speaks plaintext eD2K only — disable the obfuscated
    // handshake attempt (which the daemon otherwise tries first).
    state.server_session.set_obfuscation_preference(false, 0);

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

        // Keep socket open until client disconnects — must loop: the daemon
        // sends OP_OFFERFILES right after connect (auto-publish), so a single
        // read would return early and drop the socket.
        let mut buf = [0u8; 4096];
        loop {
            match socket.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });

    // 1. Connect via servers.connect
    let connect_req = make_req(
        1,
        "servers.connect",
        Some(json!({ "address": mock_addr.to_string() })),
    );
    let connect_resp = handle_request(&state, connect_req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        connect_resp.error.is_none(),
        "Connect must succeed: {:?}",
        connect_resp.error
    );

    // 2. Poll servers.status until connected with HighID
    let mut status_ok = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let status_req = make_req(2, "servers.status", None);
        let status_resp = handle_request(&state, status_req, &mut authenticated)
            .await
            .unwrap();
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
    assert!(
        status_ok,
        "Session must reach connected state with HighID and status"
    );

    // 3. Reconnecting to the SAME address is idempotent (eMule semantics:
    // no second session is spawned — the live one is returned).
    let concurrent_req = make_req(
        3,
        "servers.connect",
        Some(json!({ "address": mock_addr.to_string() })),
    );
    let concurrent_resp = handle_request(&state, concurrent_req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        concurrent_resp.error.is_none(),
        "Same-address connect must be idempotent: {:?}",
        concurrent_resp.error
    );
    assert_eq!(
        concurrent_resp.result.unwrap()["status"],
        "connected"
    );

    // 4. Test Kill Switch via network.disconnectAll
    let disc_all_req = make_req(4, "network.disconnectAll", None);
    let disc_resp = handle_request(&state, disc_all_req, &mut authenticated)
        .await
        .unwrap();
    assert!(disc_resp.error.is_none());

    // Verify session status is now disconnected
    let check_req = make_req(5, "servers.status", None);
    let check_resp = handle_request(&state, check_req, &mut authenticated)
        .await
        .unwrap();
    assert_eq!(check_resp.result.unwrap()["status"], "disconnected");

    mock_task.abort();
}

#[tokio::test]
async fn test_server_search_and_query_sources() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // 1. Unconnected calls must be rejected
    let search_req = make_req(1, "servers.search", Some(json!({ "term": "linux" })));
    let search_resp = handle_request(&state, search_req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        search_resp.error.is_some(),
        "Search while disconnected must fail"
    );

    let sources_req = make_req(
        2,
        "servers.querySources",
        Some(json!({
            "fileHash": "0102030405060708090a0b0c0d0e0f10",
            "fileSize": 5000
        })),
    );
    let sources_resp = handle_request(&state, sources_req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        sources_resp.error.is_some(),
        "Query sources while disconnected must fail"
    );

    // 2. Setup mock server on 127.0.0.1
    state.network_policy.enable();
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);
    // The mock server speaks plaintext eD2K only — disable the obfuscated
    // handshake attempt (which the daemon otherwise tries first).
    state.server_session.set_obfuscation_preference(false, 0);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = listener.local_addr().unwrap();

    let mut event_rx = state.event_tx.subscribe();

    let mock_task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        // A. Read OP_LOGINREQUEST
        let mut proto_hdr = [0u8; 6];
        socket.read_exact(&mut proto_hdr).await.unwrap();
        let payload_len = u32::from_le_bytes(proto_hdr[1..5].try_into().unwrap()) as usize - 1;
        let mut login_payload = vec![0u8; payload_len];
        socket.read_exact(&mut login_payload).await.unwrap();

        // Send OP_IDCHANGE (HighID)
        let mut id_pkt = Vec::new();
        id_pkt.push(PR_ED2K);
        id_pkt.extend_from_slice(&(9u32).to_le_bytes()); // 1 opcode + 8 payload
        id_pkt.push(OP_IDCHANGE);
        id_pkt.extend_from_slice(&25_000_000u32.to_le_bytes());
        id_pkt.extend_from_slice(&1u32.to_le_bytes());
        socket.write_all(&id_pkt).await.unwrap();

        // B. Read OP_SEARCHREQUEST — skip the automatic OP_OFFERFILES the
        // daemon publishes right after OP_IDCHANGE (shared-files list).
        loop {
            let mut hdr = [0u8; 6];
            socket.read_exact(&mut hdr).await.unwrap();
            assert_eq!(hdr[0], PR_ED2K);
            let payload_len = u32::from_le_bytes(hdr[1..5].try_into().unwrap()) as usize - 1;
            let mut payload = vec![0u8; payload_len];
            socket.read_exact(&mut payload).await.unwrap();
            if hdr[5] == OP_SEARCHREQUEST {
                break;
            }
        }

        // Send OP_SEARCH_RESULT (1 file)
        let mut res_payload = Vec::new();
        res_payload.extend_from_slice(&1u32.to_le_bytes()); // 1 file
        let file_hash = [0xabu8; 16];
        res_payload.extend_from_slice(&file_hash);
        res_payload.extend_from_slice(&100u32.to_le_bytes()); // client_id
        res_payload.extend_from_slice(&39735u16.to_le_bytes()); // client_port
        res_payload.extend_from_slice(&2u32.to_le_bytes()); // 2 tags

        // Tag 1: filename
        let fname = "test_file.iso";
        res_payload.push(0x02); // TAGTYPE_STRING
        res_payload.extend_from_slice(&1u16.to_le_bytes());
        res_payload.push(mule_protocol::ed2k::TAG_SPECIAL_NAME);
        res_payload.extend_from_slice(&(fname.len() as u16).to_le_bytes());
        res_payload.extend_from_slice(fname.as_bytes());

        // Tag 2: filesize
        res_payload.push(0x03); // TAGTYPE_UINT32
        res_payload.extend_from_slice(&1u16.to_le_bytes());
        res_payload.push(mule_protocol::ed2k::TAG_SPECIAL_FILESIZE);
        res_payload.extend_from_slice(&123456u32.to_le_bytes());

        let mut res_pkt = Vec::new();
        res_pkt.push(PR_ED2K);
        res_pkt.extend_from_slice(&((res_payload.len() + 1) as u32).to_le_bytes());
        res_pkt.push(OP_SEARCH_RESULT);
        res_pkt.extend_from_slice(&res_payload);
        socket.write_all(&res_pkt).await.unwrap();

        // C. Read OP_GETSOURCES or OP_GETSOURCES_OBFU — same skip loop for
        // any interleaved housekeeping packet.
        loop {
            let mut hdr = [0u8; 6];
            socket.read_exact(&mut hdr).await.unwrap();
            assert_eq!(hdr[0], PR_ED2K);
            let payload_len = u32::from_le_bytes(hdr[1..5].try_into().unwrap()) as usize - 1;
            let mut payload = vec![0u8; payload_len];
            socket.read_exact(&mut payload).await.unwrap();
            if hdr[5] == OP_GETSOURCES || hdr[5] == OP_GETSOURCES_OBFU {
                break;
            }
        }

        // Send OP_FOUNDSOURCES (1 source)
        let mut found_payload = Vec::new();
        found_payload.extend_from_slice(&[0xabu8; 16]); // file_hash
        found_payload.push(1u8); // 1 source
        let ip_val: u32 = 127 | (1 << 24);
        found_payload.extend_from_slice(&ip_val.to_le_bytes());
        found_payload.extend_from_slice(&39735u16.to_le_bytes());

        let mut found_pkt = Vec::new();
        found_pkt.push(PR_ED2K);
        found_pkt.extend_from_slice(&((found_payload.len() + 1) as u32).to_le_bytes());
        found_pkt.push(OP_FOUNDSOURCES);
        found_pkt.extend_from_slice(&found_payload);
        socket.write_all(&found_pkt).await.unwrap();

        // Keep the socket open until the client disconnects.
        let mut buf = [0u8; 4096];
        loop {
            match socket.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });

    // Connect
    let connect_req = make_req(
        3,
        "servers.connect",
        Some(json!({ "address": mock_addr.to_string() })),
    );
    let connect_resp = handle_request(&state, connect_req, &mut authenticated)
        .await
        .unwrap();
    assert!(connect_resp.error.is_none());

    // Wait for connected
    let mut is_connected = false;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let st_resp = handle_request(
            &state,
            make_req(4, "servers.status", None),
            &mut authenticated,
        )
        .await
        .unwrap();
        if st_resp.result.unwrap()["status"] == "connected" {
            is_connected = true;
            break;
        }
    }
    assert!(is_connected, "Session must reach connected state");

    // Perform search
    let search_req = make_req(5, "servers.search", Some(json!({ "term": "test" })));
    let search_resp = handle_request(&state, search_req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        search_resp.error.is_none(),
        "Search request sent successfully"
    );
    assert_eq!(search_resp.result.unwrap()["sent"], true);

    // Perform get sources
    let qsrc_req = make_req(
        6,
        "servers.querySources",
        Some(json!({
            "fileHash": "abababababababababababababababab",
            "fileSize": 123456
        })),
    );
    let qsrc_resp = handle_request(&state, qsrc_req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        qsrc_resp.error.is_none(),
        "Query sources request sent successfully"
    );
    assert_eq!(qsrc_resp.result.unwrap()["sent"], true);

    // Verify events were emitted
    let mut received_search_result = false;
    let mut received_found_sources = false;
    for _ in 0..20 {
        if let Ok(evt_str) = tokio::time::timeout(Duration::from_millis(100), event_rx.recv()).await
        {
            if let Ok(val) = serde_json::from_str::<Value>(&evt_str.unwrap_or_default()) {
                if val["method"] == "server.searchResult" {
                    received_search_result = true;
                    let files = val["params"]["files"].as_array().unwrap();
                    assert_eq!(files.len(), 1);
                    assert_eq!(files[0]["fileName"], "test_file.iso");
                    assert_eq!(files[0]["fileSize"], 123456);
                }
                if val["method"] == "server.foundSources" {
                    received_found_sources = true;
                    assert_eq!(
                        val["params"]["fileHash"],
                        "abababababababababababababababab"
                    );
                    let sources = val["params"]["sources"].as_array().unwrap();
                    assert_eq!(sources.len(), 1);
                    assert_eq!(sources[0]["address"], "127.0.0.1:39735");
                }
            }
        }
        if received_search_result && received_found_sources {
            break;
        }
    }

    assert!(
        received_search_result,
        "Must receive server.searchResult notification"
    );
    assert!(
        received_found_sources,
        "Must receive server.foundSources notification"
    );

    mock_task.abort();
}
