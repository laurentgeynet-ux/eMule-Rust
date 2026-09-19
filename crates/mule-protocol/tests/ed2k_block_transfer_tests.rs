//! Comprehensive test suite for eD2K block transfer (OP_REQUESTPARTS / OP_SENDINGPART).
//! Phase 6.4: Strictly confined to local loopback (127.0.0.1). Zero network calls, zero public peers.

use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bytes::Bytes;
use mule_protocol::ed2k::opcodes::{OP_REQUESTPARTS, OP_SENDINGPART};
use mule_protocol::ed2k::protocol::PR_ED2K;
use mule_protocol::ed2k::{
    BlockRequestKey, BlockRequestTracker, Ed2kCompatibilityProfile, Ed2kDecoder, Ed2kEncoder,
    Ed2kErrorKind, Ed2kLoopbackClient, Ed2kMessage, Ed2kRequestParts, Ed2kSendingPart, Ed2kSession,
    Ed2kSessionError, Ed2kSessionState, Ed2kTag, Ed2kTestPeer, TagName, TagValue, TestPeerMode,
    MAX_IN_FLIGHT_BLOCK_REQUESTS, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
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
            value: TagValue::String("BlockTestInitiator".to_string()),
        },
        Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_VERSION),
            value: TagValue::UInt32(60),
        },
    ]
}

async fn setup_nominal_client(peer_addr: SocketAddr) -> Ed2kLoopbackClient {
    let (client, _info) = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        39735,
        sample_client_tags(),
        0x7F000001,
        4661,
        Duration::from_millis(500),
    )
    .await
    .expect("handshake failed");
    client
}

// ---------------------------------------------------------------------------
// 1. Wire Layout & Fixture Byte-by-Byte Verification
// ---------------------------------------------------------------------------

#[test]
fn test_request_parts_wire_layout_byte_by_byte_matches_fixture() {
    let fixture_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ed2k/request_parts.bin");
    let fixture_bytes = fs::read(&fixture_path).expect("failed to read request_parts.bin");

    // Header: 1 byte protocol (0xE3) + 4 bytes LE length (41) + 1 byte opcode (0x4B)
    assert_eq!(fixture_bytes[0], PR_ED2K);
    assert_eq!(&fixture_bytes[1..5], &41u32.to_le_bytes());
    assert_eq!(fixture_bytes[5], OP_REQUESTPARTS);

    // Hash in fixture: 0xCC (16 bytes)
    let expected_hash: [u8; 16] = [0xCC; 16];
    assert_eq!(&fixture_bytes[6..22], &expected_hash);

    // Verify that default Emule050a decoder rejects legacy 0x4B opcode
    let default_decoder = Ed2kDecoder::new();
    let err = default_decoder
        .decode_request_parts(PR_ED2K, OP_REQUESTPARTS, &fixture_bytes[6..], 0)
        .unwrap_err();
    assert!(matches!(
        err.kind,
        Ed2kErrorKind::AmbiguousOpcodeForProfile {
            opcode: OP_REQUESTPARTS,
            profile: Ed2kCompatibilityProfile::Emule050a,
            ..
        }
    ));

    // Decode via Ed2kDecoder configured with LegacyEdonkey profile
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let decoded = decoder
        .decode_request_parts(PR_ED2K, OP_REQUESTPARTS, &fixture_bytes[6..], 0)
        .expect("decoding failed");
    assert_eq!(decoded.file_hash, expected_hash);
    assert_eq!(decoded.ranges.len(), 1);
    assert_eq!(decoded.ranges[0].start, 0);
    assert_eq!(decoded.ranges[0].end, 184320);

    // Encode via Ed2kEncoder configured with LegacyEdonkey profile
    let req = Ed2kRequestParts::single(expected_hash, 0, 184320);
    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let encoded = encoder
        .encode_to_bytes(&Ed2kMessage::RequestParts(req))
        .expect("encoding failed");
    assert_eq!(encoded.as_ref(), fixture_bytes.as_slice());
}

