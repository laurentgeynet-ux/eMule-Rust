//! Comprehensive roundtrip and deterministic encoding test suite for eD2K wire messages.

use std::fs;
use std::path::PathBuf;

use mule_protocol::ed2k::{
    Ed2kBlockRange, Ed2kCompatibilityProfile, Ed2kDecoder, Ed2kEmuleInfo, Ed2kEncoder,
    Ed2kErrorKind, Ed2kFileReqAnsNoFile, Ed2kFileStatus, Ed2kFileStatusReq, Ed2kHello,
    Ed2kHelloAnswer, Ed2kLimits, Ed2kMessage, Ed2kRequestParts, Ed2kSendingPart, Ed2kTag,
    PartAvailabilityMask, RawTag, TagName, TagValue, TAG_SPECIAL_EMULE_VERSION,
    TAG_SPECIAL_MISCOPTIONS1, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
};


fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ed2k")
}

// ---------------------------------------------------------------------------
// 1. Fixture Roundtrip Invariance: encode(decode(fixture)) == fixture
// ---------------------------------------------------------------------------

#[test]
fn test_fixture_roundtrip_hello_v1_minimal() {
    let raw_bytes = fs::read(fixtures_root().join("hello_v1_minimal.bin"))
        .expect("failed to read hello_v1_minimal.bin");

    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let msg = decoder
        .decode_packet(&raw_bytes)
        .expect("failed to decode hello_v1_minimal");

    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let encoded_bytes = encoder
        .encode_to_bytes(&msg)
        .expect("failed to encode hello_v1_minimal");

    assert_eq!(
        encoded_bytes.as_ref(),
        raw_bytes.as_slice(),
        "roundtrip bytes mismatch for hello_v1_minimal.bin"
    );
}

#[test]
fn test_fixture_roundtrip_hello_v1_tags() {
    let raw_bytes = fs::read(fixtures_root().join("hello_v1_tags.bin"))
        .expect("failed to read hello_v1_tags.bin");

    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let msg = decoder
        .decode_packet(&raw_bytes)
        .expect("failed to decode hello_v1_tags");

    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let encoded_bytes = encoder
        .encode_to_bytes(&msg)
        .expect("failed to encode hello_v1_tags");

    assert_eq!(
        encoded_bytes.as_ref(),
        raw_bytes.as_slice(),
        "roundtrip bytes mismatch for hello_v1_tags.bin"
    );
}

#[test]
fn test_fixture_roundtrip_hello_answer() {
    let raw_bytes = fs::read(fixtures_root().join("hello_answer.bin"))
        .expect("failed to read hello_answer.bin");

    let decoder = Ed2kDecoder::new();
    let msg = decoder
        .decode_packet(&raw_bytes)
        .expect("failed to decode hello_answer");

    let encoder = Ed2kEncoder::new();
    let encoded_bytes = encoder
        .encode_to_bytes(&msg)
        .expect("failed to encode hello_answer");

    assert_eq!(
        encoded_bytes.as_ref(),
        raw_bytes.as_slice(),
        "roundtrip bytes mismatch for hello_answer.bin"
    );
}

#[test]
fn test_fixture_roundtrip_file_status() {
    let raw_bytes = fs::read(fixtures_root().join("file_status.bin"))
        .expect("failed to read file_status.bin");

    let decoder = Ed2kDecoder::new();
    let msg = decoder
        .decode_packet(&raw_bytes)
        .expect("failed to decode file_status");

    let encoder = Ed2kEncoder::new();
    let encoded_bytes = encoder
        .encode_to_bytes(&msg)
        .expect("failed to encode file_status");

    assert_eq!(
        encoded_bytes.as_ref(),
        raw_bytes.as_slice(),
        "roundtrip bytes mismatch for file_status.bin"
    );
}

#[test]
fn test_fixture_roundtrip_request_parts() {
    let raw_bytes = fs::read(fixtures_root().join("request_parts.bin"))
        .expect("failed to read request_parts.bin");

    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let msg = decoder
        .decode_packet(&raw_bytes)
        .expect("failed to decode request_parts");
    assert!(matches!(msg, Ed2kMessage::RequestParts(_)));

    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let encoded_bytes = encoder
        .encode_to_bytes(&msg)
        .expect("failed to encode request_parts");

    assert_eq!(
        encoded_bytes.as_ref(),
        raw_bytes.as_slice(),
        "roundtrip bytes mismatch for request_parts.bin"
    );
}

#[test]
fn test_fixture_roundtrip_file_status_req() {
    let raw_bytes = fs::read(fixtures_root().join("file_status_req.bin"))
        .expect("failed to read file_status_req.bin");

    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let msg = decoder
        .decode_packet(&raw_bytes)
        .expect("failed to decode file_status_req");
    assert!(matches!(msg, Ed2kMessage::FileStatusReq(_)));

    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let encoded_bytes = encoder
        .encode_to_bytes(&msg)
        .expect("failed to encode file_status_req");

    assert_eq!(
        encoded_bytes.as_ref(),
        raw_bytes.as_slice(),
        "roundtrip bytes mismatch for file_status_req.bin"
    );
}

#[test]
fn test_fixture_roundtrip_sending_part() {
    let raw_bytes = fs::read(fixtures_root().join("sending_part.bin"))
        .expect("failed to read sending_part.bin");

    let decoder = Ed2kDecoder::new();
    let msg = decoder
        .decode_packet(&raw_bytes)
        .expect("failed to decode sending_part");

    let encoder = Ed2kEncoder::new();
    let encoded_bytes = encoder
        .encode_to_bytes(&msg)
        .expect("failed to encode sending_part");

    assert_eq!(
        encoded_bytes.as_ref(),
        raw_bytes.as_slice(),
        "roundtrip bytes mismatch for sending_part.bin"
    );
}

// ---------------------------------------------------------------------------
// 2. Synthetic Structure Invariance: decode(encode(msg)) == msg
// ---------------------------------------------------------------------------

#[test]
fn test_synthetic_roundtrip_hello_all_tag_types() {
    let hello = Ed2kHello {
        user_hash: [0x42; 16],
        client_id: 0x12345678,
        tcp_port: 4662,
        tags: vec![
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_NAME),
                value: TagValue::String("AntigravityPeer".to_string()),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_VERSION),
                value: TagValue::UInt32(60),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_EMULE_VERSION),
                value: TagValue::UInt32(0x00320000),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_MISCOPTIONS1),
                value: TagValue::UInt32(0x00000001),
            },
            Ed2kTag {
                name: TagName::Text("custom_hash".to_string()),
                value: TagValue::Hash([0x99; 16]),
            },
            Ed2kTag {
                name: TagName::Text("ratio".to_string()),
                value: TagValue::Float32(2.75),
            },

            Ed2kTag {
                name: TagName::Text("active".to_string()),
                value: TagValue::Bool(true),
            },
            Ed2kTag {
                name: TagName::Text("inactive".to_string()),
                value: TagValue::Bool(false),
            },
            Ed2kTag {
                name: TagName::Text("payload_blob".to_string()),
                value: TagValue::Blob(vec![1, 2, 3, 4, 5, 6, 7, 8]),
            },
            Ed2kTag {
                name: TagName::Text("port16".to_string()),
                value: TagValue::UInt16(4672),
            },
            Ed2kTag {
                name: TagName::Text("flag8".to_string()),
                value: TagValue::UInt8(0xEF),
            },
            Ed2kTag {
                name: TagName::Text("big_int64".to_string()),
                value: TagValue::UInt64(0x0123456789ABCDEF),
            },
        ],
        unknown_tags: vec![],
        server_ip: 0x7F000001,
        server_port: 4661,
    };

    let original_msg = Ed2kMessage::Hello(hello);

    let encoder = Ed2kEncoder::new();
    let encoded = encoder
        .encode_to_bytes(&original_msg)
        .expect("encoding failed");

    let decoder = Ed2kDecoder::new();
    let decoded = decoder
        .decode_packet(&encoded)
        .expect("decoding failed");

    assert_eq!(
        decoded, original_msg,
        "roundtrip mismatch for comprehensive hello message"
    );
}

#[test]
fn test_synthetic_roundtrip_hello_minimal_no_server_info() {
    let hello = Ed2kHello {
        user_hash: [0x11; 16],
        client_id: 0,
        tcp_port: 4662,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };

    let original_msg = Ed2kMessage::Hello(hello);

    let encoder = Ed2kEncoder::new();
    let encoded = encoder
        .encode_to_bytes(&original_msg)
        .expect("encoding failed");

    // 5-byte header + 1 opcode + 1 hash len (0x10) + 16 user hash + 4 client id + 2 port + 4 tag count + 4 server ip + 2 server port = 39 bytes
    assert_eq!(encoded.len(), 39);


    let decoder = Ed2kDecoder::new();
    let decoded = decoder
        .decode_packet(&encoded)
        .expect("decoding failed");

    assert_eq!(
        decoded, original_msg,
        "roundtrip mismatch for minimal hello without server info"
    );
}

