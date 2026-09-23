// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Tests for eMule 0.72a extended wire messages:
//! `FileIdentifier`, `OP_HASHSETREQUEST2`/`OP_HASHSETANSWER2` (0xB1/0xB2),
//! `OP_MULTIPACKET*` (0x92/0xA4/0xA9) and `OP_MULTIPACKETANSWER*` (0x93/0xB0),
//! `OP_MESSAGE` (0x4E), `OP_QUEUERANK` (0x5C), `OP_QUEUERANKING` (0x60),
//! `OP_CHANGE_SLOT` (0x5B), `OP_FILEDESC` (0x61), `OP_PORTTEST` (0xFE),
//! `OP_CHATCAPTCHAREQ`/`RES` (0xA5/0xA6).
//! Local-only: no network, no file writes.

use mule_protocol::ed2k::{
    Ed2kChangeSlot, Ed2kDecoder, Ed2kEncoder, Ed2kFileIdentifier, Ed2kHashSetAnswer,
    Ed2kHashSetAnswer2, Ed2kHashSetReq2, Ed2kMessage, Ed2kMultiPacketFileRef,
    Ed2kMultiPacketRequest, Ed2kMultiPacketSubRequest, Ed2kPortTest, Ed2kQueueRank,
    Ed2kQueueRanking, Ed2kAichPartHashSet, Ed2kChatMessage, Ed2kMultiPacketAnswer,
    Ed2kMultiPacketSubAnswer, Ed2kRequestExtInfo, OP_HASHSETREQUEST2, OP_MESSAGE,
    OP_MULTIPACKET, OP_MULTIPACKETANSWER, OP_MULTIPACKETANSWER_EXT2, OP_MULTIPACKET_EXT,
    OP_MULTIPACKET_EXT2, OP_QUEUERANKING, OP_REQFILENAME, OP_REQUESTSOURCES,
    OP_SETREQFILEID_EMULE, PR_ED2K, PR_EMULE,
};
use mule_protocol::ed2k::decoder::{
    decode_hashset_request2, decode_message, decode_multipacket_request, decode_queue_ranking,
};
use mule_protocol::ed2k::encoder::{
    encode_hashset_request2, encode_message as encode_chat_message, encode_multipacket_answer,
    encode_multipacket_request, encode_queue_ranking,
};
use mule_protocol::ed2k::Ed2kLimits;

fn limits() -> Ed2kLimits {
    Ed2kLimits::default()
}

fn hash16(seed: u8) -> [u8; 16] {
    let mut h = [0u8; 16];
    for (i, b) in h.iter_mut().enumerate() {
        *b = seed.wrapping_add(i as u8);
    }
    h
}

fn hash20(seed: u8) -> [u8; 20] {
    let mut h = [0u8; 20];
    for (i, b) in h.iter_mut().enumerate() {
        *b = seed.wrapping_add(i as u8);
    }
    h
}

fn roundtrip(msg: Ed2kMessage) -> Ed2kMessage {
    roundtrip_v(msg, 2)
}

/// Round-trip avec un `ExtendedRequestsVersion` négocié explicite (eMule
/// `m_byExtendedRequestsVer`) — nécessaire pour les corps `MULTIPACKET*` dont
/// l'ext-info de `0x58` dépend de cette version.
fn roundtrip_v(msg: Ed2kMessage, extended_requests_version: u8) -> Ed2kMessage {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default().with_extended_requests_version(extended_requests_version);
    let pkt = encoder.encode_message(&msg).unwrap();
    decoder.decode_raw_packet(&pkt).unwrap()
}

// ---------- FileIdentifier ----------

#[test]
fn file_identifier_full_layout() {
    let id = Ed2kFileIdentifier {
        md4_hash: Some(hash16(1)),
        file_size: Some(123_456),
        aich_hash: Some(hash20(2)),
        optional_bits: 0,
    };
    let mut buf = Vec::new();
    id.write_to(&mut buf);
    // options 0x07 + hash16 + u64 + hash20
    assert_eq!(buf[0], 0x07);
    assert_eq!(buf.len(), 1 + 16 + 8 + 20);
}

#[test]
fn file_identifier_rejects_unknown_required_bits() {
    // bit3 (0x08) is inside the mandatory mask but unassigned → eMule rejects.
    let mut buf = vec![0x08u8];
    buf.extend_from_slice(&[0u8; 40]);
    let mut r = mule_protocol::ed2k::TagReader::new(&buf, 0, PR_EMULE, Some(OP_MULTIPACKET_EXT2));
    assert!(Ed2kFileIdentifier::read_from(&mut r, false).is_err());
}

