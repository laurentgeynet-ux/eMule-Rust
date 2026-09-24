// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use tempfile::tempdir;
use tokio::net::UdpSocket;

use mule_protocol::kad::*;
use mule_test_support::kad_state;

#[tokio::test]
async fn test_kad_keyword_publish_and_search_flow() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let tmp = tempdir().expect("create tempdir");
    let state = kad_state(&tmp);
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
                        tags: Vec::new(),
                        aich_pub: None,
                        aich_votes: Vec::new(),
                        publish_info: None,
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
            None,
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

/// eMule `PreparePacketForTags` (`Search.cpp:1390`): `TAG_KADAICHHASHPUB`
/// is only sent to nodes whose Kad version is >= 9
/// (`KADEMLIA_VERSION9_50a`). The mock peer advertises its version in
/// bootstrap contacts and records the published entries.
#[tokio::test]
async fn test_kad_aich_pub_gated_on_target_version() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    let tmp = tempdir().expect("create tempdir");
    let state = kad_state(&tmp);
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);

    let _bound_addr = state
        .kad
        .udp()
        .start("127.0.0.1:0".parse().unwrap())
        .await
        .expect("start kad udp");

    let aich_root = [0x42u8; 20];
    let file_hash = [0xBBu8; 16];
    let file_hash_hex = hex::encode(file_hash);

    for contact_version in [8u8, 10u8] {
        let mock_socket = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("bind mock peer");
        let mock_addr = mock_socket.local_addr().expect("mock addr");
        let (aich_seen_tx, mut aich_seen_rx) =
            tokio::sync::mpsc::channel::<Option<[u8; 20]>>(4);

        let mock_task = tokio::spawn(async move {
            let mut buf = vec![0u8; 2048];
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
                            version: contact_version,
                        }];
                        let res = KadCodec::encode_bootstrap_res(
                            &b.kad_id,
                            mock_addr.port(),
                            contact_version,
                            &contacts,
                        );
                        let _ = mock_socket.send_to(&res, from_addr).await;
                    }
                    KadMessage::Req(r) => {
                        let contacts = vec![KadContactWire {
                            kad_id: [0x77u8; 16],
                            ip: [127, 0, 0, 1],
                            udp_port: mock_addr.port(),
                            tcp_port: 39735,
                            version: contact_version,
                        }];
                        let res = KadCodec::encode_find_node_res(&r.target_id, &contacts);
                        let _ = mock_socket.send_to(&res, from_addr).await;
                    }
                    KadMessage::PublishKeywordReq(p) => {
                        for e in &p.entries {
                            let _ = aich_seen_tx.send(e.aich_pub).await;
                        }
                        let res = KadCodec::encode_publish_keyword_res(&p.keyword_hash, 0);
                        let _ = mock_socket.send_to(&res, from_addr).await;
                    }
                    _ => {}
                }
            }
        });

        let imported = state
            .kad
            .bootstrap_from_node(&mock_addr.ip().to_string(), mock_addr.port())
            .await
            .expect("bootstrap should succeed");
        assert!(imported >= 1);

        let _ = state
            .kad
            .publish_keyword(&file_hash_hex, "aich-test-file.bin", 1234, Some(aich_root))
            .await
            .expect("publish keyword");

        // Collect published entries for a bounded window.
        let mut observed: Vec<Option<[u8; 20]>> = Vec::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);
        while let Ok(Some(v)) = tokio::time::timeout_at(deadline, aich_seen_rx.recv()).await {
            observed.push(v);
        }
        mock_task.abort();
        assert!(
            !observed.is_empty(),
            "mock peer received no publish request"
        );
        if contact_version >= KADEMLIA_VERSION_AICH {
            assert!(
                observed.contains(&Some(aich_root)),
                "v{contact_version} target must receive TAG_KADAICHHASHPUB"
            );
        } else {
            assert!(
                observed.iter().all(|o| o.is_none()),
                "v{contact_version} target must NOT receive TAG_KADAICHHASHPUB"
            );
        }
    }
}
