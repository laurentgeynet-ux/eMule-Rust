use mule_core::{Database, HistoryEntry, HistoryFilter};

#[test]
fn test_history_insert_and_query_filtering() {
    let mut db = Database::open_in_memory().expect("open DB");
    let now = 1_700_000_000_000i64;

    let e1 = HistoryEntry {
        id: 0,
        download_id: "dl-1".to_string(),
        file_name: "debian.iso".to_string(),
        total_size: 1000,
        transferred_bytes: 1000,
        ed2k_hash: "0123456789abcdef0123456789abcdef".to_string(),
        event_type: "completed".to_string(),
        status: "completed".to_string(),
        duration_secs: 15,
        connection_attempts: 1,
        resume_count: 0,
        verification_result: "verified_ok".to_string(),
        error_message: None,
        timestamp_unix_ms: now,
    };

    let e2 = HistoryEntry {
        id: 0,
        download_id: "dl-2".to_string(),
        file_name: "corrupted.bin".to_string(),
        total_size: 2000,
        transferred_bytes: 2000,
        ed2k_hash: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        event_type: "failed".to_string(),
        status: "failed".to_string(),
        duration_secs: 30,
        connection_attempts: 3,
        resume_count: 1,
        verification_result: "hash_mismatch".to_string(),
        error_message: Some("Root hash mismatch".to_string()),
        timestamp_unix_ms: now + 1000,
    };

    let e3 = HistoryEntry {
        id: 0,
        download_id: "dl-1".to_string(),
        file_name: "debian.iso".to_string(),
        total_size: 1000,
        transferred_bytes: 500,
        ed2k_hash: "0123456789abcdef0123456789abcdef".to_string(),
        event_type: "resumed".to_string(),
        status: "downloading".to_string(),
        duration_secs: 5,
        connection_attempts: 1,
        resume_count: 1,
        verification_result: "unverified".to_string(),
        error_message: None,
        timestamp_unix_ms: now - 5000,
    };

    let id1 = db.insert_history_entry(&e1).expect("insert e1");
    let id2 = db.insert_history_entry(&e2).expect("insert e2");
    let id3 = db.insert_history_entry(&e3).expect("insert e3");

    assert!(id1 > 0);
    assert!(id2 > id1);
    assert!(id3 > id2);

    // Query all (sorted desc by timestamp)
    let all = db
        .query_history(&HistoryFilter::default())
        .expect("query all");
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].event_type, "failed"); // newest timestamp
    assert_eq!(all[1].event_type, "completed");
    assert_eq!(all[2].event_type, "resumed"); // oldest timestamp

    // Filter by event_type
    let failed_filter = HistoryFilter {
        event_type: Some("failed".to_string()),
        ..Default::default()
    };
    let failed_list = db.query_history(&failed_filter).expect("query failed");
    assert_eq!(failed_list.len(), 1);
    assert_eq!(failed_list[0].file_name, "corrupted.bin");
    assert_eq!(failed_list[0].verification_result, "hash_mismatch");

    // Filter by timestamp range
    let range_filter = HistoryFilter {
        from_timestamp_ms: Some(now),
        to_timestamp_ms: Some(now + 2000),
        ..Default::default()
    };
    let range_list = db.query_history(&range_filter).expect("query range");
    assert_eq!(range_list.len(), 2);
}