#[test]
fn file_identifier_requires_md4_on_tcp() {
    // options = size only (0x02) → rejected when MD4 mandatory.
    let mut buf = vec![0x02u8];
    buf.extend_from_slice(&9_728_000u64.to_le_bytes());
    let mut r = mule_protocol::ed2k::TagReader::new(&buf, 0, PR_EMULE, Some(OP_MULTIPACKET_EXT2));
    assert!(Ed2kFileIdentifier::read_from(&mut r, false).is_err());
    // … but tolerated when the caller allows empty MD4 (Kad context).
    let mut r2 = mule_protocol::ed2k::TagReader::new(&buf, 0, PR_EMULE, Some(OP_MULTIPACKET_EXT2));
    let id = Ed2kFileIdentifier::read_from(&mut r2, true).unwrap();
    assert_eq!(id.file_size, Some(9_728_000));
    assert_eq!(id.md4_hash, None);
}

#[test]
fn file_identifier_preserves_optional_bits() {
    // 0x30 = reserved optional bits → tolerated and kept for round-trip.
    let mut buf = vec![0x30u8 | 0x03];
    buf.extend_from_slice(&hash16(7));
    buf.extend_from_slice(&42u64.to_le_bytes());
    let mut r = mule_protocol::ed2k::TagReader::new(&buf, 0, PR_EMULE, Some(OP_MULTIPACKET_EXT2));
    let id = Ed2kFileIdentifier::read_from(&mut r, false).unwrap();
    assert_eq!(id.optional_bits, 0x30);
    let mut out = Vec::new();
    id.write_to(&mut out);
    assert_eq!(out, buf);
}

// ---------- HASHSET2 ----------

