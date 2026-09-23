// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Hermetic test suite for eD2K download sequencing and block request planning.
//!
//! Tier 3.3 validation:
//! - Deterministic part availability checking (including complete file semantics).
//! - Part bounds calculation and alignment.
//! - Bounded block decomposition matching ED2K_BLOCK_SIZE (184,320 bytes).
//! - Strict part boundary alignment at 9,728,000 bytes.
//! - Boundary coverage of fixture 9,728,001 bytes (Part 0: 9,728,000 bytes, Part 1: 1 byte).
//! - Negative status (FileNotFound) rejection.
//! - Overflow and cross-part boundary violation rejections.
//! - Local wire frame construction for OP_REQUESTPARTS_EMULE (0x47) without transmission.

use bytes::Bytes;
use mule_protocol::ed2k::{
    Ed2kBlockRange, Ed2kBlockSink, Ed2kCompatibilityProfile, Ed2kDecoder, Ed2kDownloadSequencer,
    Ed2kEncoder, Ed2kErrorKind, Ed2kFileStatus, Ed2kMessage, Ed2kSendingPart, Ed2kSequencerError,
    Ed2kSession, Ed2kSessionError, PartAvailabilityMask, ED2K_BLOCK_SIZE, ED2K_PART_SIZE,
    MAX_REQUEST_RANGES_PER_PACKET, OP_COMPRESSEDPART, OP_REQUESTPARTS_EMULE, OP_SENDINGPART,
    PR_ED2K, PR_EMULE_COMPRESSED,
};
use mule_protocol::framing::RawPacket;
use mule_protocol::PacketCodec;

fn sample_hash() -> [u8; 16] {
    [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81, 0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82,
        0x3c,
    ]
}

// ---------------------------------------------------------------------------
// 1. Complete Source Eligibility (part_count == 0)
// ---------------------------------------------------------------------------

#[test]
fn test_sequencer_complete_source_zero_parts_all_eligible() {
    let hash = sample_hash();
    let file_size = 9_728_001u64; // 1 full part (9,728,000) + 1 byte remainder

    // Complete source reports part_count == 0 and empty bitmask
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };

    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).expect("valid sequencer");
    assert_eq!(
        ED2K_PART_SIZE, 9_728_000,
        "standard eD2K part size is 9,728,000 bytes"
    );
    assert_eq!(
        seq.total_parts(),
        2,
        "9_728_001 bytes file has exactly 2 parts"
    );
    assert_eq!(seq.file_hash(), hash);
    assert_eq!(seq.file_size(), file_size);

    // Both parts are available
    assert!(seq.is_part_available(0), "part 0 must be available");
    assert!(seq.is_part_available(1), "part 1 must be available");
    assert!(
        !seq.is_part_available(2),
        "out-of-bounds part 2 must not be available"
    );

    assert_eq!(seq.available_parts(), vec![0, 1]);
    assert_eq!(seq.select_first_available_part(), Some(0));

    // Part bounds
    assert_eq!(seq.part_byte_bounds(0).unwrap(), (0, 9_728_000));
    assert_eq!(seq.part_byte_bounds(1).unwrap(), (9_728_000, 9_728_001));
    assert!(matches!(
        seq.part_byte_bounds(2),
        Err(Ed2kSequencerError::PartIndexOutOfBounds {
            part_index: 2,
            total_parts: 2
        })
    ));
}

// ---------------------------------------------------------------------------
// 2. Negative Status (FileNotFound / OP_FILEREQANSNOFILE) Strictly Rejected
// ---------------------------------------------------------------------------

#[test]
fn test_sequencer_negative_response_filereqansnofile_strictly_rejected() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;

    // Simulate session error from OP_FILEREQANSNOFILE
    let res: Result<Ed2kFileStatus, Ed2kSessionError> =
        Err(Ed2kSessionError::FileNotFound(hex::encode(hash)));

    let err = Ed2kDownloadSequencer::from_file_status_result(hash, file_size, res).unwrap_err();
    assert!(matches!(err, Ed2kSequencerError::FileNotFound(_)));
}

// ---------------------------------------------------------------------------
// 3. Partial Source Eligibility & Selection
// ---------------------------------------------------------------------------

#[test]
fn test_sequencer_partial_source_eligibility() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;

    // Source only has part 0
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 2,
        availability_mask: PartAvailabilityMask::from_parts(&[0], 2),
    };

    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).expect("valid sequencer");
    assert!(seq.is_part_available(0));
    assert!(!seq.is_part_available(1));
    assert_eq!(seq.available_parts(), vec![0]);
    assert_eq!(seq.select_first_available_part(), Some(0));

    assert_eq!(seq.select_part(0).unwrap(), 0);
    assert!(matches!(
        seq.select_part(1),
        Err(Ed2kSequencerError::PartNotAvailable {
            part_index: 1,
            total_parts: 2
        })
    ));
}

