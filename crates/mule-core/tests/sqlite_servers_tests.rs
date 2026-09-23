// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use mule_core::model::{Ed2kServer, ServerFilterStatus};
use mule_core::persistence::Database;

#[test]
fn test_ed2k_servers_crud_operations() {
    let mut db = Database::open_in_memory().expect("open DB in memory");

    // Initially empty
    let list = db.list_servers().expect("list servers");
    assert_eq!(list.len(), 0);

    // Upsert two servers
    let s1 = Ed2kServer {
        address: "192.168.1.10:4661".to_string(),
        ip: "192.168.1.10".to_string(),
        port: 4661,
        name: "Test Server 1".to_string(),
        description: "Primary testing server".to_string(),
        ping: 25,
        preference: 10,
        failed_count: 0,
        dyn_ip: None,
        version: Some("17.15".to_string()),
        filter_status: ServerFilterStatus::Allowed,
        last_seen_unix_ms: None,
        created_at_unix_ms: 1000,
        updated_at_unix_ms: 1000,
    };

    let s2 = Ed2kServer {
        address: "10.0.0.5:4242".to_string(),
        ip: "10.0.0.5".to_string(),
        port: 4242,
        name: "Blocked Server".to_string(),
        description: "".to_string(),
        ping: 150,
        preference: 5,
        failed_count: 1,
        dyn_ip: None,
        version: None,
        filter_status: ServerFilterStatus::BlockedByFilter,
        last_seen_unix_ms: None,
        created_at_unix_ms: 1000,
        updated_at_unix_ms: 1000,
    };

    let inserted = db
        .upsert_servers(&[s1.clone(), s2.clone()])
        .expect("upsert servers");
    assert_eq!(inserted, 2);

    let list = db.list_servers().expect("list servers");
    assert_eq!(list.len(), 2);
    // Ordered by preference DESC: s1 (preference 10) comes first
    assert_eq!(list[0].address, "192.168.1.10:4661");
    assert_eq!(list[0].filter_status, ServerFilterStatus::Allowed);
    assert_eq!(list[1].address, "10.0.0.5:4242");
    assert_eq!(list[1].filter_status, ServerFilterStatus::BlockedByFilter);

    // Toggle enabled -> disabled on s1
    let updated = db
        .set_server_enabled("192.168.1.10:4661", false)
        .expect("disable server");
    assert!(updated);

    let list = db.list_servers().expect("list servers");
    assert_eq!(list[0].filter_status, ServerFilterStatus::Disabled);

    // Upsert again should preserve disabled status
    let s1_update = Ed2kServer {
        name: "Test Server 1 Renamed".to_string(),
        filter_status: ServerFilterStatus::Allowed, // Re-imported file might say allowed
        ..s1
    };
    db.upsert_servers(&[s1_update]).expect("upsert updated");

    let list = db.list_servers().expect("list servers");
    assert_eq!(list[0].name, "Test Server 1 Renamed");
    assert_eq!(
        list[0].filter_status,
        ServerFilterStatus::Disabled,
        "Should preserve manually disabled status"
    );

    // Toggle back to enabled
    db.set_server_enabled("192.168.1.10:4661", true)
        .expect("enable server");
    let list = db.list_servers().expect("list servers");
    assert_eq!(list[0].filter_status, ServerFilterStatus::Allowed);

    // Delete single server
    let deleted = db.delete_server("10.0.0.5:4242").expect("delete server");
    assert!(deleted);
    assert_eq!(db.list_servers().expect("list").len(), 1);

    // Clear servers
    let cleared = db.clear_servers().expect("clear servers");
    assert_eq!(cleared, 1);
    assert_eq!(db.list_servers().expect("list").len(), 0);
}
