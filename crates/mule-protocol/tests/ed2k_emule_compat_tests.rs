//! Compatibility fixtures modelled on eMule 0.72a and aMule 3.0.1 sources.
//!
//! Tag bytes are hand-encoded in eMule "newtags" format (`tag_type | 0x80`
//! followed by a single-byte special tag id), matching what real clients put
//! on the wire — not our own encoder's output.
//!
//! Reference tag sets:
//! - eMule 0.72a `CUpDownClient::SendHelloTypePacket` / `SendMuleInfoPacket`
//!   (srchybrid/BaseClient.cpp): CT_NAME, CT_VERSION, CT_EMULE_UDPPORTS,
//!   CT_EMULE_VERSION (0xFB), CT_EMULE_MISCOPTIONS1 (0xFA),
//!   CT_EMULE_MISCOPTIONS2 (0xFE); EMULEINFO carries ET_COMPRESSION(0x20),
//!   ET_UDPPORT(0x21), ET_UDPVER(0x22), ET_SOURCEEXCHANGE(0x23),
//!   ET_COMMENTS(0x24), ET_EXTENDEDREQUEST(0x25), ET_FEATURES(0x27).
//! - aMule 3.0.1 `CClientTCPSocket::SendHelloTypePacket`: same plus
//!   CT_EMULECOMPAT_OPTIONS (0xEF) and software id SO_AMULE (3) in the high
//!   byte of CT_EMULE_VERSION.

use mule_protocol::ed2k::decoder::{
    decode_hello_answer, decode_request_parts_i64, decode_sending_part_i64,
};
use mule_protocol::ed2k::encoder::encode_emule_info;
use mule_protocol::ed2k::limits::Ed2kLimits;
use mule_protocol::ed2k::opcodes::{OP_EMULE_INFO, OP_HELLO_ANSWER, OP_REQUESTPARTS_I64};
use mule_protocol::ed2k::packet::{Ed2kBlockRange64, Ed2kRequestPartsI64};
use mule_protocol::ed2k::protocol::{PR_ED2K, PR_EMULE};
use mule_protocol::ed2k::Ed2kPeerInfo;
use mule_protocol::ed2k::{Ed2kDecoder, Ed2kEncoder, Ed2kMessage};

// --- newtags-format tag builders -------------------------------------------

/// uint32 special tag: [0x03|0x80][id][u32 LE]
fn t_u32(id: u8, v: u32) -> Vec<u8> {
    let mut b = vec![0x83, id];
    b.extend_from_slice(&v.to_le_bytes());
    b
}

/// uint8 special tag: [0x09|0x80][id][u8]
fn t_u8(id: u8, v: u8) -> Vec<u8> {
    vec![0x89, id, v]
}

/// string special tag: [0x02|0x80][id][u16 len][bytes]
fn t_str(id: u8, s: &str) -> Vec<u8> {
    let mut b = vec![0x82, id];
    b.extend_from_slice(&(s.len() as u16).to_le_bytes());
    b.extend_from_slice(s.as_bytes());
    b
}

fn hello_answer_payload(client_id: u32, tcp_port: u16, tags: &[Vec<u8>]) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&[0xAB; 16]); // user hash
    p.extend_from_slice(&client_id.to_le_bytes());
    p.extend_from_slice(&tcp_port.to_le_bytes());
    p.extend_from_slice(&(tags.len() as u32).to_le_bytes());
    for t in tags {
        p.extend_from_slice(t);
    }
    p.extend_from_slice(&0u32.to_le_bytes()); // server ip
    p.extend_from_slice(&0u16.to_le_bytes()); // server port
    p
}

fn emule_info_payload(client_ver: u8, proto_ver: u8, tags: &[Vec<u8>]) -> Vec<u8> {
    let mut p = vec![client_ver, proto_ver];
    p.extend_from_slice(&(tags.len() as u32).to_le_bytes());
    for t in tags {
        p.extend_from_slice(t);
    }
    p
}

// --- eMule 0.72a fixture ----------------------------------------------------
//
// MiscOptions1 (BaseClient.cpp): AICH1<<29 | Unicode<<28 | UDPVer4<<24 |
// DataComp1<<20 | SecIdent3<<16 | SrcExch4<<12 | ExtReq2<<8 | Comments1<<4 |
// NoView0<<2 | MultiPkt1<<1 | Preview1<<0  = 0x34134213
const EMULE072A_MISC1: u32 = 0x3413_4213;
// MiscOptions2: FileIdent<<13 | DirectUDPCallback<<12 | Captcha<<11 |
// SrcExch2<<10 | CryptReq<<9 | CryptWants<<8 | CryptSup<<7 | ExtMultiPkt<<5 |
// LargeFiles<<4 | KadVer10  = 0x3FBA
const EMULE072A_MISC2: u32 = 0x3FBA;
// CT_EMULE_VERSION 0.72a: (0<<17)|(72<<10)|(0<<7), software id 0
const EMULE072A_VERSION: u32 = 72 << 10;

