// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Daemon-level tests for `emfriends.met` / `cancelled.met` import and
//! the `AddCancelledFileID`/`IsCancelledFileByID` service behaviour
//! (`KnownFileList.cpp:384-411`).

use std::io::Cursor;

use mule_core::AllocationPolicy;
use mule_daemon::services::downloads::DownloadService;
use mule_format::met::cancelled_mask;
use mule_format::{MET_HEADER, MET_HEADER_I64TAGS};
use mule_test_support::met_fixtures::*;
use tempfile::tempdir;

const CANCELLED_VERSION: u8 = 1;

/// Serializes one `emfriends.met` record (CFriend::LoadFromFile layout).
fn friend_record(user_hash: &[u8; 16], name: Option<&str>) -> Vec<u8> {
    let mut rec = Vec::new();
    rec.extend_from_slice(user_hash);
    rec.extend_from_slice(&0x0102_0304u32.to_le_bytes()); // last used ip
    rec.extend_from_slice(&49662u16.to_le_bytes()); // last used port
    rec.extend_from_slice(&1_700_000_000u32.to_le_bytes()); // last seen
    rec.extend_from_slice(&0u32.to_le_bytes()); // last chatted
    let mut tags = Vec::new();
    if let Some(name) = name {
        push_string_tag(&mut tags, 0x01, name); // FF_NAME
        push_hash16_tag(&mut tags, 0x02, &[0x77; 16]); // FF_KADID
    }
    let mut cr = mule_format::met::reader::CountingReader::new(Cursor::new(&tags));
    let mut n = 0u32;
    while cr.read_tag().is_ok() {
        n += 1;
    }
    rec.extend_from_slice(&n.to_le_bytes());
    rec.extend_from_slice(&tags);
    rec
}

fn emfriends_met(records: &[Vec<u8>]) -> Vec<u8> {
    let mut buf = vec![MET_HEADER];
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for r in records {
        buf.extend_from_slice(r);
    }
    buf
}

/// `0x0F` record layout: `[masked hash 16][tag_count u8][tagsâ€¦]`.
fn cancelled_met(seed: u32, masked: &[[u8; 16]]) -> Vec<u8> {
    let mut buf = vec![MET_HEADER_I64TAGS, CANCELLED_VERSION];
    buf.extend_from_slice(&seed.to_le_bytes());
    buf.extend_from_slice(&(masked.len() as u32).to_le_bytes());
    for m in masked {
        buf.extend_from_slice(m);
        buf.push(0); // no per-record tags
    }
    buf
}

/// Deprecated `0x0E` layout â€” real hashes, same per-record tag byte.
fn cancelled_met_legacy(real_hashes: &[[u8; 16]]) -> Vec<u8> {
    let mut buf = vec![MET_HEADER];
    buf.extend_from_slice(&(real_hashes.len() as u32).to_le_bytes());
    for h in real_hashes {
        buf.extend_from_slice(h);
        buf.push(0);
    }
    buf
}

#[test]
fn test_emfriends_import_adds_friends_and_skips_hashless() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let met = emfriends_met(&[
        friend_record(&[0x11; 16], Some("Alice")),
        friend_record(&[0x22; 16], None),
        friend_record(&[0x00; 16], Some("NoHash")), // IP-only friend
    ]);
    let met_path = tmp.path().join("emfriends.met");
    std::fs::write(&met_path, &met).unwrap();

    let report = svc.import_emfriends_met(&met_path).unwrap();
    assert_eq!(report.records, 3);
    assert_eq!(report.skipped_no_userhash, 1);
    assert_eq!(report.imported, 2);
    assert_eq!(report.updated, 0);
    assert_eq!(report.user_hashes.len(), 2);

    let friends = svc.list_friends().unwrap();
    assert_eq!(friends.len(), 2);
    let alice = friends
        .iter()
        .find(|f| f.user_hash == hex::encode([0x11u8; 16]))
        .unwrap();
    assert_eq!(alice.name, "Alice");

    // Re-import â†’ records already known count as `updated`.
    let report2 = svc.import_emfriends_met(&met_path).unwrap();
    assert_eq!(report2.imported, 0);
    assert_eq!(report2.updated, 2);
}

