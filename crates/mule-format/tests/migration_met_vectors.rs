// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Vectors for `emfriends.met` and `cancelled.met` readers — both are
//! eMule config formats consumed by the migration import.

use md5::Digest;
use mule_format::{
    cancelled_mask, CancelledEntry, CancelledMetFile, EmfriendsMetFile, MetError,
    CANCELLED_VERSION, MET_HEADER, MET_HEADER_I64TAGS,
};
use mule_test_support::met_fixtures::*;
use std::io::Cursor;

// ---------- emfriends.met ----------

fn emfriends_record(
    user_hash: &[u8; 16],
    ip: u32,
    port: u16,
    seen: u32,
    chatted: u32,
    name: Option<&str>,
    kad_id: Option<[u8; 16]>,
) -> Vec<u8> {
    let mut rec = Vec::new();
    rec.extend_from_slice(user_hash);
    rec.extend_from_slice(&ip.to_le_bytes());
    rec.extend_from_slice(&port.to_le_bytes());
    rec.extend_from_slice(&seen.to_le_bytes());
    rec.extend_from_slice(&chatted.to_le_bytes());
    let mut tags = Vec::new();
    let mut count = 0u32;
    if let Some(n) = name {
        push_string_tag(&mut tags, 0x01, n);
        count += 1;
    }
    if let Some(k) = kad_id {
        push_hash16_tag(&mut tags, 0x02, &k);
        count += 1;
    }
    rec.extend_from_slice(&count.to_le_bytes());
    rec.extend_from_slice(&tags);
    rec
}

#[test]
fn emfriends_parses_full_records() {
    let uh1 = [0x11u8; 16];
    let uh2 = [0x22u8; 16];
    let kad = [0x33u8; 16];
    let mut buf = vec![MET_HEADER];
    buf.extend_from_slice(&3u32.to_le_bytes());
    buf.extend(emfriends_record(
        &uh1,
        u32::from_le_bytes([85, 25, 13, 7]),
        49662,
        1_700_000_000,
        1_699_999_000,
        Some("Ami eMule"),
        Some(kad),
    ));
    // IP-only friend (zero userhash), no tags.
    buf.extend(emfriends_record(&[0; 16], u32::from_le_bytes([1, 2, 3, 4]), 49661, 0, 0, None, None));
    // Named friend without KadID.
    buf.extend(emfriends_record(&uh2, 0, 0, 42, 7, Some("Second"), None));

    let parsed = EmfriendsMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(parsed.records.len(), 3);

    let r0 = &parsed.records[0];
    assert!(r0.has_userhash());
    assert_eq!(r0.user_hash, uh1);
    assert_eq!(r0.name.as_deref(), Some("Ami eMule"));
    assert_eq!(r0.kad_id, Some(kad));
    assert_eq!(r0.last_used_port, 49662);

    assert!(!parsed.records[1].has_userhash());
    assert_eq!(parsed.records[1].last_used_ip, u32::from_le_bytes([1, 2, 3, 4]));

    assert_eq!(parsed.records[2].name.as_deref(), Some("Second"));
    assert_eq!(parsed.records[2].kad_id, None);
}

#[test]
fn emfriends_rejects_wrong_header_and_truncation() {
    let err = EmfriendsMetFile::read_from(&mut Cursor::new(&[0x0F, 0, 0, 0, 0]))
        .unwrap_err();
    assert!(matches!(err, MetError::UnsupportedHeader { .. }));

    let mut buf = vec![MET_HEADER];
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&[0xAA; 10]); // truncated record
    let err = EmfriendsMetFile::read_from(&mut Cursor::new(&buf)).unwrap_err();
    assert!(matches!(err, MetError::UnexpectedEof { .. }));
}

