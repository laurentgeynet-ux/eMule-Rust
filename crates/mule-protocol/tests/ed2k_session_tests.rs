//! Comprehensive eD2K closed loopback session tests (Phase 6.2).
//! Strictly confined to 127.0.0.1.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use mule_protocol::ed2k::{
    Ed2kLoopbackClient, Ed2kPeerInfo, Ed2kSession, Ed2kSessionError, Ed2kSessionState,
    Ed2kTag, Ed2kTestPeer, TagName, TagValue, TestPeerMode, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
};

fn sample_user_hash() -> [u8; 16] {
    let mut h = [0x11; 16];
    h[0] = 0x42;
    h
}

fn sample_client_tags() -> Vec<Ed2kTag> {
    vec![
        Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_NAME),
            value: TagValue::String("LoopbackInitiator".to_string()),
        },
        Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_VERSION),
            value: TagValue::UInt32(60),
        },
    ]
}

// ---------------------------------------------------------------------------
// 1. Nominal Loopback Handshake & TCP Confinement
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_nominal_ed2k_handshake_on_loopback() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("failed to bind test peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let (client, peer_info) = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        4662,
        sample_client_tags(),
        0x7F000001,
        4661,
        Duration::from_millis(500),
    )
    .await
    .expect("handshake failed");

    assert!(client.session().is_ready());
    assert_eq!(peer_info.name, Some("NominalPeer".to_string()));
    assert_eq!(peer_info.version, Some(60));
    assert_eq!(peer_info.server_port, 4661);

    client.close().await;
    peer_handle.await.expect("join failed").expect("peer failed");
}

#[tokio::test]
async fn test_non_loopback_connection_strictly_rejected() {
    let non_loopback_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 4662);

    let result = Ed2kLoopbackClient::connect_and_handshake(
        non_loopback_addr,
        sample_user_hash(),
        0xC0A80001,
        4662,
        sample_client_tags(),
        0,
        0,
        Duration::from_millis(500),
    )
    .await;

    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(
        matches!(err, Ed2kSessionError::Io(ref msg) if msg.contains("strictly forbidden")),
        "Expected non-loopback forbidden error, got: {:?}",
        err
    );
}

// ---------------------------------------------------------------------------
// 2. Wire Format & Byte-by-Byte Fragmentation Reconstitution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_hello_encoded_byte_by_byte_received_faithfully() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("failed to bind test peer");
    let peer_addr = test_peer.local_addr();
    let received_hello_handle = test_peer.received_hello_handle();
    let peer_handle = test_peer.spawn();

    let (client, _) = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        4662,
        sample_client_tags(),
        0x7F000001,
        4661,
        Duration::from_millis(500),
    )
    .await
    .expect("handshake failed");

    let received_hello = received_hello_handle.lock().await.clone();
    assert!(received_hello.is_some());
    let hello = received_hello.unwrap();

    assert_eq!(hello.user_hash, sample_user_hash());
    assert_eq!(hello.client_id, 0xC0A80001);
    assert_eq!(hello.tcp_port, 4662);
    assert_eq!(hello.tags.len(), 2);
    assert_eq!(hello.server_ip, 0x7F000001);
    assert_eq!(hello.server_port, 4661);

    client.close().await;
    peer_handle.await.expect("join failed").expect("peer failed");
}

#[tokio::test]
async fn test_hello_answer_fragmented_byte_by_byte_reassembled() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FragmentedByteByByte)
        .await
        .expect("failed to bind test peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let (client, peer_info) = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        4662,
        sample_client_tags(),
        0x7F000001,
        4661,
        Duration::from_millis(1500),
    )
    .await
    .expect("fragmented handshake reassembly failed");

    assert!(client.session().is_ready());
    assert_eq!(peer_info.name, Some("FragmentedPeer".to_string()));
    assert_eq!(peer_info.version, Some(60));

    client.close().await;
    peer_handle.await.expect("join failed").expect("peer failed");
}

