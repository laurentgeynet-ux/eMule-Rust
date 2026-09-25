// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Peer-to-peer shared-files browsing (eMule `ListenSocket.cpp:651-825`).
//!
//! A connected peer can browse our shared files through three PR_ED2K
//! requests when `MISCOPTIONS1` bit 2 (`NoViewSharedFiles`) is clear:
//!
//! - `OP_ASKSHAREDFILES` (`0x4A`, empty) → [`encode_shared_files_answer`]
//!   returns the complete list (`0x4B`).
//! - `OP_ASKSHAREDDIRS` (`0x5D`, empty) → [`encode_shared_dirs_answer`]
//!   returns the directory list (`0x5F`); denied → [`encode_shared_denied`].
//! - `OP_ASKSHAREDFILESDIR` (`0x5E`, `<u16 len><dir>`) →
//!   [`encode_shared_files_dir_answer`] returns `0x60` with the files of the
//!   pseudo-directory; denied → [`encode_shared_denied`].
//!
//! All answers are framed on PR_ED2K (eMule `Packet` defaults to
//! `OP_EDONKEYPROT` — `Packets.h:49`). The answer opcodes `0x4B`, `0x60` and
//! `0x61` collide with `OP_REQUESTPARTS`, `OP_QUEUERANKING` and `OP_FILEDESC`
//! respectively; eMule resolves the ambiguity by direction (answers always
//! flow share-host → browser) and by protocol byte (`QUEUERANKING`/`FILEDESC`
//! are PR_EMULE-only).
//!
//! Entries reuse the `OfferedFile` layout of `OP_OFFERFILES`
//! (`CSharedFileList::CreateOfferedFilePacket` with `pServer == NULL`):
//! `hash[16] + client_id u32 + tcp_port u16 + tag list`. For clients the
//! ID/port are our real address only on HighID, and large files carry a
//! single 64-bit `FT_FILESIZE` tag instead of the split u32 pair used for
//! servers.

use crate::ed2k::encoder::Ed2kEncoder;
use crate::ed2k::error::Ed2kEncodeError;
use crate::ed2k::limits::Ed2kLimits;
use crate::ed2k::opcodes::{
    OP_ASKSHAREDDENIEDANS, OP_ASKSHAREDDIRSANS, OP_ASKSHAREDFILESDIRANS,
    OP_ASKSHAREDFILESANSWER,
};
use crate::ed2k::protocol::PR_ED2K;
use crate::ed2k::server::OfferedFile;
use crate::ed2k::tags::{Ed2kTag, TagName, TagValue, TAG_SPECIAL_FILESIZE, TAG_SPECIAL_FILETYPE};
use crate::framing::RawPacket;

/// Pseudo-directory listing the in-progress part files
/// (eMule `OP_INCOMPLETE_SHARED_FILES`, `opcodes.h:241`).
pub const OP_INCOMPLETE_SHARED_FILES: &str = "!Incomplete Files";

/// Pseudo-directory for shares not under any shared directory
/// (eMule `OP_OTHER_SHARED_FILES`, `opcodes.h:242`).
pub const OP_OTHER_SHARED_FILES: &str = "!Other";

/// `FT_FILERATING` (`0xF7`, `<uint8>`) — eMule `opcodes.h:414`.
/// For client-bound answers the 0..5 rating is scaled to 0..255
/// (`SharedFileList.cpp:960`: `rating * 255/5`).
const FT_FILERATING: u8 = 0xF7;

/// eMule `WriteString` wire form: `<u16 len><utf8 bytes>`.
/// `MAX_CLIENT_MSG_LEN`-style safety: directory strings are bounded by the
/// generic frame limit anyway; a bogus huge length just fails the bounds check.
fn write_str(dst: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(u16::MAX as usize) as u16;
    dst.extend_from_slice(&len.to_le_bytes());
    dst.extend_from_slice(&bytes[..len as usize]);
}

