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
