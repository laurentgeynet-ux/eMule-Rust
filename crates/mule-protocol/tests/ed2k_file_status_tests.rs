// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Comprehensive test suite for eD2K file availability exchange (OP_FILESTATUSREQ / OP_FILESTATUS).
//! Phase 6.3: Strictly confined to local loopback (127.0.0.1). Zero network calls, zero .part writes.

use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use mule_protocol::ed2k::opcodes::{OP_FILESTATUS, OP_FILESTATUSREQ};
use mule_protocol::ed2k::PR_ED2K;
use mule_protocol::ed2k::{
    Ed2kCompatibilityProfile, Ed2kDecoder, Ed2kEncoder, Ed2kErrorKind, Ed2kFileStatus,
    Ed2kFileStatusReq, Ed2kHelloAnswer, Ed2kLoopbackClient, Ed2kMessage, Ed2kPeerInfo, Ed2kSession,
    Ed2kSessionError, Ed2kSessionState, Ed2kTag, Ed2kTestPeer, FileStatusTracker,
    PartAvailabilityMask, TagName, TagValue, TestPeerMode, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
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
            value: TagValue::String("StatusTestInitiator".to_string()),
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
fn test_file_status_req_wire_layout_byte_by_byte_matches_fixture() {
    let fixture_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ed2k/file_status_req.bin");
    let fixture_bytes = fs::read(&fixture_path).expect("failed to read file_status_req.bin");

    // Header: 1 byte protocol (0xE3) + 4 bytes LE length (17 = 0x11, 0x00, 0x00, 0x00)
    assert_eq!(fixture_bytes[0], PR_ED2K);
    assert_eq!(&fixture_bytes[1..5], &[0x11, 0x00, 0x00, 0x00]);
    // Opcode: 0x49 (OP_FILESTATUSREQ)
    assert_eq!(fixture_bytes[5], OP_FILESTATUSREQ);

    // Hash in fixture: 0xCC (16 bytes)
    let expected_hash: [u8; 16] = [0xCC; 16];
    assert_eq!(&fixture_bytes[6..22], &expected_hash);

    // Verify that default Emule072a decoder rejects legacy 0x49 opcode in decode_file_status_req
    let default_decoder = Ed2kDecoder::new();
    let err = default_decoder
        .decode_file_status_req(PR_ED2K, OP_FILESTATUSREQ, &fixture_bytes[6..22], 0)
        .unwrap_err();
    assert!(matches!(
        err.kind,
        Ed2kErrorKind::AmbiguousOpcodeForProfile {
            opcode: OP_FILESTATUSREQ,
            profile: Ed2kCompatibilityProfile::Emule072a,
            ..
        }
    ));

    // Encode via Ed2kEncoder with LegacyEdonkey profile
    let req = Ed2kFileStatusReq {
        file_hash: expected_hash,
    };
    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let encoded = encoder
        .encode_to_bytes(&Ed2kMessage::FileStatusReq(req))
        .expect("encoding failed");
    assert_eq!(encoded.as_ref(), fixture_bytes.as_slice());

    // Decode via Ed2kDecoder with LegacyEdonkey profile
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let decoded_req = decoder
        .decode_file_status_req(PR_ED2K, OP_FILESTATUSREQ, &fixture_bytes[6..22], 0)
        .expect("decoding failed");
    assert_eq!(decoded_req.file_hash, expected_hash);
}

// ---------------------------------------------------------------------------
// 2. Nominal Single Part Availability
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_nominal_file_status_single_part() {
    let file_hash = [0xAA; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    test_peer
        .add_shared_file(file_hash, 1, PartAvailabilityMask::from_parts(&[0], 1))
        .await;
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let status = client
        .request_file_status(file_hash, Duration::from_millis(500))
        .await
        .expect("file status request failed");

    assert_eq!(status.file_hash, file_hash);
    assert_eq!(status.part_count, 1);
    assert!(status.availability_mask.has_part(0));
    assert!(!status.availability_mask.has_part(1));
    assert_eq!(
        status
            .availability_mask
            .available_parts()
            .collect::<Vec<_>>(),
        vec![0]
    );
    assert_eq!(status.availability_mask.raw_bytes(), &[0x01]);
    assert!(client.session().is_ready());
}

// ---------------------------------------------------------------------------
// 3. Multi-Part Availability (Parts 0, 1, 7, 8, 9 of 10 parts)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_multi_parts_bits_0_1_7_8_9() {
    let file_hash = [0xBB; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    test_peer
        .add_shared_file(
            file_hash,
            10,
            PartAvailabilityMask::from_parts(&[0, 1, 7, 8, 9], 10),
        )
        .await;
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let status = client
        .request_file_status(file_hash, Duration::from_millis(500))
        .await
        .expect("file status request failed");

    assert_eq!(status.file_hash, file_hash);
    assert_eq!(status.part_count, 10);
    // Bit layout:
    // Byte 0: bit 0, 1, 7 set = 1 + 2 + 128 = 131 = 0x83
    // Byte 1: bit 0, 1 set = 1 + 2 = 3 = 0x03
    assert_eq!(status.availability_mask.raw_bytes(), &[0x83, 0x03]);
    assert!(status.availability_mask.has_part(0));
    assert!(status.availability_mask.has_part(1));
    assert!(!status.availability_mask.has_part(2));
    assert!(!status.availability_mask.has_part(6));
    assert!(status.availability_mask.has_part(7));
    assert!(status.availability_mask.has_part(8));
    assert!(status.availability_mask.has_part(9));
    assert!(!status.availability_mask.has_part(10));
    assert_eq!(
        status
            .availability_mask
            .available_parts()
            .collect::<Vec<_>>(),
        vec![0, 1, 7, 8, 9]
    );
}

// ---------------------------------------------------------------------------
// 4. Adversarial: Short Mask Truncation Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_short_mask_rejected() {
    let file_hash = [0xCC; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusShortMask)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let res = client
        .request_file_status(file_hash, Duration::from_millis(500))
        .await;

    assert!(res.is_err());
    assert!(matches!(
        client.session().state,
        Ed2kSessionState::Failed { .. }
    ));
}

// ---------------------------------------------------------------------------
// 5. Adversarial: Long Mask Extraneous Bytes Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_long_mask_rejected() {
    let file_hash = [0xDD; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusLongMask)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let res = client
        .request_file_status(file_hash, Duration::from_millis(500))
        .await;

    assert!(res.is_err());
    assert!(matches!(
        client.session().state,
        Ed2kSessionState::Failed { .. }
    ));
}

