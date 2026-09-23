// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use serde_json::{json, Value};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};

fn setup_test_state() -> (AppState, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "filter_test_token_123".to_string();

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
async fn test_filter_import_status_and_clear_rpc() {
    let (state, dir) = setup_test_state();
    let mut authenticated = false;

    // 1. Authenticate with system.hello
    let hello_req = make_req(
        1,
        "system.hello",
        Some(json!({
            "clientName": "test-client",
            "clientVersion": "0.1.0",
            "protocolVersion": 1,
            "token": "filter_test_token_123"
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
    assert!(caps.iter().any(|c| c.as_str() == Some("ip-filter")));
    assert!(caps.iter().any(|c| c.as_str() == Some("rate-limits")));

    // 2. Initial filter status -> empty
    let status_req = make_req(2, "filter.getStatus", None);
    let resp = handle_request(&state, status_req, &mut authenticated)
        .await
        .unwrap();
    let res = resp.result.unwrap();
    assert_eq!(res["range_count"], 0);
    assert_eq!(res["enabled"], true);

    // 3. Create a temporary ipfilter.dat file
    let filter_path = dir.path().join("ipfilter.dat");
    {
        let mut file = File::create(&filter_path).unwrap();
        writeln!(file, "# Test Header").unwrap();
        writeln!(file, "001.000.000.000 - 001.000.000.255 , 100 , Bad Host A").unwrap();
        writeln!(file, "001.000.001.000 - 001.000.001.255 , 100 , Bad Host B").unwrap();
        writeln!(
            file,
            "040.000.000.000 - 040.255.255.255 , 080 , Blocked ISP"
        )
        .unwrap();
    }

    // 4. Import file via filter.importFile
    let import_req = make_req(
        3,
        "filter.importFile",
        Some(json!({ "filePath": filter_path.to_str().unwrap() })),
    );
    let resp = handle_request(&state, import_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["success"], true);
    assert_eq!(res["report"]["valid_entries_parsed"], 3);
    assert_eq!(res["report"]["coalesced_count"], 1); // 2 adjacent ranges merged
    assert_eq!(res["report"]["final_range_count"], 2);

    // 5. Check filter stats
    let stats_req = make_req(4, "filter.getStats", None);
    let resp = handle_request(&state, stats_req, &mut authenticated)
        .await
        .unwrap();
    let res = resp.result.unwrap();
    assert_eq!(res["range_count"], 2);

    // 6. Disable filter
    let set_enabled_req = make_req(5, "filter.setEnabled", Some(json!({ "enabled": false })));
    let resp = handle_request(&state, set_enabled_req, &mut authenticated)
        .await
        .unwrap();
    assert_eq!(resp.result.unwrap()["enabled"], false);

    // 7. Clear filter
    let clear_req = make_req(6, "filter.clear", None);
    let resp = handle_request(&state, clear_req, &mut authenticated)
        .await
        .unwrap();
    assert_eq!(resp.result.unwrap()["range_count"], 0);
}

#[tokio::test]
async fn test_rate_limits_get_and_set_rpc() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // 1. Get initial limits
    let get_req = make_req(1, "limits.get", None);
    let resp = handle_request(&state, get_req, &mut authenticated)
        .await
        .unwrap();
    let res = resp.result.unwrap();
    assert_eq!(res["downloadBytesPerSec"], 0);
    assert_eq!(res["uploadBytesPerSec"], 0);
    assert_eq!(res["connectionsPerMinute"], 120);

    // 2. Set custom limits
    let set_req = make_req(
        2,
        "limits.set",
        Some(json!({
            "downloadBytesPerSec": 1024000,
            "uploadBytesPerSec": 512000,
            "connectionsPerMinute": 60,
            "serverRequestsPerSec": 5,
            "kadPacketsPerSec": 20
        })),
    );
    let resp = handle_request(&state, set_req, &mut authenticated)
        .await
        .unwrap();
    let res = resp.result.unwrap();
    assert_eq!(res["downloadBytesPerSec"], 1024000);
    assert_eq!(res["uploadBytesPerSec"], 512000);
    assert_eq!(res["connectionsPerMinute"], 60);
    assert_eq!(res["serverRequestsPerSec"], 5);
    assert_eq!(res["kadPacketsPerSec"], 20);
}

/// eMule `GetMaxDownloadInBytesPerSec` (« don't be a Lam3r :) ») : quand
/// la limite upload descend sous 20 Ko/s, le download *effectif* est
/// plafonné à coef×upload, mais la valeur *configurée* persistée reste
/// intacte — remonter l'upload restaure le download sans le re-saisir.
#[tokio::test]
async fn test_ud_ratio_caps_download_when_upload_low() {
    let (state, _dir) = setup_test_state();
    let mut authenticated = true;

    // Upload 5 Ko/s (coef 4) + download 1000 Ko/s → effectif 20 Ko/s,
    // configuré conservé.
    let set_req = make_req(
        1,
        "limits.set",
        Some(json!({
            "downloadBytesPerSec": 1_024_000,
            "uploadBytesPerSec": 5120
        })),
    );
    let resp = handle_request(&state, set_req, &mut authenticated)
        .await
        .unwrap();
    let res = resp.result.unwrap();
    assert_eq!(res["downloadBytesPerSec"], 20_480);
    assert_eq!(res["uploadBytesPerSec"], 5120);
    assert_eq!(res["downloadConfiguredBytesPerSec"], 1_024_000);
    assert_eq!(res["downloadRatioApplied"], true);

    // Remonter l'upload à 50 Ko/s lève le plafond sans re-saisir le
    // download configuré.
    let set_req = make_req(
        2,
        "limits.set",
        Some(json!({ "uploadBytesPerSec": 51_200 })),
    );
    let resp = handle_request(&state, set_req, &mut authenticated)
        .await
        .unwrap();
    let res = resp.result.unwrap();
    assert_eq!(res["downloadBytesPerSec"], 1_024_000);
    assert_eq!(res["downloadRatioApplied"], false);

    // Upload illimité (0) → pas de ratio.
    let set_req = make_req(3, "limits.set", Some(json!({ "uploadBytesPerSec": 0 })));
    let resp = handle_request(&state, set_req, &mut authenticated)
        .await
        .unwrap();
    assert_eq!(resp.result.unwrap()["downloadBytesPerSec"], 1_024_000);
}
