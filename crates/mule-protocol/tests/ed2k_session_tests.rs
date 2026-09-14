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

// ---------------------------------------------------------------------------
// 7. Formal Non-Mutation Invariance Property
// ---------------------------------------------------------------------------

#[test]
fn test_non_mutation_property_on_raw_and_unhandled_frames() {
    use bytes::Bytes;
    use mule_protocol::ed2k::{
        BlockRequestKey, Ed2kMessage, Ed2kSession,
        PR_ED2K, PR_EMULE,
    };
    use mule_protocol::framing::RawPacket;
    use std::time::Instant;

    // 1. Establish session in Ready state
    let mut session = Ed2kSession::new_listener(sample_user_hash(), 0, 4662);
    let hello = mule_protocol::ed2k::Ed2kHello {
        user_hash: sample_user_hash(),
        client_id: 0,
        tcp_port: 4662,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let peer = session.on_hello_received(&hello).expect("hello accepted");
    let _answer = session
        .complete_handshake_as_listener(peer, vec![], 0, 0)
        .expect("handshake completed");
    assert!(session.is_ready());

    // 2. Register active in-flight requests in both trackers
    let file_hash = [0x77; 16];
    let now = Instant::now();
    let _status_pkt = session
        .send_file_status_request(file_hash, now)
        .expect("status req registered");
    let _block_pkt = session
        .send_block_request(file_hash, 0, 184_320, 1_000_000, now)
        .expect("block req registered");

    assert_eq!(session.status_tracker.pending_count(), 1);
    assert_eq!(session.block_tracker.pending_count(), 1);

    // 3. Capture baseline immutable snapshot
    let initial_snapshot = session.snapshot();
    assert_eq!(initial_snapshot.state_name, "Ready");
    assert!(initial_snapshot.is_ready);
    assert_eq!(initial_snapshot.pending_status_count, 1);
    assert_eq!(initial_snapshot.pending_block_count, 1);
    assert_eq!(initial_snapshot.unhandled_packets_count, 0);

    // 4. Inject Raw frame 1: 0xC5 PR_EMULE extension
    let pkt_c5 = RawPacket {
        protocol: PR_EMULE,
        opcode: 0x60,
        payload: Bytes::from_static(b"kademlia_non_mutating_packet"),
    };
    let res1 = session.on_packet_received(&pkt_c5).expect("0xC5 raw processed cleanly");
    assert!(matches!(res1, Some(Ed2kMessage::Raw(_))), "0xC5 must decode to Raw");

    let snap1 = session.snapshot();
    assert_eq!(snap1.state_name, initial_snapshot.state_name);
    assert_eq!(snap1.is_ready, initial_snapshot.is_ready);
    assert_eq!(snap1.pending_status_count, initial_snapshot.pending_status_count);
    assert_eq!(snap1.pending_block_count, initial_snapshot.pending_block_count);
    assert_eq!(snap1.unhandled_packets_count, 1, "only diagnostic counter increments");

    // 5. Inject Raw frame 2: Ambiguous opcode under active profile (0x4B under Emule050a)
    let pkt_4b = RawPacket {
        protocol: PR_ED2K,
        opcode: 0x4B,
        payload: Bytes::from_static(b"legacy_0x4b_request_parts"),
    };
    let res2 = session.on_packet_received(&pkt_4b).expect("0x4B processed as Raw");
    assert!(matches!(res2, Some(Ed2kMessage::Raw(_))), "0x4B under Emule050a must decode to Raw");

    let snap2 = session.snapshot();
    assert_eq!(snap2.state_name, initial_snapshot.state_name);
    assert_eq!(snap2.is_ready, initial_snapshot.is_ready);
    assert_eq!(snap2.pending_status_count, initial_snapshot.pending_status_count);
    assert_eq!(snap2.pending_block_count, initial_snapshot.pending_block_count);
    assert_eq!(snap2.unhandled_packets_count, 2);

    // 6. Inject Raw frame 3: Unknown opcode 0xFE
    let pkt_fe = RawPacket {
        protocol: PR_ED2K,
        opcode: 0xFE,
        payload: Bytes::from_static(b"future_unknown_opcode"),
    };
    let res3 = session.on_packet_received(&pkt_fe).expect("0xFE processed as Raw");
    assert!(matches!(res3, Some(Ed2kMessage::Raw(_))), "0xFE must decode to Raw");

    let snap3 = session.snapshot();
    assert_eq!(snap3.state_name, initial_snapshot.state_name);
    assert_eq!(snap3.is_ready, initial_snapshot.is_ready);
    assert_eq!(snap3.pending_status_count, initial_snapshot.pending_status_count);
    assert_eq!(snap3.pending_block_count, initial_snapshot.pending_block_count);
    assert_eq!(snap3.unhandled_packets_count, 3);

    // 7. Verify that in-flight trackers and keys remain strictly intact
    assert!(session.status_tracker.is_pending(&file_hash), "status tracker must be unmodified");
    let key = BlockRequestKey::new(file_hash, 0, 184_320);
    assert!(session.block_tracker.is_pending(&key), "block tracker must be unmodified");
}

#[test]
fn test_session_awaiting_hello_answer_accepts_pr_emule_extension_before_hello_answer() {
    let mut session = Ed2kSession::new_initiator([0x11; 16], 0, 4662);
    // Initiator starts in AwaitingHelloAnswer
    assert!(matches!(session.state, Ed2kSessionState::AwaitingHelloAnswer));

    // 1. Peer delivers PR_EMULE (0xC5) / OP_EMULE_INFO (0x01) first
    let emule_info_pkt = RawPacket {
        protocol: mule_protocol::types::PR_EMULE,
        opcode: 0x01,
        payload: Bytes::from_static(b"\x3C\x01\x00\x00\x00\x00"), // version 60, proto 1, 0 tags
    };

    let res1 = session.on_packet_received(&emule_info_pkt).expect("PR_EMULE accepted during handshake");
    assert!(matches!(res1, Some(Ed2kMessage::EmuleInfo(_))), "PR_EMULE decoded as EmuleInfo");
    if let Some(Ed2kMessage::EmuleInfo(ref info)) = res1 {
        assert_eq!(info.client_version, 60);
        assert_eq!(info.protocol_version, 1);
        assert_eq!(info.tags.len(), 0);
    }
    assert!(matches!(session.state, Ed2kSessionState::AwaitingHelloAnswer), "state remains AwaitingHelloAnswer");

    // 2. Peer subsequently delivers PR_ED2K (0xE3) / OP_HELLOANSWER (0x4C)
    let mut answer_payload = Vec::new();
    answer_payload.extend_from_slice(&[0x22; 16]); // remote hash
    answer_payload.extend_from_slice(&0u32.to_le_bytes()); // client_id = 0
    answer_payload.extend_from_slice(&4662u16.to_le_bytes()); // tcp_port
    answer_payload.extend_from_slice(&0u32.to_le_bytes()); // tag count = 0
    let hello_answer_pkt = RawPacket {
        protocol: mule_protocol::types::PR_ED2K,
        opcode: 0x4C,
        payload: Bytes::from(answer_payload),
    };

    let res2 = session.on_packet_received(&hello_answer_pkt).expect("OP_HELLOANSWER accepted");
    assert!(matches!(res2, Some(Ed2kMessage::HelloAnswer(_))), "decoded as HelloAnswer");
    assert!(session.is_ready(), "session successfully transitioned to Ready state!");
    let peer = session.peer_info().expect("peer_info");
    assert_eq!(peer.user_hash, [0x22; 16]);
    assert!(peer.emule_info.is_some(), "emule_info must be attached to peer");
    assert_eq!(peer.emule_info.as_ref().unwrap().client_version, 60);
}

