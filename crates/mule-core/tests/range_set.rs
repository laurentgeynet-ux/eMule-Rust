use mule_core::{CoreError, RangeSet};

#[test]
fn test_single_insertion() {
    let mut set = RangeSet::new();
    set.insert(100..200).unwrap();

    assert_eq!(set.len(), 1);
    assert_eq!(set.covered_len(), 100);
    assert_eq!(set.ranges(), vec![100..200]);
    assert!(set.contains(100..200));
    assert!(set.contains(120..180));
    assert!(!set.contains(50..150));
}

#[test]
fn test_adjacent_merges() {
    let mut set = RangeSet::new();

    // 1. Insert [100, 200) and [300, 400)
    set.insert(100..200).unwrap();
    set.insert(300..400).unwrap();
    assert_eq!(set.len(), 2);
    assert_eq!(set.ranges(), vec![100..200, 300..400]);

    // 2. Insert [200, 300) -> simultaneous left + right merge into [100, 400)
    set.insert(200..300).unwrap();
    assert_eq!(set.len(), 1);
    assert_eq!(set.ranges(), vec![100..400]);
    assert_eq!(set.covered_len(), 300);
}

#[test]
fn test_partial_and_total_overlaps() {
    let mut set = RangeSet::new();

    set.insert(100..250).unwrap();
    // Partial overlap with right edge
    set.insert(200..350).unwrap();
    assert_eq!(set.ranges(), vec![100..350]);

    // Total overlap inside
    set.insert(150..250).unwrap();
    assert_eq!(set.ranges(), vec![100..350]);

    // Total overlap enclosing
    set.insert(50..400).unwrap();
    assert_eq!(set.ranges(), vec![50..400]);
    assert_eq!(set.covered_len(), 350);
}

#[test]
fn test_out_of_order_insertions() {
    let mut set = RangeSet::new();

    set.insert(500..600).unwrap();
    set.insert(100..200).unwrap();
    set.insert(300..400).unwrap();

    assert_eq!(set.ranges(), vec![100..200, 300..400, 500..600]);
    assert_eq!(set.covered_len(), 300);
}

#[test]
#[allow(clippy::reversed_empty_ranges)]
fn test_invalid_ranges_rejected() {
    let mut set = RangeSet::new();

    // Empty range [x, x)
    assert!(matches!(
        set.insert(100..100).unwrap_err(),
        CoreError::InvalidRange { .. }
    ));

    // Inverted range [start > end)
    assert!(matches!(
        set.insert(200..100).unwrap_err(),
        CoreError::InvalidRange { .. }
    ));
}

#[test]
fn test_missing_within_gaps() {
    let mut set = RangeSet::new();
    set.insert(100..200).unwrap();
    set.insert(300..400).unwrap();

    // Limit: 0..500
    let gaps = set.missing_within(0..500);
    assert_eq!(gaps, vec![0..100, 200..300, 400..500]);

    // Limit entirely inside covered range: no gaps
    assert!(set.missing_within(120..180).is_empty());

    // Limit entirely inside a missing gap
    assert_eq!(set.missing_within(220..280), vec![220..280]);
}

#[test]
fn test_range_removal() {
    let mut set = RangeSet::new();
    set.insert(100..500).unwrap();

    // Remove middle [200, 300) -> splits into [100, 200) and [300, 500)
    set.remove(200..300).unwrap();
    assert_eq!(set.ranges(), vec![100..200, 300..500]);
    assert_eq!(set.covered_len(), 300);

    // Remove left boundary [50, 150) -> [150, 200) and [300, 500)
    set.remove(50..150).unwrap();
    assert_eq!(set.ranges(), vec![150..200, 300..500]);

    // Remove entire [100, 600) -> empty set
    set.remove(100..600).unwrap();
    assert!(set.is_empty());
    assert_eq!(set.covered_len(), 0);
}
