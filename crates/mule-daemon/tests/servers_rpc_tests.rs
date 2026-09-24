// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde_json::json;
use std::fs::File;
use std::net::Ipv4Addr;

use mule_daemon::rpc::methods::handle_request;
use mule_format::{ServerEntry, ServerMetFile, Tag};
use mule_test_support::{make_req, rpc_state};

#[tokio::test]
async fn test_servers_rpc_workflow() {
    let (state, dir) = rpc_state("servers_test_token_123");
    let mut authenticated = false;

    // 1. Authenticate with system.hello
    let hello_req = make_req(
        1,
        "system.hello",
        Some(json!({
            "clientName": "test-client",
            "clientVersion": "0.1.0",
            "protocolVersion": 1,
            "token": "servers_test_token_123"
        })),
    );
    let resp = handle_request(&state, hello_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let caps = resp.result.unwrap()["capabilities"]
        .as_array()
        .unwrap()
        .clone();
    assert!(caps.iter().any(|c| c.as_str() == Some("servers")));

    // 2. Load an ipfilter rule into the daemon to verify cross-check
    // Block 10.0.0.0 - 10.255.255.255
    let ipfilter_path = dir.path().join("ipfilter.dat");
    std::fs::write(
        &ipfilter_path,
        "010.000.000.000 - 010.255.255.255 , 100 , Malicious Range\n",
    )
    .unwrap();
    let import_filter_req = make_req(
        2,
        "filter.importFile",
        Some(json!({ "filePath": ipfilter_path.to_str().unwrap() })),
    );
    let resp = handle_request(&state, import_filter_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    // 3. Create a test server.met with 2 servers:
    // Server 1: 192.168.1.50:4661 (Allowed)
    // Server 2: 10.1.2.3:4242 (Blocked by filter)
    let s1 = ServerEntry {
        ip: Ipv4Addr::new(192, 168, 1, 50),
        port: 4661,
        name: "Allowed Donkey".to_string(),
        description: "Fast server".to_string(),
        ping: 20,
        preference: 5,
        failed_count: 0,
        dyn_ip: None,
        version: Some("17.15".to_string()),
        tags: vec![
            Tag::new_id(
                mule_format::SN_NAME,
                mule_format::MetTagValue::String("Allowed Donkey".to_string()),
            ),
            Tag::new_id(
                mule_format::SN_DESCRIPTION,
                mule_format::MetTagValue::String("Fast server".to_string()),
            ),
        ],
    };
    let s2 = ServerEntry {
        ip: Ipv4Addr::new(10, 1, 2, 3),
        port: 4242,
        name: "Rogue Donkey".to_string(),
        description: "Dangerous server".to_string(),
        ping: 300,
        preference: 1,
        failed_count: 5,
        dyn_ip: None,
        version: None,
        tags: vec![Tag::new_id(
            mule_format::SN_NAME,
            mule_format::MetTagValue::String("Rogue Donkey".to_string()),
        )],
    };

    let met_file = ServerMetFile {
        version: mule_format::SERVER_MET_VERSION,
        servers: vec![s1, s2],
    };

    let server_met_path = dir.path().join("test_server.met");
    {
        let mut out = File::create(&server_met_path).unwrap();
        met_file.write_to(&mut out).unwrap();
    }

    // 4. Test servers.inspectMet (Preview without persisting)
    let inspect_req = make_req(
        3,
        "servers.inspectMet",
        Some(json!({ "filePath": server_met_path.to_str().unwrap() })),
    );
    let resp = handle_request(&state, inspect_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let inspect_res = resp.result.unwrap();
    assert_eq!(inspect_res["totalCount"], 2);
    assert_eq!(inspect_res["allowedCount"], 1);
    assert_eq!(inspect_res["blockedCount"], 1);

    // Verify servers in SQLite is still 0
    let list_req = make_req(4, "servers.list", None);
    let resp = handle_request(&state, list_req, &mut authenticated)
        .await
        .unwrap();
    assert_eq!(resp.result.unwrap()["total"], 0);

    // 5. Test servers.importMet (Persist)
    let import_req = make_req(
        5,
        "servers.importMet",
        Some(json!({ "filePath": server_met_path.to_str().unwrap() })),
    );
    let resp = handle_request(&state, import_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let import_res = resp.result.unwrap();
    assert_eq!(import_res["imported"], 2);
    assert_eq!(import_res["allowedCount"], 1);
    assert_eq!(import_res["blockedCount"], 1);

    // 6. Test servers.list (Should return 2 servers with correct filter_status)
    let list_req = make_req(6, "servers.list", None);
    let resp = handle_request(&state, list_req, &mut authenticated)
        .await
        .unwrap();
    let list_res = resp.result.unwrap();
    assert_eq!(list_res["total"], 2);
    let servers = list_res["servers"].as_array().unwrap();

    let s_allowed = servers
        .iter()
        .find(|s| s["address"] == "192.168.1.50:4661")
        .unwrap();
    assert_eq!(s_allowed["filterStatus"], "allowed");
    let s_blocked = servers
        .iter()
        .find(|s| s["address"] == "10.1.2.3:4242")
        .unwrap();
    assert_eq!(s_blocked["filterStatus"], "blocked_by_filter");

    // 7. Test servers.setEnabled (toggle disabled)
    let disable_req = make_req(
        7,
        "servers.setEnabled",
        Some(json!({
            "address": "192.168.1.50:4661",
            "enabled": false,
        })),
    );
    let resp = handle_request(&state, disable_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    let list_req = make_req(8, "servers.list", None);
    let resp = handle_request(&state, list_req, &mut authenticated)
        .await
        .unwrap();
    let s_disabled = resp.result.unwrap()["servers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["address"] == "192.168.1.50:4661")
        .unwrap()
        .clone();
    assert_eq!(s_disabled["filterStatus"], "disabled");

    // 8. Test servers.delete
    let del_req = make_req(
        9,
        "servers.delete",
        Some(json!({ "address": "10.1.2.3:4242" })),
    );
    let resp = handle_request(&state, del_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    let list_req = make_req(10, "servers.list", None);
    let resp = handle_request(&state, list_req, &mut authenticated)
        .await
        .unwrap();
    assert_eq!(resp.result.unwrap()["total"], 1);

    // 9. Test servers.clear
    let clear_req = make_req(11, "servers.clear", None);
    let resp = handle_request(&state, clear_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    let list_req = make_req(12, "servers.list", None);
    let resp = handle_request(&state, list_req, &mut authenticated)
        .await
        .unwrap();
    assert_eq!(resp.result.unwrap()["total"], 0);
}