// ---------------------------------------------------------------------------
// 4. Part 0 Block Decomposition & Boundary Alignment (9,728,000 bytes)
// ---------------------------------------------------------------------------

#[test]
fn test_sequencer_part_0_block_decomposition_and_alignment() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };

    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).unwrap();
    let blocks = seq.plan_part_blocks(0).expect("plan part 0");

    // 9,728,000 / 184,320 = 52 blocks of 184,320 + 1 block of 143,360 = 53 blocks
    assert_eq!(blocks.len(), 53, "full part has exactly 53 blocks");

    // First block: [0, 184_320)
    assert_eq!(blocks[0].start, 0);
    assert_eq!(blocks[0].end, 184_320);
    assert_eq!(blocks[0].len(), 184_320);

    // Contiguity verification
    for i in 1..blocks.len() {
        assert_eq!(
            blocks[i].start,
            blocks[i - 1].end,
            "blocks must be strictly contiguous"
        );
        assert!(blocks[i].len() <= ED2K_BLOCK_SIZE as u32);
    }

    // 52nd block (index 51): [9_400_320, 9_584_640)
    assert_eq!(blocks[51].start, 51 * 184_320);
    assert_eq!(blocks[51].end, 52 * 184_320);
    assert_eq!(blocks[51].len(), 184_320);

    // Final block in Part 0 (index 52): [9_584_640, 9_728_000)
    // 9_728_000 - 9_584_640 = 143,360 bytes
    assert_eq!(blocks[52].start, 9_584_640);
    assert_eq!(
        blocks[52].end, 9_728_000,
        "final block of part 0 must align exactly on part boundary"
    );
    assert_eq!(blocks[52].len(), 143_360);
}

// ---------------------------------------------------------------------------
// 5. Part 1 Residual Single Byte Coverage (Fixture 9,728,001 bytes)
// ---------------------------------------------------------------------------

#[test]
fn test_sequencer_part_1_residual_single_byte_coverage() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };

    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).unwrap();
    let blocks = seq.plan_part_blocks(1).expect("plan part 1");

    // Part 1 has length 1 byte (from 9_728_000 to 9_728_001)
    assert_eq!(blocks.len(), 1, "part 1 of 1 byte has exactly 1 block");
    assert_eq!(blocks[0].start, 9_728_000);
    assert_eq!(blocks[0].end, 9_728_001);
    assert_eq!(blocks[0].len(), 1, "residual block has length 1 byte");
}

// ---------------------------------------------------------------------------
// 6. Boundary Rejection Matrix
// ---------------------------------------------------------------------------

#[test]
fn test_sequencer_boundary_rejection_matrix() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };

    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).unwrap();

    // 1. start >= end
    let err_equal = seq.validate_block_range(100, 100).unwrap_err();
    assert!(matches!(
        err_equal,
        Ed2kSequencerError::InvalidBlockRange { .. }
    ));

    let err_inv = seq.validate_block_range(200, 100).unwrap_err();
    assert!(matches!(
        err_inv,
        Ed2kSequencerError::InvalidBlockRange { .. }
    ));

    // 2. end exceeds file size
    let err_overflow = seq.validate_block_range(9_728_000, 9_728_002).unwrap_err();
    assert!(matches!(
        err_overflow,
        Ed2kSequencerError::InvalidBlockRange { .. }
    ));

    // 3. Block exceeds ED2K_BLOCK_SIZE (184,320)
    let err_too_large = seq.validate_block_range(0, 184_321).unwrap_err();
    assert!(matches!(
        err_too_large,
        Ed2kSequencerError::InvalidBlockRange { .. }
    ));

    // 4. Block crosses part boundary (part 0 ends at 9_728_000)
    let err_cross = seq.validate_block_range(9_727_000, 9_728_001).unwrap_err();
    match err_cross {
        Ed2kSequencerError::CrossPartBoundary {
            start,
            end,
            part_boundary,
        } => {
            assert_eq!(start, 9_727_000);
            assert_eq!(end, 9_728_001);
            assert_eq!(part_boundary, 9_728_000);
        }
        other => panic!("expected CrossPartBoundary error, got: {other:?}"),
    }

    // 5. Empty file size rejection on creation
    let err_empty = Ed2kDownloadSequencer::new(
        hash,
        0,
        Ed2kFileStatus {
            file_hash: hash,
            part_count: 0,
            availability_mask: PartAvailabilityMask::empty(),
        },
    )
    .unwrap_err();
    assert!(matches!(err_empty, Ed2kSequencerError::EmptyFile));

    // 6. Hash mismatch rejection on creation
    let err_mismatch = Ed2kDownloadSequencer::new(
        [0xAA; 16],
        file_size,
        Ed2kFileStatus {
            file_hash: [0xBB; 16],
            part_count: 0,
            availability_mask: PartAvailabilityMask::empty(),
        },
    )
    .unwrap_err();
    assert!(matches!(
        err_mismatch,
        Ed2kSequencerError::HashMismatch { .. }
    ));
}