fn emule072a_hello_answer() -> Vec<u8> {
    hello_answer_payload(
        0x4D7F_0001, // HighID-ish client id
        4662,
        &[
            t_str(0x01, "eMule peer"),        // CT_NAME
            t_u32(0x11, 0x3C),                // CT_VERSION = 0x3C
            t_u32(0xF9, (4672 << 16) | 4662), // CT_EMULE_UDPPORTS
            t_u32(0xFB, EMULE072A_VERSION),   // CT_EMULE_VERSION
            t_u32(0xFA, EMULE072A_MISC1),     // CT_EMULE_MISCOPTIONS1
            t_u32(0xFE, EMULE072A_MISC2),     // CT_EMULE_MISCOPTIONS2
        ],
    )
}

// --- aMule 3.0.1 fixture ----------------------------------------------------
//
// CT_EMULE_VERSION = (SO_AMULE=3 << 24) | (3<<17)|(0<<10)|(1<<7)
const AMULE301_VERSION: u32 = ((3 << 24) | (3 << 17)) | (1 << 7);
// aMule MiscOptions1: SrcExch3<<12 (not 4), ExtReq2, no preview bit
const AMULE301_MISC1: u32 = 0x3413_3212;
// aMule MiscOptions2 with KadVer 8 (KADEMLIA_VERSION = 0x08)
const AMULE301_MISC2: u32 = 0x3FB8;

fn amule301_hello_answer() -> Vec<u8> {
    hello_answer_payload(
        0x4D7F_0002,
        4665,
        &[
            t_str(0x01, "aMule 3.0.1"),       // CT_NAME
            t_u32(0x11, 0x3C),                // CT_VERSION
            t_u32(0xF9, (4675 << 16) | 4665), // CT_EMULE_UDPPORTS
            t_u32(0xFB, AMULE301_VERSION),    // CT_EMULE_VERSION (SO_AMULE)
            t_u32(0xFA, AMULE301_MISC1),      // CT_EMULE_MISCOPTIONS1
            t_u32(0xFE, AMULE301_MISC2),      // CT_EMULE_MISCOPTIONS2
            t_u32(0xEF, 0x0000_0001),         // CT_EMULECOMPAT_OPTIONS
        ],
    )
}

// --- tests ------------------------------------------------------------------

#[test]
fn emule072a_hello_answer_decodes_all_capabilities() {
    let payload = emule072a_hello_answer();
    let ans = decode_hello_answer(
        &Ed2kLimits::default(),
        PR_ED2K,
        OP_HELLO_ANSWER,
        &payload,
        0,
    )
    .expect("eMule 0.72a hello answer should decode");
    let info = Ed2kPeerInfo::from_hello_answer(&ans);

    assert_eq!(info.name.as_deref(), Some("eMule peer"));
    assert_eq!(info.version, Some(0x3C));
    // CT_EMULE_VERSION arrives as tag 0xFB — must be recognised, not lost.
    assert_eq!(info.emule_version, Some(EMULE072A_VERSION));
    assert_eq!(info.software_id(), Some(0));
    assert_eq!(info.software_name(), "eMule");
    assert_eq!(info.emule_version_string().as_deref(), Some("0.72a"));

    assert!(info.is_emule());
    assert!(info.supports_aich());
    assert!(info.supports_source_exchange());
    assert!(info.supports_source_exchange2());
    assert!(info.supports_large_files());
    assert_eq!(info.secure_ident_support(), 3);
    assert_eq!(info.udp_version(), 4);
    assert_eq!(info.extended_requests_version(), 2);
    assert_eq!(info.kad_version(), 10);
    assert!(info.supports_crypt_layer() && info.requests_crypt_layer());
    assert!(info.emule_udp_port.is_some());
    assert!(info.unknown_tags.is_empty());
}