/// Connection context deciding what a browse answer reveals about us
/// (`CreateOfferedFilePacket` peer variant, `SharedFileList.cpp:883` with
/// `pServer == NULL`).
#[derive(Clone, Copy, Debug)]
pub struct OfferPeerContext {
    /// Our eD2K client ID (sent only when `our_high_id` is set).
    pub our_client_id: u32,
    /// Our listening TCP port (sent only when `our_high_id` is set).
    pub our_tcp_port: u16,
    /// Whether we currently hold a HighID — LowID writes `0`/`0`.
    pub our_high_id: bool,
    /// Whether the *browsing* peer announced large-file support
    /// (misc2 bit 4) — gates 64-bit `FT_FILESIZE` and the entry itself.
    pub peer_large_files: bool,
}

/// Builds the peer variant of `CreateOfferedFilePacket`
/// (`SharedFileList.cpp:883` with `pServer == NULL`):
///
/// - `client_id`/`client_port` carry our real HighID + TCP port when we are
///   connected and not firewalled; `0`/`0` otherwise;
/// - `FT_FILESIZE` is a single `TAGTYPE_UINT64` tag when the file is large
///   **and** the peer announced large-file support (misc2 bit 4);
/// - `FT_FILERATING` is sent when `rating > 0`, scaled `r * 51`;
/// - `FT_FILETYPE` string like for servers.
///
/// Returns `None` when the file is large but the peer does not support
/// large files (eMule skips the entry entirely — the ASSERT path).
pub fn offered_file_for_peer(
    file_hash: [u8; 16],
    file_name: &str,
    file_size: u64,
    rating: u8,
    ctx: &OfferPeerContext,
) -> Option<OfferedFile> {
    const LARGE_FILE: u64 = 0xFFFF_FFFF;
    let is_large = file_size > LARGE_FILE;
    if is_large && !ctx.peer_large_files {
        return None;
    }

    let (client_id, client_port) = if ctx.our_high_id {
        (ctx.our_client_id, ctx.our_tcp_port)
    } else {
        (0, 0)
    };

    let mut tags = vec![Ed2kTag {
        name: TagName::Id(crate::ed2k::tags::TAG_SPECIAL_NAME),
        value: TagValue::String(file_name.to_string()),
    }];
    if is_large {
        tags.push(Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_FILESIZE),
            value: TagValue::UInt64(file_size),
        });
    } else {
        tags.push(Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_FILESIZE),
            value: TagValue::UInt32(file_size as u32),
        });
    }
    if rating > 0 {
        // eMule `SharedFileList.cpp:960` — client answers rescale 0..5
        // into the 0..255 eserver format.
        tags.push(Ed2kTag {
            name: TagName::Id(FT_FILERATING),
            value: TagValue::UInt8((rating as u32 * 255 / 5) as u8),
        });
    }
    let ft = crate::ed2k::server::ed2k_file_type_term(file_name);
    if !ft.is_empty() {
        tags.push(Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_FILETYPE),
            value: TagValue::String(ft.to_string()),
        });
    }
    Some(OfferedFile {
        file_hash,
        client_id,
        client_port,
        tags,
    })
}

/// Appends one offered-file entry (`hash + id + port + tags`) to `buf`.
fn write_offered_entry(
    encoder: &Ed2kEncoder,
    opcode: u8,
    buf: &mut Vec<u8>,
    f: &OfferedFile,
) -> Result<(), Ed2kEncodeError> {
    buf.extend_from_slice(&f.file_hash);
    buf.extend_from_slice(&f.client_id.to_le_bytes());
    buf.extend_from_slice(&f.client_port.to_le_bytes());
    buf.extend_from_slice(&(f.tags.len() as u32).to_le_bytes());
    for tag in &f.tags {
        encoder.encode_tag(opcode, tag, buf)?;
    }
    Ok(())
}

fn finish_frame(
    opcode: u8,
    buf: Vec<u8>,
    limits: &Ed2kLimits,
) -> Result<RawPacket, Ed2kEncodeError> {
    let payload_len = buf.len() + 1;
    if payload_len > limits.max_frame_bytes {
        return Err(Ed2kEncodeError::new(
            opcode,
            "shared_files_payload",
            crate::ed2k::error::Ed2kErrorKind::FrameTooLarge {
                size: payload_len,
                limit: limits.max_frame_bytes,
            },
        ));
    }
    Ok(RawPacket::with_vec(PR_ED2K, opcode, buf))
}

