use md4::{Digest, Md4};
use mule_format::{
    hash_reader, Ed2kHasher, FormatError, ED2K_PART_SIZE, MD4_EMPTY_DIGEST,
};
use std::io::Cursor;

/// Helper to compute raw MD4
fn md4_digest(data: &[u8]) -> [u8; 16] {
    let mut h = Md4::new();
    h.update(data);
    h.finalize().into()
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
fn test_single_byte_hash() {
    let data = b"A";
    let expected = md4_digest(data);

    let mut hasher = Ed2kHasher::new();
    hasher.update(data);
    let res = hasher.finalize().unwrap();

    assert_eq!(res.root_hash.as_bytes(), &expected);
    assert_eq!(res.part_hashes.len(), 1);
    assert_eq!(res.total_bytes, 1);
}

#[test]
fn test_small_data_below_part_size() {
    let data = b"The quick brown fox jumps over the lazy dog";
    let expected = md4_digest(data);

    let mut hasher = Ed2kHasher::new();
    hasher.update(data);
    let res = hasher.finalize().unwrap();

    assert_eq!(res.root_hash.as_bytes(), &expected);
    assert_eq!(res.part_hashes.len(), 1);
}

#[test]
fn test_just_under_part_size() {
    // 9_727_999 bytes
    let size = ED2K_PART_SIZE - 1;
    let pattern = vec![0x5a; size];
    let expected = md4_digest(&pattern);

    let mut hasher = Ed2kHasher::new();
    hasher.update(&pattern);
    let res = hasher.finalize().unwrap();

    assert_eq!(res.root_hash.as_bytes(), &expected);
    assert_eq!(res.part_hashes.len(), 1);
    assert_eq!(res.total_bytes, size as u64);
}

#[test]
fn test_exactly_one_part_size() {
    // Exactly 9_728_000 bytes:
    // eMule rule: MD4(MD4(part_0) || MD4(empty))
    let size = ED2K_PART_SIZE;
    let pattern = vec![0x7b; size];

    let part0_md4 = md4_digest(&pattern);
    let mut outer = Md4::new();
    outer.update(&part0_md4);
    outer.update(&MD4_EMPTY_DIGEST);
    let expected_root: [u8; 16] = outer.finalize().into();

    let mut hasher = Ed2kHasher::new();
    hasher.update(&pattern);
    let res = hasher.finalize().unwrap();

    assert_eq!(res.total_bytes, size as u64);
    assert_eq!(res.part_hashes.len(), 2);
    assert_eq!(res.part_hashes[0], part0_md4);
    assert_eq!(res.part_hashes[1], MD4_EMPTY_DIGEST);
    assert_eq!(res.root_hash.as_bytes(), &expected_root);
}

#[test]
fn test_one_part_plus_one_byte() {
    // 9_728_001 bytes: 1 full part + 1 trailing byte
    let mut data = vec![0x42; ED2K_PART_SIZE];
    data.push(0x43);

    let part0_md4 = md4_digest(&data[..ED2K_PART_SIZE]);
    let part1_md4 = md4_digest(&data[ED2K_PART_SIZE..]);

    let mut outer = Md4::new();
    outer.update(&part0_md4);
    outer.update(&part1_md4);
    let expected_root: [u8; 16] = outer.finalize().into();

    let mut hasher = Ed2kHasher::new();
    hasher.update(&data);
    let res = hasher.finalize().unwrap();

    assert_eq!(res.total_bytes, ED2K_PART_SIZE as u64 + 1);
    assert_eq!(res.part_hashes.len(), 2);
    assert_eq!(res.part_hashes[0], part0_md4);
    assert_eq!(res.part_hashes[1], part1_md4);
    assert_eq!(res.root_hash.as_bytes(), &expected_root);
}

#[test]
fn test_exact_multiple_two_parts() {
    // 2 * 9_728_000 bytes:
    // Should produce part0, part1, and MD4(empty)
    let size = 2 * ED2K_PART_SIZE;
    let data = vec![0x11; size];

    let part0_md4 = md4_digest(&data[..ED2K_PART_SIZE]);
    let part1_md4 = md4_digest(&data[ED2K_PART_SIZE..]);

    let mut outer = Md4::new();
    outer.update(&part0_md4);
    outer.update(&part1_md4);
    outer.update(&MD4_EMPTY_DIGEST);
    let expected_root: [u8; 16] = outer.finalize().into();

    let mut hasher = Ed2kHasher::new();
    hasher.update(&data);
    let res = hasher.finalize().unwrap();

    assert_eq!(res.total_bytes, size as u64);
    assert_eq!(res.part_hashes.len(), 3);
    assert_eq!(res.part_hashes[0], part0_md4);
    assert_eq!(res.part_hashes[1], part1_md4);
    assert_eq!(res.part_hashes[2], MD4_EMPTY_DIGEST);
    assert_eq!(res.root_hash.as_bytes(), &expected_root);
}

#[test]
fn test_chunk_streaming_invariance() {
    // 100_000 bytes hashed with different buffer chunk sizes must produce identical hash
    let data: Vec<u8> = (0..100_000).map(|i| (i % 256) as u8).collect();
    let expected = md4_digest(&data);

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
