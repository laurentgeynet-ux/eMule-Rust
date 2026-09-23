// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use md4::{Digest, Md4};
use mule_format::{
    hash_reader, Ed2kHasher, FormatError, PatternReader, FIXTURE_1_BYTE, FIXTURE_EXACT_ONE_PART,
    FIXTURE_EXACT_TWO_PARTS, FIXTURE_JUST_UNDER_PART, FIXTURE_ONE_PART_PLUS_ONE,
    REPEATING_0_TO_255,
};
use std::io::Cursor;

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
    let mut reader = PatternReader::new(&REPEATING_0_TO_255, FIXTURE_1_BYTE.length);
    let res = hash_reader(&mut reader).unwrap();

    assert_eq!(res.total_bytes, FIXTURE_1_BYTE.length);
    assert_eq!(res.root_hash.to_hex(), FIXTURE_1_BYTE.expected_ed2k_hex);
    assert_eq!(
        res.part_hashes.len(),
        FIXTURE_1_BYTE.expected_part_hashes_count
    );
}

#[test]
fn test_fixture_just_under_part() {
    let mut reader = PatternReader::new(&REPEATING_0_TO_255, FIXTURE_JUST_UNDER_PART.length);
    let res = hash_reader(&mut reader).unwrap();

    assert_eq!(res.total_bytes, FIXTURE_JUST_UNDER_PART.length);
    assert_eq!(
        res.root_hash.to_hex(),
        FIXTURE_JUST_UNDER_PART.expected_ed2k_hex
    );
    assert_eq!(
        res.part_hashes.len(),
        FIXTURE_JUST_UNDER_PART.expected_part_hashes_count
    );
}

#[test]
fn test_fixture_exact_one_part() {
    // 9_728_000 bytes: checks append of empty MD4 block
    let mut reader = PatternReader::new(&REPEATING_0_TO_255, FIXTURE_EXACT_ONE_PART.length);
    let res = hash_reader(&mut reader).unwrap();

    assert_eq!(res.total_bytes, FIXTURE_EXACT_ONE_PART.length);
    assert_eq!(
        res.root_hash.to_hex(),
        FIXTURE_EXACT_ONE_PART.expected_ed2k_hex
    );
    assert_eq!(
        res.part_hashes.len(),
        FIXTURE_EXACT_ONE_PART.expected_part_hashes_count
    );
}

#[test]
fn test_fixture_one_part_plus_one() {
    let mut reader = PatternReader::new(&REPEATING_0_TO_255, FIXTURE_ONE_PART_PLUS_ONE.length);
    let res = hash_reader(&mut reader).unwrap();

    assert_eq!(res.total_bytes, FIXTURE_ONE_PART_PLUS_ONE.length);
    assert_eq!(
        res.root_hash.to_hex(),
        FIXTURE_ONE_PART_PLUS_ONE.expected_ed2k_hex
    );
    assert_eq!(
        res.part_hashes.len(),
        FIXTURE_ONE_PART_PLUS_ONE.expected_part_hashes_count
    );
}

#[test]
fn test_fixture_exact_two_parts() {
    // 19_456_000 bytes: checks 2 parts + empty MD4 block
    let mut reader = PatternReader::new(&REPEATING_0_TO_255, FIXTURE_EXACT_TWO_PARTS.length);
    let res = hash_reader(&mut reader).unwrap();

    assert_eq!(res.total_bytes, FIXTURE_EXACT_TWO_PARTS.length);
    assert_eq!(
        res.root_hash.to_hex(),
        FIXTURE_EXACT_TWO_PARTS.expected_ed2k_hex
    );
    assert_eq!(
        res.part_hashes.len(),
        FIXTURE_EXACT_TWO_PARTS.expected_part_hashes_count
    );
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
