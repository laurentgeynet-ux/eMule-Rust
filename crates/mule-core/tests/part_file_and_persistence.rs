// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use md4::{Digest, Md4};
use mule_core::{
    finalize_file, verify_part_hash, AllocationPolicy, CollisionStrategy, CoreError, Database,
    Download, DownloadStatus, PartFile, RangeSet,
};
use mule_format::Ed2kHash;
use tempfile::tempdir;

#[test]
fn test_download_storage_persistence_and_corruption_lifecycle() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("mule_state.db");
    let temp_part_path = dir.path().join("001.part");
    let target_dest_path = dir.path().join("final_document.bin");

    let total_size = 10_000u64;
    let dummy_hash = Ed2kHash([0xab; 16]);

    // 1. Initial process: open Database and PartFile
    let download = Download::new(
        "final_document.bin",
        total_size,
        dummy_hash,
        None,
        target_dest_path.clone(),
        temp_part_path.clone(),
    );
    let download_id = download.id;

    {
        let mut db = Database::open(&db_path).unwrap();
        db.insert_download(&download).unwrap();

        let mut part =
            PartFile::open_or_create(&temp_part_path, total_size, AllocationPolicy::None).unwrap();

        // 2. Write segments out of order:
        // Segment B: offset 5000..10000
        let seg_b = vec![0x22; 5000];
        part.write_segment(5000, &seg_b).unwrap();

        // Segment A: offset 0..5000
        let seg_a = vec![0x11; 5000];
        part.write_segment(0, &seg_a).unwrap();

        part.flush().unwrap();

        // 3. Update RangeSet and persist
        let mut range_set = RangeSet::new();
        range_set.insert(5000..10000).unwrap();
        range_set.insert(0..5000).unwrap();
        assert_eq!(range_set.ranges(), vec![0..10000]);

        db.sync_received_ranges(&download_id, &range_set).unwrap();
    } // Simulated process shutdown/crash

    // 4. Restart: Reopen DB and PartFile
    {
        let mut db = Database::open(&db_path).unwrap();
        let loaded_ranges = db.load_received_ranges(&download_id).unwrap();
        assert_eq!(loaded_ranges.ranges(), vec![0..10000]);
        assert_eq!(loaded_ranges.covered_len(), total_size);

        let mut part =
            PartFile::open_or_create(&temp_part_path, total_size, AllocationPolicy::None).unwrap();
        let read_back = part.read_range(0, total_size as usize).unwrap();

        // 5. Verify part MD4
        let mut expected_md4 = Md4::new();
        expected_md4.update(vec![0x11; 5000]);
        expected_md4.update(vec![0x22; 5000]);
        let expected_digest: [u8; 16] = expected_md4.finalize().into();

        assert!(verify_part_hash(&read_back, &expected_digest));
        db.mark_part_verified(&download_id, 0).unwrap();
        assert_eq!(db.get_verified_parts(&download_id).unwrap(), vec![0]);

        // 6. Inject corruption: flip 1 byte at offset 100
        part.write_segment(100, &[0xff]).unwrap();
        part.flush().unwrap();

        let corrupted_read = part.read_range(0, total_size as usize).unwrap();
        assert!(!verify_part_hash(&corrupted_read, &expected_digest));

        // Part corrupted -> unmark part and evict range
        db.unmark_part_verified(&download_id, 0).unwrap();
        assert!(db.get_verified_parts(&download_id).unwrap().is_empty());

        let mut updated_ranges = loaded_ranges;
        updated_ranges.remove(0..total_size).unwrap();
        db.sync_received_ranges(&download_id, &updated_ranges)
            .unwrap();
        assert!(db.load_received_ranges(&download_id).unwrap().is_empty());

        // 7. Repair: rewrite valid byte back
        part.write_segment(100, &[0x11]).unwrap();
        part.flush().unwrap();

        let repaired_read = part.read_range(0, total_size as usize).unwrap();
        assert!(verify_part_hash(&repaired_read, &expected_digest));

        updated_ranges.insert(0..total_size).unwrap();
        db.sync_received_ranges(&download_id, &updated_ranges)
            .unwrap();
        db.mark_part_verified(&download_id, 0).unwrap();
        db.update_status(&download_id, DownloadStatus::Completed)
            .unwrap();
    }

    // 8. Finalize file
    // First finalize: should succeed
    finalize_file(
        &temp_part_path,
        &target_dest_path,
        CollisionStrategy::FailIfExists,
    )
    .unwrap();
    assert!(target_dest_path.exists());
    assert!(!temp_part_path.exists());

    // Second finalize with FailIfExists when target exists: must return TargetFileExists
    let dummy_temp = dir.path().join("dummy.part");
    std::fs::write(&dummy_temp, b"test").unwrap();
    assert!(matches!(
        finalize_file(
            &dummy_temp,
            &target_dest_path,
            CollisionStrategy::FailIfExists
        )
        .unwrap_err(),
        CoreError::TargetFileExists(_)
    ));

    // Overwrite strategy: succeeds
    finalize_file(&dummy_temp, &target_dest_path, CollisionStrategy::Overwrite).unwrap();
}

