// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::fs;
use tempfile::tempdir;

use mule_core::{CoreError, Database};

#[test]
fn test_corrupted_sqlite_file_fails_explicitly_without_overwriting() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("mule.db");

    let garbage_data = b"Corrupted SQLite header and payload bytes \x00\xFF\xFE\xFD\xAA\x55";
    fs::write(&db_path, garbage_data).expect("write garbage data");

    // Opening a corrupted DB file must return an explicit Sqlite error
    let res = Database::open(&db_path);
    assert!(res.is_err(), "Opening corrupted DB must fail");

    match res.unwrap_err() {
        CoreError::Database(err_msg) => {
            // Error must be a SQLite error (not a panic or silent success)
            assert!(
                err_msg.contains("file is not a database")
                    || err_msg.contains("corrupt")
                    || err_msg.contains("unsupported file format")
                    || err_msg.contains("database disk image is malformed"),
                "Unexpected error message: {err_msg}"
            );
        }
        other => panic!("Expected CoreError::Database, got {other:?}"),
    }

    // CRITICAL: The corrupt file MUST NOT have been deleted or overwritten!
    assert!(
        db_path.exists(),
        "Corrupted DB file must still exist on disk"
    );
    let remaining_bytes = fs::read(&db_path).expect("read remaining file");
    assert_eq!(
        remaining_bytes, garbage_data,
        "Corrupted database file must not be modified, overwritten, or truncated"
    );
}
