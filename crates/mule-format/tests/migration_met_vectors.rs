// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Vectors for `emfriends.met` and `cancelled.met` readers — both are
//! eMule config formats consumed by the migration import.

use md5::Digest;
use mule_format::met::tags::*;
use mule_format::{
    cancelled_mask, CancelledEntry, CancelledMetFile, EmfriendsMetFile, MetError,
    CANCELLED_VERSION, MET_HEADER, MET_HEADER_I64TAGS,
};
use std::io::Cursor;

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

fn push_hash_tag(buf: &mut Vec<u8>, name_id: u8, hash: &[u8; 16]) {
    push_tag_id(buf, TAGTYPE_HASH16, name_id);
    buf.extend_from_slice(hash);
}

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
        push_hash_tag(&mut tags, 0x02, &k);
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
        4662,
        1_700_000_000,
        1_699_999_000,
        Some("Ami eMule"),
        Some(kad),
    ));
    // IP-only friend (zero userhash), no tags.
    buf.extend(emfriends_record(&[0; 16], u32::from_le_bytes([1, 2, 3, 4]), 4661, 0, 0, None, None));
    // Named friend without KadID.
    buf.extend(emfriends_record(&uh2, 0, 0, 42, 7, Some("Second"), None));

    let parsed = EmfriendsMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(parsed.records.len(), 3);

    let r0 = &parsed.records[0];
    assert!(r0.has_userhash());
    assert_eq!(r0.user_hash, uh1);
    assert_eq!(r0.name.as_deref(), Some("Ami eMule"));
    assert_eq!(r0.kad_id, Some(kad));
    assert_eq!(r0.last_used_port, 4662);

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
    push_u32_tag_skip(&mut buf, 0x01, 42);

    let parsed = CancelledMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(parsed.seed, Some(0xDEAD_BEEF));
    assert!(!parsed.legacy_format);
    assert_eq!(parsed.entries.len(), 2);
    assert_eq!(parsed.entries[0], CancelledEntry::Masked([0x11; 16]));
    assert_eq!(parsed.entries[1], CancelledEntry::Masked([0x22; 16]));
}

fn push_u32_tag_skip(buf: &mut Vec<u8>, name_id: u8, value: u32) {
    push_tag_id(buf, TAGTYPE_UINT32, name_id);
    buf.extend_from_slice(&value.to_le_bytes());
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
