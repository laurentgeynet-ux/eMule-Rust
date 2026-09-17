//! eMule Source Exchange (SX / SX2) protocol implementation.
//!
//! Provides deterministic wire encoding and strictly bounded decoding for:
//! - `OP_REQUESTSOURCES` (0x81, legacy SX1)
//! - `OP_REQUESTSOURCES2` (0x83, canonical SX2)
//! - `OP_ANSWERSOURCES` (0x82, legacy SX1 response)
//! - `OP_ANSWERSOURCES2` (0x84, canonical SX2 response)
//!
//! Supports both raw `PR_EMULE` (0xC5) and zlib-compressed `PR_EMULE_COMPRESSED` (0xD4)
//! payloads according to the canonical eMule specification.

use std::io::Read;

use bytes::Bytes;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use super::error::{Ed2kEncodeError, Ed2kErrorKind, Ed2kParseError};
use super::opcodes::{
    OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_REQUESTSOURCES, OP_REQUESTSOURCES2,
};
use super::protocol::{PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED};
use crate::framing::RawPacket;
use crate::limits::HEADER_SIZE;

/// Default version of Source Exchange 2 implemented by modern eMule clients.
pub const SOURCE_EXCHANGE2_VERSION: u8 = 4;

/// Maximum number of sources allowed in a single SX response packet to prevent DoS.
pub const MAX_SX_SOURCES_PER_PACKET: usize = 1000;

/// Request for sources of a given file hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kRequestSources {
    /// 16-byte MD4 hash of the requested file.
    pub file_hash: [u8; 16],
    /// Requested SX version (e.g. 4 for SX2, or 1 for legacy).
    pub version: u8,
    /// Requested SX options (reserved, typically 0).
    pub options: u16,
}

impl Ed2kRequestSources {
    /// Creates a standard SX2 source request for a file hash.
    pub fn new_v2(file_hash: [u8; 16]) -> Self {
        Self {
            file_hash,
            version: SOURCE_EXCHANGE2_VERSION,
            options: 0,
        }
    }

    /// Creates a legacy SX1 source request for a file hash.
    pub fn new_v1(file_hash: [u8; 16]) -> Self {
        Self {
            file_hash,
            version: 1,
            options: 0,
        }
    }
}

/// Representation of a single peer source transmitted over Source Exchange.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SxSource {
    /// IPv4 address as 4 raw octets [A, B, C, D] in standard network order.
    pub ip_bytes: [u8; 4],
    /// Little-endian integer representation of IPv4 address matching the eD2K standard.
    pub ip: u32,
    /// Client TCP listening port.
    pub port: u16,
    /// Connected server IPv4 address as u32.
    pub server_ip: u32,
    /// Connected server port.
    pub server_port: u16,
    /// Remote user hash MD4, if available (SX v2+).
    pub user_hash: Option<[u8; 16]>,
    /// Protocol obfuscation and encryption options (SX v4):
    /// - bit 0: supports protocol encryption layer
    /// - bit 1: requests protocol encryption layer
    /// - bit 2: requires protocol encryption layer
    pub crypt_options: u8,
}

impl SxSource {
    /// Creates a source entry with IPv4 octets, port, user hash, and crypt options.
    pub fn new(
        ip_bytes: [u8; 4],
        port: u16,
        user_hash: Option<[u8; 16]>,
        crypt_options: u8,
    ) -> Self {
        let ip = u32::from_le_bytes(ip_bytes);
        Self {
            ip_bytes,
            ip,
            port,
            server_ip: 0,
            server_port: 0,
            user_hash,
            crypt_options,
        }
    }

    /// Returns the IPv4 address string (e.g. "192.168.1.1").
    pub fn ip_string(&self) -> String {
        format!(
            "{}.{}.{}.{}",
            self.ip_bytes[0], self.ip_bytes[1], self.ip_bytes[2], self.ip_bytes[3]
        )
    }

