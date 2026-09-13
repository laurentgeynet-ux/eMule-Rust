//! Integration test suite for the passive eD2K fixture corpus and read-only decoders.

use std::fs;
use std::path::PathBuf;

use mule_protocol::ed2k::{Ed2kDecoder, Ed2kErrorKind, Ed2kLimits, Ed2kMessage, TagValue, PR_ED2K};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(rename = "schemaVersion")]
    _schema_version: u32,
    #[serde(rename = "fixtures")]
    fixtures: Vec<FixtureEntry>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FixtureEntry {
    name: String,
    file: String,
    sha256: String,
    #[serde(default)]
    expected: String,
}

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ed2k")
}

/// Computes the standard FIPS 180-4 SHA-256 digest of a byte slice in pure safe Rust.
fn compute_sha256(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h0: u32 = 0x6a09e667;
    let mut h1: u32 = 0xbb67ae85;
    let mut h2: u32 = 0x3c6ef372;
    let mut h3: u32 = 0xa54ff53a;
    let mut h4: u32 = 0x510e527f;
    let mut h5: u32 = 0x9b05688c;
    let mut h6: u32 = 0x1f83d9ab;
    let mut h7: u32 = 0x5be0cd19;

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    let (chunks, _) = msg.as_chunks::<64>();
    for chunk in chunks {
        let mut w = [0u32; 64];
        for (i, w_elem) in w.iter_mut().take(16).enumerate() {
            let mut arr = [0u8; 4];
            arr.copy_from_slice(&chunk[i * 4..(i + 1) * 4]);
            *w_elem = u32::from_be_bytes(arr);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;
        let mut f = h5;
        let mut g = h6;
        let mut h = h7;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
        h5 = h5.wrapping_add(f);
        h6 = h6.wrapping_add(g);
        h7 = h7.wrapping_add(h);
    }

    let mut out = [0u8; 32];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out[20..24].copy_from_slice(&h5.to_be_bytes());
    out[24..28].copy_from_slice(&h6.to_be_bytes());
    out[28..32].copy_from_slice(&h7.to_be_bytes());
    hex::encode(out)
}

#[test]
fn test_corpus_manifest_and_hash_integrity() {
    let root = fixtures_root();
    let manifest_path = root.join("manifest.json");
    let manifest_str = fs::read_to_string(&manifest_path).expect("failed to read manifest.json");
    let clean_str = manifest_str.trim_start_matches('\u{feff}');
    let manifest: Manifest =
        serde_json::from_str(clean_str).expect("failed to parse manifest.json");

    assert!(
        !manifest.fixtures.is_empty(),
        "manifest must contain fixtures"
    );

    for entry in &manifest.fixtures {
        let fixture_path = root.join(&entry.file);
        assert!(
            fixture_path.exists(),
            "fixture file not found: {}",
            entry.file
        );
        let bytes = fs::read(&fixture_path).expect("failed to read fixture file");
        let computed_hash = compute_sha256(&bytes);
        assert_eq!(
            computed_hash, entry.sha256,
            "SHA-256 mismatch for fixture: {}",
            entry.name
        );
    }
}

#[test]
fn test_decode_hello_v1_minimal() {
    let path = fixtures_root().join("hello_v1_minimal.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let msg = decoder.decode_packet(&bytes).expect("decoding failed");

    match msg {
        Ed2kMessage::Hello(hello) => {
            let expected_hash: Vec<u8> = (1..=16).collect();
            assert_eq!(hello.user_hash, expected_hash.as_slice());
            assert_eq!(hello.client_id, 0xC0A80101);
            assert_eq!(hello.tcp_port, 4662);
            assert_eq!(hello.tags.len(), 0);
            assert_eq!(hello.unknown_tags.len(), 0);
            assert_eq!(hello.server_ip, 0);
            assert_eq!(hello.server_port, 0);
        }
        other => panic!("expected Hello message, got: {other:?}"),
    }
}

#[test]
fn test_decode_hello_v1_tags() {
    let path = fixtures_root().join("hello_v1_tags.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let msg = decoder.decode_packet(&bytes).expect("decoding failed");

    match msg {
        Ed2kMessage::Hello(hello) => {
            assert_eq!(hello.user_hash, [0xAA; 16]);
            assert_eq!(hello.client_id, 0x11223344);
            assert_eq!(hello.tcp_port, 4662);
            assert_eq!(hello.tags.len(), 3);
            assert_eq!(hello.unknown_tags.len(), 0);

            let name_tag = hello.find_tag("name").expect("name tag missing");
            assert_eq!(name_tag.value, TagValue::String("EmuleSion".to_string()));

            let version_tag = hello.find_tag("version").expect("version tag missing");
            assert_eq!(version_tag.value, TagValue::UInt32(60));

            let prt_tag = hello.find_tag("prt").expect("prt tag missing");
            assert_eq!(prt_tag.value, TagValue::UInt16(4662));

            assert_eq!(hello.server_ip, 0x7F000001);
            assert_eq!(hello.server_port, 4661);
        }
        other => panic!("expected Hello message, got: {other:?}"),
    }
}

#[test]
fn test_decode_hello_answer() {
    let path = fixtures_root().join("hello_answer.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let msg = decoder.decode_packet(&bytes).expect("decoding failed");

    match msg {
        Ed2kMessage::HelloAnswer(answer) => {
            assert_eq!(answer.user_hash, [0xBB; 16]);
            assert_eq!(answer.client_id, 0x55667788);
            assert_eq!(answer.tcp_port, 4662);
            assert_eq!(answer.tags.len(), 1);

            let name_tag = answer.find_tag("name").expect("name tag missing");
            assert_eq!(name_tag.value, TagValue::String("RemotePeer".to_string()));

            assert_eq!(answer.server_ip, 0x0A000001);
            assert_eq!(answer.server_port, 4661);
        }
        other => panic!("expected HelloAnswer message, got: {other:?}"),
    }
}

#[test]
fn test_decode_file_status() {
    let path = fixtures_root().join("file_status.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let msg = decoder.decode_packet(&bytes).expect("decoding failed");

    match msg {
        Ed2kMessage::FileStatus(status) => {
            assert_eq!(status.file_hash, [0xCC; 16]);
            assert_eq!(status.part_count, 12);
            assert_eq!(status.status_bitfield, vec![0xFF, 0x0F]);

            // Test parts availability inspection
            for i in 0..12 {
                assert!(status.has_part(i), "part {i} should be available");
            }
            assert!(
                !status.has_part(12),
                "part 12 is out of range, must be false"
            );
        }
        other => panic!("expected FileStatus message, got: {other:?}"),
    }
}

#[test]
fn test_malformed_truncated_header() {
    let path = fixtures_root()
        .join("malformed")
        .join("hello_truncated_header.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let err = decoder.decode_packet(&bytes).unwrap_err();

    assert_eq!(err.context, "header");
    assert_eq!(err.offset, 0);
    assert!(matches!(err.kind, Ed2kErrorKind::UnexpectedEof { .. }));
}

#[test]
fn test_malformed_truncated_tag() {
    let path = fixtures_root()
        .join("malformed")
        .join("hello_truncated_tag.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let err = decoder.decode_packet(&bytes).unwrap_err();

    assert_eq!(err.context, "tag_value_string");
    assert_eq!(err.offset, 32);
    assert!(matches!(err.kind, Ed2kErrorKind::UnexpectedEof { .. }));
}

#[test]
fn test_malformed_oversized_string() {
    let path = fixtures_root()
        .join("malformed")
        .join("oversized_string.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let err = decoder.decode_packet(&bytes).unwrap_err();

    assert_eq!(err.context, "tag_value_string");
    match err.kind {
        Ed2kErrorKind::OversizedString { size, limit } => {
            assert_eq!(size, 1500);
            assert_eq!(limit, 1024);
        }
        other => panic!("expected OversizedString, got: {other:?}"),
    }
}

#[test]
fn test_malformed_too_many_tags() {
    let path = fixtures_root().join("malformed").join("too_many_tags.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let err = decoder.decode_packet(&bytes).unwrap_err();

    assert_eq!(err.context, "hello_tag_count");
    match err.kind {
        Ed2kErrorKind::TooManyTags { count, limit } => {
            assert_eq!(count, 300);
            assert_eq!(limit, 256);
        }
        other => panic!("expected TooManyTags, got: {other:?}"),
    }
}

#[test]
fn test_malformed_unsupported_protocol_0xd4() {
    let path = fixtures_root()
        .join("malformed")
        .join("unsupported_protocol_0xd4.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let err = decoder.decode_packet(&bytes).unwrap_err();

    assert_eq!(err.protocol, 0xD4);
    assert_eq!(err.context, "protocol_byte");
    assert_eq!(err.kind, Ed2kErrorKind::CompressedProtocolUnsupported);
}

#[test]
fn test_zero_packet_length_rejected() {
    let bytes = [PR_ED2K, 0x00, 0x00, 0x00, 0x00, 0x01];
    let decoder = Ed2kDecoder::new();
    let err = decoder.decode_packet(&bytes).unwrap_err();

    assert_eq!(err.context, "packet_length");
    assert_eq!(err.kind, Ed2kErrorKind::FrameTooSmall { size: 0 });
}

#[test]
fn test_unknown_protocol_rejected() {
    let bytes = [0xFA, 0x05, 0x00, 0x00, 0x00, 0x01];
    let decoder = Ed2kDecoder::new();
    let err = decoder.decode_packet(&bytes).unwrap_err();

    assert_eq!(err.protocol, 0xFA);
    assert_eq!(err.context, "protocol_byte");
    assert_eq!(err.kind, Ed2kErrorKind::UnsupportedProtocol { byte: 0xFA });
}

#[test]
fn test_custom_limits_enforced() {
    let path = fixtures_root().join("hello_v1_tags.bin");
    let bytes = fs::read(path).unwrap();

    let limits = Ed2kLimits {
        max_hello_frame_bytes: 50, // hello_v1_tags frame is 64 bytes > 50
        ..Default::default()
    };

    let decoder = Ed2kDecoder::with_limits(limits);
    let err = decoder.decode_packet(&bytes).unwrap_err();

    assert_eq!(err.context, "frame_len");
    match err.kind {
        Ed2kErrorKind::FrameTooLarge { size, limit } => {
            assert_eq!(limit, 50);
            assert_eq!(size, 64);
        }
        other => panic!("expected FrameTooLarge, got: {other:?}"),
    }
}