#[test]
fn test_synthetic_roundtrip_hello_answer() {
    let answer = Ed2kHelloAnswer {
        user_hash: [0xAA; 16],
        client_id: 0x98765432,
        tcp_port: 4662,
        tags: vec![Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_NAME),
            value: TagValue::String("RemotePeerAnswer".to_string()),
        }],
        unknown_tags: vec![],
        server_ip: 0xC0A80001,
        server_port: 4661,
    };

    let original_msg = Ed2kMessage::HelloAnswer(answer);

    let encoder = Ed2kEncoder::new();
    let encoded = encoder
        .encode_to_bytes(&original_msg)
        .expect("encoding failed");

    let decoder = Ed2kDecoder::new();
    let decoded = decoder
        .decode_packet(&encoded)
        .expect("decoding failed");

    assert_eq!(decoded, original_msg, "roundtrip mismatch for hello answer");
}

#[test]
fn test_synthetic_roundtrip_unknown_raw_tags_preservation() {
    // Tag with standard string type (0x02) that decoder will parse into a tag or custom raw bytes
    // Test that unknown_tags roundtrips faithfully when encoded
    let hello = Ed2kHello {
        user_hash: [0x55; 16],
        client_id: 0x11111111,
        tcp_port: 4662,
        tags: vec![Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_NAME),
            value: TagValue::String("TestPeer".to_string()),
        }],
        unknown_tags: vec![RawTag {
            tag_type: 0x03, // UInt32 raw byte representation
            name: TagName::Text("raw_val".to_string()),
            raw_bytes: vec![0x10, 0x20, 0x30, 0x40],
        }],
        server_ip: 0,
        server_port: 0,
    };

    let encoder = Ed2kEncoder::new();
    let packet = encoder.encode_hello(&hello).expect("encoding failed");
    assert_eq!(packet.opcode, 0x01);

    let raw_wire = encoder
        .encode_to_bytes(&Ed2kMessage::Hello(hello.clone()))
        .expect("to_bytes failed");

    let decoder = Ed2kDecoder::new();
    let decoded = decoder
        .decode_packet(&raw_wire)
        .expect("decoding failed");

    match decoded {
        Ed2kMessage::Hello(decoded_hello) => {
            assert_eq!(decoded_hello.user_hash, hello.user_hash);
            assert_eq!(decoded_hello.client_id, hello.client_id);
            assert_eq!(decoded_hello.tcp_port, hello.tcp_port);
            // The recognized tag is decoded as String
            assert_eq!(decoded_hello.tags[0].value, TagValue::String("TestPeer".to_string()));
            // The raw tag was encoded with tag_type 0x03, which the decoder recognizes as UInt32!
            assert_eq!(decoded_hello.tags[1].name, TagName::Text("raw_val".to_string()));
            assert_eq!(decoded_hello.tags[1].value, TagValue::UInt32(0x40302010));
        }
        _ => panic!("expected Hello"),
    }
}

#[test]
fn test_synthetic_roundtrip_file_status_various_part_counts() {
    let test_cases: Vec<(u16, Vec<u8>)> = vec![
        (0, vec![]),
        (1, vec![0b00000001]),
        (7, vec![0b01111111]),
        (8, vec![0b11111111]),
        (9, vec![0b11111111, 0b00000001]),
        (53, vec![0xFF; 6].into_iter().chain(std::iter::once(0x1F)).collect()),
    ];

    let encoder = Ed2kEncoder::new();
    let decoder = Ed2kDecoder::new();

    for (part_count, bitfield) in test_cases {
        let mask = PartAvailabilityMask::from_bytes(bytes::Bytes::from(bitfield), part_count)
            .expect("valid mask");
        let status = Ed2kFileStatus {
            file_hash: [0x77; 16],
            part_count,
            availability_mask: mask,
        };

        let original_msg = Ed2kMessage::FileStatus(status);
        let encoded = encoder
            .encode_to_bytes(&original_msg)
            .expect("encoding failed");

        let decoded = decoder
            .decode_packet(&encoded)
            .expect("decoding failed");

        assert_eq!(
            decoded, original_msg,
            "roundtrip mismatch for file status with {part_count} parts"
        );
    }
}

#[test]
fn test_synthetic_roundtrip_file_status_req() {
    let req = Ed2kFileStatusReq {
        file_hash: [0xAA; 16],
    };
    let msg = Ed2kMessage::FileStatusReq(req);
    let encoder = Ed2kEncoder::new();
    let decoder = Ed2kDecoder::new();

    let bytes = encoder.encode_to_bytes(&msg).expect("encoding failed");
    let decoded = decoder.decode_packet(&bytes).expect("decoding failed");

    assert_eq!(decoded, msg);
}

// ---------------------------------------------------------------------------
// 3. Encoder Resource Boundary Rejections
// ---------------------------------------------------------------------------

#[test]
fn test_encode_rejects_too_many_tags() {
    let limits = Ed2kLimits {
        max_tags: 10,
        ..Default::default()
    };
    let encoder = Ed2kEncoder::with_limits(limits);

    let mut tags = Vec::new();
    for i in 0..11 {
        tags.push(Ed2kTag {
            name: TagName::Text(format!("tag_{i}")),
            value: TagValue::UInt32(i as u32),
        });
    }

    let hello = Ed2kHello {
        user_hash: [0; 16],
        client_id: 0,
        tcp_port: 4662,
        tags,
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };

    let result = encoder.encode_hello(&hello);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.opcode, 0x01);
    assert_eq!(err.field, "hello_tag_count");
    assert!(matches!(err.kind, Ed2kErrorKind::TooManyTags { count: 11, limit: 10 }));
}

#[test]
fn test_encode_rejects_oversized_string_name() {
    let limits = Ed2kLimits {
        max_string_bytes: 32,
        ..Default::default()
    };
    let encoder = Ed2kEncoder::with_limits(limits);

    let hello = Ed2kHello {
        user_hash: [0; 16],
        client_id: 0,
        tcp_port: 4662,
        tags: vec![Ed2kTag {
            name: TagName::Text("a".repeat(33)),
            value: TagValue::UInt32(1),
        }],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };

    let result = encoder.encode_hello(&hello);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "tag_name");
    assert!(matches!(err.kind, Ed2kErrorKind::OversizedString { size: 33, limit: 32 }));
}

#[test]
fn test_encode_rejects_oversized_string_value() {
    let limits = Ed2kLimits {
        max_string_bytes: 50,
        ..Default::default()
    };
    let encoder = Ed2kEncoder::with_limits(limits);

    let hello = Ed2kHello {
        user_hash: [0; 16],
        client_id: 0,
        tcp_port: 4662,
        tags: vec![Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_NAME),
            value: TagValue::String("x".repeat(51)),
        }],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };

    let result = encoder.encode_hello(&hello);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "tag_value_string");
    assert!(matches!(err.kind, Ed2kErrorKind::OversizedString { size: 51, limit: 50 }));
}

#[test]
fn test_encode_rejects_oversized_blob() {
    let limits = Ed2kLimits {
        max_blob_bytes: 64,
        ..Default::default()
    };
    let encoder = Ed2kEncoder::with_limits(limits);

    let hello = Ed2kHello {
        user_hash: [0; 16],
        client_id: 0,
        tcp_port: 4662,
        tags: vec![Ed2kTag {
            name: TagName::Text("blob".to_string()),
            value: TagValue::Blob(vec![0xAA; 65]),
        }],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };

    let result = encoder.encode_hello(&hello);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "tag_value_blob");
    assert!(matches!(err.kind, Ed2kErrorKind::FrameTooLarge { size: 65, limit: 64 }));
}

#[test]
fn test_encode_rejects_oversized_frame() {
    let limits = Ed2kLimits {
        max_hello_frame_bytes: 40,
        ..Default::default()
    };
    let encoder = Ed2kEncoder::with_limits(limits);

    // Minimal Hello base is 26 bytes + 1 opcode = 27 bytes frame
    // Adding 20 bytes tag exceeds 40 bytes limit
    let hello = Ed2kHello {
        user_hash: [0; 16],
        client_id: 0,
        tcp_port: 4662,
        tags: vec![Ed2kTag {
            name: TagName::Text("custom".to_string()),
            value: TagValue::String("hello_world_large".to_string()),
        }],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };

    let result = encoder.encode_hello(&hello);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "frame_len");
    assert!(matches!(err.kind, Ed2kErrorKind::FrameTooLarge { limit: 40, .. }));
}

