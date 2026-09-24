// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use md4::{Digest, Md4};
use mule_format::{
    hash_reader, BytePatternFixture, Ed2kHasher, FormatError, PatternReader, FIXTURE_1_BYTE,
    FIXTURE_EXACT_ONE_PART, FIXTURE_EXACT_TWO_PARTS, FIXTURE_JUST_UNDER_PART,
    FIXTURE_ONE_PART_PLUS_ONE, REPEATING_0_TO_255,
};
use std::io::Cursor;

/// Vérifie un fixture `BytePatternFixture` : hash eD2K racine + nombre de parts.
fn check_pattern_fixture(fx: &BytePatternFixture) {
    let mut reader = PatternReader::new(&REPEATING_0_TO_255, fx.length);
    let res = hash_reader(&mut reader).unwrap();

    assert_eq!(res.total_bytes, fx.length, "total_bytes for {}", fx.name);
    assert_eq!(res.root_hash.to_hex(), fx.expected_ed2k_hex, "root {}", fx.name);
    assert_eq!(
        res.part_hashes.len(),
        fx.expected_part_hashes_count,
        "part count {}",
        fx.name
    );
}

#[test]
fn test_empty_input_rejected() {
    let hasher = Ed2kHasher::new();
    assert_eq!(hasher.finalize().unwrap_err(), FormatError::EmptyInput);

    let mut cursor = Cursor::new(b"");
    assert_eq!(
        hash_reader(&mut cursor).unwrap_err(),
        FormatError::EmptyInput
    );
}

#[test]
fn test_fixture_1_byte() {
    check_pattern_fixture(&FIXTURE_1_BYTE);
}

#[test]
fn test_fixture_just_under_part() {
    check_pattern_fixture(&FIXTURE_JUST_UNDER_PART);
}

#[test]
fn test_fixture_exact_one_part() {
    // 9_728_000 bytes: checks append of empty MD4 block
    check_pattern_fixture(&FIXTURE_EXACT_ONE_PART);
}

#[test]
fn test_fixture_one_part_plus_one() {
    check_pattern_fixture(&FIXTURE_ONE_PART_PLUS_ONE);
}

#[test]
fn test_fixture_exact_two_parts() {
    // 19_456_000 bytes: checks 2 parts + empty MD4 block
    check_pattern_fixture(&FIXTURE_EXACT_TWO_PARTS);
}

#[test]
fn test_chunk_streaming_invariance() {
    let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
    let mut md4 = Md4::new();
    md4.update(&data);
    let expected: [u8; 16] = md4.finalize().into();

    for chunk_size in [1, 7, 64, 1024, 4096, 65536] {
        let mut hasher = Ed2kHasher::new();
        for chunk in data.chunks(chunk_size) {
            hasher.update(chunk);
        }
        let res = hasher.finalize().unwrap();
        assert_eq!(
            res.root_hash.as_bytes(),
            &expected,
            "Failed for chunk size {chunk_size}"
        );
    }
}
