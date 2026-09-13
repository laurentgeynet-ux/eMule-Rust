use std::time::Duration;

use mule_protocol::framing::RawPacket;
use mule_protocol::lab::{
    InFlightError, InFlightTracker, LabMessage, PeerFailure, PeerStateMachine, ProtocolViolation,
};
use mule_protocol::traits::BlockRequest;
use mule_protocol::types::{PeerId, PR_ED2K, PR_EMULE_COMPRESSED};
use mule_protocol::ProtocolError;

const DUMMY_HASH_A: [u8; 16] = [0xAA; 16];
const DUMMY_HASH_B: [u8; 16] = [0xBB; 16];

#[test]
fn test_ninth_in_flight_request_rejected_without_leak() {
    let mut tracker = InFlightTracker::new();

    // Fill up to maximum capacity of 8
    for i in 0..8 {
        let req = BlockRequest::new(DUMMY_HASH_A, i * 184_320, 184_320);
        assert!(tracker.can_request());
        tracker.record_request(&req).expect("should accept first 8");
    }

    assert_eq!(tracker.len(), 8);
    assert!(!tracker.can_request());

    // 9th request must be rejected
    let req9 = BlockRequest::new(DUMMY_HASH_A, 8 * 184_320, 184_320);
    let err = tracker.record_request(&req9).unwrap_err();
    assert_eq!(err, InFlightError::CapacityReached { limit: 8 });

    // Capacity must not be corrupted or leaked
    assert_eq!(tracker.len(), 8);
    assert!(!tracker.can_request());
}

#[test]
fn test_duplicate_block_response_rejected() {
    let mut tracker = InFlightTracker::new();
    let req = BlockRequest::new(DUMMY_HASH_A, 0, 1000);
    tracker.record_request(&req).unwrap();

    // 1st consumption succeeds
    tracker
        .validate_and_remove(&DUMMY_HASH_A, 0, 1000)
        .expect("1st response valid");
    assert_eq!(tracker.len(), 0);
    assert!(tracker.can_request());

    // 2nd consumption for identical block must fail idempotently
    let err = tracker
        .validate_and_remove(&DUMMY_HASH_A, 0, 1000)
        .unwrap_err();
    assert_eq!(err, InFlightError::UnsolicitedBlock { start_offset: 0 });

    // Tracker capacity must not underflow
    assert_eq!(tracker.len(), 0);
    assert!(tracker.is_empty());
}

#[test]
fn test_wrong_file_hash_response_rejected() {
    let mut tracker = InFlightTracker::new();
    let req = BlockRequest::new(DUMMY_HASH_A, 5000, 1000);
    tracker.record_request(&req).unwrap();

    // Response arrives with different file hash
    let err = tracker
        .validate_and_remove(&DUMMY_HASH_B, 5000, 1000)
        .unwrap_err();
    assert_eq!(err, InFlightError::UnsolicitedBlock { start_offset: 5000 });

    // Original request must remain pending
    assert_eq!(tracker.len(), 1);
    tracker
        .validate_and_remove(&DUMMY_HASH_A, 5000, 1000)
        .expect("original succeeds");
    assert_eq!(tracker.len(), 0);
}

#[test]
fn test_length_mismatch_response_rejected() {
    let mut tracker = InFlightTracker::new();
    let req = BlockRequest::new(DUMMY_HASH_A, 0, 184_320);
    tracker.record_request(&req).unwrap();

    // Arrives with wrong length (100_000 instead of 184_320)
    let err = tracker
        .validate_and_remove(&DUMMY_HASH_A, 0, 100_000)
        .unwrap_err();
    assert_eq!(
        err,
        InFlightError::LengthMismatch {
            start_offset: 0,
            expected: 184_320,
            actual: 100_000,
        }
    );
}

