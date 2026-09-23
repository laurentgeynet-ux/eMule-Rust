// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use tempfile::tempdir;
use tokio::net::UdpSocket;

use mule_core::kad::KadId;
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
async fn test_kad_bootstrap_and_find_node_flow() {
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

    // 2. Start mock bootstrap peer
    let mock_socket = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind mock peer");
    let mock_addr = mock_socket.local_addr().expect("mock addr");

    let mock_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 1024];
        loop {
            let (len, from_addr) = mock_socket.recv_from(&mut buf).await.expect("recv");
            let decoded = KadCodec::decode(&buf[..len]).expect("decode");
            match decoded {
                KadMessage::BootstrapReq(b) => {
                    let contacts = vec![KadContactWire {
                        kad_id: [0x11u8; 16],
                        ip: [127, 0, 0, 1],
                        udp_port: mock_addr.port(), // point to mock node so find_node reaches it
                        tcp_port: 0,
                        version: 2,
                    }];
                    let res = KadCodec::encode_bootstrap_res(
                        &b.kad_id,
                        mock_socket.local_addr().unwrap().port(),
                        2,
                        &contacts,
                    );
                    mock_socket
                        .send_to(&res, from_addr)
                        .await
                        .expect("send boot res");
                }
                KadMessage::Req(r) => {
                    let contacts = vec![KadContactWire {
                        kad_id: r.target_id,
                        ip: [127, 0, 0, 1],
                        udp_port: mock_addr.port(),
                        tcp_port: 0,
                        version: 2,
                    }];
                    let res = KadCodec::encode_bootstrap_res(
                        &r.sender_id,
                        mock_socket.local_addr().unwrap().port(),
                        2,
                        &contacts,
                    );
                    mock_socket
                        .send_to(&res, from_addr)
                        .await
                        .expect("send find res");
                    break;
                }
                _ => {}
            }
        }
    });

    // 3. Trigger bootstrap
    let imported = state
        .kad
        .bootstrap_from_node(&mock_addr.ip().to_string(), mock_addr.port())
        .await
        .expect("bootstrap should succeed");

    // eMule 0.72a insère le nœud répondeur lui-même dans la routing table en
    // plus des contacts listés dans BOOTSTRAP_RES → 2 contacts importés.
    assert_eq!(imported, 2);

    let status = state.kad.get_status().await;
    assert_eq!(status.total_contacts, 2);
    assert_eq!(status.db_contacts_count, 2);

    // 4. Trigger Find Node
    let target_key = KadId([0x99u8; 16]);
    let closest = state
        .kad
        .find_node(&target_key)
        .await
        .expect("find node succeeds");

    assert!(!closest.is_empty());
    mock_task.await.expect("mock finishes");
}

/// eMule `CKademlia::Process` + `s_liBootstrapList` : tant que la table de
/// routage est vide, la boucle de drain retente les candidats appris via
/// les handshakes eD2K (~2 s entre essais). Avant ce mécanisme, un timeout
/// sur le premier candidat laissait Kad mort avec 0 contact.
#[tokio::test]
async fn test_bootstrap_drain_retries_candidates_until_success() {
    use std::net::Ipv4Addr;
    use std::time::{Duration, Instant};

    let tmp = tempdir().expect("create tempdir");
    let state = setup_test_state(&tmp);
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);

    let _bound_addr = state
        .kad
        .udp()
        .start("127.0.0.1:0".parse().unwrap())
        .await
        .expect("start kad udp");

    // Candidat A : socket ouvert mais ne répond jamais → timeout 3 s.
    let dead_socket = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind dead peer");
    let dead_addr = dead_socket.local_addr().unwrap();
    let _dead_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 256];
        loop {
            let _ = dead_socket.recv_from(&mut buf).await;
        }
    });

    // Candidat B : répond un BootstrapRes avec un contact.
    let live_socket = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind live peer");
    let live_addr = live_socket.local_addr().unwrap();
    let live_port = live_addr.port();
    let _live_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 1024];
        loop {
            let (len, from_addr) = live_socket.recv_from(&mut buf).await.expect("recv");
            if let Ok(KadMessage::BootstrapReq(b)) = KadCodec::decode(&buf[..len]) {
                let contacts = vec![KadContactWire {
                    kad_id: [0x22u8; 16],
                    ip: [127, 0, 0, 1],
                    udp_port: live_port,
                    tcp_port: 0,
                    version: 2,
                }];
                let res = KadCodec::encode_bootstrap_res(&b.kad_id, live_port, 2, &contacts);
                let _ = live_socket.send_to(&res, from_addr).await;
            }
        }
    });

    // Enregistre A : envoi immédiat → timeout (~3 s), A reste en file.
    state
        .kad
        .maybe_bootstrap_from_peer(Ipv4Addr::LOCALHOST, dead_addr.port(), false, 8)
        .await;
    assert_eq!(state.kad.pending_bootstrap_candidates().await, 1);

    // Enregistre B : en file, envoi immédiat bloqué par le throttle 10 s.
    state
        .kad
        .maybe_bootstrap_from_peer(Ipv4Addr::LOCALHOST, live_addr.port(), false, 8)
        .await;
    assert_eq!(state.kad.pending_bootstrap_candidates().await, 2);

    // Un pair Kad v1 (pré-0.47a) n'est jamais candidat.
    state
        .kad
        .maybe_bootstrap_from_peer(Ipv4Addr::LOCALHOST, dead_addr.port(), false, 1)
        .await;
    assert_eq!(state.kad.pending_bootstrap_candidates().await, 2);

    // La maintenance draine la file : A retenté (timeout) puis B → succès.
    let cancel = state.network_policy.cancellation_token();
    let _maintenance = state.kad.start_background_maintenance(cancel.clone());

    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let status = state.kad.get_status().await;
        if status.total_contacts > 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "le drain n'a jamais peuplé la table de routage"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    cancel.cancel();
}
