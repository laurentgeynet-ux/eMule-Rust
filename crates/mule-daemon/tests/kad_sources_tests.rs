use tempfile::tempdir;
use tokio::net::UdpSocket;

use mule_core::model::PeerSourceOrigin;
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
async fn test_kad_source_publish_and_search_flow() {
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
    let target_hash = [0x55u8; 16];
    let target_hash_hex = hex::encode(target_hash);

    let mock_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 1024];
        loop {
            let (len, from_addr) = mock_socket.recv_from(&mut buf).await.expect("recv");
            let decoded = KadCodec::decode(&buf[..len]).expect("decode");
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
                    mock_socket
                        .send_to(&res, from_addr)
                        .await
                        .expect("send boot res");
                }
                KadMessage::PublishSourceReq(p) => {
                    let res = KadCodec::encode_publish_source_res(&p.target_file_hash, 0);
                    mock_socket
                        .send_to(&res, from_addr)
                        .await
                        .expect("send pub res");
                }
                KadMessage::SearchSourceReq(s) => {
                    let sources = vec![KadSourceWire {
                        kad_id: [0x77u8; 16],
                        ip: [127, 0, 0, 1],
                        udp_port: mock_addr.port(),
                        tcp_port: 39735,
                        encryption: 0,
                        buddy_id: None,
                    }];
                    let res = KadCodec::encode_search_source_res(&s.target_file_hash, &sources);
                    mock_socket
                        .send_to(&res, from_addr)
                        .await
                        .expect("send search res");
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

    // 4. Publish a source
    let published = state
        .kad
        .publish_source(&target_hash_hex, 10_485_760, 39735)
        .await
        .expect("publish should succeed");
    assert!(published >= 1);

    // 5. Search sources for target hash
    let discovered = state
        .kad
        .search_sources(&target_hash_hex, 10_485_760)
        .await
        .expect("search should succeed");

    assert_eq!(discovered.len(), 1);
    assert_eq!(discovered[0].origin, PeerSourceOrigin::Kad);
    assert_eq!(discovered[0].port, 39735);
    assert_eq!(discovered[0].file_hash, target_hash_hex);

    // 6. Verify peer sources in DownloadService
    let dl = state.downloads.lock().await;
    let list = dl
        .list_peer_sources(&target_hash_hex)
        .expect("list peer sources");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].origin, PeerSourceOrigin::Kad);

    mock_task.abort();
}
