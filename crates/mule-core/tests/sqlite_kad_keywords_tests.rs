use mule_core::kad::{hash_kad_keyword, KadKeywordEntry};
use mule_core::persistence::Database;

#[test]
fn test_kad_keywords_indexing_and_ttl_purging() {
    let mut db = Database::open_in_memory().expect("open in-memory db");

    let (norm_word, kad_id) = hash_kad_keyword("debian");
    assert_eq!(norm_word, "debian");
    let keyword_hash = kad_id.to_hex();

    let now_ms = 1_700_000_000_000u64;

    let entries = vec![
        KadKeywordEntry {
            keyword_hash: keyword_hash.clone(),
            file_hash: "11111111111111111111111111111111".to_string(),
            file_name: "debian-12.0.0-amd64-netinst.iso".to_string(),
            file_size: 650_000_000,
            source_count: 25,
            discovered_at_unix_ms: now_ms as i64,
            expires_at_unix_ms: (now_ms + 3_600_000) as i64, // expires in 1h
        },
        KadKeywordEntry {
            keyword_hash: keyword_hash.clone(),
            file_hash: "22222222222222222222222222222222".to_string(),
            file_name: "debian-11.0.0-amd64-netinst.iso".to_string(),
            file_size: 620_000_000,
            source_count: 10,
            discovered_at_unix_ms: now_ms as i64,
            expires_at_unix_ms: (now_ms - 1000) as i64, // already expired
        },
    ];

    let count = db.insert_kad_keyword_entries(&entries).expect("insert");
    assert_eq!(count, 2);

    // Get active entries (only unexpired should be returned)
    let active = db
        .get_kad_keyword_entries(&keyword_hash, now_ms)
        .expect("get active");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].file_name, "debian-12.0.0-amd64-netinst.iso");
    assert_eq!(active[0].source_count, 25);

    // Purge expired
    let purged = db.purge_expired_kad_keywords(now_ms).expect("purge");
    assert_eq!(purged, 1);

    // Total count remaining in table is 1
    let total = db.count_kad_keyword_entries().expect("count");
    assert_eq!(total, 1);
}