#[test]
fn emfriends_keeps_first_name_only() {
    let uh = [0x44u8; 16];
    let mut rec = Vec::new();
    rec.extend_from_slice(&uh);
    rec.extend_from_slice(&0u32.to_le_bytes());
    rec.extend_from_slice(&0u16.to_le_bytes());
    rec.extend_from_slice(&0u32.to_le_bytes());
    rec.extend_from_slice(&0u32.to_le_bytes());
    rec.extend_from_slice(&2u32.to_le_bytes());
    push_string_tag(&mut rec, 0x01, "Premier");
    push_string_tag(&mut rec, 0x01, "Second");

    let mut buf = vec![MET_HEADER];
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&rec);
    let parsed = EmfriendsMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    // eMule `m_strName.IsEmpty()` guard — the first FF_NAME wins.
    assert_eq!(parsed.records[0].name.as_deref(), Some("Premier"));
}

// ---------- cancelled.met ----------

#[test]
fn cancelled_parses_new_format() {
    let mut buf = vec![MET_HEADER_I64TAGS, CANCELLED_VERSION];
    buf.extend_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
    buf.extend_from_slice(&2u32.to_le_bytes());
    buf.extend_from_slice(&[0x11; 16]);
    buf.push(0); // 0 tags
    buf.extend_from_slice(&[0x22; 16]);
    buf.push(1); // 1 skipped tag
    push_u32_tag(&mut buf, 0x01, 42);

    let parsed = CancelledMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(parsed.seed, Some(0xDEAD_BEEF));
    assert!(!parsed.legacy_format);
    assert_eq!(parsed.entries.len(), 2);
    assert_eq!(parsed.entries[0], CancelledEntry::Masked([0x11; 16]));
    assert_eq!(parsed.entries[1], CancelledEntry::Masked([0x22; 16]));
}

#[test]
fn cancelled_parses_legacy_format_as_real_hashes() {
    let mut buf = vec![MET_HEADER];
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&[0x77; 16]);
    buf.push(0);

    let parsed = CancelledMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert!(parsed.legacy_format);
    assert_eq!(parsed.seed, None);
    // Old layout stores the REAL file hash — masking is the importer's job.
    assert_eq!(parsed.entries[0], CancelledEntry::Real([0x77; 16]));
}

#[test]
fn cancelled_rejects_future_version_and_wrong_header() {
    let mut buf = vec![MET_HEADER_I64TAGS, 0x02];
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&0u32.to_le_bytes());
    let err = CancelledMetFile::read_from(&mut Cursor::new(&buf)).unwrap_err();
    assert!(matches!(err, MetError::UnsupportedHeader { .. }));

    let err = CancelledMetFile::read_from(&mut Cursor::new(&[0x01; 8])).unwrap_err();
    assert!(matches!(err, MetError::UnsupportedHeader { .. }));
}

#[test]
fn cancelled_mask_matches_emule_semantics() {
    // eMule `AddCancelledFileID` : MD5(seed_u32_LE || file_hash).
    let seed = 0x0102_0304u32;
    let hash = [0x55u8; 16];
    let masked = cancelled_mask(seed, &hash);
    // Independently computed: MD5 over 20 bytes [04 03 02 01][55×16].
    let mut buf = vec![0x04, 0x03, 0x02, 0x01];
    buf.extend_from_slice(&[0x55; 16]);
    let expect: [u8; 16] = md5::Md5::digest(&buf).into();
    assert_eq!(masked, expect);
    // Same seed+hash is deterministic; different seed differs.
    assert_eq!(cancelled_mask(seed, &hash), masked);
    assert_ne!(cancelled_mask(seed + 1, &hash), masked);
}

// ---------- clients.met ----------

#[test]
fn clients_met_parses_v12_records() {
    use mule_format::met::ClientsMetFile;
    let pubkey = vec![0x30u8; 48];
    let buf = clients_met(0x12, &[
        credit_record(&[0x11; 16], (1 << 33) | 7, 42, 1_700_000_000, Some(&pubkey)),
        credit_record(&[0x22; 16], 5, 6, 1_600_000_000, None),
    ]);
    let parsed = ClientsMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(parsed.version, 0x12);
    assert!(!parsed.legacy_format);
    assert_eq!(parsed.records.len(), 2);
    let r = &parsed.records[0];
    assert_eq!(r.key, [0x11; 16]);
    assert_eq!(r.uploaded, (1 << 33) | 7); // hi/lo recombination
    assert_eq!(r.downloaded, 42);
    assert_eq!(r.last_seen, 1_700_000_000);
    assert_eq!(r.pubkey.as_deref(), Some(pubkey.as_slice()));
    assert_eq!(parsed.records[1].pubkey, None);
}