// ---------------------------------------------------------------------------
// 3. Peer Information & Tag Preservation
// ---------------------------------------------------------------------------

#[test]
fn test_peer_info_extraction_and_unknown_tag_preservation() {
    use mule_protocol::ed2k::{Ed2kHello, RawTag};

    let hello = Ed2kHello {
        user_hash: sample_user_hash(),
        client_id: 0x12345678,
        tcp_port: 4662,
        tags: vec![
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_NAME),
                value: TagValue::String("KnownName".to_string()),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_VERSION),
                value: TagValue::UInt32(60),
            },
        ],
        unknown_tags: vec![RawTag {
            tag_type: 0xFD,
            name: TagName::Text("custom_experimental".to_string()),
            raw_bytes: vec![1, 2, 3, 4],
        }],
        server_ip: 0x7F000001,
        server_port: 4661,
    };

    let info = Ed2kPeerInfo::from_hello(&hello);
    assert_eq!(info.user_hash, hello.user_hash);
    assert_eq!(info.name, Some("KnownName".to_string()));
    assert_eq!(info.version, Some(60));
    assert_eq!(info.unknown_tags.len(), 1);
    assert_eq!(info.unknown_tags[0].tag_type, 0xFD);
}

// ---------------------------------------------------------------------------
// 4. Adversarial Protocol Violations & Rejections
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_zero_user_hash_rejected() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::ZeroUserHash)
        .await
        .expect("failed to bind test peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let result = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        4662,
        sample_client_tags(),
        0,
        0,
        Duration::from_millis(500),
    )
    .await;

    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(
        matches!(err, Ed2kSessionError::ZeroUserHash),
        "Expected ZeroUserHash error, got: {:?}",
        err
    );

    peer_handle.await.expect("join failed").expect("peer failed");
}

#[tokio::test]
async fn test_early_hello_answer_rejected() {
    // 1. Direct state machine transition test: A listener receiving HelloAnswer before Hello must fail
    let mut listener_session = Ed2kSession::new_listener(sample_user_hash(), 0, 4662);
    let early_answer = mule_protocol::ed2k::Ed2kHelloAnswer {
        user_hash: sample_user_hash(),
        client_id: 0,
        tcp_port: 4662,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let result = listener_session.on_hello_answer_received(&early_answer);
    assert!(result.is_err());
    assert!(matches!(
        result.err().unwrap(),
        Ed2kSessionError::HelloAnswerBeforeHello
    ));

    // 2. Loopback network test: client connects to test peer in HelloAnswerBeforeHello mode
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::HelloAnswerBeforeHello)
        .await
        .expect("failed to bind test peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let mut stream = tokio::net::TcpStream::connect(peer_addr)
        .await
        .expect("failed to connect to test peer");

    // The test peer sends early HelloAnswer
    let codec = mule_protocol::framing::PacketCodec::new();
    let mut buf = bytes::BytesMut::with_capacity(1024);

    let mut tmp = [0u8; 1024];

    let packet = loop {
        if let Some(packet) = codec.decode(&mut buf).expect("codec decode failed") {
            break packet;
        }
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut tmp)
            .await
            .expect("read failed");
        assert!(n > 0, "unexpected EOF");
        buf.extend_from_slice(&tmp[..n]);
    };

    assert_eq!(packet.opcode, mule_protocol::ed2k::opcodes::OP_HELLO_ANSWER);

    // Listener session processing this packet fails with HelloAnswerBeforeHello
    let mut session = Ed2kSession::new_listener(sample_user_hash(), 0, 4662);
    let session_res = session.on_packet_received(&packet);
    assert!(session_res.is_err());
    assert!(matches!(
        session_res.err().unwrap(),
        Ed2kSessionError::HelloAnswerBeforeHello
    ));

    peer_handle.await.expect("join failed").expect("peer failed");
}


