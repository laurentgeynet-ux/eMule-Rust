use std::time::Duration;

use bytes::Bytes;
use mule_protocol::lab::*;
use mule_protocol::{BlockRequest, ED2K_BLOCK_SIZE, MAX_BLOCK_BYTES};

#[test]
fn test_in_flight_tracker_capacity_and_duplicates() {
    let mut tracker = InFlightTracker::new();
    let file_hash = [0x55; 16];

    assert!(tracker.is_empty());
    assert_eq!(tracker.len(), 0);
    assert!(tracker.can_request());

    // Insert 8 requests
    for i in 0..8 {
        let req = BlockRequest::new(
            file_hash,
            (i * ED2K_BLOCK_SIZE) as u64,
            ED2K_BLOCK_SIZE as u32,
        );
        tracker.record_request(&req).expect("should accept up to 8");
    }

    assert_eq!(tracker.len(), 8);
    assert!(!tracker.can_request());

    // 9th request must be rejected
    let extra_req = BlockRequest::new(
        file_hash,
        (8 * ED2K_BLOCK_SIZE) as u64,
        ED2K_BLOCK_SIZE as u32,
    );
    let err = tracker.record_request(&extra_req).unwrap_err();
    assert!(matches!(err, InFlightError::CapacityReached { limit: 8 }));

    // Duplicate offset rejected
    let mut tracker2 = InFlightTracker::new();
    let r1 = BlockRequest::new(file_hash, 0, 1024);
    tracker2.record_request(&r1).unwrap();
    let dup_err = tracker2.record_request(&r1).unwrap_err();
    assert!(matches!(
        dup_err,
        InFlightError::DuplicateRequest { start_offset: 0 }
    ));
}

#[test]
fn test_in_flight_out_of_order_and_duplicates() {
    let mut tracker = InFlightTracker::new();
    let file_hash = [0x11; 16];

    let req_a = BlockRequest::new(file_hash, 0, 1000);
    let req_b = BlockRequest::new(file_hash, 1000, 1000);
    let req_c = BlockRequest::new(file_hash, 2000, 1000);

    tracker.record_request(&req_a).unwrap();
    tracker.record_request(&req_b).unwrap();
    tracker.record_request(&req_c).unwrap();
    assert_eq!(tracker.len(), 3);

    // 1. Delivery in reverse order [C, A, B]
    tracker
        .validate_and_remove(&file_hash, 2000, 1000)
        .expect("C accepted out of order");
    assert_eq!(tracker.len(), 2);

    tracker
        .validate_and_remove(&file_hash, 0, 1000)
        .expect("A accepted out of order");
    assert_eq!(tracker.len(), 1);

    tracker
        .validate_and_remove(&file_hash, 1000, 1000)
        .expect("B accepted out of order");
    assert_eq!(tracker.len(), 0);
    assert!(tracker.is_empty());

    // 2. Duplicate arrival of block A after consumption
    let dup_err = tracker
        .validate_and_remove(&file_hash, 0, 1000)
        .unwrap_err();
    assert!(matches!(
        dup_err,
        InFlightError::UnsolicitedBlock { start_offset: 0 }
    ));

    // 3. Cross-correlation mismatch: client requested A (0), source sends B (1000)
    let mut tracker3 = InFlightTracker::new();
    tracker3.record_request(&req_a).unwrap();

    let cross_err = tracker3
        .validate_and_remove(&file_hash, 1000, 1000)
        .unwrap_err();
    assert!(matches!(
        cross_err,
        InFlightError::UnsolicitedBlock { start_offset: 1000 }
    ));
}

#[test]
fn test_capacity_lifecycle_timeouts_and_rejections() {
    let mut tracker = InFlightTracker::new();
    let file_hash = [0x22; 16];

    // Fill all 8 requests
    for i in 0..8 {
        tracker
            .record_request(&BlockRequest::new(file_hash, i * 1000, 1000))
            .unwrap();
    }
    assert!(!tracker.can_request());

    // 1. Acceptance frees 1 slot
    tracker.validate_and_remove(&file_hash, 0, 1000).unwrap();
    assert!(tracker.can_request());
    assert_eq!(tracker.len(), 7);

    // 2. Legitimate rejection frees slot and returns request
    let rejected_req = tracker
        .handle_rejection(&file_hash, 1000)
        .expect("valid rejection");
    assert_eq!(rejected_req.start_offset, 1000);
    assert_eq!(rejected_req.length, 1000);
    assert_eq!(tracker.len(), 6);

    // 3. Unsolicited rejection triggers violation
    let unasked_err = tracker.handle_rejection(&file_hash, 999_999).unwrap_err();
    assert!(matches!(
        unasked_err,
        InFlightError::UnsolicitedRejection {
            start_offset: 999_999
        }
    ));

    // 4. Timeouts reaping
    // Requests created with 0 timeout are immediately reaped
    let reaped = tracker.reap_timeouts(Duration::from_millis(0));
    assert_eq!(reaped.len(), 6);
    assert!(tracker.is_empty());
    assert!(tracker.can_request());

    // 5. Cancellation on peer disconnect
    for i in 0..4 {
        tracker
            .record_request(&BlockRequest::new(file_hash, i * 1000, 1000))
            .unwrap();
    }
    assert_eq!(tracker.len(), 4);

    let cancelled = tracker.cancel_all();
    assert_eq!(cancelled.len(), 4);
    assert!(tracker.is_empty());
    assert!(tracker.can_request());
}

#[test]
fn test_lab_source_engine_serving_and_bounds() {
    let mut engine = LabSourceEngine::new();
    let file_hash = [0x99; 16];
    let payload = vec![0xAB; 500_000]; // 500 KB test file

    engine.host_file(file_hash, "test_file.bin", Bytes::from(payload.clone()));

    // 1. Offer lookup
    let offer = engine.offer_file(&file_hash).expect("file offer exists");
    assert!(matches!(
        offer,
        LabMessage::FileOffer {
            file_size: 500_000,
            ..
        }
    ));

    // 2. Valid block serving
    let block_msg = engine.serve_block(&file_hash, 0, 180_000);
    match block_msg {
        LabMessage::BlockData {
            file_hash: h,
            start_offset: off,
            data,
        } => {
            assert_eq!(h, file_hash);
            assert_eq!(off, 0);
            assert_eq!(data.len(), 180_000);
            assert_eq!(data.as_ref(), &payload[0..180_000]);
        }
        other => panic!("expected BlockData, got {:?}", other),
    }

    // 3. Unknown file
    let unknown_hash = [0x00; 16];
    let rejected_unknown = engine.serve_block(&unknown_hash, 0, 1024);
    assert!(matches!(
        rejected_unknown,
        LabMessage::BlockRejected {
            reason: LabRejectReason::FileNotFound,
            ..
        }
    ));

    // 4. Out of bounds request
    let rejected_bounds = engine.serve_block(&file_hash, 450_000, 100_000); // 450k + 100k = 550k > 500k
    assert!(matches!(
        rejected_bounds,
        LabMessage::BlockRejected {
            reason: LabRejectReason::InvalidRange,
            ..
        }
    ));

    // 5. Oversized block request exceeding MAX_BLOCK_BYTES
    let rejected_oversized = engine.serve_block(&file_hash, 0, (MAX_BLOCK_BYTES + 1) as u32);
    assert!(matches!(
        rejected_oversized,
        LabMessage::BlockRejected {
            reason: LabRejectReason::InvalidRange,
            ..
        }
    ));
}
