// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Binary codecs and packet encoders/decoders for eMule Source Exchange (SX/SX2).

use bytes::Bytes;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use std::io::Read;

use super::types::{Ed2kAnswerSources, Ed2kRequestSources, SxSource, MAX_SX_SOURCES_PER_PACKET};
use crate::ed2k::error::{Ed2kEncodeError, Ed2kErrorKind, Ed2kParseError};
use crate::ed2k::opcodes::{
    OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_REQUESTSOURCES, OP_REQUESTSOURCES2,
};
use crate::ed2k::protocol::{PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED};
use crate::framing::RawPacket;
use crate::limits::HEADER_SIZE;

/// Encodes an `Ed2kRequestSources` into a wire `RawPacket`.
pub fn encode_request_sources(req: &Ed2kRequestSources) -> Result<RawPacket, Ed2kEncodeError> {
    if req.version >= 2 {
        let mut payload = Vec::with_capacity(19);
        payload.push(req.version);
        payload.extend_from_slice(&req.options.to_le_bytes());
        payload.extend_from_slice(&req.file_hash);

        Ok(RawPacket {
            protocol: PR_EMULE,
            opcode: OP_REQUESTSOURCES2,
            payload: Bytes::from(payload),
        })
    } else {
        let mut payload = Vec::with_capacity(16);
        payload.extend_from_slice(&req.file_hash);

        Ok(RawPacket {
            protocol: PR_EMULE,
            opcode: OP_REQUESTSOURCES,
            payload: Bytes::from(payload),
        })
    }
}

/// Decodes an `Ed2kRequestSources` from incoming packet.
pub fn decode_request_sources(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<Ed2kRequestSources, Ed2kParseError> {
    if protocol != PR_EMULE && protocol != PR_ED2K {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            0,
            "protocol",
            Ed2kErrorKind::UnsupportedProtocol { byte: protocol },
        ));
    }

    match opcode {
        OP_REQUESTSOURCES2 => {
            if payload.len() < 19 {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    HEADER_SIZE + payload.len(),
                    "request_sources_payload",
                    Ed2kErrorKind::UnexpectedEof {
                        needed: 19,
                        available: payload.len(),
                    },
                ));
            }
            let version = payload[0];
            let options = u16::from_le_bytes([payload[1], payload[2]]);
            let mut file_hash = [0u8; 16];
            file_hash.copy_from_slice(&payload[3..19]);

            Ok(Ed2kRequestSources {
                file_hash,
                version,
                options,
            })
        }
        OP_REQUESTSOURCES => {
            if payload.len() < 16 {
                return Err(Ed2kParseError::new(
                    protocol,
                    Some(opcode),
                    HEADER_SIZE + payload.len(),
                    "request_sources_payload",
                    Ed2kErrorKind::UnexpectedEof {
                        needed: 16,
                        available: payload.len(),
                    },
                ));
            }
            let mut file_hash = [0u8; 16];
            file_hash.copy_from_slice(&payload[0..16]);

            Ok(Ed2kRequestSources {
                file_hash,
                version: 1,
                options: 0,
            })
        }
        _ => Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            HEADER_SIZE,
            "opcode",
            Ed2kErrorKind::MalformedField {
                message: format!("Unexpected opcode for source request: 0x{opcode:02X}"),
            },
        )),
    }
}

