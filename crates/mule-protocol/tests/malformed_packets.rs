// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use bytes::{BufMut, Bytes, BytesMut};
use mule_protocol::{
    BlockRequest, BlockWriteOutcome, DownloadPlanner, DownloadSink, FramingError, PacketCodec,
    PeerId, ProtocolError, ReceivedBlock, SourceStateUpdate, HEADER_SIZE, PR_ED2K,
};

#[test]
fn test_unsupported_protocol_bytes_rejected() {
    let codec = PacketCodec::new();
    for bad_proto in [0x00, 0x01, 0x42, 0xAA, 0xFE, 0xFF] {
        let mut buf = BytesMut::new();
        buf.put_u8(bad_proto);
        buf.put_u32_le(10);
        let result = codec.decode(&mut buf);
        assert!(matches!(
            result,
            Err(FramingError::UnsupportedProtocol { protocol }) if protocol == bad_proto
        ));
    }
}

#[test]
fn test_truncated_header_lengths() {
    let codec = PacketCodec::new();
    let full_header = [PR_ED2K, 10, 0, 0, 0];

    for len in 0..HEADER_SIZE {
        let mut buf = BytesMut::from(&full_header[..len]);
        let result = codec
            .decode(&mut buf)
            .expect("truncated header should not error");
        assert!(result.is_none(), "expected Ok(None) for len {}", len);
    }
}

#[test]
fn test_truncated_payload_waits_for_completion() {
    let codec = PacketCodec::new();
    let mut buf = BytesMut::new();
    buf.put_u8(PR_ED2K);
    buf.put_u32_le(10); // Declares 1 byte opcode + 9 bytes payload = 10
    buf.put_u8(0x01); // Opcode
    buf.put_slice(&[1, 2, 3]); // Only 3 bytes of payload instead of 9

    assert_eq!(buf.len(), 5 + 1 + 3); // 9 bytes total, needs 15
    let result = codec
        .decode(&mut buf)
        .expect("partial payload should not error");
    assert!(result.is_none());

    // Deliver remaining 6 bytes
    buf.put_slice(&[4, 5, 6, 7, 8, 9]);
    let packet = codec.decode(&mut buf).unwrap().expect("now complete");
    assert_eq!(packet.opcode, 0x01);
    assert_eq!(packet.payload.as_ref(), &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
}

#[test]
fn test_peer_id_hex_and_display() {
    let raw = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xfe, 0xdc, 0xba, 0x98, 0x76, 0x54, 0x32,
        0x10,
    ];
    let peer = PeerId::new(raw);
    let hex_str = "0123456789abcdeffedcba9876543210";
    assert_eq!(peer.to_hex(), hex_str);
    assert_eq!(format!("{}", peer), hex_str);
    assert_eq!(PeerId::from_hex(hex_str).unwrap(), peer);

    // Invalid lengths or characters
    assert!(PeerId::from_hex("0123").is_err());
    assert!(PeerId::from_hex("0123456789abcdef0123456789abcdef00").is_err()); // 34 chars
    assert!(PeerId::from_hex("0123456789abcdef0123456789abcdeg").is_err()); // 'g' is invalid hex
}

struct MockSink {
    call_count: Arc<AtomicUsize>,
}

impl DownloadSink for MockSink {
    fn write_block(
        &self,
        block: ReceivedBlock,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<BlockWriteOutcome, ProtocolError>> + Send + '_>,
    > {
        let count = self.call_count.clone();
        Box::pin(async move {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(BlockWriteOutcome {
                bytes_written: block.len(),
                part_completed: true,
                completed_part_index: Some(0),
                file_completed: false,
            })
        })
    }
}

struct MockPlanner {
    requests: Vec<BlockRequest>,
}

impl DownloadPlanner for MockPlanner {
    fn next_request(
        &self,
        _peer_id: &PeerId,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<BlockRequest>> + Send + '_>>
    {
        let first = self.requests.first().cloned();
        Box::pin(async move { first })
    }

    fn on_source_state(
        &self,
        _update: SourceStateUpdate,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {})
    }
}

#[tokio::test]
async fn test_decoupled_sink_and_planner_traits() {
    let count = Arc::new(AtomicUsize::new(0));
    let sink = MockSink {
        call_count: count.clone(),
    };

    let block = ReceivedBlock::new([0xAA; 16], 0, Bytes::from_static(b"hello ed2k data"));
    let outcome = sink.write_block(block).await.expect("sink write ok");
    assert_eq!(outcome.bytes_written, 15);
    assert!(outcome.part_completed);
    assert_eq!(outcome.completed_part_index, Some(0));
    assert_eq!(count.load(Ordering::SeqCst), 1);

    let planner = MockPlanner {
        requests: vec![BlockRequest::new([0xAA; 16], 184_320, 184_320)],
    };
    let peer = PeerId::new([0xBB; 16]);
    let req = planner.next_request(&peer).await.expect("has request");
    assert_eq!(req.start_offset, 184_320);
    assert_eq!(req.length, 184_320);
    assert_eq!(req.end_offset(), 368_640);
}
