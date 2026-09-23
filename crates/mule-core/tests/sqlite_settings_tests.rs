// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use mule_core::{Database, Download, DownloadStatus};
use mule_format::Ed2kHash;
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn test_sqlite_settings_crud_and_persistence() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_settings.db");

    // 1. Initial write
    {
        let mut db = Database::open(&db_path).unwrap();
        assert_eq!(db.get_setting("test_key").unwrap(), None);

        db.set_setting("test_key", "test_value_123").unwrap();
        assert_eq!(
            db.get_setting("test_key").unwrap(),
            Some("test_value_123".to_string())
        );

        // Update
        db.set_setting("test_key", "test_value_updated").unwrap();
        assert_eq!(
            db.get_setting("test_key").unwrap(),
            Some("test_value_updated".to_string())
        );
    }

    // 2. Re-open and verify persistence
    {
        let mut db = Database::open(&db_path).unwrap();
        assert_eq!(
            db.get_setting("test_key").unwrap(),
            Some("test_value_updated".to_string())
        );

        // Delete
        assert!(db.remove_setting("test_key").unwrap());
        assert_eq!(db.get_setting("test_key").unwrap(), None);
        assert!(!db.remove_setting("test_key").unwrap());
    }

    // 3. Re-open again and verify deletion
    {
        let db = Database::open(&db_path).unwrap();
        assert_eq!(db.get_setting("test_key").unwrap(), None);
    }
}

#[test]
fn test_sqlite_update_target_path_persists() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_target_path.db");
    let initial_target = dir.path().join("initial_downloads").join("file.bin");
    let new_target = dir.path().join("custom_downloads").join("file.bin");
    let temp_path = dir.path().join("temp").join("dl.part");
    std::fs::create_dir_all(temp_path.parent().unwrap()).unwrap();

    let dl_id = Uuid::new_v4();
    let download = Download {
        id: dl_id,
        display_name: "file.bin".to_string(),
        total_size: 1024,
        ed2k_hash: Ed2kHash([1u8; 16]),
        aich_root: None,
        target_path: initial_target.clone(),
        temp_path: temp_path.clone(),
        status: DownloadStatus::Downloading,
        created_at: 1000,
        updated_at: 1000,
        category_id: None,
        priority: "normal".to_string(),
        max_sources: None,
    };

    // 1. Insert download
    {
        let mut db = Database::open(&db_path).unwrap();
        db.insert_download(&download).unwrap();
        let loaded = db.get_all_downloads().unwrap();
        assert_eq!(loaded[0].target_path, initial_target);

        // Update target path
        db.update_target_path(&dl_id, &new_target).unwrap();
    }

    // 2. Reopen and verify target path updated while other fields preserved
    {
        let db = Database::open(&db_path).unwrap();
        let loaded = db.get_all_downloads().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, dl_id);
        assert_eq!(loaded[0].target_path, new_target);
        assert_eq!(loaded[0].temp_path, temp_path);
        assert_eq!(loaded[0].total_size, 1024);
        assert_eq!(loaded[0].status, DownloadStatus::Downloading);
    }
}