#[test]
fn test_double_hello_rejected() {
    let mut listener_session = Ed2kSession::new_listener(sample_user_hash(), 0, 4662);
    let hello = mule_protocol::ed2k::Ed2kHello {
        user_hash: sample_user_hash(),
        client_id: 0,
        tcp_port: 4662,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };

    let first = listener_session.on_hello_received(&hello);
    assert!(first.is_ok());

    let _answer = listener_session
        .complete_handshake_as_listener(first.unwrap(), vec![], 0, 0)
        .expect("handshake complete failed");
    assert!(listener_session.is_ready());

    // Second Hello while session is Ready must fail with DoubleHello
    let second = listener_session.on_hello_received(&hello);
    assert!(second.is_err());
    assert!(matches!(second.err().unwrap(), Ed2kSessionError::DoubleHello));
}

#[tokio::test]
async fn test_unexpected_opcode_rejected() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::UnexpectedOpcode)
        .await
        .expect("failed to bind test peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let result = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        4662,
        sample_client_tags(),
        0,
        0,
        Duration::from_millis(500),
    )
    .await;

    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(
        matches!(err, Ed2kSessionError::UnexpectedMessageInState { opcode: 0x99, .. }),
        "Expected UnexpectedMessageInState with opcode 0x99, got: {:?}",
        err
    );

    peer_handle.await.expect("join failed").expect("peer failed");
}

#[tokio::test]
async fn test_compressed_protocol_0xd4_explicitly_rejected() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::UnsupportedProtocol0xD4)
        .await
        .expect("failed to bind test peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let result = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        4662,
        sample_client_tags(),
        0,
        0,
        Duration::from_millis(500),
    )
    .await;

    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(
        matches!(err, Ed2kSessionError::CompressedProtocolUnsupported),
        "Expected CompressedProtocolUnsupported error, got: {:?}",
        err
    );

    peer_handle.await.expect("join failed").expect("peer failed");
}

// ---------------------------------------------------------------------------
// 5. Timeouts and Connection Drop Handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_handshake_timeout_fails_cleanly() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::HangTimeout)
        .await
        .expect("failed to bind test peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let timeout_duration = Duration::from_millis(150);
    let result = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        4662,
        sample_client_tags(),
        0,
        0,
        timeout_duration,
    )
    .await;

    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(
        matches!(err, Ed2kSessionError::Timeout(..)),
        "Expected Timeout error, got: {:?}",
        err
    );

    peer_handle.await.expect("join failed").expect("peer failed");
}

#[tokio::test]
async fn test_immediate_remote_close_fails_cleanly() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::ImmediateClose)
        .await
        .expect("failed to bind test peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let result = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        4662,
        sample_client_tags(),
        0,
        0,
        Duration::from_millis(500),
    )
    .await;

    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(
        matches!(err, Ed2kSessionError::Io(..)),
        "Expected I/O connection closed error, got: {:?}",
        err
    );

    peer_handle.await.expect("join failed").expect("peer failed");
}

// ---------------------------------------------------------------------------
// 6. Session Lifecycle & State Display
// ---------------------------------------------------------------------------

#[test]
fn test_session_state_names_and_display() {
    assert_eq!(Ed2kSessionState::Disconnected.name(), "Disconnected");
    assert_eq!(Ed2kSessionState::Connecting.name(), "Connecting");
    assert_eq!(Ed2kSessionState::AwaitingHello.name(), "AwaitingHello");
    assert_eq!(
        Ed2kSessionState::AwaitingHelloAnswer { local_handshake_id: [0; 16] }.name(),
        "AwaitingHelloAnswer"
    );
    assert_eq!(Ed2kSessionState::Closing.name(), "Closing");
    assert_eq!(Ed2kSessionState::Closed.name(), "Closed");
    assert_eq!(
        Ed2kSessionState::Failed { reason: Ed2kSessionError::DoubleHello }.name(),
        "Failed"
    );
}
