// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Tests for startup orphan `.part.met` recovery (eMule CPartFile auto-load
//! parity), keep-files tombstoning, and explicit `.part.met` flushes.

use md4::{Digest, Md4};
use mule_core::AllocationPolicy;
use mule_daemon::services::downloads::DownloadService;
use tempfile::tempdir;

fn make_orphan(temp_dir: &std::path::Path, stem: &str, content: &[u8], name: &str) -> [u8; 16] {
    let file_hash: [u8; 16] = Md4::digest(content).into();
    std::fs::write(temp_dir.join(format!("{stem}.part")), content).unwrap();
    let met = mule_format::PartMetFile {
        version: mule_format::PARTFILE_VERSION,
        mod_date: 0,
        file_hash: mule_format::Ed2kHash(file_hash),
        part_hashes: vec![file_hash],
        file_name: Some(name.to_string()),
        file_size: Some(content.len() as u64),
        transferred: content.len() as u64,
        aich_hash: None,
        gaps: vec![],
        tags: vec![],
    };
    met.save(temp_dir.join(format!("{stem}.part.met"))).unwrap();
    file_hash
}

#[test]
fn orphan_part_met_is_recovered_and_adopted_in_place() {
    let tmp = tempdir().unwrap();
    let temp_dir = tmp.path().join("temp");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let mut svc = DownloadService::in_memory(temp_dir.clone()).unwrap();

    let content = vec![0xABu8; 500];
    let file_hash = make_orphan(&temp_dir, "003", &content, "orphan.bin");

    let recovered = svc.recover_orphan_part_mets(tmp.path());
    assert_eq!(recovered, 1);

    // Adoption: source pair is consumed, staged {uuid}.part exists.
    assert!(!temp_dir.join("003.part.met").exists());
    assert!(!temp_dir.join("003.part").exists());

    let snaps = svc.list_downloads();
    assert_eq!(snaps.len(), 1);
    let snap = &snaps[0];
    assert_eq!(snap.status, "paused");
    assert_eq!(snap.file_name, "orphan.bin");
    assert_eq!(snap.received_bytes, 500);
    assert_eq!(snap.ed2k_hash, hex::encode(file_hash));
    assert_eq!(snap.source_count, 0);

    let staged = temp_dir.join(format!("{}.part", snap.id));
    assert!(staged.exists());
    assert_eq!(std::fs::metadata(&staged).unwrap().len(), 500);

    // Idempotent: nothing left to recover.
    assert_eq!(svc.recover_orphan_part_mets(tmp.path()), 0);
}

#[test]
fn tracked_part_met_is_not_reimported() {
    let tmp = tempdir().unwrap();
    let temp_dir = tmp.path().join("temp");
    let mut svc = DownloadService::in_memory(temp_dir.clone()).unwrap();

    let link = format!("ed2k://|file|known.bin|500|{}|/", hex::encode([0x77u8; 16]));
    svc.add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    // The {uuid}.part.met written by add is owned by a tracked download:
    // the orphan scan must skip it.
    assert_eq!(svc.recover_orphan_part_mets(tmp.path()), 0);
    assert_eq!(svc.list_downloads().len(), 1);
}

#[test]
fn malformed_orphan_met_is_skipped_without_panicking() {
    let tmp = tempdir().unwrap();
    let temp_dir = tmp.path().join("temp");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let mut svc = DownloadService::in_memory(temp_dir.clone()).unwrap();

    std::fs::write(temp_dir.join("007.part.met"), b"not a met file").unwrap();
    std::fs::write(temp_dir.join("007.part"), b"junk").unwrap();

    assert_eq!(svc.recover_orphan_part_mets(tmp.path()), 0);
    assert!(svc.list_downloads().is_empty());
    // Malformed input is left on disk for manual inspection.
    assert!(temp_dir.join("007.part.met").exists());
}

#[test]
fn removed_download_with_kept_files_does_not_resurrect() {
    let tmp = tempdir().unwrap();
    let temp_dir = tmp.path().join("temp");
    let mut svc = DownloadService::in_memory(temp_dir.clone()).unwrap();

    let content = vec![0x55u8; 400];
    let file_hash: [u8; 16] = Md4::digest(&content).into();
    let link = format!("ed2k://|file|keep.bin|400|{}|/", hex::encode(file_hash));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();
    svc.inject_range(&snap.id, 0, &content).unwrap();

    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    let mut met_name = temp_path.as_os_str().to_os_string();
    met_name.push(".met");
    let met_path = std::path::PathBuf::from(&met_name);
    let mut keep_name = met_name.clone();
    keep_name.push(".keep");
    let keep_path = std::path::PathBuf::from(keep_name);
    assert!(met_path.exists());

    svc.remove_download(&snap.id, false).unwrap();

    // Data preserved: .part stays, met tombstoned to .keep (scan-ignored).
    assert!(temp_path.exists());
    assert!(!met_path.exists());
    assert!(keep_path.exists());
    assert!(svc.list_downloads().is_empty());

    // The kept pair must not resurrect on the next startup scan.
    assert_eq!(svc.recover_orphan_part_mets(tmp.path()), 0);
    assert!(svc.list_downloads().is_empty());
}

#[test]
fn save_active_part_mets_flushes_all_in_progress() {
    let tmp = tempdir().unwrap();
    let temp_dir = tmp.path().join("temp");
    let mut svc = DownloadService::in_memory(temp_dir).unwrap();

    let link = format!("ed2k://|file|flush.bin|300|{}|/", hex::encode([0x11u8; 16]));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();
    svc.inject_range(&snap.id, 0, &[0xAAu8; 100]).unwrap();

    // Simulate a dirty met: corrupt the on-disk companion, then flush.
    let (temp_path, _, _, _) = svc.get_download_file_info(&snap.id).unwrap();
    let mut met_name = temp_path.as_os_str().to_os_string();
    met_name.push(".met");
    let met_path = std::path::PathBuf::from(met_name);
    std::fs::write(&met_path, b"stale").unwrap();

    svc.save_active_part_mets();

    let met = mule_format::PartMetFile::open(&met_path).unwrap();
    assert_eq!(met.gaps, vec![(100, 300)]);
    assert_eq!(met.transferred, 100);
}
