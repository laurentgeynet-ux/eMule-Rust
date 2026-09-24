// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::io::Cursor;
use std::sync::Arc;

use mule_daemon::services::downloads::DownloadService;
use mule_daemon::services::known_met_import::import_known_met;
use mule_format::ed2k::hash::hash_reader;
use mule_format::met::tags::*;
use mule_format::MET_HEADER_I64TAGS;
use tempfile::tempdir;
use tokio::sync::Mutex;

fn push_tag_id(buf: &mut Vec<u8>, tag_type: u8, name_id: u8) {
    buf.push(tag_type);
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.push(name_id);
}

fn push_string_tag(buf: &mut Vec<u8>, name_id: u8, value: &str) {
    push_tag_id(buf, TAGTYPE_STRING, name_id);
    buf.extend_from_slice(&(value.len() as u16).to_le_bytes());
    buf.extend_from_slice(value.as_bytes());
}

fn push_u32_tag(buf: &mut Vec<u8>, name_id: u8, value: u32) {
    push_tag_id(buf, TAGTYPE_UINT32, name_id);
    buf.extend_from_slice(&value.to_le_bytes());
}

fn push_u64_tag(buf: &mut Vec<u8>, name_id: u8, value: u64) {
    push_tag_id(buf, TAGTYPE_UINT64, name_id);
    buf.extend_from_slice(&value.to_le_bytes());
}

/// Serializes a `known.met` record for `payload` (real eD2K hashes).
fn record_bytes(payload: &[u8], name: &str, extra: &mut dyn FnMut(&mut Vec<u8>)) -> Vec<u8> {
    let hashed = hash_reader(&mut Cursor::new(payload)).unwrap();
    let mut rec = Vec::new();
    rec.extend_from_slice(&1_700_000_000u32.to_le_bytes());
    rec.extend_from_slice(hashed.root_hash.as_bytes());
    rec.extend_from_slice(&(hashed.part_hashes.len() as u16).to_le_bytes());
    for ph in &hashed.part_hashes {
        rec.extend_from_slice(ph);
    }
    let mut tags = Vec::new();
    push_string_tag(&mut tags, FT_FILENAME, name);
    push_u64_tag(&mut tags, FT_FILESIZE, hashed.total_bytes);
    extra(&mut tags);
    // Count tags by re-parsing the serialized buffer.
    let mut cr = mule_format::met::reader::CountingReader::new(Cursor::new(&tags));
    let mut n = 0u32;
    while cr.read_tag().is_ok() {
        n += 1;
    }
    rec.extend_from_slice(&n.to_le_bytes());
    rec.extend_from_slice(&tags);
    rec
}

fn aich_root_for(payload: &[u8]) -> mule_format::aich::AichHash {
    mule_format::aich::calculate_aich_root(payload).unwrap()
}

#[tokio::test]
async fn test_known_met_import_registers_share_without_rehash() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("mule.db");
    let svc = Arc::new(Mutex::new(
        DownloadService::new(&db_path, tmp.path().join("temp")).unwrap(),
    ));

    let incoming = tempdir().unwrap();
    let payload = vec![0x42u8; 5_000];
    std::fs::write(incoming.path().join("movie.bin"), &payload).unwrap();
    // A decoy file with no matching record.
    std::fs::write(incoming.path().join("other.bin"), vec![0x99u8; 100]).unwrap();

    let aich = aich_root_for(&payload);
    let mut met_buf = vec![MET_HEADER_I64TAGS];
    let mut records = Vec::new();
    records.extend_from_slice(&record_bytes(&payload, "movie.bin", &mut |t| {
        push_string_tag(t, FT_AICH_HASH, &aich.to_base32());
        push_u32_tag(t, FT_ATTRANSFERRED, 777);
        push_u32_tag(t, FT_ULPRIORITY, 2); // PR_HIGH
    }));
    met_buf.extend_from_slice(&1u32.to_le_bytes());
    met_buf.extend_from_slice(&records);
    let met_path = tmp.path().join("known.met");
    std::fs::write(&met_path, &met_buf).unwrap();

    let report = import_known_met(&svc, met_path, incoming.path().to_path_buf(), false)
        .await
        .unwrap();
    assert_eq!(report.records, 1);
    assert_eq!(report.imported, 1);
    assert_eq!(report.enriched, 0);
    assert_eq!(report.mismatched, 0);
    assert_eq!(report.unmatched_records, 0);
    assert_eq!(report.scanned_files, 2);

    let hashed = hash_reader(&mut Cursor::new(&payload)).unwrap();
    let hash_hex = hashed.root_hash.to_hex().to_lowercase();
    let dl = svc.lock().await;
    let sf = dl
        .get_shared_file(&hash_hex)
        .unwrap()
        .expect("share registered from known.met record");
    assert_eq!(sf.file_name, "movie.bin");
    assert_eq!(sf.file_size, 5_000);
    assert_eq!(sf.up_priority, "high");
    assert_eq!(sf.aich_root, Some(aich.0));
    assert!(!sf.is_partial);
    let parts = dl.get_part_hashes_by_hash(&hash_hex);
    assert_eq!(parts.len(), 1);
    let stats = dl.list_share_uploaded_bytes().unwrap();
    assert_eq!(stats.get(&hash_hex), Some(&777));
}

