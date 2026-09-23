// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Tests for per-part hashset persistence and incremental corruption handling.

use md4::{Digest, Md4};
use mule_core::AllocationPolicy;
use mule_daemon::services::downloads::DownloadService;
use tempfile::tempdir;

fn md4_of(data: &[u8]) -> [u8; 16] {
    Md4::digest(data).into()
}

#[test]
fn part_hashes_are_stored_from_ed2k_link_and_roundtrip() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    // 2-part file declared via link part hashes (p=... syntax carries them)
    let p0 = [0x11u8; 16];
    let p1 = [0x22u8; 16];
    let link = format!(
        "ed2k://|file|two_parts.bin|{}|{}|p={}:{}|/",
        9_728_000 + 10,
        hex::encode([0xAAu8; 16]),
        hex::encode(p0),
        hex::encode(p1),
    );

    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    let stored = svc.get_part_hashes_for_download(&snap.id);
    assert_eq!(stored, vec![p0, p1]);
}

#[test]
fn completed_part_is_marked_verified_when_hash_matches() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let content = vec![0x42u8; 100];
    let file_hash = md4_of(&content); // single-part file: root hash == part hash

    let link = format!("ed2k://|file|small.bin|100|{}|/", hex::encode(file_hash));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    // Store the hashset (1 part)
    svc.store_part_hashes_by_hash(&hex::encode(file_hash), &[file_hash])
        .unwrap();

    svc.inject_range(&snap.id, 0, &content).unwrap();

    let verified = svc.db_ref().get_verified_parts(&snap.id).unwrap();
    assert_eq!(verified, vec![0]);
    assert_eq!(svc.get_covered_len(&snap.id), Some(100));
}

#[test]
fn corrupted_part_is_evicted_for_redownload() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let content = vec![0x42u8; 100];
    let file_hash = md4_of(&content);

    let link = format!("ed2k://|file|small.bin|100|{}|/", hex::encode(file_hash));
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    // Store a deliberately wrong hashset
    svc.store_part_hashes_by_hash(&hex::encode(file_hash), &[[0xEE; 16]])
        .unwrap();

    svc.inject_range(&snap.id, 0, &content).unwrap();

    // Corrupted part must have been evicted: no covered bytes, nothing verified
    assert_eq!(svc.get_covered_len(&snap.id), Some(0));
    assert!(svc
        .db_ref()
        .get_verified_parts(&snap.id)
        .unwrap()
        .is_empty());
}

#[test]
fn evict_corrupted_parts_resets_to_downloading() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let file_hash = [0xAAu8; 16];
    let link = format!(
        "ed2k://|file|big.bin|{}|{}|/",
        9_728_000 + 10,
        hex::encode(file_hash)
    );
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();

    // Fill part 0 fully and part 1 partially
    svc.inject_range(&snap.id, 0, &vec![0u8; 9_728_000])
        .unwrap();
    svc.inject_range(&snap.id, 9_728_000, &[0u8; 10]).unwrap();
    assert_eq!(svc.get_covered_len(&snap.id), Some(9_728_000 + 10));

    // Evict part 0
    let updated = svc.evict_corrupted_parts(&snap.id, &[0]).unwrap();
    assert_eq!(updated.status.to_lowercase(), "downloading");
    assert_eq!(svc.get_covered_len(&snap.id), Some(10));

    // Persisted ranges reflect eviction
    let ranges = svc.db_ref().load_received_ranges(&snap.id).unwrap();
    assert_eq!(ranges.covered_len(), 10);
}