/// `OP_ASKSHAREDFILESANSWER` (`0x4B`, PR_ED2K):
/// `<u32 count>` + offered-file entries (`ListenSocket.cpp:676`).
/// A denied request still gets this answer — with `count == 0`
/// (eMule sends the empty list instead of `ASKSHAREDDENIEDANS`).
pub fn encode_shared_files_answer(
    files: &[OfferedFile],
    limits: &Ed2kLimits,
) -> Result<RawPacket, Ed2kEncodeError> {
    let encoder = Ed2kEncoder::with_limits(limits.clone());
    let mut buf = Vec::new();
    buf.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for f in files {
        write_offered_entry(&encoder, OP_ASKSHAREDFILESANSWER, &mut buf, f)?;
    }
    finish_frame(OP_ASKSHAREDFILESANSWER, buf, limits)
}

/// `OP_ASKSHAREDDIRSANS` (`0x5F`, PR_ED2K):
/// `<u32 count>(<u16 len><dir utf8>)*` (`SendSharedDirectories`,
/// `BaseClient.cpp:2916`).
pub fn encode_shared_dirs_answer(
    dirs: &[String],
    limits: &Ed2kLimits,
) -> Result<RawPacket, Ed2kEncodeError> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(dirs.len() as u32).to_le_bytes());
    for d in dirs {
        write_str(&mut buf, d);
    }
    finish_frame(OP_ASKSHAREDDIRSANS, buf, limits)
}

/// `OP_ASKSHAREDFILESDIRANS` (`0x60`, PR_ED2K):
/// `<u16 len><dir utf8><u32 count>` + offered-file entries
/// (`ListenSocket.cpp:766-787`). The echoed string is the *requested*
/// pseudo name verbatim (`strOrgReqDir`), even when it resolved to nothing.
pub fn encode_shared_files_dir_answer(
    requested_dir: &str,
    files: &[OfferedFile],
    limits: &Ed2kLimits,
) -> Result<RawPacket, Ed2kEncodeError> {
    let encoder = Ed2kEncoder::with_limits(limits.clone());
    let mut buf = Vec::new();
    write_str(&mut buf, requested_dir);
    buf.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for f in files {
        write_offered_entry(&encoder, OP_ASKSHAREDFILESDIRANS, &mut buf, f)?;
    }
    finish_frame(OP_ASKSHAREDFILESDIRANS, buf, limits)
}

/// `OP_ASKSHAREDDENIEDANS` (`0x61`, PR_ED2K, empty payload) — sent when the
/// share-viewing preference denies the requester (`ListenSocket.cpp:701`).
pub fn encode_shared_denied() -> RawPacket {
    RawPacket::with_vec(PR_ED2K, OP_ASKSHAREDDENIEDANS, Vec::new())
}

