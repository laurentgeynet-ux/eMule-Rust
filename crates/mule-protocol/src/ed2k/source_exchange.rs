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

pub mod codec;
pub mod types;

pub use codec::{
    decode_answer_sources, decode_request_sources, encode_answer_sources, encode_request_sources,
};
pub use types::{
    Ed2kAnswerSources, Ed2kRequestSources, SxSource, MAX_SX_SOURCES_PER_PACKET,
    SOURCE_EXCHANGE1_MAX_VERSION, SOURCE_EXCHANGE2_VERSION,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ed2k::opcodes::{
        OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_REQUESTSOURCES, OP_REQUESTSOURCES2,
    };
    use crate::ed2k::protocol::{PR_EMULE, PR_EMULE_COMPRESSED};

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
        let packet = encode_answer_sources(&ans, true, false).expect("encode answer");

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
    fn test_answer_sources_sx1_envelope_and_endianness() {
        // eMule `CreateSrcInfoPacket` for SX1: `<hash16><count u16>` — no
        // version byte on the wire; v3+ entries use hybrid (little-endian)
        // IPs, v<3 entries use `htonl` (`PartFile.cpp:3670-3691`).
        let hash = [0x33u8; 16];
        let src = SxSource::new([80, 10, 20, 30], 4662, Some([0xCC; 16]), 0x03);

        // SX1 v3 (negotiated `ET_SOURCEEXCHANGE = 3`): 28-byte entries.
        let ans = Ed2kAnswerSources::new_v(3, hash, vec![src.clone()]);
        let packet = encode_answer_sources(&ans, false, false).expect("encode sx1 v3");
        assert_eq!(packet.protocol, PR_EMULE);
        assert_eq!(packet.opcode, OP_ANSWERSOURCES);
        assert_eq!(packet.payload.len(), 18 + 28);

        let decoded = decode_answer_sources(packet.protocol, packet.opcode, &packet.payload)
            .expect("decode sx1 v3");
        assert_eq!(decoded.version, 3);
        assert_eq!(decoded.sources[0].ip_bytes, [80, 10, 20, 30]);
        assert_eq!(decoded.sources[0].port, 4662);
        assert_eq!(decoded.sources[0].user_hash, Some([0xCC; 16]));
        assert_eq!(decoded.sources[0].crypt_options, 0);

        // SX1 v1: 12-byte entries, IP travels in network byte order.
        let ans_v1 = Ed2kAnswerSources::new_v(1, hash, vec![src.clone()]);
        let packet_v1 = encode_answer_sources(&ans_v1, false, false).expect("encode sx1 v1");
        assert_eq!(packet_v1.payload.len(), 18 + 12);
        // First entry bytes are the reversed IP (htonl of the hybrid ID).
        assert_eq!(&packet_v1.payload[18..22], &[30, 20, 10, 80]);

        let decoded_v1 =
            decode_answer_sources(packet_v1.protocol, packet_v1.opcode, &packet_v1.payload)
                .expect("decode sx1 v1");
        assert_eq!(decoded_v1.version, 1);
        assert_eq!(decoded_v1.sources[0].ip_bytes, [80, 10, 20, 30]);
        assert_eq!(decoded_v1.sources[0].user_hash, None);
    }

    #[test]
    fn test_answer_sources_rejects_unknown_sx2_version() {
        // eMule drops SX2 answers with version 0 or > SOURCEEXCHANGE2_VERSION
        // (`PartFile.cpp:3732-3740`).
        let mut payload = vec![0x05u8];
        payload.extend_from_slice(&[0x11u8; 16]);
        payload.extend_from_slice(&0u16.to_le_bytes());
        assert!(decode_answer_sources(PR_EMULE, OP_ANSWERSOURCES2, &payload).is_err());

        payload[0] = 0x00;
        assert!(decode_answer_sources(PR_EMULE, OP_ANSWERSOURCES2, &payload).is_err());
    }

    #[test]
    fn test_answer_sources_rejects_trailing_garbage_sx2() {
        // eMule requires `count * entry_size == remaining` for SX2
        // (`PartFile.cpp:3743-3770`).
        let mut payload = vec![0x04u8];
        payload.extend_from_slice(&[0x22u8; 16]);
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.extend_from_slice(&[0xEEu8; 3]); // trailing garbage
        assert!(decode_answer_sources(PR_EMULE, OP_ANSWERSOURCES2, &payload).is_err());
    }

    #[test]
    fn test_answer_sources_rejects_mismatched_sx1_size() {
        // eMule drops SX1 packets whose trailing size matches none of the
        // known entry sizes (`PartFile.cpp:3777-3827`).
        let mut payload = Vec::new();
        payload.extend_from_slice(&[0x44u8; 16]);
        payload.extend_from_slice(&2u16.to_le_bytes());
        payload.extend_from_slice(&[0x55u8; 20]); // neither 12|28|29 * 2
        assert!(decode_answer_sources(PR_EMULE, OP_ANSWERSOURCES, &payload).is_err());
    }

    #[test]
    fn test_answer_sources_roundtrip_compressed() {
        let hash = [0x88u8; 16];
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
        let packet = encode_answer_sources(&ans, true, true).expect("encode compressed answer");

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