// ---------------------------------------------------------------------------
// 6. Adversarial: Excessive Part Count (65535 > MAX_PARTS_PER_FILE) Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_excessive_parts_rejected() {
    let file_hash = [0xEE; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusExcessiveParts)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let res = client
        .request_file_status(file_hash, Duration::from_millis(500))
        .await;

    assert!(res.is_err());
    assert!(matches!(
        client.session().state,
        Ed2kSessionState::Failed { .. }
    ));
}

// ---------------------------------------------------------------------------
// 7. Adversarial: Trailing Unused Bits Non-Zero Strictly Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_trailing_bits_non_zero_strictly_rejected() {
    let file_hash = [0x12; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusTrailingBitsNonZero)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let res = client
        .request_file_status(file_hash, Duration::from_millis(500))
        .await;

    match res {
        Err(Ed2kSessionError::Parse(ref msg)) => {
            assert!(
                msg.contains("trailing bits"),
                "expected trailing bits in error: {msg}"
            );
        }
        other => panic!("expected Parse trailing bits error, got: {other:?}"),
    }
    assert!(matches!(
        client.session().state,
        Ed2kSessionState::Failed { .. }
    ));
}

// ---------------------------------------------------------------------------
// 8. Adversarial: Mismatched File Hash in Response Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_mismatched_hash_rejected() {
    let file_hash = [0x34; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusMismatchHash)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let res = client
        .request_file_status(file_hash, Duration::from_millis(500))
        .await;

    match res {
        Err(Ed2kSessionError::UnsolicitedFileStatus(_)) => {}
        other => panic!("expected UnsolicitedFileStatus, got: {other:?}"),
    }
    assert!(matches!(
        client.session().state,
        Ed2kSessionState::Failed { .. }
    ));
}

// ---------------------------------------------------------------------------
// 9. Adversarial: Unsolicited OP_FILESTATUS Packet Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_unsolicited_rejected() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusUnsolicited)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;

    // Receive unsolicited packet from remote peer
    let pkt = client
        .recv_packet()
        .await
        .expect("failed to receive packet");
    assert_eq!(pkt.opcode, OP_FILESTATUS);

    let res = client.session_mut().on_packet_received(&pkt);
    match res {
        Err(Ed2kSessionError::UnsolicitedFileStatus(_)) => {}
        other => panic!("expected UnsolicitedFileStatus error, got: {other:?}"),
    }
    assert!(matches!(
        client.session().state,
        Ed2kSessionState::Failed { .. }
    ));
}

