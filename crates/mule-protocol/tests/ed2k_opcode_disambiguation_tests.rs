// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Regression vectors for the `0x49`/`0x4D` opcode disambiguation (AUD-P01).
//!
//! eMule 0.72a (`srchybrid/Opcodes.h`, `ListenSocket.cpp:537/590`):
//! - `0x49` = `OP_END_OF_DOWNLOAD` (`hash16`, dequeue from upload queue),
//! - `0x4D` = `OP_CHANGE_CLIENT_ID` (`u32 newUserID + u32 newServerIP`).
//!
//! Legacy eDonkey2000 uses the same values for `OP_FILESTATUSREQ` and
//! `OP_END_OF_DOWNLOAD` respectively — disambiguation is profile-driven.
//! See `docs/ed2k/messages/op_end_of_download.md`.

use mule_protocol::ed2k::opcodes::{
    OP_CHANGE_CLIENT_ID, OP_END_OF_DOWNLOAD, OP_FILESTATUSREQ, OP_SETREQFILEID_EMULE,
};
use mule_protocol::ed2k::packet::Ed2kFileStatusReq;
use mule_protocol::ed2k::profile::Ed2kCompatibilityProfile;
use mule_protocol::ed2k::protocol::PR_ED2K;
use mule_protocol::ed2k::{Ed2kDecoder, Ed2kEncoder, Ed2kMessage};
use mule_protocol::framing::RawPacket;

const HASH: [u8; 16] = *b"JRISTERMINATED!!";

/// Spec vector: `E3 11 00 00 00 49 <hash16>`.
fn end_of_download_frame() -> Vec<u8> {
    let mut f = vec![PR_ED2K, 0x11, 0x00, 0x00, 0x00, 0x49];
    f.extend_from_slice(&HASH);
    f
}

#[test]
fn opcode_constants_match_emule_072a() {
    // eMule 0.72a Opcodes.h: OP_END_OF_DOWNLOAD=0x49, OP_CHANGE_CLIENT_ID=0x4D.
    assert_eq!(OP_END_OF_DOWNLOAD, 0x49);
    assert_eq!(OP_CHANGE_CLIENT_ID, 0x4D);
    // Legacy name kept for the eDonkey profile — same wire value.
    assert_eq!(OP_FILESTATUSREQ, 0x49);
    assert_eq!(OP_SETREQFILEID_EMULE, 0x4F);
}

#[test]
fn encoder_never_emits_0x49_as_status_req_under_emule() {
    let encoder = Ed2kEncoder::default();
    let pkt = encoder
        .encode_file_status_req(&Ed2kFileStatusReq { file_hash: HASH })
        .expect("encode file status req");
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_SETREQFILEID_EMULE);
    assert_eq!(&pkt.payload[..], &HASH);
}

#[test]
fn decoder_routes_0x49_to_raw_under_emule_profile() {
    let decoder = Ed2kDecoder::default(); // Emule072a
    let raw = RawPacket::new(PR_ED2K, OP_END_OF_DOWNLOAD, HASH.to_vec().into());
    match decoder.decode_raw_packet(&raw).expect("decode 0x49") {
        Ed2kMessage::Raw(pkt) => {
            assert_eq!(pkt.opcode, 0x49);
            assert_eq!(&pkt.payload[..], &HASH);
        }
        other => panic!("expected Raw for 0x49 under Emule072a, got {other:?}"),
    }
}

#[test]
fn decoder_routes_0x49_to_filestatusreq_under_edonkey_profile() {
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::LegacyEdonkey);
    let raw = RawPacket::new(PR_ED2K, OP_FILESTATUSREQ, HASH.to_vec().into());
    match decoder.decode_raw_packet(&raw).expect("decode 0x49") {
        Ed2kMessage::FileStatusReq(req) => assert_eq!(req.file_hash, HASH),
        other => panic!("expected FileStatusReq for 0x49 under LegacyEdonkey, got {other:?}"),
    }
}

#[test]
fn file_status_req_decoder_rejects_0x49_under_emule() {
    let decoder = Ed2kDecoder::default();
    let err = decoder
        .decode_file_status_req_with_profile(
            Ed2kCompatibilityProfile::Emule072a,
            PR_ED2K,
            0x49,
            &HASH,
            0,
        )
        .expect_err("0x49 must not decode as file status req under eMule");
    assert!(err.to_string().contains("0x49"));
}

#[test]
fn change_client_id_payload_layout() {
    // Spec vector: `E3 09 00 00 00 4D <newUserID u32><newServerIP u32>`.
    let frame = [
        PR_ED2K, 0x09, 0x00, 0x00, 0x00, OP_CHANGE_CLIENT_ID, 0x04, 0x03, 0x02, 0x01, 0x0D, 0x0C,
        0x0B, 0x0A,
    ];
    assert_eq!(frame[6..].len(), 8);
    let new_user_id = u32::from_le_bytes(frame[6..10].try_into().unwrap());
    let new_server_ip = u32::from_le_bytes(frame[10..14].try_into().unwrap());
    assert_eq!(new_user_id, 0x0102_0304);
    assert_eq!(new_server_ip, 0x0A0B_0C0D);
}

#[test]
fn end_of_download_frame_vector() {
    // Full wire frame from docs/ed2k/messages/op_end_of_download.md.
    let frame = end_of_download_frame();
    assert_eq!(&frame[..6], &[0xE3, 0x11, 0x00, 0x00, 0x00, 0x49]);
    assert_eq!(&frame[6..], &HASH);
    let decoder = Ed2kDecoder::default();
    let raw = RawPacket::new(PR_ED2K, frame[5], frame[6..].to_vec().into());
    assert!(matches!(
        decoder.decode_raw_packet(&raw).unwrap(),
        Ed2kMessage::Raw(_)
    ));
}