#[test]
fn test_cancelled_met_import_masked_and_lookup() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let file_seed = 0xDEAD_BEEFu32;
    let real_hash = [0x42u8; 16];
    let masked = cancelled_mask(file_seed, &real_hash);
    let met = cancelled_met(file_seed, &[masked, cancelled_mask(file_seed, &[0x99; 16])]);
    let met_path = tmp.path().join("cancelled.met");
    std::fs::write(&met_path, &met).unwrap();

    let report = svc.import_cancelled_met(&met_path).unwrap();
    assert_eq!(report.records, 2);
    assert_eq!(report.masked_imported, 2);
    assert_eq!(report.legacy_masked, 0);
    assert!(!report.legacy_format);
    assert_eq!(report.total, 2);

    // Lookup recomputes the mask under every known seed â€” the file's.
    assert!(svc.is_cancelled_file(&hex::encode(real_hash)).unwrap());
    assert!(!svc.is_cancelled_file(&hex::encode([0x00u8; 16])).unwrap());
}

#[test]
fn test_cancelled_met_legacy_import_masks_under_our_seed() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let real_hash = [0x33u8; 16];
    let met = cancelled_met_legacy(&[real_hash]);
    let met_path = tmp.path().join("cancelled.met");
    std::fs::write(&met_path, &met).unwrap();

    let report = svc.import_cancelled_met(&met_path).unwrap();
    assert_eq!(report.legacy_masked, 1);
    assert_eq!(report.masked_imported, 0);
    assert!(report.legacy_format);
    assert!(svc.is_cancelled_file(&hex::encode(real_hash)).unwrap());
}

#[tokio::test]
async fn test_remove_download_records_cancelled_hash() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let hash = [0xAAu8; 16];
    let link = format!(
        "ed2k://|file|abort.bin|{}|{}|/",
        500_000,
        hex::encode(hash)
    );
    let snap = svc
        .add_ed2k_link(&link, tmp.path(), AllocationPolicy::None, None)
        .unwrap();
    assert!(!svc.is_cancelled_file(&hex::encode(hash)).unwrap());

    svc.remove_download(&snap.id, false).unwrap();
    assert!(svc.is_cancelled_file(&hex::encode(hash)).unwrap());
    assert_eq!(svc.cancelled_count().unwrap(), 1);
}

/// `Preferences_Ext_Struct` prefix: `[version u8][userhash 16]`.
fn preferences_dat(version: u8, user_hash: &[u8; 16]) -> Vec<u8> {
    let mut buf = vec![version];
    buf.extend_from_slice(user_hash);
    buf.extend_from_slice(&[0u8; 44]); // WINDOWPLACEMENT
    buf
}

#[test]
fn test_identity_import_cryptkey_and_preferences() {
    let emule_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();

    // Source: a valid eMule-format cryptkey.dat + preferences.dat.
    let src_key = mule_crypto::cryptkey::generate_cryptkey(emule_dir.path()).unwrap();
    let emule_userhash = {
        let mut h = [0x77u8; 16];
        h[5] = 14;
        h[14] = 111;
        h
    };
    let prefs_path = emule_dir.path().join("preferences.dat");
    std::fs::write(&prefs_path, preferences_dat(0x14, &emule_userhash)).unwrap();

    // First import into an empty data dir â€” no replace needed.
    let report = mule_daemon::services::IdentityService::import_emule_identity(
        data_dir.path(),
        Some(&emule_dir.path().join("cryptkey.dat")),
        Some(&prefs_path),
        false,
    )
    .unwrap();
    assert!(report.key_imported);
    assert!(report.userhash_imported);

    // Reloaded identity uses the imported key and the imported user hash.
    let svc = mule_daemon::services::IdentityService::load_or_generate(data_dir.path());
    assert_eq!(svc.user_hash(), emule_userhash);
    use rsa::pkcs1::EncodeRsaPublicKey;
    let src_pub = rsa::RsaPublicKey::from(&src_key).to_pkcs1_der().unwrap();
    assert_eq!(svc.public_key_bytes(), src_pub.as_bytes());

    // Second import without replace â†’ refused (existing identity).
    let err = mule_daemon::services::IdentityService::import_emule_identity(
        data_dir.path(),
        Some(&emule_dir.path().join("cryptkey.dat")),
        None,
        false,
    )
    .unwrap_err();
    assert!(err.to_string().contains("replace"));

    // Rotation drops the imported user hash â€” it belongs to the eMule key.
    let rotated = mule_daemon::services::IdentityService::rotate_key(data_dir.path()).unwrap();
    assert_ne!(rotated.user_hash(), emule_userhash);
    assert!(!data_dir.path().join("userhash.dat").exists());
}