#[test]
fn test_sending_part_wire_layout_byte_by_byte_matches_fixture() {
    let fixture_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ed2k/sending_part.bin");
    let fixture_bytes = fs::read(&fixture_path).expect("failed to read sending_part.bin");

    // Header: 1 byte protocol (0xE3) + 4 bytes LE length (1049) + 1 byte opcode (0x46)
    assert_eq!(fixture_bytes[0], PR_ED2K);
    assert_eq!(&fixture_bytes[1..5], &1049u32.to_le_bytes());
    assert_eq!(fixture_bytes[5], OP_SENDINGPART);

    // Hash in fixture: 0xCC (16 bytes)
    let expected_hash: [u8; 16] = [0xCC; 16];
    assert_eq!(&fixture_bytes[6..22], &expected_hash);

    // Decode via Ed2kDecoder
    let decoder = Ed2kDecoder::new();
    let decoded = decoder
        .decode_sending_part(PR_ED2K, OP_SENDINGPART, &fixture_bytes[6..], 0)
        .expect("decoding failed");
    assert_eq!(decoded.file_hash, expected_hash);
    assert_eq!(decoded.start_offset, 0);
    assert_eq!(decoded.end_offset, 1024);
    assert_eq!(decoded.data.len(), 1024);

    // Encode via Ed2kEncoder
    let part = Ed2kSendingPart {
        file_hash: expected_hash,
        start_offset: 0,
        end_offset: 1024,
        data: decoded.data.clone(),
    };
    let encoder = Ed2kEncoder::new();
    let encoded = encoder
        .encode_to_bytes(&Ed2kMessage::SendingPart(part))
        .expect("encoding failed");
    assert_eq!(encoded.as_ref(), fixture_bytes.as_slice());
}

// ---------------------------------------------------------------------------
// 2. Nominal Single Block Transfer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_nominal_single_block_transfer() {
    let file_hash = [0x33; 16];
    let file_size: u64 = 184320;
    let block_len = 184320;

    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();

    // Prepare known content in memory
    let mut source_bytes = vec![0u8; block_len];
    for (i, b) in source_bytes.iter_mut().enumerate() {
        *b = ((i * 7 + 13) % 256) as u8;
    }
    test_peer
        .add_shared_file_data(file_hash, Bytes::from(source_bytes.clone()))
        .await;

    let peer_handle = test_peer.spawn();
    let mut client = setup_nominal_client(peer_addr).await;

    let part = client
        .request_block(
            file_hash,
            0,
            block_len as u64,
            file_size,
            Duration::from_millis(500),
        )
        .await
        .expect("block request failed");

    assert_eq!(part.file_hash, file_hash);
    assert_eq!(part.start_offset, 0);
    assert_eq!(part.end_offset, block_len as u32);
    assert_eq!(part.data.as_ref(), source_bytes.as_slice());

    // Verify session tracker has no remaining pending requests
    assert_eq!(client.session().pending_block_request_count(), 0);

    client.close().await;
    let _ = peer_handle.await;
}

// ---------------------------------------------------------------------------
// 3. Nominal Multi-Block Transfer (In-Order and Out-of-Order)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_nominal_multiblock_transfer_sequential() {
    let file_hash = [0x55; 16];
    let block_size: u64 = 65536; // 64 KiB
    let file_size: u64 = block_size * 3; // 192 KiB

    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();

    let mut full_file = vec![0u8; file_size as usize];
    for (i, b) in full_file.iter_mut().enumerate() {
        *b = ((i ^ 0xAA) % 256) as u8;
    }
    test_peer
        .add_shared_file_data(file_hash, Bytes::from(full_file.clone()))
        .await;

    let peer_handle = test_peer.spawn();
    let mut client = setup_nominal_client(peer_addr).await;

    let mut received_file = vec![0u8; file_size as usize];

    for block_idx in 0..3 {
        let start = block_idx * block_size;
        let end = (block_idx + 1) * block_size;
        let part = client
            .request_block(file_hash, start, end, file_size, Duration::from_millis(500))
            .await
            .expect("sequential block failed");

        assert_eq!(part.start_offset, start as u32);
        assert_eq!(part.end_offset, end as u32);
        received_file[start as usize..end as usize].copy_from_slice(&part.data);
    }

    assert_eq!(received_file, full_file);
    assert_eq!(client.session().pending_block_request_count(), 0);

    client.close().await;
    let _ = peer_handle.await;
}