#[test]
fn amule301_hello_answer_identifies_software_and_preserves_compat_tag() {
    let payload = amule301_hello_answer();
    let ans = decode_hello_answer(
        &Ed2kLimits::default(),
        PR_ED2K,
        OP_HELLO_ANSWER,
        &payload,
        0,
    )
    .expect("aMule 3.0.1 hello answer should decode");
    let info = Ed2kPeerInfo::from_hello_answer(&ans);

    assert_eq!(info.name.as_deref(), Some("aMule 3.0.1"));
    assert_eq!(info.software_id(), Some(3));
    assert_eq!(info.software_name(), "aMule");
    // update field 1 renders as suffix 'b' (eMule letter encoding).
    assert_eq!(info.emule_version_string().as_deref(), Some("3.0b"));

    assert!(info.is_emule());
    assert_eq!(info.extended_requests_version(), 2);
    assert_eq!(info.kad_version(), 8);
    assert!(info.supports_large_files());

    // CT_EMULECOMPAT_OPTIONS (0xEF) must be preserved among the decoded tags.
    assert!(info
        .tags
        .iter()
        .any(|t| matches!(t.name, mule_protocol::ed2k::tags::TagName::Id(0xEF))));
}

#[test]
fn emule072a_emuleinfo_packet_decodes() {
    // 7-tag EMULEINFO as sent by eMule 0.72a SendMuleInfoPacket.
    let payload = emule_info_payload(
        0x48, // version short for 0.72a
        0x01,
        &[
            t_u8(0x20, 1),    // ET_COMPRESSION
            t_u8(0x22, 4),    // ET_UDPVER
            t_u8(0x21, 0xC4), // ET_UDPPORT (4662 low byte; u8 tag on wire)
            t_u8(0x23, 4),    // ET_SOURCEEXCHANGE
            t_u8(0x24, 1),    // ET_COMMENTS
            t_u8(0x25, 2),    // ET_EXTENDEDREQUEST
            t_u8(0x27, 3),    // ET_FEATURES (sec ident v1|v2)
        ],
    );

    let decoder = Ed2kDecoder::default();
    let info = decoder
        .decode_emule_info(PR_EMULE, OP_EMULE_INFO, &payload, 0)
        .expect("eMule 0.72a OP_EMULEINFO should decode");

    assert_eq!(info.client_version, 0x48);
    assert_eq!(info.protocol_version, 0x01);
    assert_eq!(info.tags.len(), 7);
    assert!(info.has_compression_capability());
    assert!(info.has_source_exchange_capability());
    assert!(info.unknown_tags.is_empty());
}

#[test]
fn amule301_emuleinfo_with_compat_and_modversion_decodes() {
    // aMule's 9-tag EMULEINFO adds ET_COMPATIBLECLIENT (0x26) and
    // ET_MOD_VERSION — the latter is a *named* string tag ("fv"), encoded in
    // the old tag format: [type][u16 name_len][name][u16 value_len][value].
    let mut modver = vec![0x02]; // TAGTYPE_STRING, no 0x80 → named tag
    modver.extend_from_slice(&2u16.to_le_bytes());
    modver.extend_from_slice(b"fv");
    modver.extend_from_slice(&5u16.to_le_bytes());
    modver.extend_from_slice(b"aMule");

    let payload = emule_info_payload(
        0x1E, // aMule version short
        0x01,
        &[
            t_u8(0x20, 1),
            t_u8(0x22, 4),
            t_u8(0x21, 0xC5),
            t_u8(0x23, 3), // aMule source exchange v3
            t_u8(0x24, 1),
            t_u8(0x25, 2),
            t_u8(0x27, 3),
            t_u8(0x26, 3), // ET_COMPATIBLECLIENT = SO_AMULE
            modver,
        ],
    );

    let decoder = Ed2kDecoder::default();
    let info = decoder
        .decode_emule_info(PR_EMULE, OP_EMULE_INFO, &payload, 0)
        .expect("aMule OP_EMULEINFO should decode");

    assert_eq!(info.tags.len(), 9);
    assert!(info.has_compression_capability());
    // Nothing may be silently dropped: every tag is typed or preserved.
    assert_eq!(info.tags.len() + info.unknown_tags.len(), 9);
}