#[test]
fn test_encode_rejects_invalid_file_status_bitfield_length() {
    let encoder = Ed2kEncoder::new();

    // 1. Mask constructor rejects mismatch
    let mask_err = PartAvailabilityMask::from_bytes(bytes::Bytes::from_static(&[0xFF]), 10);
    assert!(mask_err.is_err());
    assert!(matches!(mask_err.unwrap_err(), Ed2kErrorKind::MalformedField { .. }));

    // 2. Encoder rejects mismatched mask length if raw buffer was altered
    let raw_mask = PartAvailabilityMask::from_parts(&[0, 1], 2); // 1 byte
    let status = Ed2kFileStatus {
        file_hash: [0x88; 16],
        part_count: 10, // requires 2 bytes, but mask has 1 byte
        availability_mask: raw_mask,
    };

    let result = encoder.encode_file_status(&status);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "filestatus_bitfield");
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));
}

#[test]
fn test_synthetic_roundtrip_request_parts_multi_ranges() {
    let encoder = Ed2kEncoder::new();
    let decoder = Ed2kDecoder::new();

    // 1. Single range
    let req1 = Ed2kRequestParts::single([0x12; 16], 0, 184320);
    let msg1 = Ed2kMessage::RequestParts(req1.clone());
    let bytes1 = encoder.encode_to_bytes(&msg1).expect("encoding failed");
    let decoded1 = decoder.decode_packet(&bytes1).expect("decoding failed");
    assert_eq!(decoded1, msg1);

    // 2. Two ranges
    let req2 = Ed2kRequestParts::multi(
        [0x34; 16],
        vec![
            Ed2kBlockRange::new(0, 184320),
            Ed2kBlockRange::new(184320, 368640),
        ],
    );
    let msg2 = Ed2kMessage::RequestParts(req2.clone());
    let bytes2 = encoder.encode_to_bytes(&msg2).expect("encoding failed");
    let decoded2 = decoder.decode_packet(&bytes2).expect("decoding failed");
    assert_eq!(decoded2, msg2);

    // 3. Three ranges
    let req3 = Ed2kRequestParts::multi(
        [0x56; 16],
        vec![
            Ed2kBlockRange::new(0, 184320),
            Ed2kBlockRange::new(184320, 368640),
            Ed2kBlockRange::new(368640, 552960),
        ],
    );
    let msg3 = Ed2kMessage::RequestParts(req3.clone());
    let bytes3 = encoder.encode_to_bytes(&msg3).expect("encoding failed");
    let decoded3 = decoder.decode_packet(&bytes3).expect("decoding failed");
    assert_eq!(decoded3, msg3);
}

#[test]
fn test_synthetic_roundtrip_sending_part_various_sizes() {
    let encoder = Ed2kEncoder::new();
    let decoder = Ed2kDecoder::new();

    for size in [1, 128, 1024, 184320] {
        let data = bytes::Bytes::from(vec![0x7A; size]);
        let part = Ed2kSendingPart::new([0x99; 16], 1000, 1000 + size as u32, data);
        let msg = Ed2kMessage::SendingPart(part.clone());
        let bytes = encoder.encode_to_bytes(&msg).expect("encoding failed");
        let decoded = decoder.decode_packet(&bytes).expect("decoding failed");
        assert_eq!(decoded, msg);
    }
}

#[test]
fn test_encode_rejects_sending_part_start_ge_end() {
    let encoder = Ed2kEncoder::new();
    let part = Ed2kSendingPart::new(
        [0x11; 16],
        5000,
        5000, // start == end
        bytes::Bytes::new(),
    );
    let result = encoder.encode_sending_part(&part);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "sending_part_offsets");
    assert!(matches!(err.kind, Ed2kErrorKind::InvalidBlockOffsets { start: 5000, end: 5000 }));
}

#[test]
fn test_encode_rejects_sending_part_payload_mismatch() {
    let encoder = Ed2kEncoder::new();
    let part = Ed2kSendingPart::new(
        [0x11; 16],
        0,
        1000, // expected 1000 bytes
        bytes::Bytes::from_static(&[0x42; 500]), // only 500 bytes
    );
    let result = encoder.encode_sending_part(&part);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "sending_part_payload");
    assert!(matches!(err.kind, Ed2kErrorKind::BlockPayloadMismatch { expected: 1000, actual: 500 }));
}

#[test]
fn test_encode_rejects_request_parts_start_ge_end() {
    let encoder = Ed2kEncoder::new();
    let req = Ed2kRequestParts::single([0x22; 16], 2000, 1000); // start > end
    let result = encoder.encode_request_parts(&req);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "request_parts_range");
    assert!(matches!(err.kind, Ed2kErrorKind::InvalidBlockOffsets { start: 2000, end: 1000 }));
}

#[test]
fn test_encode_rejects_request_parts_more_than_3_ranges() {
    let encoder = Ed2kEncoder::new();
    let req = Ed2kRequestParts::multi(
        [0x33; 16],
        vec![
            Ed2kBlockRange::new(0, 100),
            Ed2kBlockRange::new(100, 200),
            Ed2kBlockRange::new(200, 300),
            Ed2kBlockRange::new(300, 400), // 4th range exceeds limit
        ],
    );
    let result = encoder.encode_request_parts(&req);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "request_parts_ranges");
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));
}

#[test]
fn test_block_range_wire_boundary_conversions() {
    // 1. Canonical eMule exclusive wire bounds: (0, 184320) direct mapping
    let canonical = Ed2kBlockRange::from_wire_bounds(0, 184320).expect("valid canonical wire bounds");
    assert_eq!(canonical.start, 0);
    assert_eq!(canonical.end, 184320);
    assert_eq!(canonical.len(), 184320);
    assert_eq!(canonical.to_wire_bounds(), (0, 184320));

    // Single-byte residual block canonical: (9728000, 9728001)
    let residual_canonical = Ed2kBlockRange::from_wire_bounds(9_728_000, 9_728_001)
        .expect("valid canonical single-byte range");
    assert_eq!(residual_canonical.len(), 1);
    assert_eq!(residual_canonical.to_wire_bounds(), (9_728_000, 9_728_001));

    // Canonical invalid (start >= end)
    assert!(Ed2kBlockRange::from_wire_bounds(100, 100).is_err());
    assert!(Ed2kBlockRange::from_wire_bounds(200, 100).is_err());

    // 2. Legacy inclusive conversion: [0, 184319] inclusive -> [0, 184320) half-open
    let legacy_range = Ed2kBlockRange::from_legacy_inclusive_wire_bounds(0, 184319)
        .expect("valid inclusive range");
    assert_eq!(legacy_range.start, 0);
    assert_eq!(legacy_range.end, 184320);
    assert_eq!(legacy_range.len(), 184320);

    // Roundtrip back to legacy inclusive
    let (start_inc, end_inc) = legacy_range.to_legacy_inclusive_wire_bounds().expect("valid conversion");
    assert_eq!(start_inc, 0);
    assert_eq!(end_inc, 184319);

    // 3. Overflow protection on legacy u32::MAX
    let overflow_res = Ed2kBlockRange::from_legacy_inclusive_wire_bounds(100, u32::MAX);
    assert!(overflow_res.is_err());
    assert!(matches!(overflow_res.unwrap_err(), Ed2kErrorKind::MalformedField { .. }));

    // 4. Inverted offsets rejected
    let inverted_res = Ed2kBlockRange::from_legacy_inclusive_wire_bounds(500, 400);
    assert!(inverted_res.is_err());
    assert!(matches!(inverted_res.unwrap_err(), Ed2kErrorKind::InvalidBlockOffsets { .. }));
}

#[test]
fn test_decode_request_parts_sentinels_zero_and_u32_max() {
    let decoder = Ed2kDecoder::new();

    // 40-byte payload: 16 bytes hash, 3 start offsets, 3 end offsets
    // Slot 0: [0, 184320]
    // Slot 1: [0, 0] sentinel
    // Slot 2: [u32::MAX, u32::MAX] sentinel
    let mut payload = vec![0xAB; 16];
    // Start offsets
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes()); // slot 1 sentinel
    payload.extend_from_slice(&u32::MAX.to_le_bytes()); // slot 2 sentinel
    // End offsets
    payload.extend_from_slice(&184320u32.to_le_bytes());
    payload.extend_from_slice(&0u32.to_le_bytes()); // slot 1 sentinel
    payload.extend_from_slice(&u32::MAX.to_le_bytes()); // slot 2 sentinel

    let req = decoder
        .decode_request_parts(0xE3, 0x47, &payload, 0)
        .expect("decodes valid request with mixed sentinels");
    assert_eq!(req.file_hash, [0xAB; 16]);
    assert_eq!(req.ranges.len(), 1);
    assert_eq!(req.ranges[0], Ed2kBlockRange::new(0, 184320));
}

