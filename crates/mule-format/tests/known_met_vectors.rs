// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use mule_format::ed2k::hash::hash_reader;
use mule_format::met::tags::*;
use mule_format::{KnownMetFile, MetError, MET_HEADER_I64TAGS};
use std::io::Cursor;

/// Writes a .met tag in the `[type][u16 namelen=1][u8 name_id][value]`
/// layout `CTag::WriteTagToFile` produces for named-id tags.
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

/// Builds one `known.met` record from a payload hashed with the real
/// eD2K hasher so the root/part-hash self-check passes.
fn push_record(
    buf: &mut Vec<u8>,
    payload: &[u8],
    name: &str,
    last_modified: u32,
    extra: &mut dyn FnMut(&mut Vec<u8>),
) {
    let hashed = hash_reader(&mut Cursor::new(payload)).unwrap();
    buf.extend_from_slice(&last_modified.to_le_bytes());
    buf.extend_from_slice(hashed.root_hash.as_bytes());
    buf.extend_from_slice(&(hashed.part_hashes.len() as u16).to_le_bytes());
    for ph in &hashed.part_hashes {
        buf.extend_from_slice(ph);
    }

    let mut tags = Vec::new();
    push_string_tag(&mut tags, FT_FILENAME, name);
    push_u64_tag(&mut tags, FT_FILESIZE, hashed.total_bytes);
    extra(&mut tags);

    buf.extend_from_slice(&(count_tags(&tags)).to_le_bytes());
    buf.extend_from_slice(&tags);
}

/// Counts the number of serialized tags in `tags` by re-parsing.
fn count_tags(tags: &[u8]) -> u32 {
    let mut cr = mule_format::met::reader::CountingReader::new(Cursor::new(tags));
    let mut n = 0u32;
    while cr.read_tag().is_ok() {
        n += 1;
    }
    n
}

#[test]
fn test_known_met_nominal_two_records() {
    let payload_a = vec![0xAAu8; 5_000]; // < 1 part → 1 hash
    let payload_b = vec![0xBBu8; 10_000_000]; // > 1 part → 2 hashes

    let mut buf = vec![MET_HEADER_I64TAGS];
    buf.extend_from_slice(&2u32.to_le_bytes());
    push_record(&mut buf, &payload_a, "small.bin", 1_700_000_000, &mut |t| {
        push_u32_tag(t, FT_LASTSHARED, 1_699_999_000);
        push_u32_tag(t, FT_ATTRANSFERRED, 123_456);
        push_u32_tag(t, FT_ATTRANSFERREDHI, 2);
        push_u32_tag(t, FT_ATREQUESTED, 77);
        push_u32_tag(t, FT_ATACCEPTED, 55);
        push_u32_tag(t, FT_ULPRIORITY, 2); // PR_HIGH
        push_u32_tag(t, FT_KADLASTPUBLISHSRC, 1_690_000_000);
        push_u32_tag(t, FT_FLAGS, 2);
    });
    push_record(&mut buf, &payload_b, "big.bin", 1_700_000_100, &mut |t| {
        push_u32_tag(t, FT_ULPRIORITY, 5); // PR_AUTO → PR_HIGH
    });

    let met = KnownMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert!(met.i64_tags);
    assert_eq!(met.records.len(), 2);
    assert_eq!(met.dropped_records, 0);

    let a = &met.records[0];
    assert_eq!(a.last_modified_utc, 1_700_000_000);
    assert_eq!(a.file_name.as_deref(), Some("small.bin"));
    assert_eq!(a.file_size, Some(5_000));
    assert_eq!(a.part_hashes.len(), 1);
    assert_eq!(a.last_shared_utc, Some(1_699_999_000));
    assert_eq!(a.transferred_bytes, (2u64 << 32) | 123_456);
    assert_eq!(a.requested, 77);
    assert_eq!(a.accepted, 55);
    assert_eq!(a.up_priority, Some(2));
    assert_eq!(a.kad_last_publish_src, Some(1_690_000_000));
    assert_eq!(a.meta_data_ver, 2);

    let b = &met.records[1];
    assert_eq!(b.file_name.as_deref(), Some("big.bin"));
    assert_eq!(b.file_size, Some(10_000_000));
    // 10_000_000 / 9_728_000 → floor 1 + 1 = 2 parts (trailing partial)
    assert_eq!(b.part_hashes.len(), 2);
    assert_eq!(b.up_priority, Some(2)); // PR_AUTO resolved to PR_HIGH
}

