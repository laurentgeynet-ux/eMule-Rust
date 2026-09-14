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

use mule_protocol::ed2k::{
    Ed2kBlockRange, Ed2kDecoder, Ed2kDownloadSequencer, Ed2kEncoder, Ed2kFileStatus,
    Ed2kMessage, Ed2kSequencerError, Ed2kSessionError, PartAvailabilityMask,
    ED2K_BLOCK_SIZE, ED2K_PART_SIZE, MAX_REQUEST_RANGES_PER_PACKET,
    OP_REQUESTPARTS_EMULE, PR_ED2K,
};

fn sample_hash() -> [u8; 16] {
    [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81,
        0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82, 0x3c,
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
    assert_eq!(ED2K_PART_SIZE, 9_728_000, "standard eD2K part size is 9,728,000 bytes");
    assert_eq!(seq.total_parts(), 2, "9_728_001 bytes file has exactly 2 parts");
    assert_eq!(seq.file_hash(), hash);
    assert_eq!(seq.file_size(), file_size);

    // Both parts are available
    assert!(seq.is_part_available(0), "part 0 must be available");
    assert!(seq.is_part_available(1), "part 1 must be available");
    assert!(!seq.is_part_available(2), "out-of-bounds part 2 must not be available");

    assert_eq!(seq.available_parts(), vec![0, 1]);
    assert_eq!(seq.select_first_available_part(), Some(0));

    // Part bounds
    assert_eq!(seq.part_byte_bounds(0).unwrap(), (0, 9_728_000));
    assert_eq!(seq.part_byte_bounds(1).unwrap(), (9_728_000, 9_728_001));
    assert!(matches!(
        seq.part_byte_bounds(2),
        Err(Ed2kSequencerError::PartIndexOutOfBounds { part_index: 2, total_parts: 2 })
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
        Err(Ed2kSequencerError::PartNotAvailable { part_index: 1, total_parts: 2 })
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
    assert_eq!(blocks[52].end, 9_728_000, "final block of part 0 must align exactly on part boundary");
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
    assert!(matches!(err_equal, Ed2kSequencerError::InvalidBlockRange { .. }));

    let err_inv = seq.validate_block_range(200, 100).unwrap_err();
    assert!(matches!(err_inv, Ed2kSequencerError::InvalidBlockRange { .. }));

    // 2. end exceeds file size
    let err_overflow = seq.validate_block_range(9_728_000, 9_728_002).unwrap_err();
    assert!(matches!(err_overflow, Ed2kSequencerError::InvalidBlockRange { .. }));

    // 3. Block exceeds ED2K_BLOCK_SIZE (184,320)
    let err_too_large = seq.validate_block_range(0, 184_321).unwrap_err();
    assert!(matches!(err_too_large, Ed2kSequencerError::InvalidBlockRange { .. }));

    // 4. Block crosses part boundary (part 0 ends at 9_728_000)
    let err_cross = seq.validate_block_range(9_727_000, 9_728_001).unwrap_err();
    match err_cross {
        Ed2kSequencerError::CrossPartBoundary { start, end, part_boundary } => {
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
    assert!(matches!(err_mismatch, Ed2kSequencerError::HashMismatch { .. }));
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
    assert_eq!(pkt_single.payload.len(), 40, "wire payload must be exactly 40 bytes");
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
    let decoded = decoder.decode_raw_packet(&pkt_single).expect("decode wire packet");
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
        Ed2kSequencerError::TooManyRanges { count: 4, max: MAX_REQUEST_RANGES_PER_PACKET }
    ));

    // 5. Empty ranges rejection
    let err_empty_ranges = seq.build_request_packet(&[], &encoder).unwrap_err();
    assert!(matches!(err_empty_ranges, Ed2kSequencerError::EmptyRanges));
}
