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
fn test_in_flight_tracker_validation_and_rejections() {
    let mut tracker = InFlightTracker::new();
    let file_hash = [0x77; 16];

    let req = BlockRequest::new(file_hash, 10_000, 1024);
    tracker.record_request(&req).unwrap();

    // 1. Unsolicited block rejection
    let unsolicited_err = tracker
        .validate_and_remove(&file_hash, 20_000, 1024)
        .unwrap_err();
    assert!(matches!(
        unsolicited_err,
        InFlightError::UnsolicitedBlock {
            start_offset: 20_000
        }
    ));

    // 2. Length mismatch rejection
    let length_err = tracker
        .validate_and_remove(&file_hash, 10_000, 512)
        .unwrap_err();
    assert!(matches!(
        length_err,
        InFlightError::LengthMismatch {
            start_offset: 10_000,
            expected: 1024,
            actual: 512
        }
    ));

    // 3. Exact match succeeds and clears entry
    tracker
        .validate_and_remove(&file_hash, 10_000, 1024)
        .expect("valid match");
    assert!(tracker.is_empty());
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