#[test]
fn test_reap_timeouts_and_late_block_rejection() {
    let mut tracker = InFlightTracker::new();
    let req = BlockRequest::new(DUMMY_HASH_A, 10_000, 5000);
    tracker.record_request(&req).unwrap();

    // Reaping with zero duration simulates immediate expiration
    let expired = tracker.reap_timeouts(Duration::ZERO);
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].start_offset, 10_000);
    assert_eq!(expired[0].length, 5000);
    assert_eq!(tracker.len(), 0);

    // If the peer delivers the block late after timeout
    let err = tracker
        .validate_and_remove(&DUMMY_HASH_A, 10_000, 5000)
        .unwrap_err();
    assert_eq!(
        err,
        InFlightError::UnsolicitedBlock {
            start_offset: 10_000
        }
    );
}

#[test]
fn test_cancel_all_frees_capacity_on_disconnect() {
    let mut tracker = InFlightTracker::new();
    for i in 0..8 {
        let req = BlockRequest::new(DUMMY_HASH_A, i * 1000, 1000);
        tracker.record_request(&req).unwrap();
    }
    assert_eq!(tracker.len(), 8);

    // Peer abruptly disconnects: cancel all requests
    let cancelled = tracker.cancel_all();
    assert_eq!(cancelled.len(), 8);
    assert_eq!(tracker.len(), 0);
    assert!(tracker.is_empty());
    assert!(tracker.can_request());
}

#[test]
fn test_valid_block_rejected_frees_single_entry() {
    let mut tracker = InFlightTracker::new();
    for i in 0..8 {
        let req = BlockRequest::new(DUMMY_HASH_A, i * 1000, 1000);
        tracker.record_request(&req).unwrap();
    }

    // Source rejects offset 2000
    let rejected_req = tracker
        .handle_rejection(&DUMMY_HASH_A, 2000)
        .expect("handle rejection succeeds");
    assert_eq!(rejected_req.start_offset, 2000);
    assert_eq!(rejected_req.length, 1000);

    // Remaining 7 requests must still be pending
    assert_eq!(tracker.len(), 7);
    assert!(tracker.can_request());

    // Validating an un-rejected entry succeeds
    tracker
        .validate_and_remove(&DUMMY_HASH_A, 0, 1000)
        .expect("offset 0 still valid");
    assert_eq!(tracker.len(), 6);
}

#[test]
fn test_unknown_block_rejected_fails() {
    let mut tracker = InFlightTracker::new();
    let err = tracker
        .handle_rejection(&DUMMY_HASH_A, 999_999)
        .unwrap_err();
    assert_eq!(
        err,
        InFlightError::UnsolicitedRejection {
            start_offset: 999_999
        }
    );
}

#[test]
fn test_compressed_frame_0xd4_explicitly_rejected() {
    // 1. Raw packet level rejection via from_raw_packet
    let compressed_packet = RawPacket::with_vec(PR_EMULE_COMPRESSED, 0x01, vec![1, 2, 3]);
    let res = LabMessage::from_raw_packet(&compressed_packet);
    assert_eq!(
        res.unwrap_err(),
        ProtocolError::CompressedProtocolUnsupported
    );

    // 2. State machine level rejection via on_raw_packet
    let mut sm = PeerStateMachine::new_listener(PeerId::new([0x01; 16]), 1);
    let err = sm.on_raw_packet(&compressed_packet).unwrap_err();
    assert_eq!(err, ProtocolViolation::CompressedProtocolUnsupported);
    assert_eq!(
        sm.state(),
        &mule_protocol::lab::PeerState::Failed {
            reason: PeerFailure::Violation(ProtocolViolation::CompressedProtocolUnsupported)
        }
    );
}

#[test]
fn test_unexpected_protocol_byte_rejected() {
    let ed2k_packet = RawPacket::with_vec(PR_ED2K, 0x01, vec![1, 2, 3]);
    let mut sm = PeerStateMachine::new_listener(PeerId::new([0x01; 16]), 1);
    let err = sm.on_raw_packet(&ed2k_packet).unwrap_err();
    assert_eq!(
        err,
        ProtocolViolation::UnexpectedProtocol { protocol: PR_ED2K }
    );
    assert!(matches!(
        sm.state(),
        mule_protocol::lab::PeerState::Failed { .. }
    ));
}