#[test]
fn test_canonical_emule_opcodes_roundtrip() {
    let encoder = Ed2kEncoder::new();
    let decoder = Ed2kDecoder::new();

    // 1. Canonical eMule OP_REQUESTPARTS (0x47)
    let req = Ed2kRequestParts::single([0x33; 16], 0, 184320);
    let raw_pkt = encoder
        .encode_request_parts_with_opcode(&req, mule_protocol::ed2k::OP_REQUESTPARTS_EMULE)
        .expect("encodes 0x47");
    assert_eq!(raw_pkt.opcode, 0x47);
    let decoded_msg = decoder.decode_raw_packet(&raw_pkt).expect("decodes 0x47");
    assert_eq!(decoded_msg, Ed2kMessage::RequestParts(req));

    // 2. Canonical eMule OP_SETREQFILEID (0x4F)
    let status_req = Ed2kFileStatusReq {
        file_hash: [0x55; 16],
    };
    let raw_status_pkt = encoder
        .encode_file_status_req_with_opcode(&status_req, mule_protocol::ed2k::OP_SETREQFILEID_EMULE)
        .expect("encodes 0x4F");
    assert_eq!(raw_status_pkt.opcode, 0x4F);
    let decoded_status_msg = decoder.decode_raw_packet(&raw_status_pkt).expect("decodes 0x4F");
    assert_eq!(decoded_status_msg, Ed2kMessage::FileStatusReq(status_req));
}

#[test]
fn test_profile_emule050a_accepts_0x47_and_rejects_0x4b() {
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::Emule050a);
    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::Emule050a);

    assert_eq!(decoder.profile.request_parts_opcode(), Some(0x47));
    assert_eq!(encoder.profile.request_parts_opcode(), Some(0x47));

    let req = Ed2kRequestParts::single([0x42; 16], 0, 184320);
    let pkt_47 = encoder.encode_request_parts(&req).expect("encodes 0x47");
    assert_eq!(pkt_47.opcode, 0x47);

    let decoded = decoder.decode_raw_packet(&pkt_47).expect("decodes 0x47");
    assert_eq!(decoded, Ed2kMessage::RequestParts(req.clone()));

    // 0x4B payload with valid 3-slot layout
    let pkt_4b = encoder
        .encode_request_parts_with_opcode(&req, 0x4B)
        .expect("encodes 0x4B");
    let err = decoder
        .decode_request_parts(0xE3, 0x4B, &pkt_4b.payload, 0)
        .unwrap_err();
    assert!(matches!(
        err.kind,
        Ed2kErrorKind::AmbiguousOpcodeForProfile {
            opcode: 0x4B,
            profile: Ed2kCompatibilityProfile::Emule050a,
            ..
        }
    ));

    // Under decode_raw_packet, 0x4B is kept as unparsed raw packet
    let raw_msg = decoder.decode_raw_packet(&pkt_4b).expect("fallback to raw");
    assert!(matches!(raw_msg, Ed2kMessage::Raw(_)));
}

#[test]
fn test_profile_legacy_edonkey_accepts_0x4b_and_rejects_0x47() {
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);

    assert_eq!(decoder.profile.request_parts_opcode(), Some(0x4B));
    assert_eq!(encoder.profile.request_parts_opcode(), Some(0x4B));

    let req = Ed2kRequestParts::single([0x42; 16], 0, 184320);
    let pkt_4b = encoder.encode_request_parts(&req).expect("encodes 0x4B");
    assert_eq!(pkt_4b.opcode, 0x4B);

    let decoded = decoder.decode_raw_packet(&pkt_4b).expect("decodes 0x4B");
    assert_eq!(decoded, Ed2kMessage::RequestParts(req.clone()));

    // 0x47 is rejected under LegacyEdonkey decode_request_parts
    let pkt_47 = encoder
        .encode_request_parts_with_opcode(&req, 0x47)
        .expect("encodes 0x47");
    let err = decoder
        .decode_request_parts(0xE3, 0x47, &pkt_47.payload, 0)
        .unwrap_err();
    assert!(matches!(
        err.kind,
        Ed2kErrorKind::AmbiguousOpcodeForProfile {
            opcode: 0x47,
            profile: Ed2kCompatibilityProfile::LegacyEdonkey,
            ..
        }
    ));
}

#[test]
fn test_profile_unknown_strict_rejects_ambiguous_opcodes() {
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::UnknownStrict);
    let encoder = Ed2kEncoder::new();

    let req = Ed2kRequestParts::single([0x42; 16], 0, 184320);
    let pkt_47 = encoder
        .encode_request_parts_with_opcode(&req, 0x47)
        .expect("encodes");
    let pkt_4b = encoder
        .encode_request_parts_with_opcode(&req, 0x4B)
        .expect("encodes");

    let err_47 = decoder.decode_raw_packet(&pkt_47).unwrap_err();
    assert!(matches!(
        err_47.kind,
        Ed2kErrorKind::AmbiguousOpcodeForProfile {
            opcode: 0x47,
            profile: Ed2kCompatibilityProfile::UnknownStrict,
            ..
        }
    ));

    let err_4b = decoder.decode_raw_packet(&pkt_4b).unwrap_err();
    assert!(matches!(
        err_4b.kind,
        Ed2kErrorKind::AmbiguousOpcodeForProfile {
            opcode: 0x4B,
            profile: Ed2kCompatibilityProfile::UnknownStrict,
            ..
        }
    ));

    let status_req = Ed2kFileStatusReq {
        file_hash: [0x55; 16],
    };
    let pkt_4f = encoder
        .encode_file_status_req_with_opcode(&status_req, 0x4F)
        .expect("encodes");
    let pkt_49 = encoder
        .encode_file_status_req_with_opcode(&status_req, 0x49)
        .expect("encodes");

    let err_4f = decoder.decode_raw_packet(&pkt_4f).unwrap_err();
    assert!(matches!(
        err_4f.kind,
        Ed2kErrorKind::AmbiguousOpcodeForProfile {
            opcode: 0x4F,
            profile: Ed2kCompatibilityProfile::UnknownStrict,
            ..
        }
    ));

    let err_49 = decoder.decode_raw_packet(&pkt_49).unwrap_err();
    assert!(matches!(
        err_49.kind,
        Ed2kErrorKind::AmbiguousOpcodeForProfile {
            opcode: 0x49,
            profile: Ed2kCompatibilityProfile::UnknownStrict,
            ..
        }
    ));
}

#[test]
fn test_profile_filestatus_disambiguation_0x4f_vs_0x49() {
    let emule_dec = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::Emule050a);
    let legacy_dec = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);

    let status_req = Ed2kFileStatusReq {
        file_hash: [0x55; 16],
    };

    // 0x4F (OP_SETREQFILEID) is accepted by Emule050a, treated as Raw by LegacyEdonkey
    let emule_encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::Emule050a);
    let pkt_4f = emule_encoder
        .encode_file_status_req(&status_req)
        .expect("encodes 0x4F");
    assert_eq!(pkt_4f.opcode, 0x4F);

    let emule_decoded = emule_dec.decode_raw_packet(&pkt_4f).expect("emule decodes 0x4F");
    assert_eq!(emule_decoded, Ed2kMessage::FileStatusReq(status_req.clone()));

    let legacy_on_4f = legacy_dec
        .decode_raw_packet(&pkt_4f)
        .expect("legacy treats 0x4F as raw");
    assert!(matches!(legacy_on_4f, Ed2kMessage::Raw(_)));

    // 0x49 (OP_FILESTATUSREQ) is accepted by LegacyEdonkey, treated as Raw by Emule050a
    let legacy_encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let pkt_49 = legacy_encoder
        .encode_file_status_req(&status_req)
        .expect("encodes 0x49");
    assert_eq!(pkt_49.opcode, 0x49);

    let legacy_decoded = legacy_dec.decode_raw_packet(&pkt_49).expect("legacy decodes 0x49");
    assert_eq!(legacy_decoded, Ed2kMessage::FileStatusReq(status_req.clone()));

    let emule_on_49 = emule_dec
        .decode_raw_packet(&pkt_49)
        .expect("emule treats 0x49 as raw");
    assert!(matches!(emule_on_49, Ed2kMessage::Raw(_)));
}