// ---------------------------------------------------------------------------
// 7. Wire Framing for OP_REQUESTPARTS_EMULE (0x47)
// ---------------------------------------------------------------------------

#[test]
fn test_sequencer_wire_framing_emule050a() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };

    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).unwrap();
    let encoder = Ed2kEncoder::new(); // default profile: Emule050a
    let decoder = Ed2kDecoder::new();

    // 1. Single range request: Part 0 Block 0
    let range0 = Ed2kBlockRange::new(0, 184_320);
    let pkt_single = seq
        .build_request_packet(&[range0], &encoder)
        .expect("encode single block request");

    assert_eq!(pkt_single.protocol, PR_ED2K);
    assert_eq!(pkt_single.opcode, OP_REQUESTPARTS_EMULE); // 0x47
    assert_eq!(
        pkt_single.payload.len(),
        40,
        "wire payload must be exactly 40 bytes"
    );
    assert_eq!(&pkt_single.payload[0..16], &hash);

    // Verify 3 start offsets and 3 end offsets in 40-byte payload
    // Start offsets at [16..28]: range0.start = 0, 0, 0
    assert_eq!(&pkt_single.payload[16..20], &0u32.to_le_bytes());
    assert_eq!(&pkt_single.payload[20..24], &0u32.to_le_bytes());
    assert_eq!(&pkt_single.payload[24..28], &0u32.to_le_bytes());

    // End offsets at [28..40]: range0.end = 184320, 0, 0
    assert_eq!(&pkt_single.payload[28..32], &184_320u32.to_le_bytes());
    assert_eq!(&pkt_single.payload[32..36], &0u32.to_le_bytes());
    assert_eq!(&pkt_single.payload[36..40], &0u32.to_le_bytes());

    // Decode mirror verification
    let decoded = decoder
        .decode_raw_packet(&pkt_single)
        .expect("decode wire packet");
    match decoded {
        Ed2kMessage::RequestParts(req) => {
            assert_eq!(req.file_hash, hash);
            assert_eq!(req.ranges, vec![range0]);
        }
        other => panic!("expected RequestParts, got: {other:?}"),
    }

    // 2. Residual Part 1 Block 0 request: [9_728_000, 9_728_001)
    let residual_range = Ed2kBlockRange::new(9_728_000, 9_728_001);
    let pkt_residual = seq
        .build_request_packet(&[residual_range], &encoder)
        .expect("encode residual block request");

    assert_eq!(pkt_residual.opcode, OP_REQUESTPARTS_EMULE);
    assert_eq!(&pkt_residual.payload[16..20], &9_728_000u32.to_le_bytes());
    assert_eq!(&pkt_residual.payload[28..32], &9_728_001u32.to_le_bytes());

    // 3. Multi-range request: 3 ranges in Part 0
    let range1 = Ed2kBlockRange::new(184_320, 368_640);
    let range2 = Ed2kBlockRange::new(368_640, 552_960);
    let pkt_multi = seq
        .build_request_packet(&[range0, range1, range2], &encoder)
        .expect("encode 3 ranges");
    assert_eq!(pkt_multi.payload.len(), 40);

    let decoded_multi = decoder.decode_raw_packet(&pkt_multi).expect("decode multi");
    match decoded_multi {
        Ed2kMessage::RequestParts(req) => {
            assert_eq!(req.file_hash, hash);
            assert_eq!(req.ranges, vec![range0, range1, range2]);
        }
        other => panic!("expected RequestParts, got: {other:?}"),
    }

    // 4. Excessive ranges rejection (4 > MAX_REQUEST_RANGES_PER_PACKET)
    let range3 = Ed2kBlockRange::new(552_960, 737_280);
    let err_too_many = seq
        .build_request_packet(&[range0, range1, range2, range3], &encoder)
        .unwrap_err();
    assert!(matches!(
        err_too_many,
        Ed2kSequencerError::TooManyRanges {
            count: 4,
            max: MAX_REQUEST_RANGES_PER_PACKET
        }
    ));

    // 5. Empty ranges rejection
    let err_empty_ranges = seq.build_request_packet(&[], &encoder).unwrap_err();
    assert!(matches!(err_empty_ranges, Ed2kSequencerError::EmptyRanges));
}