#[tokio::test]
async fn test_nominal_multiblock_transfer_out_of_order() {
    let file_hash = [0x77; 16];
    let block_size: u64 = 32768; // 32 KiB
    let file_size: u64 = block_size * 3;

    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();

    let mut full_file = vec![0u8; file_size as usize];
    for (i, b) in full_file.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
    test_peer
        .add_shared_file_data(file_hash, Bytes::from(full_file.clone()))
        .await;

    let peer_handle = test_peer.spawn();
    let mut client = setup_nominal_client(peer_addr).await;

    // Order: Block 2, then Block 0, then Block 1
    let order = [2, 0, 1];
    let mut received_file = vec![0u8; file_size as usize];

    for &block_idx in &order {
        let start = block_idx * block_size;
        let end = (block_idx + 1) * block_size;
        let part = client
            .request_block(file_hash, start, end, file_size, Duration::from_millis(500))
            .await
            .expect("out of order block failed");

        assert_eq!(part.start_offset, start as u32);
        assert_eq!(part.end_offset, end as u32);
        received_file[start as usize..end as usize].copy_from_slice(&part.data);
    }

    assert_eq!(received_file, full_file);
    assert_eq!(client.session().pending_block_request_count(), 0);

    client.close().await;
    let _ = peer_handle.await;
}

