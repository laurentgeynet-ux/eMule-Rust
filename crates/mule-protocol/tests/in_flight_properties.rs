use mule_protocol::lab::{InFlightError, InFlightTracker};
use mule_protocol::traits::BlockRequest;
use proptest::prelude::*;

proptest! {
    #[test]
    fn prop_in_flight_capacity_and_length_invariants(
        req_counts in 0usize..20,
        offsets in prop::collection::vec(0u64..1_000_000, 20),
    ) {
        let mut tracker = InFlightTracker::new();
        let dummy_hash = [0x42; 16];
        let mut recorded = Vec::new();

        #[allow(clippy::len_zero)]
        for &offset in offsets.iter().take(req_counts) {
            let req = BlockRequest::new(dummy_hash, offset, 1000);

            let res = tracker.record_request(&req);
            if recorded.len() >= 8 {
                prop_assert_eq!(res, Err(InFlightError::CapacityReached { limit: 8 }));
            } else if recorded.contains(&offset) {
                prop_assert_eq!(res, Err(InFlightError::DuplicateRequest { start_offset: offset }));
            } else {
                prop_assert!(res.is_ok());
                recorded.push(offset);
            }

            // Invariants:
            prop_assert!(tracker.len() <= 8);
            prop_assert_eq!(tracker.len(), recorded.len());
            prop_assert_eq!(tracker.can_request(), tracker.len() < 8);
            prop_assert_eq!(tracker.is_empty(), tracker.len() == 0);
        }

        // Completion phase:
        for offset in recorded {
            prop_assert!(tracker.validate_and_remove(&dummy_hash, offset, 1000).is_ok());
            // An already completed block cannot be completed twice
            prop_assert_eq!(
                tracker.validate_and_remove(&dummy_hash, offset, 1000),
                Err(InFlightError::UnsolicitedBlock { start_offset: offset })
            );
        }

        prop_assert_eq!(tracker.len(), 0);
        prop_assert!(tracker.is_empty());
        prop_assert!(tracker.can_request());
    }

    #[test]
    fn prop_in_flight_cancel_all_restores_full_capacity(
        offsets in prop::collection::hash_set(0u64..100_000, 1..=8),
    ) {
        let mut tracker = InFlightTracker::new();
        let dummy_hash = [0x55; 16];
        let expected_len = offsets.len();

        for &offset in &offsets {
            let req = BlockRequest::new(dummy_hash, offset, 2048);
            prop_assert!(tracker.record_request(&req).is_ok());
        }

        prop_assert_eq!(tracker.len(), expected_len);

        let cancelled = tracker.cancel_all();
        prop_assert_eq!(cancelled.len(), expected_len);
        prop_assert_eq!(tracker.len(), 0);
        prop_assert!(tracker.is_empty());
        prop_assert!(tracker.can_request());
    }
}