#[test]
fn test_multi_profile_invariance_matrix() {
    let hash = [0x42; 16];
    let req_parts = Ed2kRequestParts::single(hash, 0, 184_320);
    let status_req = Ed2kFileStatusReq { file_hash: hash };

    let emule_enc = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::Emule050a);
    let legacy_enc = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);

    let pkt_47 = emule_enc
        .encode_request_parts(&req_parts)
        .expect("encodes 0x47");
    assert_eq!(pkt_47.opcode, 0x47);

    let pkt_4b = legacy_enc
        .encode_request_parts(&req_parts)
        .expect("encodes 0x4B");
    assert_eq!(pkt_4b.opcode, 0x4B);

    let pkt_4f = emule_enc
        .encode_file_status_req(&status_req)
        .expect("encodes 0x4F");
    assert_eq!(pkt_4f.opcode, 0x4F);

    let pkt_49 = legacy_enc
        .encode_file_status_req(&status_req)
        .expect("encodes 0x49");
    assert_eq!(pkt_49.opcode, 0x49);

    let profiles = [
        Ed2kCompatibilityProfile::Emule050a,
        Ed2kCompatibilityProfile::Amule233,
        Ed2kCompatibilityProfile::LegacyEdonkey,
        Ed2kCompatibilityProfile::UnknownStrict,
    ];

    for profile in profiles {
        let dec = Ed2kDecoder::with_profile(profile);

        match profile {
            Ed2kCompatibilityProfile::Emule050a | Ed2kCompatibilityProfile::Amule233 => {
                // 0x47 -> RequestParts
                let res_47 = dec.decode_raw_packet(&pkt_47).expect("decodes 0x47");
                assert_eq!(res_47, Ed2kMessage::RequestParts(req_parts.clone()));

                // 0x4B -> Raw
                let res_4b = dec.decode_raw_packet(&pkt_4b).expect("0x4B as Raw");
                assert!(matches!(res_4b, Ed2kMessage::Raw(_)));

                // 0x4F -> FileStatusReq
                let res_4f = dec.decode_raw_packet(&pkt_4f).expect("decodes 0x4F");
                assert_eq!(res_4f, Ed2kMessage::FileStatusReq(status_req.clone()));

                // 0x49 -> Raw
                let res_49 = dec.decode_raw_packet(&pkt_49).expect("0x49 as Raw");
                assert!(matches!(res_49, Ed2kMessage::Raw(_)));
            }
            Ed2kCompatibilityProfile::LegacyEdonkey => {
                // 0x47 -> Raw
                let res_47 = dec.decode_raw_packet(&pkt_47).expect("0x47 as Raw");
                assert!(matches!(res_47, Ed2kMessage::Raw(_)));

                // 0x4B -> RequestParts
                let res_4b = dec.decode_raw_packet(&pkt_4b).expect("decodes 0x4B");
                assert_eq!(res_4b, Ed2kMessage::RequestParts(req_parts.clone()));

                // 0x4F -> Raw
                let res_4f = dec.decode_raw_packet(&pkt_4f).expect("0x4F as Raw");
                assert!(matches!(res_4f, Ed2kMessage::Raw(_)));

                // 0x49 -> FileStatusReq
                let res_49 = dec.decode_raw_packet(&pkt_49).expect("decodes 0x49");
                assert_eq!(res_49, Ed2kMessage::FileStatusReq(status_req.clone()));
            }
            Ed2kCompatibilityProfile::UnknownStrict => {
                // All 4 ambiguous opcodes rejected with AmbiguousOpcodeForProfile
                for (pkt, expected_op) in [(&pkt_47, 0x47), (&pkt_4b, 0x4B), (&pkt_4f, 0x4F), (&pkt_49, 0x49)] {
                    let err = dec.decode_raw_packet(pkt).unwrap_err();
                    assert!(matches!(
                        err.kind,
                        Ed2kErrorKind::AmbiguousOpcodeForProfile {
                            opcode,
                            profile: Ed2kCompatibilityProfile::UnknownStrict,
                            ..
                        } if opcode == expected_op
                    ));
                }
            }
        }

        // INVARIANCE CHECK: Canonical undisputed messages decode identically across ALL 4 profiles, including UnknownStrict!
        let hello = Ed2kHello {
            user_hash: [0x11; 16],
            client_id: 0,
            tcp_port: 4662,
            tags: vec![],
            unknown_tags: vec![],
            server_ip: 0,
            server_port: 0,
        };
        let hello_pkt = emule_enc.encode_hello(&hello).expect("encodes hello");
        let decoded_hello = dec.decode_raw_packet(&hello_pkt).expect("decodes hello under all profiles");
        assert_eq!(decoded_hello, Ed2kMessage::Hello(hello));

        let file_status = Ed2kFileStatus {
            file_hash: hash,
            part_count: 2,
            availability_mask: PartAvailabilityMask::from_parts(&[0], 2),
        };
        let status_pkt = emule_enc.encode_file_status(&file_status).expect("encodes status");
        let decoded_status = dec.decode_raw_packet(&status_pkt).expect("decodes file status under all profiles");
        assert_eq!(decoded_status, Ed2kMessage::FileStatus(file_status));

        let sending_part = Ed2kSendingPart::new(hash, 0, 10, bytes::Bytes::from_static(b"0123456789"));
        let sending_pkt = emule_enc.encode_sending_part(&sending_part).expect("encodes sending part");
        let decoded_sending = dec.decode_raw_packet(&sending_pkt).expect("decodes sending part under all profiles");
        assert_eq!(decoded_sending, Ed2kMessage::SendingPart(sending_part));
    }
}

#[test]
fn test_boundary_cases_1_byte_residual_and_u32_sentinels() {
    let file_size: u64 = 9_728_001;
    let residual_start: u32 = 9_728_000;
    let residual_end: u32 = 9_728_001;
    let hash = [0x77; 16];

    // 1. Block range creation and bounds verification
    let residual_range = Ed2kBlockRange::from_wire_bounds(residual_start, residual_end)
        .expect("valid single-byte residual range");
    assert_eq!(residual_range.len(), 1);
    assert_eq!(residual_range.start, residual_start);
    assert_eq!(residual_range.end, residual_end);
    assert_eq!(residual_range.to_wire_bounds(), (residual_start, residual_end));

    // Verify it fits precisely at file boundary
    assert_eq!(residual_range.end as u64, file_size);

    // 2. Single-range RequestParts with 1-byte residual
    let req_single = Ed2kRequestParts::single(hash, residual_start, residual_end);
    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::Emule050a);
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::Emule050a);

    let pkt_single = encoder.encode_request_parts(&req_single).expect("encodes 1-byte request");
    assert_eq!(pkt_single.opcode, 0x47);
    assert_eq!(pkt_single.payload.len(), 40);

    // Verify 24-byte single-slot layout:
    // - Strictly rejected under Emule050a/Amule233 (which mandate 40-byte 3-slot wire layout)
    // - Accepted under LegacyEdonkey (OP_REQUESTPARTS 0x4B)
    let mut raw_24 = Vec::with_capacity(24);
    raw_24.extend_from_slice(&hash);
    raw_24.extend_from_slice(&residual_start.to_le_bytes());
    raw_24.extend_from_slice(&residual_end.to_le_bytes());

    let pkt_24_emule = mule_protocol::RawPacket::with_vec(0xE3, 0x47, raw_24.clone());
    let err_24 = decoder.decode_raw_packet(&pkt_24_emule).unwrap_err();
    assert!(matches!(err_24.kind, Ed2kErrorKind::MalformedField { .. }));

    let legacy_dec = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let pkt_24_legacy = mule_protocol::RawPacket::with_vec(0xE3, 0x4B, raw_24);
    let decoded_24 = legacy_dec.decode_raw_packet(&pkt_24_legacy).expect("decodes 24-byte single range under legacy");
    assert_eq!(decoded_24, Ed2kMessage::RequestParts(req_single));

    // 3. Multi-range RequestParts with slot sentinels: [residual, (0, 0), (u32::MAX, u32::MAX)]
    let mut raw_40 = Vec::with_capacity(40);
    raw_40.extend_from_slice(&hash);
    // 3 starts: [residual_start, 0, u32::MAX]
    raw_40.extend_from_slice(&residual_start.to_le_bytes());
    raw_40.extend_from_slice(&0u32.to_le_bytes());
    raw_40.extend_from_slice(&u32::MAX.to_le_bytes());
    // 3 ends: [residual_end, 0, u32::MAX]
    raw_40.extend_from_slice(&residual_end.to_le_bytes());
    raw_40.extend_from_slice(&0u32.to_le_bytes());
    raw_40.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(raw_40.len(), 40);

    let pkt_40 = mule_protocol::RawPacket::with_vec(0xE3, 0x47, raw_40);
    let decoded_40 = decoder.decode_raw_packet(&pkt_40).expect("decodes sentinels");
    if let Ed2kMessage::RequestParts(rp) = decoded_40 {
        assert_eq!(rp.ranges.len(), 1, "sentinel slots (0,0) and (MAX,MAX) must be ignored");
        assert_eq!(rp.ranges[0], residual_range);
    } else {
        panic!("expected RequestParts");
    }

    // 4. All slots sentinel must be rejected
    let mut all_sentinels_zero = Vec::with_capacity(40);
    all_sentinels_zero.extend_from_slice(&hash);
    all_sentinels_zero.extend_from_slice(&[0u8; 24]);
    let pkt_all_zero = mule_protocol::RawPacket::with_vec(0xE3, 0x47, all_sentinels_zero);
    let err_all_zero = decoder.decode_raw_packet(&pkt_all_zero).unwrap_err();
    assert!(matches!(err_all_zero.kind, Ed2kErrorKind::MalformedField { .. }));

    let mut all_sentinels_max = Vec::with_capacity(40);
    all_sentinels_max.extend_from_slice(&hash);
    all_sentinels_max.extend_from_slice(&[0xFF; 24]);
    let pkt_all_max = mule_protocol::RawPacket::with_vec(0xE3, 0x47, all_sentinels_max);
    let err_all_max = decoder.decode_raw_packet(&pkt_all_max).unwrap_err();
    assert!(matches!(err_all_max.kind, Ed2kErrorKind::MalformedField { .. }));

    // 5. SendingPart 1-byte residual delivery
    let sending_part = Ed2kSendingPart::new(
        hash,
        residual_start,
        residual_end,
        bytes::Bytes::from_static(b"Z"),
    );
    let pkt_sending = encoder.encode_sending_part(&sending_part).expect("encodes sending part");
    assert_eq!(pkt_sending.payload.len(), 25); // 16 hash + 4 start + 4 end + 1 byte data
    let decoded_sending = decoder.decode_raw_packet(&pkt_sending).expect("decodes 1-byte sending part");
    assert_eq!(decoded_sending, Ed2kMessage::SendingPart(sending_part));

    // 6. SendingPart payload mismatch checks (expected 1 byte, got 0)
    let sending_empty = mule_protocol::RawPacket::with_vec(
        0xE3,
        0x46,
        pkt_sending.payload[..24].to_vec(),
    );
    let err_mismatch = decoder.decode_raw_packet(&sending_empty).unwrap_err();
    assert!(matches!(
        err_mismatch.kind,
        Ed2kErrorKind::BlockPayloadMismatch {
            expected: 1,
            actual: 0
        }
    ));

    // 7. Offset invalidity: start >= end
    let err_equal = Ed2kBlockRange::from_wire_bounds(500, 500).unwrap_err();
    assert!(matches!(err_equal, Ed2kErrorKind::InvalidBlockOffsets { start: 500, end: 500 }));

    let err_inverted = Ed2kBlockRange::from_wire_bounds(501, 500).unwrap_err();
    assert!(matches!(err_inverted, Ed2kErrorKind::InvalidBlockOffsets { start: 501, end: 500 }));

    // 8. Overflow in legacy inclusive bound (u32::MAX + 1 overflows)
    let err_overflow = Ed2kBlockRange::from_legacy_inclusive_wire_bounds(0, u32::MAX).unwrap_err();
    assert!(matches!(err_overflow, Ed2kErrorKind::MalformedField { .. }));
}

