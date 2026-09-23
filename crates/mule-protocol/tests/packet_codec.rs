// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use bytes::{BufMut, Bytes, BytesMut};
use mule_protocol::{
    PacketCodec, RawPacket, ED2K_BLOCK_SIZE, MAX_FRAME_BYTES, OP_HELLO, OP_SENDINGPART, PR_ED2K,
    PR_EMULE, PR_ZLIB,
};

#[test]
fn test_roundtrip_basic_ed2k_packet() {
    let codec = PacketCodec::new();
    let original = RawPacket::with_vec(PR_ED2K, OP_HELLO, vec![1, 2, 3, 4, 5]);

    let mut encoded = BytesMut::new();
    codec
        .encode(&original, &mut encoded)
        .expect("encode succeeds");

    // Total wire len should be 5 header + 1 opcode + 5 payload = 11 bytes
    assert_eq!(encoded.len(), 11);
    assert_eq!(encoded[0], PR_ED2K);

    let decoded = codec
        .decode(&mut encoded)
        .expect("decode succeeds")
        .expect("packet should be present");

    assert_eq!(decoded, original);
    assert!(encoded.is_empty());
}

#[test]
fn test_all_supported_protocol_magic_bytes() {
    let codec = PacketCodec::new();
    for proto in [PR_ED2K, PR_EMULE, PR_ZLIB] {
        let packet = RawPacket::with_vec(proto, 0x42, vec![10, 20, 30]);
        let bytes = codec.encode_to_bytes(&packet).expect("encode");
        let mut buf = BytesMut::from(bytes.as_ref());
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded.protocol, proto);
        assert_eq!(decoded.opcode, 0x42);
        assert_eq!(decoded.payload.as_ref(), &[10, 20, 30]);
    }
}

#[test]
fn test_empty_payload_packet() {
    let codec = PacketCodec::new();
    let packet = RawPacket::empty(PR_ED2K, 0x57); // OP_OUTOFPARTS has no payload
    assert_eq!(packet.payload_len(), 0);
    assert_eq!(packet.frame_len(), 1);
    assert_eq!(packet.total_wire_len(), 6);

    let mut buf = BytesMut::new();
    codec.encode(&packet, &mut buf).unwrap();
    assert_eq!(buf.len(), 6);

    let decoded = codec.decode(&mut buf).unwrap().unwrap();
    assert_eq!(decoded, packet);
    assert!(decoded.payload.is_empty());
}

#[test]
fn test_standard_block_size_payload() {
    let codec = PacketCodec::new();
    let block_data = vec![0xAA; ED2K_BLOCK_SIZE];
    let packet = RawPacket::with_vec(PR_ED2K, OP_SENDINGPART, block_data.clone());

    let mut buf = BytesMut::new();
    codec.encode(&packet, &mut buf).unwrap();

    let decoded = codec.decode(&mut buf).unwrap().unwrap();
    assert_eq!(decoded.opcode, OP_SENDINGPART);
    assert_eq!(decoded.payload.len(), ED2K_BLOCK_SIZE);
    assert_eq!(decoded.payload.as_ref(), block_data.as_slice());
}

#[test]
fn test_streaming_fragmentation_byte_by_byte() {
    let codec = PacketCodec::new();
    let payload = vec![7, 8, 9, 10, 11, 12, 13, 14];
    let original = RawPacket::with_vec(PR_ED2K, 0x50, payload);

    let mut encoded = BytesMut::new();
    codec.encode(&original, &mut encoded).unwrap();
    let total_len = encoded.len();

    let mut accumulator = BytesMut::new();
    for i in 0..(total_len - 1) {
        accumulator.put_u8(encoded[i]);
        let result = codec
            .decode(&mut accumulator)
            .expect("should not error on partial data");
        assert!(result.is_none(), "expected None at byte offset {}", i);
    }

    // Add the final byte
    accumulator.put_u8(encoded[total_len - 1]);
    let result = codec
        .decode(&mut accumulator)
        .expect("should decode successfully");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), original);
    assert!(accumulator.is_empty());
}

#[test]
fn test_multiple_packets_in_single_buffer() {
    let codec = PacketCodec::new();
    let p1 = RawPacket::with_vec(PR_ED2K, 0x01, vec![1, 2]);
    let p2 = RawPacket::with_vec(PR_EMULE, 0x02, vec![3, 4, 5, 6]);
    let p3 = RawPacket::empty(PR_ZLIB, 0x03);

    let mut buf = BytesMut::new();
    codec.encode(&p1, &mut buf).unwrap();
    codec.encode(&p2, &mut buf).unwrap();
    codec.encode(&p3, &mut buf).unwrap();

    let decoded1 = codec.decode(&mut buf).unwrap().expect("p1");
    let decoded2 = codec.decode(&mut buf).unwrap().expect("p2");
    let decoded3 = codec.decode(&mut buf).unwrap().expect("p3");
    let empty = codec.decode(&mut buf).unwrap();

    assert_eq!(decoded1, p1);
    assert_eq!(decoded2, p2);
    assert_eq!(decoded3, p3);
    assert!(empty.is_none());
    assert!(buf.is_empty());
}

#[test]
fn test_exact_max_frame_bytes_packet() {
    let codec = PacketCodec::new();
    // Frame size = 1 opcode + payload len. So max payload = MAX_FRAME_BYTES - 1.
    let payload = Bytes::from(vec![0x77; MAX_FRAME_BYTES - 1]);
    let packet = RawPacket::new(PR_ED2K, 0x01, payload);

    assert_eq!(packet.frame_len(), MAX_FRAME_BYTES);
    let mut buf = BytesMut::new();
    codec
        .encode(&packet, &mut buf)
        .expect("boundary encode should succeed");

    let decoded = codec
        .decode(&mut buf)
        .expect("decode should succeed")
        .expect("packet");
    assert_eq!(decoded.payload.len(), MAX_FRAME_BYTES - 1);
    assert_eq!(decoded.frame_len(), MAX_FRAME_BYTES);
}
