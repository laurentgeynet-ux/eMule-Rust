use bytes::{BufMut, Bytes, BytesMut};
use mule_protocol::{
    FramingError, PacketCodec, RawPacket, MAX_BUFFERED_BYTES, MAX_FRAME_BYTES, PR_ED2K,
};

#[test]
fn test_oversized_packet_encoding_rejected() {
    let codec = PacketCodec::new();
    // Payload exceeding MAX_FRAME_BYTES
    let payload = Bytes::from(vec![0u8; MAX_FRAME_BYTES]);
    let packet = RawPacket::new(PR_ED2K, 0x01, payload);

    assert_eq!(packet.frame_len(), MAX_FRAME_BYTES + 1);

    let mut buf = BytesMut::new();
    let result = codec.encode(&packet, &mut buf);
    assert!(matches!(
        result,
        Err(FramingError::FrameTooLarge { size, limit })
            if size == MAX_FRAME_BYTES + 1 && limit == MAX_FRAME_BYTES
    ));
}

#[test]
fn test_oversized_frame_header_decoding_rejected() {
    let codec = PacketCodec::new();
    let mut buf = BytesMut::new();
    buf.put_u8(PR_ED2K);
    // Declare a length exceeding MAX_FRAME_BYTES by 1
    buf.put_u32_le((MAX_FRAME_BYTES + 1) as u32);

    let result = codec.decode(&mut buf);
    assert!(matches!(
        result,
        Err(FramingError::FrameTooLarge { size, limit })
            if size == MAX_FRAME_BYTES + 1 && limit == MAX_FRAME_BYTES
    ));
}

#[test]
fn test_u32_max_frame_length_rejected_safely() {
    let codec = PacketCodec::new();
    let mut buf = BytesMut::new();
    buf.put_u8(PR_ED2K);
    buf.put_u32_le(u32::MAX);

    let result = codec.decode(&mut buf);
    assert!(matches!(
        result,
        Err(FramingError::FrameTooLarge { size, limit })
            if size == (u32::MAX as usize) && limit == MAX_FRAME_BYTES
    ));
}

#[test]
fn test_zero_length_frame_rejected() {
    let codec = PacketCodec::new();
    let mut buf = BytesMut::new();
    buf.put_u8(PR_ED2K);
    buf.put_u32_le(0); // Invalid: must have at least 1 byte for opcode

    let result = codec.decode(&mut buf);
    assert!(matches!(
        result,
        Err(FramingError::FrameTooSmall { size }) if size == 0
    ));
}

#[test]
fn test_buffer_overflow_rejection() {
    let codec = PacketCodec::new();
    // Exceed MAX_BUFFERED_BYTES
    let mut buf = BytesMut::from(vec![0u8; MAX_BUFFERED_BYTES + 1].as_slice());

    let result = codec.decode(&mut buf);
    assert!(matches!(
        result,
        Err(FramingError::BufferOverflow { buffered, limit })
            if buffered == MAX_BUFFERED_BYTES + 1 && limit == MAX_BUFFERED_BYTES
    ));
}

#[test]
fn test_custom_limits_respected() {
    let custom_frame_limit = 64;
    let custom_buffer_limit = 128;
    let codec = PacketCodec::with_limits(custom_frame_limit, custom_buffer_limit);

    // Frame of 65 bytes should be rejected
    let mut buf = BytesMut::new();
    buf.put_u8(PR_ED2K);
    buf.put_u32_le(65);
    let result = codec.decode(&mut buf);
    assert!(matches!(
        result,
        Err(FramingError::FrameTooLarge { size, limit })
            if size == 65 && limit == custom_frame_limit
    ));

    // Buffer of 129 bytes should overflow
    let mut overflow_buf = BytesMut::from(vec![0u8; 129].as_slice());
    let result = codec.decode(&mut overflow_buf);
    assert!(matches!(
        result,
        Err(FramingError::BufferOverflow { buffered, limit })
            if buffered == 129 && limit == custom_buffer_limit
    ));
}
