use mule_core::Database;
use mule_daemon::services::downloads::DownloadService;
use mule_format::{PARTFILE_VERSION, met::tags::*};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

fn create_part_met_file(
    path: &std::path::Path,
    filename: &str,
    file_size: u64,
    transferred: u64,
    file_hash: [u8; 16],
    part_hashes: &[[u8; 16]],
    gaps: &[(u64, u64)],
) {
    let mut bytes = Vec::new();
    // Header
    bytes.push(PARTFILE_VERSION); // 0xe0
    bytes.extend_from_slice(&1_700_000_000u32.to_le_bytes()); // mod date
    bytes.extend_from_slice(&file_hash); // file hash

    // Part hashes count
    bytes.extend_from_slice(&(part_hashes.len() as u16).to_le_bytes());
    for h in part_hashes {
        bytes.extend_from_slice(h);
    }

    // Tags count: 3 (filename, filesize, transferred) + optional filesize_hi + gaps (2 tags per gap)
    let has_hi = file_size > u32::MAX as u64;
    let tag_count = 3 + (if has_hi { 1 } else { 0 }) + (gaps.len() as u32 * 2);
    bytes.extend_from_slice(&tag_count.to_le_bytes());

    // Tag: FT_FILENAME
    bytes.push(0x82); // string
    bytes.push(FT_FILENAME);
    let fn_bytes = filename.as_bytes();
    bytes.extend_from_slice(&(fn_bytes.len() as u16).to_le_bytes());
    bytes.extend_from_slice(fn_bytes);

    // Tag: FT_FILESIZE
    bytes.push(0x83); // uint32
    bytes.push(FT_FILESIZE);
    bytes.extend_from_slice(&(file_size as u32).to_le_bytes());

    if has_hi {
        bytes.push(0x83); // uint32
        bytes.push(FT_FILESIZE_HI);
        bytes.extend_from_slice(&((file_size >> 32) as u32).to_le_bytes());
    }

    // Tag: FT_TRANSFERRED
    bytes.push(0x83); // uint32
    bytes.push(FT_TRANSFERRED);
    bytes.extend_from_slice(&(transferred as u32).to_le_bytes());

    // Gaps
    for &(s, e) in gaps {
        // GAPSTART
        bytes.push(0x83);
        bytes.push(FT_GAPSTART);
        bytes.extend_from_slice(&(s as u32).to_le_bytes());

        // GAPEND
        bytes.push(0x83);
        bytes.push(FT_GAPEND);
        bytes.extend_from_slice(&(e as u32).to_le_bytes());
    }

    let mut f = File::create(path).unwrap();
    f.write_all(&bytes).unwrap();
}

#[test]
fn test_part_met_inspection_and_import_nominal() {
    let temp_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let download_target_dir = tempdir().unwrap();

    let db_path = temp_dir.path().join("mule.db");
    let mut service = DownloadService::new(&db_path, temp_dir.path().join("temp")).unwrap();

    // Create 1000 bytes file data and its eD2K hash
    let part_data = vec![0x42u8; 1000];
    let mut hasher = mule_format::ed2k::hash::Ed2kHasher::new();
    hasher.update(&part_data);
    let part_hash = hasher.finalize().unwrap().root_hash.0;

    let met_path = external_dir.path().join("001.part.met");
    let part_path = external_dir.path().join("001.part");

    // Create .part file
    std::fs::write(&part_path, &part_data).unwrap();

    // Create .part.met with no gaps (100% complete single part)
    create_part_met_file(
        &met_path,
        "sample.bin",
        1000,
        1000,
        part_hash,
        &[part_hash],
        &[], // no gaps
    );

    // 1. Test inspect_part_met
    let inspection = service.inspect_part_met(&met_path, None).unwrap();
    assert_eq!(inspection.file_name, "sample.bin");
    assert_eq!(inspection.total_size, 1000);
    assert_eq!(inspection.part_count, 1);
    assert!(inspection.part_file_exists);
    assert_eq!(inspection.part_file_size, Some(1000));
    assert_eq!(inspection.estimated_transferred_bytes, 1000);

    // 2. Test import_part_met
    let result = service
        .import_part_met(&met_path, None, download_target_dir.path(), None)
        .unwrap();

    // Guaranteed invariants:
    // Status MUST be paused!
    assert_eq!(result.download.status, "paused");
    assert_eq!(result.total_parts, 1);
    assert_eq!(result.verified_parts, 1);
    assert_eq!(result.corrupted_parts, 0);
    assert_eq!(result.transferred_bytes, 1000);

    // Verify temp file exists in internal temp directory
    let internal_part = temp_dir.path().join("temp").join(format!("{}.part", result.download.id));
    assert!(internal_part.exists());

    // Verify external file is still intact
    assert!(part_path.exists());
    assert_eq!(std::fs::metadata(&part_path).unwrap().len(), 1000);

    // Verify verified_parts in DB
    let db = Database::open(&db_path).unwrap();
    let verified = db.get_verified_parts(&result.download.id).unwrap();
    assert_eq!(verified, vec![0]);

    // Verify history event "imported" recorded
    let history = service
        .query_history(&mule_core::HistoryFilter {
            limit: Some(10),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].event_type, "imported");
    assert_eq!(history[0].verification_result, "verified_ok");
}

