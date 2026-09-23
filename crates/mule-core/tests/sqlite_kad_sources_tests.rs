// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use mule_core::kad::{KadId, KadIndexedSource};
use mule_core::persistence::Database;
use std::net::Ipv4Addr;

#[test]
fn test_kad_sources_indexing_and_ttl_purging() {
    let mut db = Database::open_in_memory().expect("open in-memory db");
    let file_hash = [0xABu8; 16];

    // 1. Initial count is 0
    assert_eq!(db.count_kad_indexed_sources().unwrap(), 0);

    let src1 = KadIndexedSource {
        file_hash,
        source_kad_id: KadId::new([0x11; 16]),
        ip: Ipv4Addr::new(127, 0, 0, 1),
        udp_port: 39737,
        tcp_port: 39735,
        file_size: 1_000_000,
        encryption: 0,
        expires_at_unix_ms: 10_000,
        updated_at_unix_ms: 1_000,
    };

    let src2 = KadIndexedSource {
        file_hash,
        source_kad_id: KadId::new([0x22; 16]),
        ip: Ipv4Addr::new(127, 0, 0, 2),
        udp_port: 4673,
        tcp_port: 4663,
        file_size: 1_000_000,
        encryption: 1,
        expires_at_unix_ms: 20_000,
        updated_at_unix_ms: 2_000,
    };

    // 2. Insert batch
    let inserted = db
        .insert_kad_indexed_sources(&[src1.clone(), src2.clone()])
        .unwrap();
    assert_eq!(inserted, 2);
    assert_eq!(db.count_kad_indexed_sources().unwrap(), 2);

    // 3. Query at now = 5_000 -> both active
    let found = db.get_kad_indexed_sources(&file_hash, 50, 5_000).unwrap();
    assert_eq!(found.len(), 2);

    // 4. Query at now = 15_000 -> src1 expired
    let found_partial = db.get_kad_indexed_sources(&file_hash, 50, 15_000).unwrap();
    assert_eq!(found_partial.len(), 1);
    assert_eq!(found_partial[0].source_kad_id, src2.source_kad_id);

    // 5. Purge expired at 15_000
    let purged = db.purge_expired_kad_sources(15_000).unwrap();
    assert_eq!(purged, 1);
    assert_eq!(db.count_kad_indexed_sources().unwrap(), 1);
}
