// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Tests for the AICH wire codec:
//! `OP_AICHREQUEST` (0x9B), `OP_AICHANSWER` (0x9C),
//! `OP_AICHFILEHASHREQ` (0x9E), `OP_AICHFILEHASHANS` (0x9D) — valeurs eMule 0.72a.
//! Local-only: no network, no file writes.

use mule_protocol::ed2k::{
    Ed2kAichAnswer, Ed2kAichFileHashReq, Ed2kAichFileHashRes, Ed2kAichRecovery, Ed2kAichRequest,
    Ed2kDecoder, Ed2kEncoder, Ed2kErrorKind, Ed2kMessage, OP_AICHFILEHASHREQ, OP_AICHFILEHASHRES,
    OP_AICH_ANSWER, OP_AICH_REQUEST, PR_EMULE,
};

fn sample_file_hash() -> [u8; 16] {
    let mut h = [0u8; 16];
    for (i, b) in h.iter_mut().enumerate() {
        *b = i as u8;
    }
    h
}

fn sample_aich_root() -> [u8; 20] {
    let mut h = [0u8; 20];
    for (i, b) in h.iter_mut().enumerate() {
        *b = 0xA0 + i as u8;
    }
    h
}

#[test]
fn aich_file_hash_req_roundtrip() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let req = Ed2kAichFileHashReq {
        file_hash: sample_file_hash(),
    };

    let pkt = encoder.encode_aich_file_hash_req(&req).unwrap();
    assert_eq!(pkt.protocol, PR_EMULE);
    assert_eq!(pkt.opcode, OP_AICHFILEHASHREQ);
    assert_eq!(pkt.payload.len(), 16);

    let decoded = decoder
        .decode_aich_file_hash_req(pkt.protocol, pkt.opcode, &pkt.payload, 0)
        .unwrap();
    assert_eq!(decoded.file_hash, req.file_hash);
}

#[test]
fn aich_file_hash_res_roundtrip() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let res = Ed2kAichFileHashRes {
        file_hash: sample_file_hash(),
        aich_hash: sample_aich_root(),
    };

    let pkt = encoder.encode_aich_file_hash_res(&res).unwrap();
    assert_eq!(pkt.protocol, PR_EMULE);
    assert_eq!(pkt.opcode, OP_AICHFILEHASHRES);
    assert_eq!(pkt.payload.len(), 36);

    let decoded = decoder
        .decode_aich_file_hash_res(pkt.protocol, pkt.opcode, &pkt.payload, 0)
        .unwrap();
    assert_eq!(decoded, res);
}

#[test]
fn aich_request_roundtrip() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let req = Ed2kAichRequest {
        file_hash: sample_file_hash(),
        part: 7,
        master_hash: sample_aich_root(),
    };

    let pkt = encoder.encode_aich_request(&req).unwrap();
    assert_eq!(pkt.protocol, PR_EMULE);
    assert_eq!(pkt.opcode, OP_AICH_REQUEST);
    assert_eq!(pkt.payload.len(), 38);
    assert_eq!(&pkt.payload[16..18], &7u16.to_le_bytes());

    let decoded = decoder
        .decode_aich_request(pkt.protocol, pkt.opcode, &pkt.payload, 0)
        .unwrap();
    assert_eq!(decoded, req);
}

#[test]
fn aich_answer_with_recovery_roundtrip() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let ans = Ed2kAichAnswer {
        file_hash: sample_file_hash(),
        recovery: Some(Ed2kAichRecovery {
            part: 3,
            master_hash: sample_aich_root(),
            entries16: vec![(0x0006, [1u8; 20]), (0x0007, [2u8; 20])],
            entries32: vec![(0x0001_0004, [3u8; 20])],
        }),
    };

    let pkt = encoder.encode_aich_answer(&ans).unwrap();
    assert_eq!(pkt.protocol, PR_EMULE);
    assert_eq!(pkt.opcode, OP_AICH_ANSWER);
    // 16 + 2 + 20 + 2 + 2*22 + 2 + 1*24
    assert_eq!(pkt.payload.len(), 38 + 2 + 44 + 2 + 24);

    let decoded = decoder
        .decode_aich_answer(pkt.protocol, pkt.opcode, &pkt.payload, 0)
        .unwrap();
    assert_eq!(decoded, ans);
}

#[test]
fn aich_answer_failed_answer_is_bare_file_hash() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let ans = Ed2kAichAnswer {
        file_hash: sample_file_hash(),
        recovery: None,
    };

    let pkt = encoder.encode_aich_answer(&ans).unwrap();
    assert_eq!(pkt.payload.len(), 16);

    let decoded = decoder
        .decode_aich_answer(pkt.protocol, pkt.opcode, &pkt.payload, 0)
        .unwrap();
    assert_eq!(decoded.file_hash, sample_file_hash());
    assert!(decoded.recovery.is_none());
}

