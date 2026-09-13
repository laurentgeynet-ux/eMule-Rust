//! Comprehensive roundtrip and deterministic encoding test suite for eD2K wire messages.

use std::fs;
use std::path::PathBuf;

use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kErrorKind, Ed2kFileStatus, Ed2kHello,
    Ed2kHelloAnswer, Ed2kLimits, Ed2kMessage, Ed2kTag, RawTag, TagName, TagValue,
    TAG_SPECIAL_EMULE_VERSION, TAG_SPECIAL_MISCOPTIONS1, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
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

    let decoder = Ed2kDecoder::new();
    let msg = decoder
        .decode_packet(&raw_bytes)
        .expect("failed to decode hello_v1_minimal");

    let encoder = Ed2kEncoder::new();
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

    let decoder = Ed2kDecoder::new();
    let msg = decoder
        .decode_packet(&raw_bytes)
        .expect("failed to decode hello_v1_tags");

    let encoder = Ed2kEncoder::new();
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

    // 5-byte header + 1 opcode + 16 user hash + 4 client id + 2 port + 4 tag count + 4 server ip + 2 server port = 38 bytes
    assert_eq!(encoded.len(), 38);


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
        let status = Ed2kFileStatus {
            file_hash: [0x77; 16],
            part_count,
            status_bitfield: bitfield,
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

    // part_count = 10 requires ceil(10/8) = 2 bytes bitfield, but 1 byte provided
    let status = Ed2kFileStatus {
        file_hash: [0x88; 16],
        part_count: 10,
        status_bitfield: vec![0xFF],
    };

    let result = encoder.encode_file_status(&status);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.field, "filestatus_bitfield");
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));
}