// ---------------------------------------------------------------------------
// Tier 3.4 Hermetic Tests: Single Bounded Block Wire Framing & Validation
// ---------------------------------------------------------------------------

#[test]
fn test_tier3_requestparts_emule_single_range_wire_framing() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;

    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).expect("valid sequencer");

    let range0 = Ed2kBlockRange::new(0, 184_320);
    let encoder = Ed2kEncoder::new(); // Emule050a profile by default

    let pkt = seq
        .build_request_packet(&[range0], &encoder)
        .expect("encode single range request packet");

    // 1. Wire assertions
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_REQUESTPARTS_EMULE); // 0x47
    assert_eq!(
        pkt.payload.len(),
        40,
        "OP_REQUESTPARTS_EMULE payload is exactly 40 bytes"
    );

    let frame = PacketCodec::new()
        .encode_to_bytes(&pkt)
        .expect("encode frame");
    assert_eq!(
        frame.len(),
        46,
        "1 (proto) + 4 (len) + 1 (opcode) + 40 (payload) = 46 bytes"
    );
    assert_eq!(frame[0], PR_ED2K);
    assert_eq!(
        &frame[1..5],
        &41u32.to_le_bytes(),
        "4-byte LE length must be 41"
    );
    assert_eq!(frame[5], OP_REQUESTPARTS_EMULE);

    // 2. Payload assertions
    assert_eq!(
        &pkt.payload[0..16],
        &hash,
        "16-byte file hash must match fixture"
    );
    // Start offsets: range0 = 0, range1 = 0 (inactive), range2 = 0 (inactive)
    assert_eq!(
        &pkt.payload[16..20],
        &0u32.to_le_bytes(),
        "start 0 must be 0"
    );
    assert_eq!(
        &pkt.payload[20..24],
        &0u32.to_le_bytes(),
        "start 1 (inactive) must be 0"
    );
    assert_eq!(
        &pkt.payload[24..28],
        &0u32.to_le_bytes(),
        "start 2 (inactive) must be 0"
    );
    // End offsets: range0 = 184_320, range1 = 0 (inactive), range2 = 0 (inactive)
    assert_eq!(
        &pkt.payload[28..32],
        &184_320u32.to_le_bytes(),
        "end 0 must be 184,320"
    );
    assert_eq!(
        &pkt.payload[32..36],
        &0u32.to_le_bytes(),
        "end 1 (inactive) must be 0"
    );
    assert_eq!(
        &pkt.payload[36..40],
        &0u32.to_le_bytes(),
        "end 2 (inactive) must be 0"
    );

    // 3. Mirror decode under Emule050a profile
    let decoder = Ed2kDecoder::with_profile(Ed2kCompatibilityProfile::Emule050a);
    let decoded = decoder
        .decode_raw_packet(&pkt)
        .expect("mirror decode succeeds");
    match decoded {
        Ed2kMessage::RequestParts(req) => {
            assert_eq!(req.file_hash, hash);
            assert_eq!(
                req.ranges.len(),
                1,
                "inactive ranges are pruned during decoding"
            );
            assert_eq!(req.ranges[0], range0);
        }
        other => panic!("expected RequestParts, got: {other:?}"),
    }
}