// ---------------------------------------------------------------------------
// 4. Adversarial Test Cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_adversarial_sending_part_before_ready() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::SendingPartBeforeReady)
        .await
        .expect("bind peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let res = Ed2kLoopbackClient::connect_and_handshake(
        peer_addr,
        sample_user_hash(),
        0xC0A80001,
        39735,
        sample_client_tags(),
        0x7F000001,
        4661,
        Duration::from_millis(500),
    )
    .await;

    assert!(res.is_err());
    let err = match res {
        Err(e) => e,
        Ok(_) => panic!("Expected handshake to fail"),
    };
    match err {
        Ed2kSessionError::ResponseBeforeReady(state) => {
            assert_eq!(state, "AwaitingHelloAnswer");
        }
        Ed2kSessionError::UnexpectedMessageInState { opcode, .. } => {
            assert_eq!(opcode, OP_SENDINGPART);
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    let _ = peer_handle.await;
}

#[tokio::test]
async fn test_adversarial_unexpected_file_hash() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::SendingPartUnexpectedHash)
        .await
        .expect("bind peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let file_hash = [0x11; 16];

    let res = client
        .request_block(file_hash, 0, 1024, 1024, Duration::from_millis(500))
        .await;

    assert!(res.is_err());
    match res.unwrap_err() {
        Ed2kSessionError::UnsolicitedBlock(info) => {
            assert!(info.contains(&hex::encode([0x99; 16])));
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    assert!(client.session().is_failed());
    client.close().await;
    let _ = peer_handle.await;
}

#[tokio::test]
async fn test_adversarial_unexpected_offset() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::SendingPartUnexpectedOffset)
        .await
        .expect("bind peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let file_hash = [0x22; 16];

    let res = client
        .request_block(file_hash, 0, 1024, 1024, Duration::from_millis(500))
        .await;

    assert!(res.is_err());
    match res.unwrap_err() {
        Ed2kSessionError::UnsolicitedBlock(info) => {
            assert!(info.contains("10-1034"));
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    assert!(client.session().is_failed());
    client.close().await;
    let _ = peer_handle.await;
}

#[tokio::test]
async fn test_adversarial_length_mismatch() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::SendingPartLengthMismatch)
        .await
        .expect("bind peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let file_hash = [0x33; 16];

    let res = client
        .request_block(file_hash, 0, 1024, 1024, Duration::from_millis(500))
        .await;

    assert!(res.is_err());
    match res.unwrap_err() {
        Ed2kSessionError::Parse(msg) => {
            assert!(msg.contains("payload mismatch") || msg.contains("mismatch"));
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    assert!(client.session().is_failed());
    client.close().await;
    let _ = peer_handle.await;
}

#[tokio::test]
async fn test_adversarial_unsolicited_block() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::SendingPartUnsolicited)
        .await
        .expect("bind peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;

    // Client does not send request_block, but listens for incoming packet
    let res = client
        .receive_sending_part(Duration::from_millis(500))
        .await;

    assert!(res.is_err());
    match res.unwrap_err() {
        Ed2kSessionError::UnsolicitedBlock(info) => {
            assert!(info.contains(&hex::encode([0x55; 16])));
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    assert!(client.session().is_failed());
    client.close().await;
    let _ = peer_handle.await;
}

#[tokio::test]
async fn test_adversarial_duplicate_block() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::SendingPartDuplicate)
        .await
        .expect("bind peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let file_hash = [0x44; 16];

    // First receive fulfills the in-flight request
    let first = client
        .request_block(file_hash, 0, 1024, 1024, Duration::from_millis(500))
        .await;
    assert!(first.is_ok());

    // Second receive (duplicate) is rejected as unsolicited!
    let second = client
        .receive_sending_part(Duration::from_millis(500))
        .await;
    assert!(second.is_err());
    match second.unwrap_err() {
        Ed2kSessionError::UnsolicitedBlock(info) => {
            assert!(info.contains(&hex::encode(file_hash)));
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    assert!(client.session().is_failed());
    client.close().await;
    let _ = peer_handle.await;
}

#[tokio::test]
async fn test_adversarial_hang_timeout() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::SendingPartHangTimeout)
        .await
        .expect("bind peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let file_hash = [0x55; 16];

    // Send request with short timeout (50ms)
    let res = client
        .request_block(file_hash, 0, 1024, 1024, Duration::from_millis(50))
        .await;

    assert!(res.is_err());
    match res.unwrap_err() {
        Ed2kSessionError::Timeout(d) => {
            assert_eq!(d, Duration::from_millis(50));
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    // In-flight tracker must be cleaned
    assert_eq!(client.session().pending_block_request_count(), 0);

    client.close().await;
    let _ = peer_handle.await;
}

#[tokio::test]
async fn test_adversarial_compressed_0xd4_rejected() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::SendingPartCompressed0xD4)
        .await
        .expect("bind peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let file_hash = [0x66; 16];

    let res = client
        .request_block(file_hash, 0, 1024, 1024, Duration::from_millis(500))
        .await;

    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err(),
        Ed2kSessionError::CompressedProtocolUnsupported
    );
    assert!(client.session().is_failed());

    client.close().await;
    let _ = peer_handle.await;
}

#[tokio::test]
async fn test_adversarial_immediate_close() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::SendingPartImmediateClose)
        .await
        .expect("bind peer");
    let peer_addr = test_peer.local_addr();
    let peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let file_hash = [0x77; 16];

    let res = client
        .request_block(file_hash, 0, 1024, 1024, Duration::from_millis(500))
        .await;

    assert!(res.is_err());
    match res.unwrap_err() {
        Ed2kSessionError::Io(msg) => {
            assert!(
                msg.contains("Connection closed") || msg.contains("closed") || msg.contains("EOF")
            );
        }
        other => panic!("Unexpected error: {other:?}"),
    }

    client.close().await;
    let _ = peer_handle.await;
}

// ---------------------------------------------------------------------------
// 5. Invariants: Tracker Limits, Overlaps, Numerical Bounds
// ---------------------------------------------------------------------------

#[test]
fn test_block_tracker_max_in_flight_enforced() {
    let mut tracker = BlockRequestTracker::new(MAX_IN_FLIGHT_BLOCK_REQUESTS);
    let now = Instant::now();
    let hash = [0x11; 16];

    // Register 8 contiguous non-overlapping requests
    for i in 0..MAX_IN_FLIGHT_BLOCK_REQUESTS {
        let start = (i as u64) * 1000;
        let end = start + 1000;
        let key = BlockRequestKey::new(hash, start, end);
        assert!(tracker.register_request(key, now).is_ok());
    }

    assert_eq!(tracker.pending_count(), 8);
    assert!(!tracker.can_request());

    // 9th request must be rejected
    let key_overflow = BlockRequestKey::new(hash, 8000, 9000);
    let res = tracker.register_request(key_overflow, now);
    assert_eq!(
        res.unwrap_err(),
        Ed2kSessionError::TooManyPendingBlockRequests(MAX_IN_FLIGHT_BLOCK_REQUESTS)
    );
}

#[test]
fn test_block_tracker_overlap_rejected() {
    let mut tracker = BlockRequestTracker::new(MAX_IN_FLIGHT_BLOCK_REQUESTS);
    let now = Instant::now();
    let hash = [0x22; 16];

    // Request [0..1000]
    let key1 = BlockRequestKey::new(hash, 0, 1000);
    assert!(tracker.register_request(key1, now).is_ok());

    // Overlapping [500..1500] must be rejected
    let key_overlap = BlockRequestKey::new(hash, 500, 1500);
    let res = tracker.register_request(key_overlap, now);
    assert!(matches!(
        res,
        Err(Ed2kSessionError::DuplicateBlockRequest(_))
    ));

    // Exact duplicate [0..1000] must be rejected
    let key_exact = BlockRequestKey::new(hash, 0, 1000);
    let res2 = tracker.register_request(key_exact, now);
    assert!(matches!(
        res2,
        Err(Ed2kSessionError::DuplicateBlockRequest(_))
    ));

    // Non-overlapping [1000..2000] is accepted
    let key_ok = BlockRequestKey::new(hash, 1000, 2000);
    assert!(tracker.register_request(key_ok, now).is_ok());
}

fn dummy_peer_info() -> mule_protocol::ed2k::Ed2kPeerInfo {
    let hello = mule_protocol::ed2k::Ed2kHello {
        user_hash: [0x22; 16],
        client_id: 2,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    mule_protocol::ed2k::Ed2kPeerInfo::from_hello(&hello)
}

#[test]
fn test_request_bounds_and_limits_rejected() {
    let mut session = Ed2kSession::new_initiator([0x11; 16], 1, 39735);
    // Move to ready state
    session.state = Ed2kSessionState::Ready {
        peer: dummy_peer_info(),
    };

    let hash = [0x33; 16];
    let file_size = 500_000;
    let now = Instant::now();

    // 1. start >= end
    let res_inverted = session.send_block_request(hash, 1000, 500, file_size, now);
    assert!(matches!(
        res_inverted,
        Err(Ed2kSessionError::InvalidBlockRequest(_))
    ));

    let res_equal = session.send_block_request(hash, 1000, 1000, file_size, now);
    assert!(matches!(
        res_equal,
        Err(Ed2kSessionError::InvalidBlockRequest(_))
    ));

    // 2. end > file_size
    let res_excess = session.send_block_request(hash, 400_000, 500_001, file_size, now);
    assert!(matches!(
        res_excess,
        Err(Ed2kSessionError::InvalidBlockRequest(_))
    ));

    // 3. block_len > 184,320
    let res_too_large = session.send_block_request(hash, 0, 184_321, file_size, now);
    assert!(matches!(
        res_too_large,
        Err(Ed2kSessionError::InvalidBlockRequest(_))
    ));

    // 4. valid request (184,320 bytes exactly)
    let res_valid = session.send_block_request(hash, 0, 184_320, file_size, now);
    assert!(res_valid.is_ok());
}

#[test]
fn test_disconnect_cancels_all_in_flight_requests() {
    let mut session = Ed2kSession::new_initiator([0x11; 16], 1, 39735);
    session.state = Ed2kSessionState::Ready {
        peer: dummy_peer_info(),
    };

    let hash = [0x44; 16];
    let now = Instant::now();

    assert!(session
        .send_block_request(hash, 0, 1000, 10000, now)
        .is_ok());
    assert!(session
        .send_block_request(hash, 1000, 2000, 10000, now)
        .is_ok());
    assert_eq!(session.pending_block_request_count(), 2);

    let cancelled = session.cancel_all_block_requests();
    assert_eq!(cancelled.len(), 2);
    assert_eq!(session.pending_block_request_count(), 0);
}