#[test]
fn test_illegal_protocol_opcode_combinations() {
    let decoder = Ed2kDecoder::new();

    // 1. 0xD4 (PR_EMULE_COMPRESSED) with various opcodes must strictly fail with CompressedProtocolUnsupported
    for opcode in [0x01, 0x46, 0x47, 0x4B, 0x4F, 0x50, 0x51] {
        let pkt = mule_protocol::RawPacket::with_vec(0xD4, opcode, vec![0u8; 30]);
        let err = decoder.decode_raw_packet(&pkt).unwrap_err();
        assert!(matches!(
            err.kind,
            Ed2kErrorKind::CompressedProtocolUnsupported
        ));
    }

    // 2. Unsupported protocol bytes (0x00, 0x42, 0xEE, 0xFF) must fail with UnsupportedProtocol
    for bad_proto in [0x00, 0x42, 0xEE, 0xFF] {
        let pkt = mule_protocol::RawPacket::with_vec(bad_proto, 0x01, vec![0u8; 20]);
        let err = decoder.decode_raw_packet(&pkt).unwrap_err();
        assert!(matches!(
            err.kind,
            Ed2kErrorKind::UnsupportedProtocol { byte } if byte == bad_proto
        ));
    }

    // 3. 0xC5 (PR_EMULE / Kad / extension) with standard eD2K opcodes
    // Must be preserved as Raw packet without parsing as standard session messages
    for opcode in [0x46, 0x47, 0x4F, 0x50] {
        let pkt = mule_protocol::RawPacket::with_vec(0xC5, opcode, vec![0xAA; 20]);
        let msg = decoder.decode_raw_packet(&pkt).expect("0xC5 preserved as Raw");
        assert_eq!(msg, Ed2kMessage::Raw(pkt));
    }
}

#[test]
fn test_metamorphic_tag_permutations_and_unknown_tags() {
    let encoder = Ed2kEncoder::new();
    let decoder = Ed2kDecoder::new();

    // Base tags: Name, Version, MiscOptions1
    let tag_name = Ed2kTag {
        name: TagName::Id(TAG_SPECIAL_NAME),
        value: TagValue::String("MetamorphicUser".to_string()),
    };
    let tag_ver = Ed2kTag {
        name: TagName::Id(TAG_SPECIAL_VERSION),
        value: TagValue::UInt32(60),
    };
    let tag_misc = Ed2kTag {
        name: TagName::Id(TAG_SPECIAL_MISCOPTIONS1),
        value: TagValue::UInt32(0x00000001),
    };
    let raw_tag_1 = RawTag {
        tag_type: 0x03, // UInt32
        name: TagName::Text("custom_tag_1".to_string()),
        raw_bytes: vec![0x10, 0x20, 0x30, 0x40],
    };
    let raw_tag_2 = RawTag {
        tag_type: 0x03, // UInt32
        name: TagName::Text("custom_tag_2".to_string()),
        raw_bytes: vec![0xDE, 0xAD, 0xBE, 0xEF],
    };

    // 1. Permutation invariance of known tags
    // Create two Hello messages with permuted known tags: [name, ver, misc] vs [misc, name, ver]
    let hello_order1 = Ed2kHello {
        user_hash: [0x12; 16],
        client_id: 12345,
        tcp_port: 4662,
        tags: vec![tag_name.clone(), tag_ver.clone(), tag_misc.clone()],
        unknown_tags: vec![raw_tag_1.clone(), raw_tag_2.clone()],
        server_ip: 0,
        server_port: 0,
    };
    let hello_order2 = Ed2kHello {
        user_hash: [0x12; 16],
        client_id: 12345,
        tcp_port: 4662,
        tags: vec![tag_misc.clone(), tag_name.clone(), tag_ver.clone()],
        unknown_tags: vec![raw_tag_2.clone(), raw_tag_1.clone()],
        server_ip: 0,
        server_port: 0,
    };

    // Both encode valid frames
    let pkt1 = encoder.encode_hello(&hello_order1).expect("encodes order1");
    let pkt2 = encoder.encode_hello(&hello_order2).expect("encodes order2");

    let dec1 = match decoder.decode_raw_packet(&pkt1).expect("decodes order1") {
        Ed2kMessage::Hello(h) => h,
        _ => panic!("expected Hello"),
    };
    let dec2 = match decoder.decode_raw_packet(&pkt2).expect("decodes order2") {
        Ed2kMessage::Hello(h) => h,
        _ => panic!("expected Hello"),
    };

    // Semantic query invariance: find_tag must return identical tags regardless of permutation
    assert_eq!(dec1.find_tag("Name"), dec2.find_tag("Name"));
    assert_eq!(dec1.find_tag("Version"), dec2.find_tag("Version"));
    assert_eq!(dec1.find_tag("MiscOptions1"), dec2.find_tag("MiscOptions1"));
    assert_eq!(dec1.find_tag("custom_tag_1"), dec2.find_tag("custom_tag_1"));
    assert_eq!(dec1.find_tag("custom_tag_2"), dec2.find_tag("custom_tag_2"));

    // 2. Modifying an unknown tag does not alter known tags
    let mut raw_tag_1_mutated = raw_tag_1.clone();
    raw_tag_1_mutated.raw_bytes = vec![0xFF, 0xEE, 0xDD, 0xCC];

    let hello_mutated = Ed2kHello {
        user_hash: [0x12; 16],
        client_id: 12345,
        tcp_port: 4662,
        tags: vec![tag_name.clone(), tag_ver.clone(), tag_misc.clone()],
        unknown_tags: vec![raw_tag_1_mutated, raw_tag_2.clone()],
        server_ip: 0,
        server_port: 0,
    };
    let pkt_mutated = encoder.encode_hello(&hello_mutated).expect("encodes mutated");
    let dec_mutated = match decoder.decode_raw_packet(&pkt_mutated).expect("decodes mutated") {
        Ed2kMessage::Hello(h) => h,
        _ => panic!("expected Hello"),
    };
    assert_eq!(dec_mutated.find_tag("Name"), dec1.find_tag("Name"), "known tag Name must be completely unaffected");
    assert_eq!(dec_mutated.find_tag("Version"), dec1.find_tag("Version"), "known tag Version must be completely unaffected");
    assert_eq!(dec_mutated.find_tag("MiscOptions1"), dec1.find_tag("MiscOptions1"), "known tag MiscOptions1 must be completely unaffected");
    assert_ne!(dec_mutated.find_tag("custom_tag_1"), dec1.find_tag("custom_tag_1"), "custom_tag_1 must reflect mutation");

    // 3. Inserting unknown tags under limits: declared length and physical buffer remain strictly synchronized
    let mut large_unknown_tags = Vec::new();
    for i in 0..50 {
        large_unknown_tags.push(RawTag {
            tag_type: 0x03,
            name: TagName::Text(format!("meta_tag_{i}")),
            raw_bytes: (i as u32).to_le_bytes().to_vec(),
        });
    }
    let hello_many = Ed2kHello {
        user_hash: [0x34; 16],
        client_id: 0,
        tcp_port: 4662,
        tags: vec![tag_name.clone()],
        unknown_tags: large_unknown_tags,
        server_ip: 0,
        server_port: 0,
    };
    let pkt_many = encoder.encode_hello(&hello_many).expect("encodes 51 tags under limit");
    // Verify physical payload length matches total encoded frame
    assert_eq!(pkt_many.frame_len(), 1 + pkt_many.payload.len());
    let dec_many = match decoder.decode_raw_packet(&pkt_many).expect("decodes 51 tags") {
        Ed2kMessage::Hello(h) => h,
        _ => panic!("expected Hello"),
    };
    assert_eq!(dec_many.tags.len(), 51);
}