#[test]
fn test_tier3_sendingpart_wire_framing_first_block() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;

    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).expect("valid sequencer");

    let block_len = ED2K_BLOCK_SIZE; // 184,320 bytes
    let block_data = Bytes::from(vec![0xAA; block_len]);

    let part = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 0,
        end_offset: 184_320,
        data: block_data.clone(),
    };

    let encoder = Ed2kEncoder::new();
    let pkt = encoder
        .encode_sending_part(&part)
        .expect("encode sending part");

    // 1. Frame assertions
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_SENDINGPART); // 0x46
    assert_eq!(
        pkt.payload.len(),
        24 + block_len,
        "header 24 bytes + 184,320 data bytes = 184,344 bytes"
    );

    let frame = PacketCodec::new()
        .encode_to_bytes(&pkt)
        .expect("encode frame");
    let expected_frame_len = 1 + 4 + 1 + 24 + block_len; // 184,350 bytes
    assert_eq!(frame.len(), expected_frame_len);
    assert_eq!(frame[0], PR_ED2K);
    let payload_and_opcode_len = (1 + 24 + block_len) as u32;
    assert_eq!(&frame[1..5], &payload_and_opcode_len.to_le_bytes());
    assert_eq!(frame[5], OP_SENDINGPART);

    // 2. Decode via Ed2kDecoder
    let decoder = Ed2kDecoder::new();
    let decoded = decoder
        .decode_sending_part(PR_ED2K, OP_SENDINGPART, &pkt.payload, 0)
        .expect("decode sending part");
    assert_eq!(decoded.file_hash, hash);
    assert_eq!(decoded.start_offset, 0);
    assert_eq!(decoded.end_offset, 184_320);
    assert_eq!(decoded.data.len(), block_len);
    assert_eq!(decoded.data, block_data);

    // 3. Sequencer validation
    let range = seq
        .validate_sending_part(&decoded)
        .expect("valid sending part");
    assert_eq!(range.start, 0);
    assert_eq!(range.end, 184_320);

    // 4. Rejection: Truncated header (< 24 bytes)
    let err_trunc_hdr = decoder
        .decode_sending_part(PR_ED2K, OP_SENDINGPART, &pkt.payload[..20], 0)
        .unwrap_err();
    assert!(matches!(
        err_trunc_hdr.kind,
        Ed2kErrorKind::UnexpectedEof { .. }
    ));

    // 5. Rejection: Truncated payload (< 184,320 bytes)
    let err_trunc_payload = decoder
        .decode_sending_part(
            PR_ED2K,
            OP_SENDINGPART,
            &pkt.payload[..pkt.payload.len() - 10],
            0,
        )
        .unwrap_err();
    assert!(matches!(
        err_trunc_payload.kind,
        Ed2kErrorKind::BlockPayloadMismatch {
            expected: 184_320,
            actual: 184_310
        }
    ));

    // 6. Rejection: Superfluous bytes (> 184,320 bytes)
    let mut superfluous_payload = pkt.payload.to_vec();
    superfluous_payload.extend_from_slice(&[0xFF; 10]);
    let err_superfluous = decoder
        .decode_sending_part(PR_ED2K, OP_SENDINGPART, &superfluous_payload, 0)
        .unwrap_err();
    assert!(matches!(
        err_superfluous.kind,
        Ed2kErrorKind::BlockPayloadMismatch {
            expected: 184_320,
            actual: 184_330
        }
    ));

    // 7. Rejection: Inverted offsets (start >= end)
    let mut inverted_payload = pkt.payload.to_vec();
    inverted_payload[16..20].copy_from_slice(&184_320u32.to_le_bytes());
    inverted_payload[20..24].copy_from_slice(&0u32.to_le_bytes());
    let err_inverted = decoder
        .decode_sending_part(PR_ED2K, OP_SENDINGPART, &inverted_payload, 0)
        .unwrap_err();
    assert!(matches!(
        err_inverted.kind,
        Ed2kErrorKind::InvalidBlockOffsets { .. }
    ));

    // 8. Rejection: Hash mismatch against sequencer
    let wrong_hash = [0xBB; 16];
    let part_wrong_hash = Ed2kSendingPart {
        file_hash: wrong_hash,
        start_offset: 0,
        end_offset: 184_320,
        data: block_data,
    };
    let err_hash = seq.validate_sending_part(&part_wrong_hash).unwrap_err();
    assert!(matches!(err_hash, Ed2kSequencerError::HashMismatch { .. }));
}

#[test]
fn test_tier3_rejects_sendingpart_crossing_part_boundary() {
    let hash = sample_hash();
    let file_size = 15_000_000u64;

    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).expect("valid sequencer");

    // Invalid range crossing 9,728,000 part boundary: [9_727_900, 9_728_100) (200 bytes)
    let cross_start = 9_727_900u32;
    let cross_end = 9_728_100u32;
    let cross_data = Bytes::from(vec![0x77; (cross_end - cross_start) as usize]);

    let part = Ed2kSendingPart {
        file_hash: hash,
        start_offset: cross_start,
        end_offset: cross_end,
        data: cross_data,
    };

    // Validation must fail with CrossPartBoundary
    let err = seq.validate_sending_part(&part).unwrap_err();
    assert_eq!(
        err,
        Ed2kSequencerError::CrossPartBoundary {
            start: 9_727_900,
            end: 9_728_100,
            part_boundary: 9_728_000,
        }
    );

    // Sink write must also be rejected without modifying buffer
    let mut sink = Ed2kBlockSink::new(file_size);
    let sink_err = sink.write_block(&seq, &part).unwrap_err();
    assert_eq!(
        sink_err,
        Ed2kSequencerError::CrossPartBoundary {
            start: 9_727_900,
            end: 9_728_100,
            part_boundary: 9_728_000,
        }
    );
    assert_eq!(sink.received_ranges().len(), 0);
    assert_eq!(sink.received_bytes_count(), 0);
    assert!(
        sink.buffer().iter().all(|&b| b == 0),
        "storage must remain completely untouched"
    );
}

