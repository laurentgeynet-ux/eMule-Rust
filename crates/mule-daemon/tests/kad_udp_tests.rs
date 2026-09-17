use tempfile::tempdir;
use tokio::net::UdpSocket;

use mule_core::kad::KadId;
use mule_daemon::{DaemonConfig, WorkDirMode};
use mule_daemon::services::downloads::DownloadService;
use mule_daemon::state::AppState;
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
async fn test_kad_udp_nominal_ping_pong_and_contact_verification() {
    let tmp = tempdir().expect("create tempdir");
    let state = setup_test_state(&tmp);
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);

    // 1. Start Kad UDP on loopback ephemeral port
    let bound_addr = state
        .kad
        .udp()
        .start("127.0.0.1:0".parse().unwrap())
        .await
        .expect("start kad udp");

    assert!(bound_addr.port() > 0);

    // 2. Start a mock Kad peer socket on loopback
    let mock_peer_socket = UdpSocket::bind("127.0.0.1:0").await.expect("bind mock peer");
    let mock_peer_addr = mock_peer_socket.local_addr().expect("mock addr");

    // Spawn mock peer answering with Pong
    let mock_task = tokio::spawn(async move {
        let mut buf = vec![0u8; 1024];
        let (len, from_addr) = mock_peer_socket.recv_from(&mut buf).await.expect("recv");
        let decoded = KadCodec::decode(&buf[..len]).expect("decode mock ping");
        match decoded {
            KadMessage::Ping(_) => {
                let pong = KadCodec::encode_pong(from_addr.port());
                mock_peer_socket.send_to(&pong, from_addr).await.expect("send pong");
            }
            _ => panic!("Expected Ping"),
        }
    });

    // 3. Ping the mock peer from daemon
    let target_kad_id = KadId::random();
    let latency_ms = state
        .kad
        .ping_contact(
            &mock_peer_addr.ip().to_string(),
            mock_peer_addr.port(),
            Some(&target_kad_id.to_hex()),
        )
        .await
        .expect("ping should succeed");

    assert!(latency_ms < 2000);
    mock_task.await.expect("mock peer task finishes");

    // 4. Verify contact is marked as verified in status & list
    let status = state.kad.get_status().await;
    assert_eq!(status.packets_sent, 1);
    assert_eq!(status.packets_received, 1);
    assert_eq!(status.total_contacts, 1);

    let contacts = state.kad.list_contacts(10).await;
    assert_eq!(contacts.len(), 1);
    assert_eq!(contacts[0].kad_id, target_kad_id.to_hex());
    assert!(contacts[0].verified);
    assert_eq!(contacts[0].fail_count, 0);
}

#[tokio::test]
async fn test_kad_udp_timeout_and_transaction_cleanup() {
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

    // Bind mock peer that ignores incoming packets
    let mock_peer_socket = UdpSocket::bind("127.0.0.1:0").await.expect("bind mock peer");
    let mock_peer_addr = mock_peer_socket.local_addr().expect("mock addr");

    // Ping non-responding peer
    let err = state
        .kad
        .ping_contact(&mock_peer_addr.ip().to_string(), mock_peer_addr.port(), None)
        .await
        .unwrap_err();

    assert!(err.contains("timed out"));

    let snap = state.kad.udp().get_snapshot().await;
    assert_eq!(snap.active_transactions, 0);
}

#[tokio::test]
async fn test_kad_udp_kill_switch_teardown() {
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

    // Trigger Kill switch
    state.network_policy.disconnect_all();

    // Outbound ping must immediately fail under policy
    let err = state
        .kad
        .ping_contact("127.0.0.1", 39737, None)
        .await
        .unwrap_err();

    assert!(err.contains("disabled") || err.contains("stopped") || err.contains("Policy denied"));

    let snap = state.kad.udp().get_snapshot().await;
    assert_eq!(snap.active_transactions, 0);
}