#[test]
fn hashset2_request_roundtrip() {
    let req = Ed2kHashSetReq2 {
        identifier: Ed2kFileIdentifier::new(hash16(1), 1000),
        want_md4: true,
        want_aich: true,
    };
    let pkt = encode_hashset_request2(&limits(), &req).unwrap();
    assert_eq!(pkt.protocol, PR_EMULE);
    assert_eq!(pkt.opcode, OP_HASHSETREQUEST2);
    // FileIdentifier (1 + 16 + 8) + options byte
    assert_eq!(pkt.payload.len(), 26);
    assert_eq!(*pkt.payload.last().unwrap(), 0x03);

    match roundtrip(Ed2kMessage::HashSetReq2(req.clone())) {
        Ed2kMessage::HashSetReq2(d) => assert_eq!(d, req),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn hashset2_request_rejects_empty_options() {
    let req = Ed2kHashSetReq2 {
        identifier: Ed2kFileIdentifier::new(hash16(1), 1000),
        want_md4: false,
        want_aich: false,
    };
    assert!(encode_hashset_request2(&limits(), &req).is_err());

    // Decoder side: options byte 0x00 must be rejected.
    let mut payload = Vec::new();
    Ed2kFileIdentifier::new(hash16(1), 1000).write_to(&mut payload);
    payload.push(0x00);
    assert!(decode_hashset_request2(PR_EMULE, OP_HASHSETREQUEST2, &payload, 0).is_err());
}

#[test]
fn hashset2_answer_roundtrip() {
    let ans = Ed2kHashSetAnswer2 {
        identifier: Ed2kFileIdentifier {
            md4_hash: Some(hash16(1)),
            file_size: Some(19_456_000),
            aich_hash: Some(hash20(9)),
            optional_bits: 0,
        },
        md4_set: Some(Ed2kHashSetAnswer {
            file_hash: hash16(1),
            part_hashes: vec![hash16(10), hash16(20)],
        }),
        aich_set: Some(Ed2kAichPartHashSet {
            master_hash: hash20(9),
            part_hashes: vec![hash20(30), hash20(40)],
        }),
    };
    match roundtrip(Ed2kMessage::HashSetAnswer2(ans.clone())) {
        Ed2kMessage::HashSetAnswer2(d) => assert_eq!(d, ans),
        other => panic!("unexpected {other:?}"),
    }
}

// ---------- MULTIPACKET ----------

#[test]
fn multipacket_legacy_roundtrip() {
    let req = Ed2kMultiPacketRequest {
        file: Ed2kMultiPacketFileRef::Hash(hash16(3)),
        sub_requests: vec![
            Ed2kMultiPacketSubRequest::FileName { ext_info: None },
            Ed2kMultiPacketSubRequest::SetReqFileId,
            Ed2kMultiPacketSubRequest::RequestSources,
        ],
    };
    let pkt = encode_multipacket_request(&limits(), &req).unwrap();
    assert_eq!(pkt.opcode, OP_MULTIPACKET);
    assert_eq!(pkt.protocol, PR_EMULE);
    match roundtrip_v(Ed2kMessage::MultiPacketRequest(req.clone()), 0) {
        Ed2kMessage::MultiPacketRequest(d) => assert_eq!(d, req),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn multipacket_ext_roundtrip() {
    let req = Ed2kMultiPacketRequest {
        file: Ed2kMultiPacketFileRef::HashAndSize {
            file_hash: hash16(4),
            file_size: 5_000_000_000,
        },
        sub_requests: vec![
            Ed2kMultiPacketSubRequest::FileName {
                ext_info: Some(Ed2kRequestExtInfo {
                    part_count: 10,
                    part_status: bytes::Bytes::from_static(&[0xFF, 0x03]),
                    complete_sources: Some(7),
                }),
            },
            Ed2kMultiPacketSubRequest::RequestSources2 {
                version: 4,
                options: 0,
            },
        ],
    };
    let pkt = encode_multipacket_request(&limits(), &req).unwrap();
    assert_eq!(pkt.opcode, OP_MULTIPACKET_EXT);
    match roundtrip(Ed2kMessage::MultiPacketRequest(req.clone())) {
        Ed2kMessage::MultiPacketRequest(d) => assert_eq!(d, req),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn multipacket_ext2_roundtrip() {
    let req = Ed2kMultiPacketRequest {
        file: Ed2kMultiPacketFileRef::Identifier(Ed2kFileIdentifier {
            md4_hash: Some(hash16(5)),
            file_size: Some(999),
            aich_hash: Some(hash20(6)),
            optional_bits: 0,
        }),
        sub_requests: vec![Ed2kMultiPacketSubRequest::FileName { ext_info: None }],
    };
    let pkt = encode_multipacket_request(&limits(), &req).unwrap();
    assert_eq!(pkt.opcode, OP_MULTIPACKET_EXT2);
    match roundtrip_v(Ed2kMessage::MultiPacketRequest(req.clone()), 0) {
        Ed2kMessage::MultiPacketRequest(d) => assert_eq!(d, req),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn multipacket_rejects_unknown_subopcode() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&hash16(1));
    payload.push(0x77); // not a valid sub-opcode
    assert!(
        decode_multipacket_request(PR_EMULE, OP_MULTIPACKET, &payload, 0, &limits(), 0).is_err()
    );
}

// ---------- MULTIPACKET ext-info (ExtendedRequestsVersion) ----------

#[test]
fn multipacket_extinfo_version0_absent() {
    // Version 0 : les octets suivant 0x58 sont des sous-opcodes (pas d'ext-info).
    let mut payload = Vec::new();
    payload.extend_from_slice(&hash16(2));
    payload.push(OP_REQFILENAME);
    payload.push(OP_SETREQFILEID_EMULE);
    let req = decode_multipacket_request(PR_EMULE, OP_MULTIPACKET, &payload, 0, &limits(), 0)
        .expect("v0 decode");
    assert_eq!(
        req.sub_requests,
        vec![
            Ed2kMultiPacketSubRequest::FileName { ext_info: None },
            Ed2kMultiPacketSubRequest::SetReqFileId,
        ]
    );
}

#[test]
fn multipacket_extinfo_version1_bitfield_only() {
    // Version 1 : u16 part count + bitfield, pas de complete_sources.
    let mut payload = Vec::new();
    payload.extend_from_slice(&hash16(2));
    payload.push(OP_REQFILENAME);
    payload.extend_from_slice(&10u16.to_le_bytes());
    payload.extend_from_slice(&[0xFF, 0x03]);
    payload.push(OP_SETREQFILEID_EMULE);
    let req = decode_multipacket_request(PR_EMULE, OP_MULTIPACKET, &payload, 0, &limits(), 1)
        .expect("v1 decode");
    match &req.sub_requests[0] {
        Ed2kMultiPacketSubRequest::FileName { ext_info } => {
            let info = ext_info.as_ref().expect("ext info present");
            assert_eq!(info.part_count, 10);
            assert_eq!(info.complete_sources, None);
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(req.sub_requests[1], Ed2kMultiPacketSubRequest::SetReqFileId);
}

#[test]
fn multipacket_extinfo_version2_with_complete_sources() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&hash16(2));
    payload.push(OP_REQFILENAME);
    payload.extend_from_slice(&10u16.to_le_bytes());
    payload.extend_from_slice(&[0xFF, 0x03]);
    payload.extend_from_slice(&42u16.to_le_bytes());
    payload.push(OP_REQUESTSOURCES);
    let req = decode_multipacket_request(PR_EMULE, OP_MULTIPACKET, &payload, 0, &limits(), 2)
        .expect("v2 decode");
    match &req.sub_requests[0] {
        Ed2kMultiPacketSubRequest::FileName { ext_info } => {
            let info = ext_info.as_ref().expect("ext info present");
            assert_eq!(info.complete_sources, Some(42));
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(req.sub_requests[1], Ed2kMultiPacketSubRequest::RequestSources);
}

#[test]
fn multipacket_extinfo_truncated_rejected() {
    // Version 1, bitfield annoncé pour 10 parts mais tronqué à 1 octet.
    let mut payload = Vec::new();
    payload.extend_from_slice(&hash16(2));
    payload.push(OP_REQFILENAME);
    payload.extend_from_slice(&10u16.to_le_bytes());
    payload.push(0xFF); // manque le 2e octet du bitfield
    assert!(
        decode_multipacket_request(PR_EMULE, OP_MULTIPACKET, &payload, 0, &limits(), 1).is_err()
    );
}

#[test]
fn multipacket_extinfo_strict_profile() {
    // UnknownStrict : la version est explicite — le parsing reste
    // déterministe (ici v0 → 0x4F lu comme sous-opcode, pas d'ext-info).
    let mut payload = Vec::new();
    payload.extend_from_slice(&hash16(2));
    payload.push(OP_REQFILENAME);
    payload.push(OP_SETREQFILEID_EMULE);
    let req = decode_multipacket_request(PR_EMULE, OP_MULTIPACKET, &payload, 0, &limits(), 0)
        .expect("strict v0 decode");
    assert_eq!(req.sub_requests.len(), 2);
}

#[test]
fn multipacket_answer_roundtrip() {
    let ans = Ed2kMultiPacketAnswer {
        file: Ed2kMultiPacketFileRef::Hash(hash16(8)),
        sub_answers: vec![
            Ed2kMultiPacketSubAnswer::FileName("debian.iso".to_string()),
            Ed2kMultiPacketSubAnswer::FileStatus {
                part_count: 9,
                part_status: bytes::Bytes::from_static(&[0xFF, 0x01]),
            },
            Ed2kMultiPacketSubAnswer::AichFileHash(hash20(11)),
        ],
    };
    let pkt = encode_multipacket_answer(&limits(), &ans).unwrap();
    assert_eq!(pkt.opcode, OP_MULTIPACKETANSWER);
    match roundtrip(Ed2kMessage::MultiPacketAnswer(ans.clone())) {
        Ed2kMessage::MultiPacketAnswer(d) => assert_eq!(d, ans),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn multipacket_answer_ext2_roundtrip() {
    let ans = Ed2kMultiPacketAnswer {
        file: Ed2kMultiPacketFileRef::Identifier(Ed2kFileIdentifier::new(hash16(9), 42)),
        sub_answers: vec![Ed2kMultiPacketSubAnswer::FileName("f.bin".to_string())],
    };
    let pkt = encode_multipacket_answer(&limits(), &ans).unwrap();
    assert_eq!(pkt.opcode, OP_MULTIPACKETANSWER_EXT2);
    match roundtrip(Ed2kMessage::MultiPacketAnswer(ans.clone())) {
        Ed2kMessage::MultiPacketAnswer(d) => assert_eq!(d, ans),
        other => panic!("unexpected {other:?}"),
    }
}

// ---------- Queue / chat / misc ----------

#[test]
fn message_roundtrip_and_strict_length() {
    let msg = Ed2kChatMessage {
        text: "hello".to_string(),
    };
    match roundtrip(Ed2kMessage::Message(msg.clone())) {
        Ed2kMessage::Message(d) => assert_eq!(d, msg),
        other => panic!("unexpected {other:?}"),
    }
    let pkt = encode_chat_message(&limits(), &msg).unwrap();
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_MESSAGE);

    // Trailing garbage must be rejected (eMule requires len+2 == payload).
    let mut bad = pkt.payload.to_vec();
    bad.push(0xAA);
    assert!(decode_message(PR_ED2K, OP_MESSAGE, &bad, 0, &limits()).is_err());
}

#[test]
fn queue_rank_roundtrip() {
    let qr = Ed2kQueueRank { rank: 1337 };
    match roundtrip(Ed2kMessage::QueueRank(qr)) {
        Ed2kMessage::QueueRank(d) => assert_eq!(d.rank, 1337),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn queue_ranking_exactly_12_bytes() {
    let qr = Ed2kQueueRanking {
        rank: 42,
        reserved: [0u8; 10],
    };
    let pkt = encode_queue_ranking(&limits(), &qr).unwrap();
    assert_eq!(pkt.opcode, OP_QUEUERANKING);
    assert_eq!(pkt.protocol, PR_EMULE);
    assert_eq!(pkt.payload.len(), 12);
    match roundtrip(Ed2kMessage::QueueRanking(qr.clone())) {
        Ed2kMessage::QueueRanking(d) => assert_eq!(d, qr),
        other => panic!("unexpected {other:?}"),
    }
    // eMule requires exactly 12 bytes.
    assert!(decode_queue_ranking(PR_EMULE, OP_QUEUERANKING, &[0u8; 11], 0).is_err());
}

#[test]
fn change_slot_and_port_test() {
    let cs = Ed2kChangeSlot {
        file_hash: hash16(1),
    };
    match roundtrip(Ed2kMessage::ChangeSlot(cs.clone())) {
        Ed2kMessage::ChangeSlot(d) => assert_eq!(d, cs),
        other => panic!("unexpected {other:?}"),
    }
    let pt = Ed2kPortTest { value: 0x12 }; // eMule's answer byte
    match roundtrip(Ed2kMessage::PortTest(pt)) {
        Ed2kMessage::PortTest(d) => assert_eq!(d.value, 0x12),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn captcha_response_roundtrip() {
    use mule_protocol::ed2k::{Ed2kCaptchaResponse, OP_CHATCAPTCHARES};
    let res = Ed2kCaptchaResponse { status: 0 };
    match roundtrip(Ed2kMessage::CaptchaResponse(res)) {
        Ed2kMessage::CaptchaResponse(d) => assert_eq!(d.status, 0),
        other => panic!("unexpected {other:?}"),
    }
    let _ = OP_CHATCAPTCHARES;
}

// ---------- OP_REQFILENAME extended info (negotiated version) ----------

/// eMule 0.72a `SendFileRequest`: `0x58` = hash16 + ext-info selon la
/// `ExtendedRequestsVersion` négociée. Un paquet nu vers un pair ext_req>0
/// provoque une exception filaire côté eMule (connexion fermée).
#[test]
fn req_filename_ext_info_matches_negotiated_version() {
    use mule_protocol::ed2k::encoder::encode_req_filename;
    use mule_protocol::ed2k::PartAvailabilityMask;

    let hash = hash16(0x40);

    // v0 : hash seul (legacy)
    let pkt = encode_req_filename(
        &limits(),
        &hash,
        &PartAvailabilityMask::empty(),
        0,
        0,
    )
    .unwrap();
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_REQFILENAME);
    assert_eq!(pkt.payload.len(), 16);

    // v1 : hash + u16 part_count + bitfield
    let mask = PartAvailabilityMask::from_parts(&[0, 2], 3); // 0b00000101
    let pkt = encode_req_filename(&limits(), &hash, &mask, 7, 1).unwrap();
    assert_eq!(pkt.payload.len(), 16 + 2 + 1);
    assert_eq!(&pkt.payload[16..], &[0x03, 0x00, 0x05]);

    // v2 : + u16 complete_sources
    let pkt = encode_req_filename(&limits(), &hash, &mask, 7, 2).unwrap();
    assert_eq!(pkt.payload.len(), 16 + 2 + 1 + 2);
    assert_eq!(&pkt.payload[19..], &[0x07, 0x00]);
}

// ---------- AICH opcode regression (0.72a values) ----------

#[test]
fn aich_opcodes_match_emule_072a() {
    use mule_protocol::ed2k::{
        OP_AICHFILEHASHREQ, OP_AICHFILEHASHRES, OP_AICH_ANSWER, OP_AICH_REQUEST,
    };
    assert_eq!(OP_AICH_REQUEST, 0x9B);
    assert_eq!(OP_AICH_ANSWER, 0x9C);
    assert_eq!(OP_AICHFILEHASHRES, 0x9D);
    assert_eq!(OP_AICHFILEHASHREQ, 0x9E);
}