/// Encodes an `Ed2kAnswerSources` into a wire `RawPacket`.
///
/// `is_sx2` selects the envelope like eMule `bIsSX2Packet`
/// (`PartFile.cpp:3629-3646`):
/// - `true` → `OP_ANSWERSOURCES2` (`0x84`): `<version u8><hash16><count u16>`;
/// - `false` → `OP_ANSWERSOURCES` (`0x82`): `<hash16><count u16>` (no version
///   byte on the wire).
///
/// Each entry is `<ip u32><port u16><server_ip u32><server_port u16>` plus
/// `<user_hash 16>` when `version >= 2` and `<crypt_options u8>` when
/// `version >= 4`. For `version < 3` the IP field is written in network byte
/// order (`htonl`), while v3+ uses the little-endian "hybrid" format
/// (`PartFile.cpp:3670-3691`). Payloads larger than 354 bytes are zlib
/// packed (`PackPacket`), matching eMule exactly.
pub fn encode_answer_sources(
    ans: &Ed2kAnswerSources,
    is_sx2: bool,
    force_compress: bool,
) -> Result<RawPacket, Ed2kEncodeError> {
    let out_opcode = if is_sx2 {
        OP_ANSWERSOURCES2
    } else {
        OP_ANSWERSOURCES
    };
    let source_count = ans.sources.len().min(MAX_SX_SOURCES_PER_PACKET);
    let mut raw_buf = Vec::with_capacity(19 + source_count * 29);

    if is_sx2 {
        raw_buf.push(ans.version);
    }
    raw_buf.extend_from_slice(&ans.file_hash);
    raw_buf.extend_from_slice(&(source_count as u16).to_le_bytes());

    for src in &ans.sources[..source_count] {
        if ans.version < 3 {
            // SX1 v1/v2: IDs travel in network byte order (`htonl`).
            raw_buf.extend_from_slice(&src.ip.to_be_bytes());
        } else {
            raw_buf.extend_from_slice(&src.ip_bytes);
        }
        raw_buf.extend_from_slice(&src.port.to_le_bytes());
        raw_buf.extend_from_slice(&src.server_ip.to_le_bytes());
        raw_buf.extend_from_slice(&src.server_port.to_le_bytes());

        if ans.version >= 2 {
            raw_buf.extend_from_slice(&src.user_hash.unwrap_or([0u8; 16]));
        }
        if ans.version >= 4 {
            raw_buf.push(src.crypt_options);
        }
    }

    if force_compress || raw_buf.len() > 354 {
        use std::io::Write;
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw_buf).map_err(|e| {
            Ed2kEncodeError::new(
                out_opcode,
                "zlib_compression",
                Ed2kErrorKind::MalformedField {
                    message: format!("Zlib compression error: {e}"),
                },
            )
        })?;
        let compressed = encoder.finish().map_err(|e| {
            Ed2kEncodeError::new(
                out_opcode,
                "zlib_finish",
                Ed2kErrorKind::MalformedField {
                    message: format!("Zlib finish error: {e}"),
                },
            )
        })?;

        Ok(RawPacket {
            protocol: PR_EMULE_COMPRESSED,
            opcode: out_opcode,
            payload: Bytes::from(compressed),
        })
    } else {
        Ok(RawPacket {
            protocol: PR_EMULE,
            opcode: out_opcode,
            payload: Bytes::from(raw_buf),
        })
    }
}

