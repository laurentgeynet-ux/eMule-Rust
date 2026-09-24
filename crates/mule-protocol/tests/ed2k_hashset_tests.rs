// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Tests for OP_HASHSETREQUEST (0x51) / OP_HASHSETANSWER (0x52) wire codec.
//! Local-only: no network, no file writes.

use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kErrorKind, Ed2kHashSetAnswer, Ed2kHashSetReq, Ed2kMessage,
    OP_HASHSETANSWER, OP_HASHSETREQUEST, PR_ED2K,
};

mod common;
use common::fixtures::sample_file_hash;

fn sample_hashes(n: usize) -> Vec<[u8; 16]> {
    (0..n)
        .map(|i| {
            let mut h = [0u8; 16];
            h[0] = i as u8;
            h[15] = 0xFF - i as u8;
            h
        })
        .collect()
}

#[test]
fn hashset_request_roundtrip() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let req = Ed2kHashSetReq {
        file_hash: sample_file_hash(),
    };

    let pkt = encoder.encode_hashset_request(&req).unwrap();
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_HASHSETREQUEST);
    assert_eq!(pkt.payload.len(), 16);
    assert_eq!(&pkt.payload[..], &sample_file_hash());

    let decoded = decoder
        .decode_hashset_request(pkt.protocol, pkt.opcode, &pkt.payload, 0)
        .unwrap();
    assert_eq!(decoded.file_hash, req.file_hash);
}

#[test]
fn hashset_request_rejects_short_and_zero_hash() {
    let decoder = Ed2kDecoder::default();
    let err = decoder
        .decode_hashset_request(PR_ED2K, OP_HASHSETREQUEST, &[0u8; 8], 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::UnexpectedEof { .. }));

    let err = decoder
        .decode_hashset_request(PR_ED2K, OP_HASHSETREQUEST, &[0u8; 16], 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));
}

#[test]
fn hashset_answer_roundtrip() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let ans = Ed2kHashSetAnswer {
        file_hash: sample_file_hash(),
        part_hashes: sample_hashes(3),
    };

    let pkt = encoder.encode_hashset_answer(&ans).unwrap();
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_HASHSETANSWER);
    assert_eq!(pkt.payload.len(), 18 + 3 * 16);
    assert_eq!(&pkt.payload[..16], &sample_file_hash());
    assert_eq!(&pkt.payload[16..18], &3u16.to_le_bytes());

    let decoded = decoder
        .decode_hashset_answer(pkt.protocol, pkt.opcode, &pkt.payload, 0)
        .unwrap();
    assert_eq!(decoded, ans);
}

#[test]
fn hashset_answer_rejects_truncated_and_count_mismatch() {
    let decoder = Ed2kDecoder::default();

    // Too short for the header
    let err = decoder
        .decode_hashset_answer(PR_ED2K, OP_HASHSETANSWER, &[0u8; 10], 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::UnexpectedEof { .. }));

    // Declared count 2 but only one hash present
    let mut payload = Vec::new();
    payload.extend_from_slice(&sample_file_hash());
    payload.extend_from_slice(&2u16.to_le_bytes());
    payload.extend_from_slice(&[0xAA; 16]);
    let err = decoder
        .decode_hashset_answer(PR_ED2K, OP_HASHSETANSWER, &payload, 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));

    // Trailing garbage bytes
    let mut payload = Vec::new();
    payload.extend_from_slice(&sample_file_hash());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&[0xBB; 4]);
    let err = decoder
        .decode_hashset_answer(PR_ED2K, OP_HASHSETANSWER, &payload, 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));
}

#[test]
fn hashset_messages_dispatch_through_decode_packet_and_message_enum() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    let req_bytes = encoder
        .encode_to_bytes(&Ed2kMessage::HashSetReq(Ed2kHashSetReq {
            file_hash: sample_file_hash(),
        }))
        .unwrap();
    match decoder.decode_packet(&req_bytes).unwrap() {
        Ed2kMessage::HashSetReq(r) => assert_eq!(r.file_hash, sample_file_hash()),
        other => panic!("unexpected message: {other:?}"),
    }

    let ans = Ed2kHashSetAnswer {
        file_hash: sample_file_hash(),
        part_hashes: sample_hashes(2),
    };
    let ans_bytes = encoder
        .encode_to_bytes(&Ed2kMessage::HashSetAnswer(ans.clone()))
        .unwrap();
    match decoder.decode_packet(&ans_bytes).unwrap() {
        Ed2kMessage::HashSetAnswer(a) => assert_eq!(a, ans),
        other => panic!("unexpected message: {other:?}"),
    }
}

#[test]
fn hashset_answer_encode_rejects_oversized_set() {
    let mut encoder = Ed2kEncoder::default();
    encoder.limits.max_parts = 4;
    let ans = Ed2kHashSetAnswer {
        file_hash: sample_file_hash(),
        part_hashes: sample_hashes(5),
    };
    let err = encoder.encode_hashset_answer(&ans).unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));
}