#[test]
fn test_tier3_rejects_compressedpart_in_passive_profile() {
    let compressed_pkt = RawPacket::with_vec(
        PR_EMULE_COMPRESSED, // 0xD4
        OP_COMPRESSEDPART,   // 0x40
        vec![0x1F, 0x8B, 0x08, 0x00],
    );

    // 1. Decoder rejects 0xD4 frame immediately
    let decoder = Ed2kDecoder::new();
    let decode_err = decoder.decode_raw_packet(&compressed_pkt).unwrap_err();
    assert_eq!(
        decode_err.kind,
        Ed2kErrorKind::CompressedProtocolUnsupported
    );

    // 2. Active session rejects 0xD4 packet without attempting decompression
    let mut session = Ed2kSession::new_initiator([0x11; 16], 0, 39735);
    let session_err = session.on_packet_received(&compressed_pkt).unwrap_err();
    assert_eq!(session_err, Ed2kSessionError::CompressedProtocolUnsupported);
}

#[test]
fn test_tier3_single_block_transfer_writes_exact_fixture_prefix() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;

    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).expect("valid sequencer");

    // 1. Generate exact deterministic fixture prefix of 184,320 bytes (pattern 0..=255)
    let prefix_len = ED2K_BLOCK_SIZE; // 184,320
    let prefix_bytes: Vec<u8> = (0..=255u8).cycle().take(prefix_len).collect();
    assert_eq!(prefix_bytes.len(), prefix_len);

    let part = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 0,
        end_offset: 184_320,
        data: Bytes::copy_from_slice(&prefix_bytes),
    };

    // 2. Write block to sink
    let mut sink = Ed2kBlockSink::new(file_size);
    assert_eq!(sink.file_size(), file_size);
    assert_eq!(sink.received_bytes_count(), 0);
    assert!(!sink.has_range(0, 184_320));

    let range = sink.write_block(&seq, &part).expect("first write succeeds");
    assert_eq!(range.start, 0);
    assert_eq!(range.end, 184_320);

    // 3. Verify exact written state
    assert!(sink.has_range(0, 184_320));
    assert_eq!(sink.received_ranges(), &[(0, 184_320)]);
    assert_eq!(sink.received_bytes_count(), prefix_len);
    assert_eq!(
        &sink.buffer()[0..prefix_len],
        prefix_bytes.as_slice(),
        "written bytes must match fixture prefix exactly"
    );
    assert!(
        sink.buffer()[prefix_len..].iter().all(|&b| b == 0),
        "unwritten file remainder must remain zero"
    );

    // 4. Duplicate reception rejection: second write of identical range must fail
    let err_dup = sink.write_block(&seq, &part).unwrap_err();
    assert_eq!(
        err_dup,
        Ed2kSequencerError::DuplicateBlock {
            start: 0,
            end: 184_320,
        }
    );
    assert_eq!(
        sink.received_ranges().len(),
        1,
        "duplicate write must not duplicate range entry"
    );
    assert_eq!(sink.received_bytes_count(), prefix_len);
}

#[test]
fn test_tier3_block_sink_chunked_assembly_18_chunks() {
    let hash = sample_hash();
    let file_size = 9_728_001u64;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).expect("valid sequencer");
    let mut sink = Ed2kBlockSink::new(file_size);

    let prefix_bytes: Vec<u8> = (0..=255u8).cycle().take(184_320).collect();

    // Write 18 chunks of 10,240 bytes (standard eMule chunking)
    for i in 0..18 {
        let start = i * 10240;
        let end = start + 10240;
        let chunk_part = Ed2kSendingPart {
            file_hash: hash,
            start_offset: start as u32,
            end_offset: end as u32,
            data: Bytes::copy_from_slice(&prefix_bytes[start as usize..end as usize]),
        };
        sink.write_block(&seq, &chunk_part).expect("write chunk");
    }

    assert_eq!(sink.received_bytes_count(), 184_320);
    assert!(sink.has_range(0, 184_320));
    assert!(sink.is_range_covered(0, 184_320));
    assert_eq!(&sink.buffer()[0..184_320], prefix_bytes.as_slice());

    // Duplicate chunk rejection
    let dup_chunk = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 0,
        end_offset: 10240,
        data: Bytes::copy_from_slice(&prefix_bytes[0..10240]),
    };
    let err_dup = sink.write_block(&seq, &dup_chunk).unwrap_err();
    assert_eq!(
        err_dup,
        Ed2kSequencerError::DuplicateBlock {
            start: 0,
            end: 10240,
        }
    );
}