#[test]
fn clients_met_parses_legacy_29a_layout() {
    use mule_format::met::ClientsMetFile;
    // 0x11 records stop at nReserved3 — 38 bytes, no pubkey fields.
    let mut rec = Vec::new();
    rec.extend_from_slice(&[0x33; 16]);
    rec.extend_from_slice(&10u32.to_le_bytes());
    rec.extend_from_slice(&20u32.to_le_bytes());
    rec.extend_from_slice(&1_500_000_000u32.to_le_bytes());
    rec.extend_from_slice(&0u32.to_le_bytes());
    rec.extend_from_slice(&0u32.to_le_bytes());
    rec.extend_from_slice(&0u16.to_le_bytes());
    assert_eq!(rec.len(), 38);
    let buf = clients_met(0x11, &[rec]);
    let parsed = ClientsMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert!(parsed.legacy_format);
    assert_eq!(parsed.records[0].key, [0x33; 16]);
    assert_eq!(parsed.records[0].pubkey, None);
    assert_eq!(parsed.records[0].uploaded, 10);
}

#[test]
fn clients_met_rejects_unknown_version_and_truncation() {
    use mule_format::met::ClientsMetFile;
    let err = ClientsMetFile::read_from(&mut Cursor::new(&clients_met(0x99, &[]))).unwrap_err();
    assert!(matches!(err, MetError::UnsupportedHeader { .. }));
    // Declared record but missing bytes → EOF error, not a partial read.
    let mut buf = vec![0x12];
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&[0u8; 10]);
    let err = ClientsMetFile::read_from(&mut Cursor::new(&buf)).unwrap_err();
    assert!(matches!(err, MetError::UnexpectedEof { .. }));
}

#[test]
fn clients_met_rejects_oversized_pubkey_field() {
    use mule_format::met::ClientsMetFile;
    // nKeySize > MAXPUBKEYSIZE(80) — eMule `MAXPUBKEYSIZE < nIdentLen`
    // invalidates secure ident → record carries no usable key.
    let mut rec = credit_record(&[0x44; 16], 1, 2, 1_700_000_000, None);
    rec[38] = 0x51; // 81 > 80
    let buf = clients_met(0x12, &[rec]);
    let parsed = ClientsMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(parsed.records[0].pubkey, None);
}

// ---------- preferences.dat ----------

#[test]
fn preferences_dat_extracts_userhash_and_rejects_bad() {
    use mule_format::met::{is_bad_hash, PreferencesDat, PREFFILE_VERSION};
    let mut buf = vec![PREFFILE_VERSION];
    let mut user_hash = [0x66u8; 16];
    user_hash[5] = 14;
    user_hash[14] = 111;
    buf.extend_from_slice(&user_hash);
    buf.extend_from_slice(&[0u8; 44]); // WINDOWPLACEMENT
    let parsed = PreferencesDat::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(parsed.version, PREFFILE_VERSION);
    assert_eq!(parsed.user_hash, user_hash);

    // Degenerate hash (isbadhash) → corrupt file, rejected.
    let mut bad = vec![PREFFILE_VERSION];
    bad.extend_from_slice(&[0u8; 16]);
    let err = PreferencesDat::read_from(&mut Cursor::new(&bad)).unwrap_err();
    assert!(matches!(err, MetError::InconsistentMetadata { .. }));
    assert!(is_bad_hash(&[0u8; 16]));
    assert!(!is_bad_hash(&user_hash));

    // Truncated file → EOF.
    let err = PreferencesDat::read_from(&mut Cursor::new(&[0x14, 1, 2, 3])).unwrap_err();
    assert!(matches!(err, MetError::UnexpectedEof { .. }));
}