#[test]
fn test_part_met_import_with_corrupted_part() {
    let temp_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let download_target_dir = tempdir().unwrap();

    let db_path = temp_dir.path().join("mule.db");
    let mut service = DownloadService::new(&db_path, temp_dir.path().join("temp")).unwrap();

    // .part data has 0x99 bytes, but .part.met has hash for 0x42 bytes
    let part_data = vec![0x99u8; 1000];
    let fake_expected_hash = [0x55u8; 16];

    let met_path = external_dir.path().join("002.part.met");
    let part_path = external_dir.path().join("002.part");

    std::fs::write(&part_path, &part_data).unwrap();

    create_part_met_file(
        &met_path,
        "corrupted.bin",
        1000,
        1000,
        fake_expected_hash,
        &[fake_expected_hash],
        &[],
    );

    let result = service
        .import_part_met(&met_path, None, download_target_dir.path(), None)
        .unwrap();

    // Corrupted part was detected and evicted from ranges!
    assert_eq!(result.verified_parts, 0);
    assert_eq!(result.corrupted_parts, 1);
    assert_eq!(result.transferred_bytes, 0); // evicted from range set
    assert_eq!(result.download.status, "paused");
}

#[test]
fn test_part_met_import_oversized_physical_file_rejected() {
    let temp_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let download_target_dir = tempdir().unwrap();

    let db_path = temp_dir.path().join("mule.db");
    let mut service = DownloadService::new(&db_path, temp_dir.path().join("temp")).unwrap();

    let part_data = vec![0x11u8; 2000]; // 2000 bytes, but declared size is 1000
    let met_path = external_dir.path().join("003.part.met");
    let part_path = external_dir.path().join("003.part");

    std::fs::write(&part_path, &part_data).unwrap();

    create_part_met_file(
        &met_path,
        "oversized.bin",
        1000, // declared: 1000 bytes
        1000,
        [0; 16],
        &[[0; 16]],
        &[],
    );

    let err = service
        .import_part_met(&met_path, None, download_target_dir.path(), None)
        .unwrap_err();

    assert!(err.to_string().contains("exceeds declared file size"));
}

#[test]
fn test_part_met_import_persists_across_daemon_restart() {
    let temp_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let download_target_dir = tempdir().unwrap();

    let db_path = temp_dir.path().join("mule.db");
    let temp_staging_dir = temp_dir.path().join("temp");

    let part_data = vec![0x42u8; 1000];
    let mut hasher = mule_format::ed2k::hash::Ed2kHasher::new();
    hasher.update(&part_data);
    let part_hash = hasher.finalize().unwrap().root_hash.0;

    let met_path = external_dir.path().join("004.part.met");
    let part_path = external_dir.path().join("004.part");
    std::fs::write(&part_path, &part_data).unwrap();

    create_part_met_file(
        &met_path,
        "reboot_preserved.bin",
        1000,
        1000,
        part_hash,
        &[part_hash],
        &[],
    );

    let imported_id;
    {
        let mut service = DownloadService::new(&db_path, temp_staging_dir.clone()).unwrap();

        let result = service
            .import_part_met(
                &met_path,
                None,
                download_target_dir.path(),
                Some("cat-archives".to_string()),
            )
            .unwrap();

        assert_eq!(result.verified_parts, 1);
        assert_eq!(result.corrupted_parts, 0);
        assert_eq!(result.download.status, "paused");
        assert_eq!(result.download.category_id.as_deref(), Some("cat-archives"));

        let report = result.report.expect("Sanitized audit report must be present");
        assert!(report.contains("Format Version: 0xE0 (Standard eMule)"));
        assert!(report.contains("reboot_preserved.bin"));
        assert!(report.contains("STRICTLY PAUSED"));
        assert!(report.contains("Verified Valid Parts: 1"));

        imported_id = result.download.id;
        // Service dropped here, simulating daemon shutdown
    }

    // Reboot / restart daemon by recreating DownloadService on the same persistent database and temp dir
    {
        let fresh_service = DownloadService::new(&db_path, temp_staging_dir).unwrap();
        let downloads = fresh_service.list_downloads();
        let dl = downloads
            .iter()
            .find(|d| d.id == imported_id)
            .expect("Imported download must persist across daemon reboot");

        assert_eq!(dl.file_name, "reboot_preserved.bin");
        assert_eq!(dl.status, "paused");
        assert_eq!(dl.category_id.as_deref(), Some("cat-archives"));
        assert_eq!(dl.received_bytes, 1000);
        assert_eq!(dl.source_count, 0);

        // Check verified parts in SQLite
        let db = Database::open(&db_path).unwrap();
        let verified = db.get_verified_parts(&imported_id).unwrap();
        assert_eq!(verified, vec![0]);

        // Check history persistence
        let history = fresh_service
            .query_history(&mule_core::HistoryFilter {
                limit: Some(10),
                ..Default::default()
            })
            .unwrap();
        let import_event = history
            .iter()
            .find(|h| h.download_id == imported_id.to_string())
            .expect("Import history entry must persist across daemon reboot");

        assert_eq!(import_event.event_type, "imported");
        assert_eq!(import_event.file_name, "reboot_preserved.bin");
    }
}

#[test]
fn test_part_met_import_safety_limits_enforced() {
    let temp_dir = tempdir().unwrap();
    let external_dir = tempdir().unwrap();
    let download_target_dir = tempdir().unwrap();

    let db_path = temp_dir.path().join("mule.db");
    let mut service = DownloadService::new(&db_path, temp_dir.path().join("temp")).unwrap();

    let met_path = external_dir.path().join("005.part.met");
    let part_path = external_dir.path().join("005.part");
    std::fs::write(&part_path, b"dummy").unwrap();

    // 2 TB declared size (> 1 TB safety limit)
    create_part_met_file(
        &met_path,
        "too_large.bin",
        2 * 1024 * 1024 * 1024 * 1024,
        0,
        [0; 16],
        &[],
        &[],
    );

    let err = service
        .import_part_met(&met_path, None, download_target_dir.path(), None)
        .unwrap_err();

    assert!(err.to_string().contains("maximum import safety limit"));
}