#[test]
fn test_known_met_drops_record_with_wrong_root() {
    let payload = vec![0xCCu8; 5_000];
    let hashed = hash_reader(&mut Cursor::new(&payload)).unwrap();

    let mut buf = vec![MET_HEADER_I64TAGS];
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&1_700_000_000u32.to_le_bytes());
    // Corrupt root hash (does not match the hashset)
    buf.extend_from_slice(&[0xDE; 16]);
    buf.extend_from_slice(&(hashed.part_hashes.len() as u16).to_le_bytes());
    for ph in &hashed.part_hashes {
        buf.extend_from_slice(ph);
    }
    let mut tags = Vec::new();
    push_string_tag(&mut tags, FT_FILENAME, "corrupt.bin");
    push_u64_tag(&mut tags, FT_FILESIZE, hashed.total_bytes);
    buf.extend_from_slice(&(count_tags(&tags)).to_le_bytes());
    buf.extend_from_slice(&tags);

    let met = KnownMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(met.records.len(), 0);
    assert_eq!(met.dropped_records, 1);
}

#[test]
fn test_known_met_drops_record_with_wrong_part_count() {
    // file_size declares 2 parts but the hashset stores 1
    let payload = vec![0xDDu8; 5_000];
    let hashed = hash_reader(&mut Cursor::new(&payload)).unwrap();

    let mut buf = vec![MET_HEADER_I64TAGS];
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&1_700_000_000u32.to_le_bytes());
    buf.extend_from_slice(hashed.root_hash.as_bytes());
    buf.extend_from_slice(&(hashed.part_hashes.len() as u16).to_le_bytes());
    for ph in &hashed.part_hashes {
        buf.extend_from_slice(ph);
    }
    let mut tags = Vec::new();
    push_string_tag(&mut tags, FT_FILENAME, "count.bin");
    // Lie: pretend the file is > 1 part → expected count = 2
    push_u64_tag(&mut tags, FT_FILESIZE, 10_000_000);
    buf.extend_from_slice(&(count_tags(&tags)).to_le_bytes());
    buf.extend_from_slice(&tags);

    let met = KnownMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(met.records.len(), 0);
    assert_eq!(met.dropped_records, 1);
}

#[test]
fn test_known_met_bad_header_rejected() {
    let buf = vec![0x11, 0x00, 0x00, 0x00, 0x00];
    let err = KnownMetFile::read_from(&mut Cursor::new(&buf)).unwrap_err();
    assert!(matches!(
        err,
        MetError::UnsupportedHeader {
            offset: 0,
            value: 0x11
        }
    ));
}

#[test]
fn test_known_met_oversized_record_count_rejected() {
    let mut buf = vec![MET_HEADER_I64TAGS];
    buf.extend_from_slice(&600_000u32.to_le_bytes());
    let err = KnownMetFile::read_from(&mut Cursor::new(&buf)).unwrap_err();
    assert!(matches!(err, MetError::InvalidLength { .. }));
}

#[test]
fn test_known_met_truncated_eof() {
    let mut buf = vec![MET_HEADER_I64TAGS];
    buf.extend_from_slice(&1u32.to_le_bytes());
    buf.extend_from_slice(&1_700_000_000u32.to_le_bytes());
    // no hash data follows
    let err = KnownMetFile::read_from(&mut Cursor::new(&buf)).unwrap_err();
    assert!(matches!(err, MetError::UnexpectedEof { .. }));
}

#[test]
fn test_known_met_extra_tags_preserved() {
    let payload = vec![0xEEu8; 4_000];
    let mut buf = vec![MET_HEADER_I64TAGS];
    buf.extend_from_slice(&1u32.to_le_bytes());
    push_record(&mut buf, &payload, "song.mp3", 1_700_000_000, &mut |t| {
        // FT_MEDIA_ARTIST = 0xD0 — goes to extra_tags (ConvertED2KTag path)
        push_string_tag(t, 0xD0, "Some Artist");
    });
    let met = KnownMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    let rec = &met.records[0];
    assert_eq!(rec.extra_tags.len(), 1);
    assert_eq!(rec.extra_tags[0].name_id, 0xD0);
    assert_eq!(rec.extra_tags[0].as_str(), Some("Some Artist"));
}
