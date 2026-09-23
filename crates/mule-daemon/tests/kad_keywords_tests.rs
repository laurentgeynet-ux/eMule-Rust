// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use tempfile::tempdir;
use tokio::net::UdpSocket;

use mule_daemon::services::downloads::DownloadService;
use mule_daemon::state::AppState;
use mule_daemon::{DaemonConfig, WorkDirMode};
use mule_protocol::kad::*;

fn setup_test_state(tmp: &tempfile::TempDir) -> AppState {
    let data_dir = tmp.path().join("data");
    let dl_dir = tmp.path().join("downloads");
    std::fs::create_dir_all(&data_dir).unwrap();
    std::fs::create_dir_all(&dl_dir).unwrap();

    let db_path = data_dir.join("mule.db");
    let downloads = DownloadService::new(&db_path, data_dir.join("temp")).unwrap();
    let config = DaemonConfig::new(data_dir, "test-token")
        .with_bind_addr("127.0.0.1:0".parse().unwrap())
        .with_default_download_dir(dl_dir)
        .with_workdir_mode(WorkDirMode::ExplicitCli);

    AppState::new(config, downloads)
}

#[tokio::test]
async fn test_kad_keyword_publish_and_search_flow() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let tmp = tempdir().expect("create tempdir");
    let state = setup_test_state(&tmp);
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);

    // 1. Start daemon Kad UDP
    let _bound_addr = state
        .kad
        .udp()
        .start("127.0.0.1:0".parse().unwrap())
        .await
        .expect("start kad udp");

    // 2. Start mock Kad peer
    let mock_socket = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind mock peer");
    let mock_addr = mock_socket.local_addr().expect("mock addr");
    let file_hash = [0xAAu8; 16];
    let file_hash_hex = hex::encode(file_hash);

    let mock_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 1024];
        loop {
            let (len, from_addr) = match mock_socket.recv_from(&mut buf).await {
                Ok(res) => res,
                Err(_) => break,
            };
            let decoded = match KadCodec::decode(&buf[..len]) {
                Ok(d) => d,
                Err(_) => continue,
            };
            match decoded {
                KadMessage::BootstrapReq(b) => {
                    let contacts = vec![KadContactWire {
                        kad_id: [0x77u8; 16],
                        ip: [127, 0, 0, 1],
                        udp_port: mock_addr.port(),
                        tcp_port: 39735,
                        version: 2,
                    }];
                    let res =
                        KadCodec::encode_bootstrap_res(&b.kad_id, mock_addr.port(), 2, &contacts);
                    let _ = mock_socket.send_to(&res, from_addr).await;
                }
                KadMessage::Req(r) => {
                    let contacts = vec![KadContactWire {
                        kad_id: [0x77u8; 16],
                        ip: [127, 0, 0, 1],
                        udp_port: mock_addr.port(),
                        tcp_port: 39735,
                        version: 2,
                    }];
                    let res = KadCodec::encode_find_node_res(&r.target_id, &contacts);
                    let _ = mock_socket.send_to(&res, from_addr).await;
                }
                KadMessage::PublishKeywordReq(p) => {
                    let res = KadCodec::encode_publish_keyword_res(&p.keyword_hash, 0);
                    let _ = mock_socket.send_to(&res, from_addr).await;
                }
                KadMessage::SearchKeywordReq(s) => {
                    let entries = vec![KadKeywordEntryWire {
                        file_hash: [0xAAu8; 16],
                        file_name: "Ubuntu-22.04-live-server.iso".to_string(),
                        file_size: 1_400_000_000,
                        source_count: 15,
                    }];
                    let res = KadCodec::encode_search_keyword_res(&s.keyword_hash, &entries);
                    let _ = mock_socket.send_to(&res, from_addr).await;
                }
                _ => {}
            }
        }
    });

    // 3. Bootstrap from mock node
    let imported = state
        .kad
        .bootstrap_from_node(&mock_addr.ip().to_string(), mock_addr.port())
        .await
        .expect("bootstrap should succeed");
    // eMule 0.72a insère le nœud répondeur lui-même en plus des contacts de
    // BOOTSTRAP_RES → 2 contacts importés.
    assert_eq!(imported, 2);

    // 4. Publish keyword for a file
    let published_count = state
        .kad
        .publish_keyword(
            &file_hash_hex,
            "Ubuntu-22.04-live-server.iso",
            1_400_000_000,
        )
        .await
        .expect("publish keyword should succeed");
    assert!(published_count >= 1);

    // 5. Search keyword
    let results = state
        .kad
        .search_keywords("ubuntu", None, None)
        .await
        .expect("search keyword should succeed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["fileHash"], file_hash_hex);
    assert_eq!(results[0]["fileName"], "Ubuntu-22.04-live-server.iso");
    assert_eq!(results[0]["fileSize"], 1_400_000_000);
    assert_eq!(results[0]["sourceCount"], 15);

    mock_task.abort();
}