/// Decodes an `OP_ASKSHAREDFILESDIR` (`0x5E`) payload `<u16 len><dir>`.
/// Returns `None` on truncation — eMule tolerates trailing garbage but the
/// length prefix must be present and in-bounds.
pub fn decode_ask_files_dir(payload: &[u8]) -> Option<String> {
    if payload.len() < 2 {
        return None;
    }
    let len = u16::from_le_bytes([payload[0], payload[1]]) as usize;
    if payload.len() < 2 + len {
        return None;
    }
    String::from_utf8(payload[2..2 + len].to_vec()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ed2k::tags::{TAG_SPECIAL_NAME, TAG_SPECIAL_FILESIZE};

    fn entry(hash: u8, id: u32, port: u16, name: &str, size: u64, large64: bool) -> OfferedFile {
        let size_tag = if large64 {
            TagValue::UInt64(size)
        } else {
            TagValue::UInt32(size as u32)
        };
        OfferedFile {
            file_hash: [hash; 16],
            client_id: id,
            client_port: port,
            tags: vec![
                Ed2kTag {
                    name: TagName::Id(TAG_SPECIAL_NAME),
                    value: TagValue::String(name.to_string()),
                },
                Ed2kTag {
                    name: TagName::Id(TAG_SPECIAL_FILESIZE),
                    value: size_tag,
                },
            ],
        }
    }

    #[test]
    fn shared_files_answer_wire_layout() {
        // eMule `ListenSocket.cpp:676` — `<u32 count><entry>*` on PR_ED2K 0x4B.
        let f = entry(0xAA, 0x01020304, 39735, "x.bin", 42, false);
        let pkt = encode_shared_files_answer(&[f], &Ed2kLimits::default()).unwrap();
        assert_eq!(pkt.protocol, PR_ED2K);
        assert_eq!(pkt.opcode, 0x4B);
        assert_eq!(&pkt.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&pkt.payload[4..20], &[0xAA; 16]);
        assert_eq!(&pkt.payload[20..24], &0x01020304u32.to_le_bytes());
        assert_eq!(&pkt.payload[24..26], &39735u16.to_le_bytes());
    }

    #[test]
    fn denied_is_empty_pr_ed2k_0x61() {
        let pkt = encode_shared_denied();
        assert_eq!(pkt.protocol, PR_ED2K);
        assert_eq!(pkt.opcode, 0x61);
        assert!(pkt.payload.is_empty());
    }

    #[test]
    fn dirs_answer_strings() {
        let dirs = vec!["!Incomplete Files".to_string(), "incoming".to_string()];
        let pkt = encode_shared_dirs_answer(&dirs, &Ed2kLimits::default()).unwrap();
        assert_eq!(pkt.opcode, 0x5F);
        assert_eq!(&pkt.payload[..4], &2u32.to_le_bytes());
        // First string: len 17 + bytes
        let l = u16::from_le_bytes([pkt.payload[4], pkt.payload[5]]) as usize;
        assert_eq!(l, 17);
        assert_eq!(&pkt.payload[6..6 + l], b"!Incomplete Files");
    }

    #[test]
    fn files_dir_answer_echoes_requested_dir() {
        let pkt = encode_shared_files_dir_answer("!Other", &[], &Ed2kLimits::default()).unwrap();
        assert_eq!(pkt.opcode, 0x60);
        let l = u16::from_le_bytes([pkt.payload[0], pkt.payload[1]]) as usize;
        assert_eq!(&pkt.payload[2..2 + l], b"!Other");
        assert_eq!(
            &pkt.payload[2 + l..2 + l + 4],
            &0u32.to_le_bytes(),
            "unknown pseudo dir -> empty list (eMule sends count=0)"
        );
    }

    #[test]
    fn ask_files_dir_decode() {
        assert_eq!(decode_ask_files_dir(&[4, 0, b't', b'e', b's', b't']), Some("test".into()));
        assert_eq!(decode_ask_files_dir(&[9, 0, b's']), None);
        assert_eq!(decode_ask_files_dir(&[]), None);
        // Trailing bytes tolerated (eMule ASSERT only in debug).
        assert_eq!(
            decode_ask_files_dir(&[2, 0, b'h', b'i', 0xFF]),
            Some("hi".into())
        );
    }

    #[test]
    fn for_peer_high_id_and_large_file_rules() {
        let high = OfferPeerContext {
            our_client_id: 0x7F000001,
            our_tcp_port: 39735,
            our_high_id: true,
            peer_large_files: true,
        };
        // HighID -> real id+port; large file + peer support -> u64 tag.
        let f = offered_file_for_peer([1; 16], "big.iso", 5_000_000_000, 4, &high).unwrap();
        assert_eq!(f.client_id, 0x7F000001);
        assert_eq!(f.client_port, 39735);
        assert!(matches!(f.tags[1].value, TagValue::UInt64(5_000_000_000)));
        // rating 4 -> 4*51 = 204
        assert!(matches!(f.tags[2].value, TagValue::UInt8(204)));

        // LowID -> zeros.
        let low = OfferPeerContext {
            our_high_id: false,
            ..high
        };
        let f2 = offered_file_for_peer([1; 16], "a", 1, 0, &low).unwrap();
        assert_eq!((f2.client_id, f2.client_port), (0, 0));

        // Large file + no peer support -> skipped entirely.
        let no_large = OfferPeerContext {
            our_client_id: 0,
            our_tcp_port: 0,
            our_high_id: false,
            peer_large_files: false,
        };
        assert!(offered_file_for_peer([1; 16], "b", 5_000_000_000, 0, &no_large).is_none());
    }
}