#[test]
fn test_identity_import_rejects_degenerate_userhash() {
    let emule_dir = tempdir().unwrap();
    let data_dir = tempdir().unwrap();
    // All-zero user hash â†’ `isbadhash` â†’ eMule would have regenerated it.
    let prefs_path = emule_dir.path().join("preferences.dat");
    std::fs::write(&prefs_path, preferences_dat(0x14, &[0u8; 16])).unwrap();
    let err = mule_daemon::services::IdentityService::import_emule_identity(
        data_dir.path(),
        None,
        Some(&prefs_path),
        false,
    )
    .unwrap_err();
    assert!(err.to_string().contains("isbadhash") || err.to_string().contains("degenerate"));
    assert!(!data_dir.path().join("userhash.dat").exists());
}



#[test]
fn test_clients_met_import_keys_credits_on_pubkey() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let pubkey = vec![0x30u8; 48]; // DER-ish bytes — MD4 keyed either way
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    let met = clients_met(0x12, &[
        credit_record(&[0x11; 16], 1_000_000_000, 500_000, now - 100, Some(&pubkey)),
        credit_record(&[0x22; 16], 7, 9, now - 50, None), // no pubkey → skip
        credit_record(&[0x33; 16], 4, 5, now - 200 * 86_400, Some(&pubkey)), // expired
    ]);
    let met_path = tmp.path().join("clients.met");
    std::fs::write(&met_path, &met).unwrap();

    let report = svc.import_clients_met(&met_path).unwrap();
    assert_eq!(report.records, 3);
    assert_eq!(report.imported, 1);
    assert_eq!(report.skipped_no_pubkey, 1);
    assert_eq!(report.expired, 1);
    assert!(!report.legacy_format);

    let credits = svc.list_peer_credits(10).unwrap();
    assert_eq!(credits.len(), 1);
    let c = &credits[0];
    use md4::{Digest, Md4};
    assert_eq!(c.pubkey_hash, hex::encode(Md4::digest(&pubkey)));
    assert_eq!(c.pubkey_hex, hex::encode(&pubkey));
    assert_eq!(c.uploaded_bytes, 1_000_000_000);
    assert_eq!(c.downloaded_bytes, 500_000);
}

#[test]
fn test_remember_cancelled_off_disables_set() {
    let tmp = tempdir().unwrap();
    let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();

    let real_hash = [0x55u8; 16];
    let met = cancelled_met(7, &[cancelled_mask(7, &real_hash)]);
    let met_path = tmp.path().join("cancelled.met");
    std::fs::write(&met_path, &met).unwrap();
    svc.import_cancelled_met(&met_path).unwrap();
    assert!(svc.is_cancelled_file(&hex::encode(real_hash)).unwrap());

    // `IsRememberingCancelledFiles` off â†’ lookup always false.
    svc.set_setting("remember_cancelled", "0").unwrap();
    assert!(!svc.is_cancelled_file(&hex::encode(real_hash)).unwrap());
}