/// Decodes an `Ed2kAnswerSources` packet from raw or compressed bytes.
pub fn decode_answer_sources(
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<Ed2kAnswerSources, Ed2kParseError> {
    if protocol != PR_EMULE && protocol != PR_EMULE_COMPRESSED && protocol != PR_ED2K {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            0,
            "protocol",
            Ed2kErrorKind::UnsupportedProtocol { byte: protocol },
        ));
    }

    if opcode != OP_ANSWERSOURCES2 && opcode != OP_ANSWERSOURCES {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            HEADER_SIZE,
            "opcode",
            Ed2kErrorKind::MalformedField {
                message: format!("Unexpected opcode for source answer: 0x{opcode:02X}"),
            },
        ));
    }

    let decompressed_buf: Vec<u8>;
    let data: &[u8] = if protocol == PR_EMULE_COMPRESSED {
        let mut decoder = ZlibDecoder::new(payload);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).map_err(|e| {
            Ed2kParseError::new(
                protocol,
                Some(opcode),
                HEADER_SIZE,
                "zlib_payload",
                Ed2kErrorKind::MalformedField {
                    message: format!("Zlib decompression failed: {e}"),
                },
            )
        })?;
        decompressed_buf = out;
        &decompressed_buf
    } else {
        payload
    };

    let is_sx2 = opcode == OP_ANSWERSOURCES2;
    let (version, file_hash, count, mut cursor) = if is_sx2 {
        if data.len() < 19 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                HEADER_SIZE + data.len(),
                "answer_sources_header",
                Ed2kErrorKind::UnexpectedEof {
                    needed: 19,
                    available: data.len(),
                },
            ));
        }
        let v = data[0];
        // eMule `AddClientSources`: SX2 packets with an unknown version are
        // dropped entirely (`PartFile.cpp:3732-3740`).
        if v == 0 || v > super::types::SOURCE_EXCHANGE2_VERSION {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                HEADER_SIZE,
                "answer_sources_version",
                Ed2kErrorKind::MalformedField {
                    message: format!("Unknown SX2 answer version {v}"),
                },
            ));
        }
        let mut h = [0u8; 16];
        h.copy_from_slice(&data[1..17]);
        let cnt = u16::from_le_bytes([data[17], data[18]]);
        (v, h, cnt as usize, 19)
    } else {
        if data.len() < 18 {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                HEADER_SIZE + data.len(),
                "answer_sources_header",
                Ed2kErrorKind::UnexpectedEof {
                    needed: 18,
                    available: data.len(),
                },
            ));
        }
        let mut h = [0u8; 16];
        h.copy_from_slice(&data[0..16]);
        let cnt = u16::from_le_bytes([data[16], data[17]]);
        let remaining = data.len() - 18;
        // eMule sniffs the entry size for legacy SX1 packets and drops the
        // packet when `nCount * entry_size != uDataSize`
        // (`PartFile.cpp:3777-3827`). 29-byte entries are v4, 28-byte
        // entries are v3 (hybrid IP, user hash; a pure v2 client is treated
        // like eMule does for negotiated v3), 12-byte entries are v1.
        let v = if remaining == (cnt as usize) * 29 {
            4
        } else if remaining == (cnt as usize) * 28 {
            3
        } else if remaining == (cnt as usize) * 12 {
            1
        } else {
            return Err(Ed2kParseError::new(
                protocol,
                Some(opcode),
                HEADER_SIZE + data.len(),
                "answer_sources_entries",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "SX1 answer size mismatch: count={cnt}, trailing={remaining} bytes"
                    ),
                },
            ));
        };
        (v, h, cnt as usize, 18)
    };

    let entry_size = match version {
        v if v >= 4 => 29,
        v if v >= 2 => 28,
        _ => 12,
    };

    // eMule requires an exact `count * entry_size` match for SX2 packets
    // (`PartFile.cpp:3743-3770`); trailing garbage drops the whole packet.
    let needed_len = cursor + count * entry_size;
    if is_sx2 && data.len() != needed_len {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            HEADER_SIZE + data.len(),
            "answer_sources_entries",
            Ed2kErrorKind::MalformedField {
                message: format!(
                    "SX2 answer size mismatch: count={count} needs {needed_len} bytes, got {}",
                    data.len()
                ),
            },
        ));
    }
    if !is_sx2 && data.len() < needed_len {
        return Err(Ed2kParseError::new(
            protocol,
            Some(opcode),
            HEADER_SIZE + data.len(),
            "answer_sources_entries",
            Ed2kErrorKind::UnexpectedEof {
                needed: needed_len,
                available: data.len(),
            },
        ));
    }

    let parse_count = count.min(MAX_SX_SOURCES_PER_PACKET);
    let mut sources = Vec::with_capacity(parse_count);

    for _ in 0..parse_count {
        let mut ip_bytes = [
            data[cursor],
            data[cursor + 1],
            data[cursor + 2],
            data[cursor + 3],
        ];
        // SX v1/v2 carry the ID in network byte order (`htonl`); v3+ uses the
        // little-endian "hybrid" format (`PartFile.cpp:3843`,
        // `dwIDED2K = (uPacketSXVersion < 3) ? dwID : htonl(dwID)`).
        if version < 3 {
            ip_bytes.reverse();
        }
        let ip = u32::from_le_bytes(ip_bytes);
        let port = u16::from_le_bytes([data[cursor + 4], data[cursor + 5]]);
        let server_ip = u32::from_le_bytes([
            data[cursor + 6],
            data[cursor + 7],
            data[cursor + 8],
            data[cursor + 9],
        ]);
        let server_port = u16::from_le_bytes([data[cursor + 10], data[cursor + 11]]);

        let (user_hash, crypt_options) = if version >= 4 {
            let mut uh = [0u8; 16];
            uh.copy_from_slice(&data[cursor + 12..cursor + 28]);
            let crypt = data[cursor + 28];
            (Some(uh), crypt)
        } else if version >= 2 {
            let mut uh = [0u8; 16];
            uh.copy_from_slice(&data[cursor + 12..cursor + 28]);
            (Some(uh), 0)
        } else {
            (None, 0)
        };

        sources.push(SxSource {
            ip_bytes,
            ip,
            port,
            server_ip,
            server_port,
            user_hash,
            crypt_options,
        });

        cursor += entry_size;
    }

    Ok(Ed2kAnswerSources {
        version,
        file_hash,
        sources,
    })
}