#[tokio::test]
async fn test_known_met_import_verify_rejects_mismatch() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("mule.db");
    let svc = Arc::new(Mutex::new(
        DownloadService::new(&db_path, tmp.path().join("temp")).unwrap(),
    ));

    let incoming = tempdir().unwrap();
    // File content differs from what the record hashed — same name+size.
    let real_payload = vec![0x11u8; 5_000];
    let stale_payload = vec![0x22u8; 5_000];
    std::fs::write(incoming.path().join("changed.bin"), &real_payload).unwrap();

    let mut met_buf = vec![MET_HEADER_I64TAGS];
    let mut records = Vec::new();
    records.extend_from_slice(&record_bytes(&stale_payload, "changed.bin", &mut |_| {}));
    met_buf.extend_from_slice(&1u32.to_le_bytes());
    met_buf.extend_from_slice(&records);
    let met_path = tmp.path().join("known.met");
    std::fs::write(&met_path, &met_buf).unwrap();

    // verify=false trusts the (stale) record.
    let report = import_known_met(
        &svc,
        met_path.clone(),
        incoming.path().to_path_buf(),
        false,
    )
    .await
    .unwrap();
    assert_eq!(report.imported, 1);

    // Reset: drop the imported share, then verify=true must reject it.
    let stale_hex = {
        let hashed = hash_reader(&mut Cursor::new(&stale_payload)).unwrap();
        hashed.root_hash.to_hex().to_lowercase()
    };
    {
        let mut dl = svc.lock().await;
        dl.remove_shared_file(&stale_hex, false).unwrap();
    }
    let report = import_known_met(&svc, met_path, incoming.path().to_path_buf(), true)
        .await
        .unwrap();
    assert_eq!(report.mismatched, 1);
    assert_eq!(report.imported, 0);
    let dl = svc.lock().await;
    assert!(dl.get_shared_file(&stale_hex).unwrap().is_none());
}

#[tokio::test]
async fn test_known_met_import_enriches_existing_share() {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("mule.db");
    let svc = Arc::new(Mutex::new(
        DownloadService::new(&db_path, tmp.path().join("temp")).unwrap(),
    ));

    let incoming = tempdir().unwrap();
    let payload = vec![0x33u8; 4_000];
    std::fs::write(incoming.path().join("song.mp3"), &payload).unwrap();

    // Pre-register the share with a user-set priority — import must not
    // reset it (enrich-only path).
    let hashed = hash_reader(&mut Cursor::new(&payload)).unwrap();
    let hash_hex = hashed.root_hash.to_hex().to_lowercase();
    {
        let mut dl = svc.lock().await;
        let mut sf = mule_core::model::SharedFile::new(
            &hash_hex,
            "song.mp3",
            4_000,
            incoming
                .path()
                .join("song.mp3")
                .to_string_lossy()
                .as_ref(),
        );
        sf.up_priority = "verylow".to_string();
        dl.add_shared_file(&sf).unwrap();
    }

    let aich = aich_root_for(&payload);
    let mut met_buf = vec![MET_HEADER_I64TAGS];
    let mut records = Vec::new();
    records.extend_from_slice(&record_bytes(&payload, "song.mp3", &mut |t| {
        push_string_tag(t, FT_AICH_HASH, &aich.to_base32());
        push_u32_tag(t, FT_ATTRANSFERRED, 42);
        push_u32_tag(t, FT_ULPRIORITY, 3); // PR_VERYHIGH — must NOT override
    }));
    met_buf.extend_from_slice(&1u32.to_le_bytes());
    met_buf.extend_from_slice(&records);
    let met_path = tmp.path().join("known.met");
    std::fs::write(&met_path, &met_buf).unwrap();

    let report = import_known_met(&svc, met_path, incoming.path().to_path_buf(), false)
        .await
        .unwrap();
    assert_eq!(report.imported, 0);
    assert_eq!(report.enriched, 1);

    let dl = svc.lock().await;
    let sf = dl.get_shared_file(&hash_hex).unwrap().unwrap();
    assert_eq!(sf.up_priority, "verylow"); // user config preserved
    assert_eq!(sf.aich_root, Some(aich.0)); // enriched
    let stats = dl.list_share_uploaded_bytes().unwrap();
    assert_eq!(stats.get(&hash_hex), Some(&42));
}