#[test]
fn test_tier3_op_hello_wire_framing_emule050a_compliance() {
    let encoder = Ed2kEncoder::with_profile(mule_protocol::ed2k::Ed2kCompatibilityProfile::Emule050a);
    let decoder = Ed2kDecoder::with_profile(mule_protocol::ed2k::Ed2kCompatibilityProfile::Emule050a);

    let user_hash = [
        0x31, 0x41, 0x59, 0x26, 0x53, 0x0E, 0x58, 0x97,
        0x93, 0x23, 0x84, 0x62, 0x64, 0x33, 0x6F, 0x83,
    ];
    let hello = Ed2kHello {
        user_hash,
        client_id: 0,
        tcp_port: 54321,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };

    let raw = encoder.encode_hello(&hello).expect("encode minimal hello");
    let wire = encoder
        .encode_to_bytes(&Ed2kMessage::Hello(hello.clone()))
        .expect("encode to wire bytes");

    // 1. Total wire frame length must be exactly 39 bytes:
    // 1 (protocol) + 4 (payload length) + 34 (payload) = 39
    assert_eq!(wire.len(), 39, "wire frame must be exactly 39 bytes");

    // 2. Exact wire byte assertions:
    assert_eq!(wire[0], 0xE3, "byte 0: PR_ED2K");
    assert_eq!(&wire[1..5], &34u32.to_le_bytes(), "bytes 1..5: payload length 34 LE (0x22, 0x00, 0x00, 0x00)");
    assert_eq!(wire[5], 0x01, "byte 5: OP_HELLO");
    assert_eq!(wire[6], 0x10, "byte 6: mandatory hash length prefix 0x10 for eMule/eDonkey");
    assert_eq!(&wire[7..23], &user_hash, "bytes 7..23: 16-byte user hash");
    assert_eq!(&wire[23..27], &0u32.to_le_bytes(), "bytes 23..27: client_id = 0");
    assert_eq!(&wire[27..29], &54321u16.to_le_bytes(), "bytes 27..29: tcp_port LE");
    assert_eq!(&wire[29..33], &0u32.to_le_bytes(), "bytes 29..33: tag_count = 0");
    assert_eq!(&wire[33..37], &0u32.to_le_bytes(), "bytes 33..37: server_ip = 0");
    assert_eq!(&wire[37..39], &0u16.to_le_bytes(), "bytes 37..39: server_port = 0");

    // 3. Decoding must roundtrip cleanly
    let decoded = decoder.decode_raw_packet(&raw).expect("decode wire hello");
    match decoded {
        Ed2kMessage::Hello(h) => {
            assert_eq!(h.user_hash, user_hash);
            assert_eq!(h.client_id, 0);
            assert_eq!(h.tcp_port, 54321);
            assert_eq!(h.tags.len(), 0);
        }
        _ => panic!("expected Hello message"),
    }
}

#[test]
fn test_tier3_op_hello_answer_wire_framing_emule050a_compliance() {
    let decoder = Ed2kDecoder::with_profile(mule_protocol::ed2k::Ed2kCompatibilityProfile::Emule050a);

    // Build raw OP_HELLO_ANSWER payload mimicking official eMule 0.50a response:
    // No 0x10 prefix! Starts directly with 16-byte hash.
    let peer_hash = [
        0xAA, 0xBB, 0xCC, 0xDD, 0x11, 0x0E, 0x22, 0x33,
        0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0x6F, 0xEE,
    ];
    let client_id: u32 = 0;
    let tcp_port: u16 = 50000;
    let tag_count: u32 = 4;

    let mut payload = Vec::new();
    payload.extend_from_slice(&peer_hash);
    payload.extend_from_slice(&client_id.to_le_bytes());
    payload.extend_from_slice(&tcp_port.to_le_bytes());
    payload.extend_from_slice(&tag_count.to_le_bytes());

    // Tag 1: Name "RefPeer050a" (Type String = 0x02, ID = 0x01)
    payload.push(0x02); // string
    payload.extend_from_slice(&1u16.to_le_bytes()); // name length
    payload.push(0x01); // TAG_SPECIAL_NAME
    let name_bytes = b"RefPeer050a";
    payload.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    payload.extend_from_slice(name_bytes);

    // Tag 2: Version 60 (Type UInt32 = 0x03, ID = 0x11)
    payload.push(0x03);
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.push(0x11); // TAG_SPECIAL_VERSION
    payload.extend_from_slice(&60u32.to_le_bytes());

    // Tag 3: Canonical eMule UDP port (Type UInt32 = 0x03, ID = 0xF9)
    payload.push(0x03);
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.push(0xF9); // TAG_SPECIAL_EMULE_UDPPORTS
    payload.extend_from_slice(&50001u32.to_le_bytes());

    // Tag 4: Canonical eMule misc options 1 (Type UInt32 = 0x03, ID = 0xFA)
    payload.push(0x03);
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.push(0xFA); // TAG_SPECIAL_EMULE_MISCOPTIONS1
    payload.extend_from_slice(&873677342u32.to_le_bytes());

    let raw = mule_protocol::RawPacket::with_vec(0xE3, 0x4C, payload);
    let msg = decoder.decode_raw_packet(&raw).expect("decode hello answer");

    match msg {
        Ed2kMessage::HelloAnswer(answer) => {
            let peer = mule_protocol::ed2k::Ed2kPeerInfo::from_hello_answer(&answer);
            assert_eq!(peer.name.as_deref(), Some("RefPeer050a"));
            assert_eq!(peer.version, Some(60));
            assert_eq!(peer.emule_udp_port, Some(50001));
            assert_eq!(peer.misc_options1, Some(873677342));
        }
        _ => panic!("expected HelloAnswer message"),
    }
}

#[test]
fn test_tier3_op_emule_info_roundtrip_nominal() {
    let encoder = Ed2kEncoder::new();
    let decoder = Ed2kDecoder::new();

    let tag_compression = Ed2kTag {
        name: TagName::Id(0x20),
        value: TagValue::UInt32(1),
    };
    let tag_source_exchange = Ed2kTag {
        name: TagName::Id(0x04),
        value: TagValue::UInt32(1),
    };
    let raw_tag = RawTag {
        tag_type: 0x03,
        name: TagName::Text("ExtCustom".to_string()),
        raw_bytes: vec![1, 2, 3, 4],
    };

    let original = Ed2kEmuleInfo {
        client_version: 60,
        protocol_version: 1,
        tags: vec![tag_compression, tag_source_exchange],
        unknown_tags: vec![raw_tag],
    };

    let encoded_pkt = encoder.encode_emule_info(&original).expect("encode emule info");
    assert_eq!(encoded_pkt.protocol, mule_protocol::types::PR_EMULE);
    assert_eq!(encoded_pkt.opcode, mule_protocol::ed2k::OP_EMULE_INFO);

    let decoded_msg = decoder.decode_raw_packet(&encoded_pkt).expect("decode emule info");
    match decoded_msg {
        Ed2kMessage::EmuleInfo(info) => {
            assert_eq!(info.client_version, 60);
            assert_eq!(info.protocol_version, 1);
            assert_eq!(info.tags.len(), 3);
            assert!(info.has_compression_capability());
            assert!(info.has_source_exchange_capability());
            assert_eq!(
                info.find_tag("ExtCustom"),
                Some(&Ed2kTag {
                    name: TagName::Text("ExtCustom".to_string()),
                    value: TagValue::UInt32(u32::from_le_bytes([1, 2, 3, 4])),
                })
            );
        }
        _ => panic!("expected Ed2kMessage::EmuleInfo"),
    }
}