// ---------------------------------------------------------------------------
// 10. Adversarial: Double Response Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_double_response_rejected() {
    let file_hash = [0x56; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusDoubleAnswer)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;

    // First response succeeds and validates the request
    let status = client
        .request_file_status(file_hash, Duration::from_millis(500))
        .await
        .expect("first response should succeed");
    assert_eq!(status.file_hash, file_hash);

    // Second duplicated response arrives
    let pkt = client
        .recv_packet()
        .await
        .expect("failed to receive second packet");
    assert_eq!(pkt.opcode, OP_FILESTATUS);

    let res = client.session_mut().on_packet_received(&pkt);
    match res {
        Err(Ed2kSessionError::UnsolicitedFileStatus(_)) => {}
        other => panic!("expected UnsolicitedFileStatus on double response, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 11. State Guard: OP_FILESTATUSREQ Before Ready Rejected
// ---------------------------------------------------------------------------

#[test]
fn test_file_status_req_before_ready_rejected() {
    let mut session = Ed2kSession::new_initiator([0x11; 16], 1, 39735);
    assert_eq!(session.state.name(), "Connecting");

    let res = session.send_file_status_request([0x22; 16], Instant::now());
    match res {
        Err(Ed2kSessionError::RequestBeforeReady(state)) => {
            assert_eq!(state, "Connecting");
        }
        other => panic!("expected RequestBeforeReady error, got: {other:?}"),
    }
    assert!(matches!(session.state, Ed2kSessionState::Failed { .. }));
}

// ---------------------------------------------------------------------------
// 12. State Guard: OP_FILESTATUS Received Before Ready Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_received_before_ready_rejected() {
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusBeforeReady)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

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

    match res {
        Err(Ed2kSessionError::ResponseBeforeReady(state)) => {
            assert_eq!(state, "AwaitingHelloAnswer");
        }
        Err(e) => panic!("expected ResponseBeforeReady, got: {e}"),
        Ok(_) => panic!("expected handshake to fail, but succeeded"),
    }
}

// ---------------------------------------------------------------------------
// 13. Adversarial: Compressed Protocol 0xD4 Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_compressed_protocol_0xd4_rejected() {
    let file_hash = [0x78; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusCompressed0xD4)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;
    let res = client
        .request_file_status(file_hash, Duration::from_millis(500))
        .await;

    match res {
        Err(Ed2kSessionError::CompressedProtocolUnsupported) => {}
        other => panic!("expected CompressedProtocolUnsupported, got: {other:?}"),
    }
    assert!(matches!(
        client.session().state,
        Ed2kSessionState::Failed { .. }
    ));
}