// ---------------------------------------------------------------------------
// Preview part priority (eMule PR_VERYHIGH on first/last part)
// ---------------------------------------------------------------------------

#[test]
fn test_sequencer_least_received_part_selected_first() {
    let hash = sample_hash();
    // 4 parts; the peer offers all of them. Part 0 is fully received,
    // part 1 is 50% done, parts 2-3 untouched → least-received picks part 2.
    let file_size = 4 * 9_728_000u64;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0, // complete source
        availability_mask: PartAvailabilityMask::empty(),
    };
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status).unwrap();

    let completed = vec![
        (0u64, 9_728_000u64),                    // part 0 done
        (9_728_000u64, 9_728_000 + 4_864_000u64), // part 1 half done
    ];
    let (part, block) = seq
        .plan_next_block_for_peer64("peer", &completed, &[], 0)
        .unwrap()
        .expect("a block must be planned");
    assert_eq!(part, 2, "least-received part (2) wins over half-done part 1");
    assert_eq!(block.start, 2 * 9_728_000);
}

#[test]
fn test_sequencer_preview_priority_picks_edge_parts() {
    let hash = sample_hash();
    let file_size = 4 * 9_728_000u64;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status)
        .unwrap()
        .with_preview_priority(true);

    // Middle parts already partially downloaded — preview still forces edges.
    let completed = vec![(9_728_000u64, 2 * 9_728_000u64)]; // part 1 done
    let (part, _) = seq
        .plan_next_block_for_peer64("peer", &completed, &[], 0)
        .unwrap()
        .unwrap();
    assert_eq!(part, 0, "preview mode: first part requested first");

    // Once part 0 is done, the LAST part is next (not part 2).
    let completed = vec![
        (0u64, 9_728_000u64),
        (9_728_000u64, 2 * 9_728_000u64),
    ];
    let (part, block) = seq
        .plan_next_block_for_peer64("peer", &completed, &[], 0)
        .unwrap()
        .unwrap();
    assert_eq!(part, 3, "preview mode: last part before middle parts");
    assert_eq!(block.start, 3 * 9_728_000);
}

#[test]
fn test_sequencer_preview_skips_edge_parts_not_offered_by_peer() {
    let hash = sample_hash();
    let file_size = 4 * 9_728_000u64;
    // Partial source offering only parts 1-2.
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 4,
        availability_mask: PartAvailabilityMask::from_parts(&[1, 2], 4),
    };
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status)
        .unwrap()
        .with_preview_priority(true);

    let (part, _) = seq
        .plan_next_block_for_peer64("peer", &[], &[], 0)
        .unwrap()
        .unwrap();
    assert!(part == 1 || part == 2, "edge parts unavailable → normal selection");
}

// ---------------------------------------------------------------------------
// End-game redundancy (eMule duplicates the last blocks across peers)
// ---------------------------------------------------------------------------

use mule_protocol::ed2k::InFlightBlock;

/// Builds a sequencer for a file small enough to enter end-game once a single
/// block remains (file size ≤ 1 part keeps part math trivial).
fn endgame_sequencer(file_size: u64) -> Ed2kDownloadSequencer {
    let hash = sample_hash();
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    Ed2kDownloadSequencer::new(hash, file_size, status).unwrap()
}

#[test]
fn test_endgame_allows_duplicate_block_from_other_peer() {
    // Single block remaining (≤ ENDGAME_REMAINING_BYTES = 3 blocks) already
    // in-flight on peer A. Peer B must receive a duplicate request for the
    // same block — the fastest source wins the last chunk.
    let file_size = ED2K_BLOCK_SIZE as u64;
    let seq = endgame_sequencer(file_size);
    let in_flight = vec![InFlightBlock::new("peerA".into(), 0, 0, file_size, 0, 30_000)];

    let (part, block) = seq
        .plan_next_block_for_peer64("peerB", &[], &in_flight, 1_000)
        .unwrap()
        .expect("end-game must duplicate the in-flight block");
    assert_eq!(part, 0);
    assert_eq!((block.start, block.end), (0, file_size));
}