/// eMule `CPartFile::PerformFileComplete`: a completed download never
/// overwrites an existing destination — it generates `name(1).ext`,
/// `name(2).ext`, … continuing numbering from an existing `(N)` suffix.
#[test]
fn test_finalize_rename_collision() {
    let dir = tempdir().unwrap();

    let target = dir.path().join("file.bin");
    std::fs::write(&target, b"existing").unwrap();

    let temp = dir.path().join("001.part");
    std::fs::write(&temp, b"newdata").unwrap();
    let resolved = finalize_file(&temp, &target, CollisionStrategy::Rename).unwrap();
    assert_eq!(
        resolved.file_name().unwrap().to_str().unwrap(),
        "file(1).bin"
    );
    assert_eq!(std::fs::read(&resolved).unwrap(), b"newdata");
    // The pre-existing file is untouched.
    assert_eq!(std::fs::read(&target).unwrap(), b"existing");

    // Second collision: numbering continues at (2).
    let temp2 = dir.path().join("002.part");
    std::fs::write(&temp2, b"newer").unwrap();
    let resolved2 = finalize_file(&temp2, &target, CollisionStrategy::Rename).unwrap();
    assert_eq!(
        resolved2.file_name().unwrap().to_str().unwrap(),
        "file(2).bin"
    );

    // A stem that already ends with `(N)` continues numbering from N+1:
    // eMule strips `file(7)` back to `file` before incrementing.
    let numbered = dir.path().join("file(7).bin");
    std::fs::write(&numbered, b"exists").unwrap();
    let temp3 = dir.path().join("003.part");
    std::fs::write(&temp3, b"payload").unwrap();
    let resolved3 = finalize_file(&temp3, &numbered, CollisionStrategy::Rename).unwrap();
    assert_eq!(
        resolved3.file_name().unwrap().to_str().unwrap(),
        "file(8).bin"
    );

    // Extensionless file: `name(N)` without extension.
    let noext = dir.path().join("README");
    std::fs::write(&noext, b"old").unwrap();
    let temp4 = dir.path().join("004.part");
    std::fs::write(&temp4, b"new").unwrap();
    let resolved4 = finalize_file(&temp4, &noext, CollisionStrategy::Rename).unwrap();
    assert_eq!(
        resolved4.file_name().unwrap().to_str().unwrap(),
        "README(1)"
    );

    // No collision: the requested path is used unchanged.
    let free = dir.path().join("free.bin");
    let temp5 = dir.path().join("005.part");
    std::fs::write(&temp5, b"data").unwrap();
    let resolved5 = finalize_file(&temp5, &free, CollisionStrategy::Rename).unwrap();
    assert_eq!(resolved5, free);
}

#[test]
fn test_get_all_downloads_restoration() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("persistence_test.db");

    let dl1 = Download::new(
        "file1.bin",
        1234,
        Ed2kHash([0x11; 16]),
        None,
        dir.path().join("file1.bin"),
        dir.path().join("001.part"),
    );
    let dl2 = Download::new(
        "file2.bin",
        5678,
        Ed2kHash([0x22; 16]),
        None,
        dir.path().join("file2.bin"),
        dir.path().join("002.part"),
    );

    {
        let mut db = Database::open(&db_path).unwrap();
        db.insert_download(&dl1).unwrap();
        db.insert_download(&dl2).unwrap();
    }

    {
        let db = Database::open(&db_path).unwrap();
        let list = db.get_all_downloads().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, dl1.id);
        assert_eq!(list[0].display_name, "file1.bin");
        assert_eq!(list[0].total_size, 1234);
        assert_eq!(list[0].ed2k_hash, Ed2kHash([0x11; 16]));

        assert_eq!(list[1].id, dl2.id);
        assert_eq!(list[1].display_name, "file2.bin");
        assert_eq!(list[1].total_size, 5678);
        assert_eq!(list[1].ed2k_hash, Ed2kHash([0x22; 16]));
    }
}

#[test]
fn test_database_delete_download_persists_across_reopen() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("mule_delete.db");

    let dl = Download::new(
        "tobedeleted.bin",
        4096,
        Ed2kHash([0x33; 16]),
        None,
        dir.path().join("tobedeleted.bin"),
        dir.path().join("del.part"),
    );
    let dl_id = dl.id;

    // 1. Insert download, received ranges, and verified parts
    {
        let mut db = Database::open(&db_path).unwrap();
        db.insert_download(&dl).unwrap();

        let mut ranges = RangeSet::new();
        ranges.insert(0..2048).unwrap();
        db.sync_received_ranges(&dl_id, &ranges).unwrap();

        db.mark_part_verified(&dl_id, 0).unwrap();

        let all = db.get_all_downloads().unwrap();
        assert_eq!(all.len(), 1);
    }

    // 2. Delete the download
    {
        let mut db = Database::open(&db_path).unwrap();
        let deleted = db.delete_download(&dl_id).unwrap();
        assert!(deleted);
    }

    // 3. Reopen DB in a fresh instance (simulating restart)
    {
        let db = Database::open(&db_path).unwrap();
        let all = db.get_all_downloads().unwrap();
        assert_eq!(
            all.len(),
            0,
            "Deleted download must not exist after restart"
        );

        let ranges = db.load_received_ranges(&dl_id).unwrap();
        assert!(ranges.is_empty(), "Ranges must be purged on delete");

        let parts = db.get_verified_parts(&dl_id).unwrap();
        assert!(parts.is_empty(), "Verified parts must be purged on delete");
    }
}