// ---------------------------------------------------------------------------
// 14. Timeout Cleans Tracker & Late Response is Rejected
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_file_status_timeout_cleans_tracker_late_response_rejected() {
    let file_hash = [0x9A; 16];
    let test_peer = Ed2kTestPeer::bind(TestPeerMode::FileStatusHangTimeout)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;

    // Short timeout (50ms)
    let res = client
        .request_file_status(file_hash, Duration::from_millis(50))
        .await;

    match res {
        Err(Ed2kSessionError::Timeout(_)) => {}
        other => panic!("expected Timeout error, got: {other:?}"),
    }

    // Tracker should be empty
    assert_eq!(client.session().status_tracker.pending_count(), 0);

    // Simulate late response
    let late_status = Ed2kFileStatus {
        file_hash,
        part_count: 1,
        availability_mask: PartAvailabilityMask::from_parts(&[0], 1),
    };
    let res = client.session_mut().on_file_status_received(&late_status);
    match res {
        Err(Ed2kSessionError::UnsolicitedFileStatus(_)) => {}
        other => panic!("expected UnsolicitedFileStatus on late response, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 15. Disconnection Clears In-Flight Tracker
// ---------------------------------------------------------------------------

#[test]
fn test_disconnect_clears_pending_file_status_tracker() {
    let mut session = Ed2kSession::new_initiator([0x11; 16], 1, 39735);
    // Move session to Ready
    let answer = Ed2kHelloAnswer {
        user_hash: [0xEE; 16],
        client_id: 2,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    session.state = Ed2kSessionState::Ready {
        peer: Ed2kPeerInfo::from_hello_answer(&answer),
    };

    let file_hash = [0xBC; 16];
    session
        .send_file_status_request(file_hash, Instant::now())
        .expect("send request failed");
    assert_eq!(session.status_tracker.pending_count(), 1);

    // Remote closes
    session.on_remote_closed();
    assert_eq!(session.status_tracker.pending_count(), 0);
    assert_eq!(session.state, Ed2kSessionState::Closed);
}

// ---------------------------------------------------------------------------
// 16. Multiple Distinct File Requests: No Crosstalk
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multiple_distinct_file_requests_no_crosstalk() {
    let hash_a = [0x0A; 16];
    let hash_b = [0x0B; 16];

    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    test_peer
        .add_shared_file(hash_a, 3, PartAvailabilityMask::from_parts(&[0, 2], 3))
        .await;
    test_peer
        .add_shared_file(hash_b, 5, PartAvailabilityMask::from_parts(&[1, 3, 4], 5))
        .await;
    let peer_addr = test_peer.local_addr();
    let _peer_handle = test_peer.spawn();

    let mut client = setup_nominal_client(peer_addr).await;

    let status_a = client
        .request_file_status(hash_a, Duration::from_millis(500))
        .await
        .expect("file A failed");
    assert_eq!(status_a.file_hash, hash_a);
    assert_eq!(status_a.part_count, 3);
    assert_eq!(
        status_a
            .availability_mask
            .available_parts()
            .collect::<Vec<_>>(),
        vec![0, 2]
    );

    let status_b = client
        .request_file_status(hash_b, Duration::from_millis(500))
        .await
        .expect("file B failed");
    assert_eq!(status_b.file_hash, hash_b);
    assert_eq!(status_b.part_count, 5);
    assert_eq!(
        status_b
            .availability_mask
            .available_parts()
            .collect::<Vec<_>>(),
        vec![1, 3, 4]
    );

    assert_eq!(client.session().status_tracker.pending_count(), 0);
}

// ---------------------------------------------------------------------------
// 17. Tracker Direct Bounds, Capacity & Duplicate Rejection
// ---------------------------------------------------------------------------

#[test]
fn test_tracker_bounds_and_duplicate_rejection() {
    let mut tracker = FileStatusTracker::new(16);
    let now = Instant::now();
    let hash_1 = [1u8; 16];

    // 1. Initial registration
    assert!(tracker.register_request(hash_1, now).is_ok());
    assert!(tracker.is_pending(&hash_1));
    assert_eq!(tracker.pending_count(), 1);

    // 2. Duplicate registration rejected
    match tracker.register_request(hash_1, now) {
        Err(Ed2kSessionError::DuplicateFileStatusRequest(_)) => {}
        other => panic!("expected DuplicateFileStatusRequest, got: {other:?}"),
    }

    // 3. Fill capacity up to 16
    for i in 2..=16 {
        let mut h = [0u8; 16];
        h[0] = i;
        assert!(tracker.register_request(h, now).is_ok());
    }
    assert_eq!(tracker.pending_count(), 16);

    // 4. 17th registration fails with TooManyPendingRequests
    let overflow_hash = [17u8; 16];
    match tracker.register_request(overflow_hash, now) {
        Err(Ed2kSessionError::TooManyPendingRequests(max)) => {
            assert_eq!(max, 16);
        }
        other => panic!("expected TooManyPendingRequests, got: {other:?}"),
    }

    // 5. Validate and remove
    let removed = tracker
        .validate_and_remove(&hash_1)
        .expect("validate failed");
    assert_eq!(removed.file_hash, hash_1);
    assert_eq!(tracker.pending_count(), 15);

    // 6. Second validate fails with UnsolicitedFileStatus
    match tracker.validate_and_remove(&hash_1) {
        Err(Ed2kSessionError::UnsolicitedFileStatus(_)) => {}
        other => panic!("expected UnsolicitedFileStatus, got: {other:?}"),
    }

    // 7. Expire timed-out
    let later = now + Duration::from_millis(200);
    let expired = tracker.expire_timed_out(Duration::from_millis(100), later);
    assert_eq!(expired.len(), 15);
    assert_eq!(tracker.pending_count(), 0);
}