#[test]
fn test_endgame_never_duplicates_block_for_same_peer() {
    let file_size = 2 * ED2K_BLOCK_SIZE as u64;
    let seq = endgame_sequencer(file_size);
    // Peer A already owns the first block; the second is free → normal path
    // assigns the free block. If BOTH blocks were already assigned to peer A,
    // end-game must return None.
    let in_flight = vec![
        InFlightBlock::new("peerA".into(), 0, 0, ED2K_BLOCK_SIZE as u64, 0, 30_000),
        InFlightBlock::new(
            "peerA".into(),
            0,
            ED2K_BLOCK_SIZE as u64,
            2 * ED2K_BLOCK_SIZE as u64,
            0,
            30_000,
        ),
    ];
    let res = seq
        .plan_next_block_for_peer64("peerA", &[], &in_flight, 1_000)
        .unwrap();
    assert!(res.is_none(), "same peer never gets a duplicate request");
}

#[test]
fn test_endgame_respects_max_two_assignments_per_block() {
    let file_size = ED2K_BLOCK_SIZE as u64; // single block remaining
    let seq = endgame_sequencer(file_size);
    let in_flight = vec![
        InFlightBlock::new("peerA".into(), 0, 0, file_size, 0, 30_000),
        InFlightBlock::new("peerB".into(), 0, 0, file_size, 0, 30_000),
    ];
    let res = seq
        .plan_next_block_for_peer64("peerC", &[], &in_flight, 1_000)
        .unwrap();
    assert!(
        res.is_none(),
        "block already assigned twice → no third request"
    );
}

#[test]
fn test_no_endgame_when_remaining_exceeds_threshold() {
    // 4 blocks > ENDGAME_REMAINING_BYTES (3 blocks) → in-flight overlap must
    // still be rejected.
    let file_size = 4 * ED2K_BLOCK_SIZE as u64;
    let seq = endgame_sequencer(file_size);
    // Blocks 0..2 in-flight on peerA; block 3 free → normal path picks it.
    // Make ALL blocks in-flight to force the fallback path.
    let in_flight: Vec<_> = (0..4u64)
        .map(|i| {
            InFlightBlock::new(
                "peerA".into(),
                0,
                i * ED2K_BLOCK_SIZE as u64,
                (i + 1) * ED2K_BLOCK_SIZE as u64,
                0,
                30_000,
            )
        })
        .collect();
    let res = seq
        .plan_next_block_for_peer64("peerB", &[], &in_flight, 1_000)
        .unwrap();
    assert!(
        res.is_none(),
        "outside end-game no duplicate request is allowed"
    );
}

// ---------------------------------------------------------------------------
// Swarm healing (EMule-Rust) — rarest-part-first via scarcity hint
// ---------------------------------------------------------------------------

#[test]
fn test_swarm_healing_prefers_rarest_part() {
    let hash = sample_hash();
    // 3 parts, all offered by a complete source (part_count == 0).
    let file_size = ED2K_PART_SIZE as u64 * 3;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    // Part 1 is held by a single source; 0 and 2 are common.
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status)
        .unwrap()
        .with_part_scarcity(vec![5, 1, 5]);

    let res = seq
        .plan_next_block_for_peer64("peerA", &[], &[], 1_000)
        .unwrap()
        .expect("a block must be planned");
    assert_eq!(res.0, 1, "swarm healing must pick the rarest part first");
}

#[test]
fn test_swarm_healing_scarcity_tie_falls_back_to_progress() {
    let hash = sample_hash();
    let file_size = ED2K_PART_SIZE as u64 * 3;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status)
        .unwrap()
        .with_part_scarcity(vec![3, 3, 3]);

    // Part 0 partially received → least-received tie-break picks part 1
    // (0 bytes received) over part 0 despite the lower index.
    let completed = vec![(0u64, 1024u64)];
    let res = seq
        .plan_next_block_for_peer64("peerA", &completed, &[], 1_000)
        .unwrap()
        .expect("a block must be planned");
    assert_eq!(res.0, 1, "equal scarcity must fall back to least-received part");
}

#[test]
fn test_swarm_healing_ignores_mismatched_scarcity_len() {
    let hash = sample_hash();
    let file_size = ED2K_PART_SIZE as u64 * 2;
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    // Wrong length → hint ignored, plain progress-first.
    let seq = Ed2kDownloadSequencer::new(hash, file_size, status)
        .unwrap()
        .with_part_scarcity(vec![99]);
    let res = seq
        .plan_next_block_for_peer64("peerA", &[], &[], 1_000)
        .unwrap()
        .expect("a block must be planned");
    assert_eq!(res.0, 0);
}