#[test]
fn test_tier3_op_emule_info_truncated_payload_fails() {
    let decoder = Ed2kDecoder::new();

    // Truncated payload: only 1 byte instead of at least 6 bytes (client_ver + proto_ver + tag_count)
    let raw = mule_protocol::RawPacket::with_vec(mule_protocol::types::PR_EMULE, 0x01, vec![0x3C]);
    let err = decoder.decode_raw_packet(&raw).unwrap_err();
    assert!(matches!(
        err.kind,
        mule_protocol::ed2k::Ed2kErrorKind::UnexpectedEof { .. }
    ));
}

#[test]
fn test_tier3_op_emule_info_oversized_tag_count_fails() {
    let decoder = Ed2kDecoder::new();

    let mut payload = Vec::new();
    payload.push(60); // client_version
    payload.push(1); // protocol_version
    payload.extend_from_slice(&10_000u32.to_le_bytes()); // tag count exceeds limit (default 1024)

    let raw = mule_protocol::RawPacket::with_vec(mule_protocol::types::PR_EMULE, 0x01, payload);
    let err = decoder.decode_raw_packet(&raw).unwrap_err();
    assert!(matches!(
        err.kind,
        mule_protocol::ed2k::Ed2kErrorKind::TooManyTags { .. }
    ));
}

#[test]
fn test_tier3_op_file_status_req_wire_framing_emule050a() {
    let hash = [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81,
        0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82, 0x3c,
    ];
    let req = Ed2kFileStatusReq { file_hash: hash };
    let encoder = Ed2kEncoder::with_profile(Ed2kCompatibilityProfile::Emule050a);
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::Emule050a);

    let pkt = encoder.encode_file_status_req(&req).expect("encode 0x4F");
    assert_eq!(pkt.protocol, mule_protocol::types::PR_ED2K);
    assert_eq!(pkt.opcode, mule_protocol::ed2k::OP_SETREQFILEID_EMULE); // 0x4F
    assert_eq!(pkt.payload.len(), 16);
    assert_eq!(&pkt.payload[..], &hash);

    // Frame size: 1 byte opcode + 16 bytes payload = 17 bytes (total wire frame: 5 header + 17 = 22 bytes)
    assert_eq!(pkt.frame_len(), 17);

    let decoded = decoder.decode_raw_packet(&pkt).expect("decode 0x4F");
    assert_eq!(decoded, Ed2kMessage::FileStatusReq(req));
}

#[test]
fn test_tier3_op_file_status_bitfield_two_parts_emule050a() {
    let hash = [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81,
        0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82, 0x3c,
    ];
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::Emule050a);

    // Valid 2-parts status: part 0 and part 1 available -> 0b0000_0011 = 0x03
    let mut payload = Vec::new();
    payload.extend_from_slice(&hash);
    payload.extend_from_slice(&2u16.to_le_bytes());
    payload.push(0x03);

    let raw = mule_protocol::RawPacket::with_vec(
        mule_protocol::types::PR_ED2K,
        mule_protocol::ed2k::OP_FILESTATUS,
        payload,
    );
    let msg = decoder.decode_raw_packet(&raw).expect("decode valid 2-parts file status");
    match msg {
        Ed2kMessage::FileStatus(status) => {
            assert_eq!(status.file_hash, hash);
            assert_eq!(status.part_count, 2);
            assert!(status.has_part(0));
            assert!(status.has_part(1));
            assert!(!status.has_part(2)); // out of range
            assert_eq!(status.raw_bytes(), &[0x03]);
        }
        _ => panic!("expected FileStatus"),
    }

    // Invalid bitfield: bit 2 is set (0x07) for a 2-parts file -> must be rejected
    let mut invalid_payload = Vec::new();
    invalid_payload.extend_from_slice(&hash);
    invalid_payload.extend_from_slice(&2u16.to_le_bytes());
    invalid_payload.push(0x07); // 0b0000_0111: bit 2 is non-zero trailing bit

    let invalid_raw = mule_protocol::RawPacket::with_vec(
        mule_protocol::types::PR_ED2K,
        mule_protocol::ed2k::OP_FILESTATUS,
        invalid_payload,
    );
    let err = decoder.decode_raw_packet(&invalid_raw).unwrap_err();
    assert!(matches!(
        err.kind,
        mule_protocol::ed2k::Ed2kErrorKind::InvalidBitfieldTrailingBits { .. }
    ));
}

#[test]
fn test_tier3_op_file_req_ans_no_file_wire_framing() {
    let hash = [0x55; 16];
    let no_file = Ed2kFileReqAnsNoFile { file_hash: hash };
    let encoder = Ed2kEncoder::new();
    let decoder = Ed2kDecoder::new();

    let pkt = encoder.encode_file_req_ans_no_file(&no_file).expect("encode 0x48");
    assert_eq!(pkt.protocol, mule_protocol::types::PR_ED2K);
    assert_eq!(pkt.opcode, mule_protocol::ed2k::OP_FILEREQANSNOFILE); // 0x48
    assert_eq!(pkt.payload.len(), 16);
    assert_eq!(&pkt.payload[..], &hash);

    let decoded = decoder.decode_raw_packet(&pkt).expect("decode 0x48");
    assert_eq!(decoded, Ed2kMessage::FileReqAnsNoFile(no_file));

    // Trailing bytes rejection
    let mut corrupt_payload = Vec::new();
    corrupt_payload.extend_from_slice(&hash);
    corrupt_payload.push(0xFF); // 17th byte
    let corrupt_raw = mule_protocol::RawPacket::with_vec(
        mule_protocol::types::PR_ED2K,
        mule_protocol::ed2k::OP_FILEREQANSNOFILE,
        corrupt_payload,
    );
    let err = decoder.decode_raw_packet(&corrupt_raw).unwrap_err();
    assert!(matches!(
        err.kind,
        mule_protocol::ed2k::Ed2kErrorKind::MalformedField { .. }
    ));
}

#[test]
fn test_tier3_op_file_status_complete_file_zero_parts_emule050a() {
    let hash = [0x77; 16];
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    let encoder = Ed2kEncoder::new();
    let decoder = Ed2kDecoder::new();

    let pkt = encoder.encode_file_status(&status).expect("encode complete file 0x50");
    assert_eq!(pkt.protocol, mule_protocol::types::PR_ED2K);
    assert_eq!(pkt.opcode, mule_protocol::ed2k::OP_FILESTATUS); // 0x50
    assert_eq!(pkt.payload.len(), 18, "wire length must be exactly 18 bytes (16 hash + 2 part_count)");
    assert_eq!(&pkt.payload[0..16], &hash);
    assert_eq!(&pkt.payload[16..18], &[0x00, 0x00], "part_count 0 encoded as 2 zero bytes LE");

    let decoded = decoder.decode_raw_packet(&pkt).expect("decode complete file 0x50");
    match decoded {
        Ed2kMessage::FileStatus(dec_status) => {
            assert_eq!(dec_status.file_hash, hash);
            assert_eq!(dec_status.part_count, 0);
            assert!(dec_status.is_complete(), "part_count 0 must report complete file");
            assert!(dec_status.raw_bytes().is_empty(), "0 parts must have 0 bitfield bytes");
            assert!(dec_status.has_part(0), "part 0 must be reported available on complete file");
            assert!(dec_status.has_part(42), "any part must be reported available on complete file");
        }
        other => panic!("expected FileStatus, got: {other:?}"),
    }

    // Trailing bytes rejection on 18-byte complete file status
    let mut corrupt_payload = Vec::new();
    corrupt_payload.extend_from_slice(&hash);
    corrupt_payload.extend_from_slice(&0u16.to_le_bytes());
    corrupt_payload.push(0x01); // 19th byte
    let corrupt_raw = mule_protocol::RawPacket::with_vec(
        mule_protocol::types::PR_ED2K,
        mule_protocol::ed2k::OP_FILESTATUS,
        corrupt_payload,
    );
    let err = decoder.decode_raw_packet(&corrupt_raw).unwrap_err();
    assert!(matches!(
        err.kind,
        mule_protocol::ed2k::Ed2kErrorKind::MalformedField { .. }
    ));
}