#[test]
fn emule_info_roundtrip_and_answer_dispatch() {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    let info = mule_protocol::ed2k::Ed2kEmuleInfo {
        client_version: 70,
        protocol_version: 0x01,
        tags: vec![mule_protocol::ed2k::tags::Ed2kTag {
            name: mule_protocol::ed2k::tags::TagName::Id(0x25),
            value: mule_protocol::ed2k::tags::TagValue::UInt32(2),
        }],
        unknown_tags: vec![],
    };

    // OP_EMULEINFO roundtrip through the full packet dispatcher.
    let pkt = encode_emule_info(&Ed2kLimits::default(), &info).unwrap();
    let mut frame = Vec::new();
    frame.push(pkt.protocol);
    frame.extend_from_slice(&(pkt.frame_len() as u32).to_le_bytes());
    frame.push(pkt.opcode);
    frame.extend_from_slice(&pkt.payload);

    let msg = decoder
        .decode_packet(&frame)
        .expect("decode framed EMULEINFO");
    let Ed2kMessage::EmuleInfo(decoded) = msg else {
        panic!("expected EmuleInfo message");
    };
    assert_eq!(decoded.client_version, 70);
    assert_eq!(decoded.tags.len(), 1);

    // The answer opcode must reach the same decoder through the dispatcher.
    let answer_pkt = encoder.encode_emule_info_answer(&info).unwrap();
    let mut frame = Vec::new();
    frame.push(answer_pkt.protocol);
    frame.extend_from_slice(&(answer_pkt.frame_len() as u32).to_le_bytes());
    frame.push(answer_pkt.opcode);
    frame.extend_from_slice(&answer_pkt.payload);

    let msg = decoder
        .decode_packet(&frame)
        .expect("decode framed EMULEINFO_ANSWER");
    assert!(matches!(msg, Ed2kMessage::EmuleInfo(_)));
}

#[test]
fn request_parts_i64_roundtrip_beyond_4gib() {
    let encoder = Ed2kEncoder::default();
    let hash = [0x11u8; 16];
    let start = u32::MAX as u64 + 0x10_0000; // past 4 GiB
    let end = start + 184_320;

    let req = Ed2kRequestPartsI64 {
        file_hash: hash,
        ranges: vec![Ed2kBlockRange64::new(start, end)],
    };

    let pkt = encoder.encode_request_parts_i64(&req).unwrap();
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_REQUESTPARTS_I64);
    assert_eq!(pkt.payload.len(), 64); // hash16 + 3*u64 + 3*u64

    let decoded = decode_request_parts_i64(PR_ED2K, OP_REQUESTPARTS_I64, &pkt.payload, 0).unwrap();
    assert_eq!(decoded.file_hash, hash);
    assert_eq!(decoded.ranges.len(), 1);
    assert_eq!(decoded.ranges[0].start, start);
    assert_eq!(decoded.ranges[0].end, end);
}

#[test]
fn sending_part_i64_roundtrip_beyond_4gib() {
    let encoder = Ed2kEncoder::default();
    let hash = [0x22u8; 16];
    let start = u32::MAX as u64 + 0x20_0000;
    let data = bytes::Bytes::from(vec![0x55u8; 1024]);

    let part = mule_protocol::ed2k::Ed2kSendingPartI64 {
        file_hash: hash,
        start_offset: start,
        end_offset: start + data.len() as u64,
        data: data.clone(),
    };

    let pkt = encoder.encode_sending_part_i64(&part).unwrap();
    let decoded = decode_sending_part_i64(PR_ED2K, pkt.opcode, &pkt.payload, 0).unwrap();
    assert_eq!(decoded.start_offset, start);
    assert_eq!(decoded.end_offset, start + 1024);
    assert_eq!(decoded.data, data);
}

/// Real peer on the live network (observed: LowID client announcing
/// `emule_version 2.7c`) sends OP_FILESTATUS with `part_count = 0` (complete
/// file) plus one trailing padding byte. eMule ignores trailing bytes —
/// strict rejection used to drop the whole session.
#[test]
fn filestatus_complete_file_tolerates_trailing_bytes() {
    let decoder = Ed2kDecoder::default();
    let mut payload = Vec::new();
    payload.extend_from_slice(&[0x33u8; 16]); // file hash
    payload.extend_from_slice(&0u16.to_le_bytes()); // part_count = 0 → complete
    payload.push(0x00); // trailing byte sent by the real client

    let mut frame = vec![PR_ED2K];
    frame.extend_from_slice(&((payload.len() + 1) as u32).to_le_bytes());
    frame.push(0x50); // OP_FILESTATUS
    frame.extend_from_slice(&payload);

    let msg = decoder.decode_packet(&frame).expect("decode filestatus");
    let Ed2kMessage::FileStatus(status) = msg else {
        panic!("expected FileStatus");
    };
    assert_eq!(status.file_hash, [0x33u8; 16]);
    assert_eq!(status.part_count, 0);
}