#[test]
fn test_history_stats_aggregation() {
    let mut db = Database::open_in_memory().expect("open DB");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let session_start = now_ms - 60_000; // 1 minute ago

    // Insert completed in session
    db.insert_history_entry(&HistoryEntry {
        id: 0,
        download_id: "dl-1".to_string(),
        file_name: "file1.iso".to_string(),
        total_size: 500_000_000,
        transferred_bytes: 500_000_000,
        ed2k_hash: "hash1".to_string(),
        event_type: "completed".to_string(),
        status: "completed".to_string(),
        duration_secs: 100,
        connection_attempts: 2,
        resume_count: 1,
        verification_result: "verified_ok".to_string(),
        error_message: None,
        timestamp_unix_ms: now_ms - 10_000,
    })
    .expect("insert completed");

    // Insert failure
    db.insert_history_entry(&HistoryEntry {
        id: 0,
        download_id: "dl-2".to_string(),
        file_name: "bad.bin".to_string(),
        total_size: 10_000,
        transferred_bytes: 10_000,
        ed2k_hash: "hash2".to_string(),
        event_type: "failed".to_string(),
        status: "failed".to_string(),
        duration_secs: 10,
        connection_attempts: 1,
        resume_count: 0,
        verification_result: "hash_mismatch".to_string(),
        error_message: Some("Corrupted".to_string()),
        timestamp_unix_ms: now_ms - 5_000,
    })
    .expect("insert failed");

    // Insert resume
    db.insert_history_entry(&HistoryEntry {
        id: 0,
        download_id: "dl-1".to_string(),
        file_name: "file1.iso".to_string(),
        total_size: 500_000_000,
        transferred_bytes: 200_000_000,
        ed2k_hash: "hash1".to_string(),
        event_type: "resumed".to_string(),
        status: "downloading".to_string(),
        duration_secs: 50,
        connection_attempts: 1,
        resume_count: 1,
        verification_result: "unverified".to_string(),
        error_message: None,
        timestamp_unix_ms: now_ms - 30_000,
    })
    .expect("insert resume");

    let stats = db.get_history_stats(session_start).expect("get stats");
    assert_eq!(stats.total_completed_count, 1);
    assert_eq!(stats.total_completed_bytes, 500_000_000);
    assert_eq!(stats.total_failed_count, 1);
    assert_eq!(stats.total_resumes_count, 1);
    assert_eq!(stats.total_integrity_failures, 1);

    assert_eq!(stats.session_completed_count, 1);
    assert_eq!(stats.session_completed_bytes, 500_000_000);
    assert_eq!(stats.last_24h_completed_count, 1);
    assert_eq!(stats.last_24h_completed_bytes, 500_000_000);
    assert_eq!(stats.last_7d_completed_count, 1);
    assert_eq!(stats.last_7d_completed_bytes, 500_000_000);
}

#[test]
fn test_history_retention_and_clear() {
    let mut db = Database::open_in_memory().expect("open DB");
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let old_ts = now_ms - (40 * 86_400 * 1000); // 40 days ago
    let fresh_ts = now_ms - (5 * 86_400 * 1000); // 5 days ago

    db.insert_history_entry(&HistoryEntry {
        id: 0,
        download_id: "dl-old".to_string(),
        file_name: "old.iso".to_string(),
        total_size: 100,
        transferred_bytes: 100,
        ed2k_hash: "h_old".to_string(),
        event_type: "completed".to_string(),
        status: "completed".to_string(),
        duration_secs: 10,
        connection_attempts: 1,
        resume_count: 0,
        verification_result: "verified_ok".to_string(),
        error_message: None,
        timestamp_unix_ms: old_ts,
    })
    .expect("insert old");

    db.insert_history_entry(&HistoryEntry {
        id: 0,
        download_id: "dl-fresh".to_string(),
        file_name: "fresh.iso".to_string(),
        total_size: 200,
        transferred_bytes: 200,
        ed2k_hash: "h_fresh".to_string(),
        event_type: "completed".to_string(),
        status: "completed".to_string(),
        duration_secs: 20,
        connection_attempts: 1,
        resume_count: 0,
        verification_result: "verified_ok".to_string(),
        error_message: None,
        timestamp_unix_ms: fresh_ts,
    })
    .expect("insert fresh");

    assert_eq!(
        db.query_history(&HistoryFilter::default()).unwrap().len(),
        2
    );

    // Apply retention of 30 days: old entry (40 days ago) should be pruned
    let pruned = db.apply_retention_policy(30).expect("apply retention");
    assert_eq!(pruned, 1);

    let remaining = db.query_history(&HistoryFilter::default()).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].file_name, "fresh.iso");

    // Clear history
    let cleared = db.clear_history().expect("clear history");
    assert_eq!(cleared, 1);
    assert_eq!(
        db.query_history(&HistoryFilter::default()).unwrap().len(),
        0
    );
}
