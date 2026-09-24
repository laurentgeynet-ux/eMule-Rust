// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Integration test suite for the passive eD2K fixture corpus and read-only decoders.

use std::fs;
use std::path::PathBuf;

use mule_protocol::ed2k::{
    Ed2kCompatibilityProfile, Ed2kDecoder, Ed2kErrorKind, Ed2kLimits, Ed2kMessage, Ed2kParseError,
    TagValue, PR_ED2K,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(rename = "schemaVersion")]
    _schema_version: u32,
    #[serde(rename = "fixtures")]
    fixtures: Vec<FixtureEntry>,
}

#[derive(Debug, Deserialize)]
struct FixtureEntry {
    name: String,
    file: String,
    sha256: String,
    #[serde(rename = "protocolByte")]
    protocol_byte: Option<String>,
    opcode: Option<String>,
    direction: String,
    origin: String,
    expected: String,
    #[serde(rename = "expectedError")]
    expected_error: Option<String>,
    #[serde(rename = "expectedOffset")]
    expected_offset: Option<usize>,
}

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ed2k")
}

mod common;
use common::sha256::compute_sha256;

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
        assert!(!entry.name.is_empty(), "fixture name must not be empty");
        assert!(
            !entry.origin.is_empty(),
            "origin must not be empty for {}",
            entry.name
        );
        assert!(
            !entry.direction.is_empty(),
            "direction must not be empty for {}",
            entry.name
        );
        assert!(
            entry.protocol_byte.is_some(),
            "protocolByte must be set in manifest for {}",
            entry.name
        );
        assert!(
            entry.opcode.is_some(),
            "opcode must be set in manifest for {}",
            entry.name
        );

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

        let decoder =
            if entry.opcode.as_deref() == Some("0x49") || entry.opcode.as_deref() == Some("0x4B") {
                Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey)
            } else {
                Ed2kDecoder::new()
            };
        let decode_result = decoder.decode_packet(&bytes);

        if entry.expected == "valid" {
            assert!(
                decode_result.is_ok(),
                "expected valid decoding for {}, got error: {:?}",
                entry.name,
                decode_result
            );
            assert!(
                entry.expected_error.is_none(),
                "expectedError should be null for valid fixture {}",
                entry.name
            );
        } else {
            assert!(
                decode_result.is_err(),
                "expected error for {}, got ok: {:?}",
                entry.name,
                decode_result
            );
            let err = decode_result.unwrap_err();
            if let Some(expected_err) = &entry.expected_error {
                match expected_err.as_str() {
                    "unexpected_eof" => {
                        assert!(matches!(err.kind, Ed2kErrorKind::UnexpectedEof { .. }));
                    }
                    "oversized_string" => {
                        assert!(matches!(err.kind, Ed2kErrorKind::OversizedString { .. }));
                    }
                    "too_many_tags" => {
                        assert!(matches!(err.kind, Ed2kErrorKind::TooManyTags { .. }));
                    }
                    "compressed_protocol_unsupported" => {
                        assert!(matches!(
                            err.kind,
                            Ed2kErrorKind::CompressedProtocolUnsupported
                        ));
                    }
                    "invalid_block_offsets" => {
                        assert!(matches!(
                            err.kind,
                            Ed2kErrorKind::InvalidBlockOffsets { .. }
                        ));
                    }
                    "block_payload_mismatch" => {
                        assert!(matches!(
                            err.kind,
                            Ed2kErrorKind::BlockPayloadMismatch { .. }
                        ));
                    }
                    other => panic!("unknown expected error in manifest: {other}"),
                }
            }
            if let Some(expected_offset) = entry.expected_offset {
                assert_eq!(
                    err.offset, expected_offset,
                    "offset mismatch for fixture {}",
                    entry.name
                );
            }
        }
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
            assert_eq!(name_tag.value, TagValue::String("eMuleRust".to_string()));

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
            assert_eq!(status.raw_bytes(), &[0xFF, 0x0F]);

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
fn test_decode_file_status_multi_parts() {
    let path = fixtures_root().join("file_status_multi_parts.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let msg = decoder.decode_packet(&bytes).expect("decoding failed");

    match msg {
        Ed2kMessage::FileStatus(status) => {
            assert_eq!(status.file_hash, [0xDD; 16]);
            assert_eq!(status.part_count, 10);
            assert_eq!(status.raw_bytes(), &[0x83, 0x03]);

            // Parts 0, 1, 7, 8, 9 are available
            assert!(status.has_part(0));
            assert!(status.has_part(1));
            assert!(!status.has_part(2));
            assert!(!status.has_part(6));
            assert!(status.has_part(7));
            assert!(status.has_part(8));
            assert!(status.has_part(9));
            assert!(!status.has_part(10));

            let available: Vec<u16> = status.availability_mask.available_parts().collect();
            assert_eq!(available, vec![0, 1, 7, 8, 9]);
        }
        other => panic!("expected FileStatus message, got: {other:?}"),
    }
}

#[test]
fn test_decode_file_status_req() {
    let path = fixtures_root().join("file_status_req.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let msg = decoder.decode_packet(&bytes).expect("decoding failed");

    match msg {
        Ed2kMessage::FileStatusReq(req) => {
            assert_eq!(req.file_hash, [0xCC; 16]);
        }
        other => panic!("expected FileStatusReq message, got: {other:?}"),
    }

    // Default Emule050a decoder treats legacy 0x49 as raw packet
    let default_decoder = Ed2kDecoder::new();
    let default_msg = default_decoder
        .decode_packet(&bytes)
        .expect("decoding raw failed");
    assert!(matches!(default_msg, Ed2kMessage::Raw(_)));
}

/// Décode un fixture du répertoire `malformed/` et renvoie l'erreur attendue.
fn decode_malformed(fixture: &str) -> Ed2kParseError {
    let path = fixtures_root().join("malformed").join(fixture);
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    Ed2kDecoder::new().decode_packet(&bytes).unwrap_err()
}

#[test]
fn test_malformed_truncated_header() {
    let err = decode_malformed("hello_truncated_header.bin");

    assert_eq!(err.context, "header");
    assert_eq!(err.offset, 0);
    assert!(matches!(err.kind, Ed2kErrorKind::UnexpectedEof { .. }));
}

#[test]
fn test_malformed_truncated_tag() {
    let err = decode_malformed("hello_truncated_tag.bin");

    assert_eq!(err.context, "tag_value_string");
    assert_eq!(err.offset, 32);
    assert!(matches!(err.kind, Ed2kErrorKind::UnexpectedEof { .. }));
}

#[test]
fn test_malformed_oversized_string() {
    let err = decode_malformed("oversized_string.bin");

    assert_eq!(err.context, "tag_value_string");
    assert_eq!(err.offset, 30);
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
    let err = decode_malformed("too_many_tags.bin");

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
    let err = decode_malformed("unsupported_protocol_0xd4.bin");

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

#[test]
fn test_decode_request_parts_fixture() {
    let path = fixtures_root().join("request_parts.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let msg = decoder.decode_packet(&bytes).expect("decoding failed");

    match msg {
        Ed2kMessage::RequestParts(req) => {
            assert_eq!(req.file_hash, [0xCC; 16]);
            assert_eq!(req.ranges.len(), 1);
            assert_eq!(req.ranges[0].start, 0);
            assert_eq!(req.ranges[0].end, 184320);
            assert_eq!(req.ranges[0].len(), 184320);
        }
        other => panic!("expected RequestParts, got: {other:?}"),
    }

    // Default Emule050a decoder treats legacy 0x4B as raw packet
    let default_decoder = Ed2kDecoder::new();
    let default_msg = default_decoder
        .decode_packet(&bytes)
        .expect("decoding raw failed");
    assert!(matches!(default_msg, Ed2kMessage::Raw(_)));
}

#[test]
fn test_decode_sending_part_fixture() {
    let path = fixtures_root().join("sending_part.bin");
    let bytes = fs::read(path).unwrap();

    let decoder = Ed2kDecoder::new();
    let msg = decoder.decode_packet(&bytes).expect("decoding failed");

    match msg {
        Ed2kMessage::SendingPart(part) => {
            assert_eq!(part.file_hash, [0xCC; 16]);
            assert_eq!(part.start_offset, 0);
            assert_eq!(part.end_offset, 1024);
            assert_eq!(part.len(), 1024);
            for (i, byte) in part.data.iter().enumerate() {
                assert_eq!(*byte, (i % 256) as u8);
            }
        }
        other => panic!("expected SendingPart, got: {other:?}"),
    }
}