#[test]
fn aich_answer_rejects_malformed_payloads() {
    let decoder = Ed2kDecoder::default();

    // 17..41 bytes: too short for the recovery header.
    let err = decoder
        .decode_aich_answer(PR_EMULE, OP_AICH_ANSWER, &[0u8; 30], 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::UnexpectedEof { .. }));

    // count16 = 2 but only one entry present.
    let mut payload = Vec::new();
    payload.extend_from_slice(&sample_file_hash());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&sample_aich_root());
    payload.extend_from_slice(&2u16.to_le_bytes());
    payload.extend_from_slice(&0x0004u16.to_le_bytes());
    payload.extend_from_slice(&[9u8; 20]);
    let err = decoder
        .decode_aich_answer(PR_EMULE, OP_AICH_ANSWER, &payload, 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::UnexpectedEof { .. }));

    // Trailing garbage.
    let mut payload = Vec::new();
    payload.extend_from_slice(&sample_file_hash());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&sample_aich_root());
    payload.extend_from_slice(&1u16.to_le_bytes());
    payload.extend_from_slice(&0x0004u16.to_le_bytes());
    payload.extend_from_slice(&[9u8; 20]);
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&[0xCC; 3]);
    let err = decoder
        .decode_aich_answer(PR_EMULE, OP_AICH_ANSWER, &payload, 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));

    // Both counts zero.
    let mut payload = Vec::new();
    payload.extend_from_slice(&sample_file_hash());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&sample_aich_root());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&0u16.to_le_bytes());
    let err = decoder
        .decode_aich_answer(PR_EMULE, OP_AICH_ANSWER, &payload, 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));
}

#[test]
fn aich_request_rejects_wrong_size_and_zero_hashes() {
    let decoder = Ed2kDecoder::default();

    let err = decoder
        .decode_aich_request(PR_EMULE, OP_AICH_REQUEST, &[0u8; 37], 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));

    // Zero file hash.
    let mut payload = vec![0u8; 38];
    payload[18..38].copy_from_slice(&sample_aich_root());
    let err = decoder
        .decode_aich_request(PR_EMULE, OP_AICH_REQUEST, &payload, 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));

    // Zero master hash.
    let mut payload = vec![0u8; 38];
    payload[..16].copy_from_slice(&sample_file_hash());
    let err = decoder
        .decode_aich_request(PR_EMULE, OP_AICH_REQUEST, &payload, 0)
        .unwrap_err();
    assert!(matches!(err.kind, Ed2kErrorKind::MalformedField { .. }));
}

#[test]
fn aich_messages_dispatch_through_decode_packet_and_message_enum() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    let req_bytes = encoder
        .encode_to_bytes(&Ed2kMessage::AichFileHashReq(Ed2kAichFileHashReq {
            file_hash: sample_file_hash(),
        }))
        .unwrap();
    match decoder.decode_packet(&req_bytes).unwrap() {
        Ed2kMessage::AichFileHashReq(r) => assert_eq!(r.file_hash, sample_file_hash()),
        other => panic!("unexpected message: {other:?}"),
    }

    let res = Ed2kAichFileHashRes {
        file_hash: sample_file_hash(),
        aich_hash: sample_aich_root(),
    };
    let res_bytes = encoder
        .encode_to_bytes(&Ed2kMessage::AichFileHashRes(res.clone()))
        .unwrap();
    match decoder.decode_packet(&res_bytes).unwrap() {
        Ed2kMessage::AichFileHashRes(r) => assert_eq!(r, res),
        other => panic!("unexpected message: {other:?}"),
    }

    let req = Ed2kAichRequest {
        file_hash: sample_file_hash(),
        part: 1,
        master_hash: sample_aich_root(),
    };
    let req_bytes = encoder
        .encode_to_bytes(&Ed2kMessage::AichRequest(req.clone()))
        .unwrap();
    match decoder.decode_packet(&req_bytes).unwrap() {
        Ed2kMessage::AichRequest(r) => assert_eq!(r, req),
        other => panic!("unexpected message: {other:?}"),
    }

    let ans = Ed2kAichAnswer {
        file_hash: sample_file_hash(),
        recovery: Some(Ed2kAichRecovery {
            part: 1,
            master_hash: sample_aich_root(),
            entries16: vec![(0x0006, [4u8; 20])],
            entries32: vec![],
        }),
    };
    let ans_bytes = encoder
        .encode_to_bytes(&Ed2kMessage::AichAnswer(ans.clone()))
        .unwrap();
    match decoder.decode_packet(&ans_bytes).unwrap() {
        Ed2kMessage::AichAnswer(a) => assert_eq!(a, ans),
        other => panic!("unexpected message: {other:?}"),
    }
}