    /// Returns the socket address string (e.g. "192.168.1.1:4662").
    pub fn address_string(&self) -> String {
        format!("{}:{}", self.ip_string(), self.port)
    }

    /// Returns true if this source is a LowID (< 16_777_216).
    pub fn is_low_id(&self) -> bool {
        self.ip < 16_777_216
    }
}

/// Response containing sources for a given file hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kAnswerSources {
    /// SX version of the packet (e.g. 4 for SX2 v4, or 1..3).
    pub version: u8,
    /// 16-byte MD4 hash of the file.
    pub file_hash: [u8; 16],
    /// Discovered or shared peer sources.
    pub sources: Vec<SxSource>,
}

impl Ed2kAnswerSources {
    /// Creates a new SX2 answer payload.
    pub fn new(file_hash: [u8; 16], sources: Vec<SxSource>) -> Self {
        Self {
            version: SOURCE_EXCHANGE2_VERSION,
            file_hash,
            sources,
        }
    }
}

/// Encodes an `Ed2kRequestSources` into a wire `RawPacket`.
pub fn encode_request_sources(req: &Ed2kRequestSources) -> Result<RawPacket, Ed2kEncodeError> {
    if req.version >= 2 {
        // OP_REQUESTSOURCES2: <Version 1><Options 2><HASH 16> (19 bytes payload)
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
        // OP_REQUESTSOURCES: <HASH 16> (16 bytes payload)
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

/// Encodes an `Ed2kAnswerSources` into an `OP_ANSWERSOURCES2` wire `RawPacket`.
/// Automatically compresses the payload with zlib if it exceeds 354 bytes or if `force_compress` is true.
pub fn encode_answer_sources(
    ans: &Ed2kAnswerSources,
    force_compress: bool,
) -> Result<RawPacket, Ed2kEncodeError> {
    let source_count = ans.sources.len().min(MAX_SX_SOURCES_PER_PACKET);
    let mut raw_buf = Vec::with_capacity(19 + source_count * 29);

    // 1. Version byte
    raw_buf.push(ans.version);
    // 2. File hash (16 bytes)
    raw_buf.extend_from_slice(&ans.file_hash);
    // 3. Count (2 bytes, LE)
    raw_buf.extend_from_slice(&(source_count as u16).to_le_bytes());

    for src in &ans.sources[..source_count] {
        // In SX2 v3+, the ID on wire is the IP octets written in network byte order:
        // byte 0..3: src.ip_bytes
        raw_buf.extend_from_slice(&src.ip_bytes);
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
                OP_ANSWERSOURCES2,
                "zlib_compression",
                Ed2kErrorKind::MalformedField {
                    message: format!("Zlib compression error: {e}"),
                },
            )
        })?;
        let compressed = encoder.finish().map_err(|e| {
            Ed2kEncodeError::new(
                OP_ANSWERSOURCES2,
                "zlib_finish",
                Ed2kErrorKind::MalformedField {
                    message: format!("Zlib finish error: {e}"),
                },
            )
        })?;

        Ok(RawPacket {
            protocol: PR_EMULE_COMPRESSED,
            opcode: OP_ANSWERSOURCES2,
            payload: Bytes::from(compressed),
        })
    } else {
        Ok(RawPacket {
            protocol: PR_EMULE,
            opcode: OP_ANSWERSOURCES2,
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

    // Handle decompression if framed with PR_EMULE_COMPRESSED (0xD4)
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
        // Determine version from remaining length
        let remaining = data.len() - 18;
        let v = if cnt > 0 && remaining == (cnt as usize) * 29 {
            4
        } else if cnt > 0 && remaining == (cnt as usize) * 28 {
            2
        } else {
            1
        };
        (v, h, cnt as usize, 18)
    };

    let entry_size = match version {
        v if v >= 4 => 29,
        v if v >= 2 => 28,
        _ => 12,
    };

    let needed_len = cursor + count * entry_size;
    if data.len() < needed_len {
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
        let ip_bytes = [
            data[cursor],
            data[cursor + 1],
            data[cursor + 2],
            data[cursor + 3],
        ];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_sources_roundtrip_v2() {
        let hash = [0x42u8; 16];
        let req = Ed2kRequestSources::new_v2(hash);
        let packet = encode_request_sources(&req).expect("encode req");

        assert_eq!(packet.protocol, PR_EMULE);
        assert_eq!(packet.opcode, OP_REQUESTSOURCES2);
        assert_eq!(packet.payload.len(), 19);

        let decoded = decode_request_sources(packet.protocol, packet.opcode, &packet.payload)
            .expect("decode req");
        assert_eq!(decoded.file_hash, hash);
        assert_eq!(decoded.version, SOURCE_EXCHANGE2_VERSION);
        assert_eq!(decoded.options, 0);
    }

    #[test]
    fn test_request_sources_roundtrip_v1() {
        let hash = [0x55u8; 16];
        let req = Ed2kRequestSources::new_v1(hash);
        let packet = encode_request_sources(&req).expect("encode req v1");

        assert_eq!(packet.protocol, PR_EMULE);
        assert_eq!(packet.opcode, OP_REQUESTSOURCES);
        assert_eq!(packet.payload.len(), 16);

        let decoded = decode_request_sources(packet.protocol, packet.opcode, &packet.payload)
            .expect("decode req v1");
        assert_eq!(decoded.file_hash, hash);
        assert_eq!(decoded.version, 1);
    }

    #[test]
    fn test_answer_sources_roundtrip_uncompressed() {
        let hash = [0x77u8; 16];
        let src1 = SxSource::new([192, 168, 1, 100], 4662, Some([0xAA; 16]), 0x01);
        let src2 = SxSource::new([10, 0, 0, 50], 4672, Some([0xBB; 16]), 0x07);

        let ans = Ed2kAnswerSources::new(hash, vec![src1.clone(), src2.clone()]);
        let packet = encode_answer_sources(&ans, false).expect("encode answer");

        assert_eq!(packet.protocol, PR_EMULE);
        assert_eq!(packet.opcode, OP_ANSWERSOURCES2);

        let decoded = decode_answer_sources(packet.protocol, packet.opcode, &packet.payload)
            .expect("decode answer");
        assert_eq!(decoded.file_hash, hash);
        assert_eq!(decoded.version, 4);
        assert_eq!(decoded.sources.len(), 2);
        assert_eq!(decoded.sources[0].ip_bytes, [192, 168, 1, 100]);
        assert_eq!(decoded.sources[0].port, 4662);
        assert_eq!(decoded.sources[0].user_hash, Some([0xAA; 16]));
        assert_eq!(decoded.sources[0].crypt_options, 0x01);
        assert_eq!(decoded.sources[1].ip_bytes, [10, 0, 0, 50]);
        assert_eq!(decoded.sources[1].crypt_options, 0x07);
    }

    #[test]
    fn test_answer_sources_roundtrip_compressed() {
        let hash = [0x88u8; 16];
        // Create 20 sources so that payload triggers or test force_compress
        let mut sources = Vec::new();
        for i in 0..20 {
            sources.push(SxSource::new(
                [172, 16, 0, i as u8],
                4000 + i as u16,
                Some([i as u8; 16]),
                0x03,
            ));
        }

        let ans = Ed2kAnswerSources::new(hash, sources.clone());
        let packet = encode_answer_sources(&ans, true).expect("encode compressed answer");

        assert_eq!(packet.protocol, PR_EMULE_COMPRESSED);
        assert_eq!(packet.opcode, OP_ANSWERSOURCES2);

        let decoded = decode_answer_sources(packet.protocol, packet.opcode, &packet.payload)
            .expect("decode compressed answer");
        assert_eq!(decoded.file_hash, hash);
        assert_eq!(decoded.sources.len(), 20);
        assert_eq!(decoded.sources[5].port, 4005);
        assert_eq!(decoded.sources[5].ip_bytes, [172, 16, 0, 5]);
    }
}
