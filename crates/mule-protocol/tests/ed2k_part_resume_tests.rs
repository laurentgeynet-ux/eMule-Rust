// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Hermetic unit and integration tests for eD2K Part 0 storage persistence,
//! crash-resilient `.resume.json` state journaling, gap calculation, and MD4 verification.

use bytes::Bytes;
use md4::{Digest, Md4};
use mule_protocol::ed2k::{
    Ed2kDownloadSequencer, Ed2kFileStatus, Ed2kLoopbackClient, Ed2kPartDownloadStatus,
    Ed2kPartFileSink, Ed2kSendingPart, Ed2kSequencerError, Ed2kTestPeer, PartAvailabilityMask,
    TestPeerMode, ED2K_BLOCK_SIZE, ED2K_PART_SIZE,
};
use std::fs;
use std::time::Duration;
use tempfile::tempdir;

fn test_file_hash() -> [u8; 16] {
    [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81, 0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82,
        0x3c,
    ]
}

const TEST_FILE_SIZE: u64 = 9_728_001; // Part 0 (9,728,000) + Part 1 (1 byte)

fn make_sequencer() -> Ed2kDownloadSequencer {
    let hash = test_file_hash();
    let status = Ed2kFileStatus {
        file_hash: hash,
        part_count: 0,
        availability_mask: PartAvailabilityMask::empty(),
    };
    Ed2kDownloadSequencer::new(hash, TEST_FILE_SIZE, status).expect("valid sequencer")
}

#[test]
fn test_part_sink_initial_creation_and_sparse_file_size() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("test_download.part");
    let hash = test_file_hash();

    let sink = Ed2kPartFileSink::create_new(part_path.clone(), hash, TEST_FILE_SIZE)
        .expect("create new sink");

    assert!(part_path.exists(), "part file must exist");
    assert_eq!(
        fs::metadata(&part_path).expect("metadata").len(),
        TEST_FILE_SIZE,
        "part file size must match target file size"
    );

    let resume_path = sink.resume_file_path();
    assert!(resume_path.exists(), "companion resume file must exist");

    let state = sink.resume_state();
    assert_eq!(state.file_hash, hex::encode(hash));
    assert_eq!(state.file_size, TEST_FILE_SIZE);
    assert_eq!(state.completed_ranges.len(), 0);
    assert_eq!(state.total_bytes_received, 0);
    assert_eq!(sink.total_bytes_received(), 0);
    assert_eq!(sink.completed_ranges().len(), 0);
}

#[test]
fn test_part_sink_incremental_writes_and_interval_merging() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("merge_test.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();

    let mut sink =
        Ed2kPartFileSink::create_new(part_path, hash, TEST_FILE_SIZE).expect("create new sink");

    // Write chunk 0: [0, 10240)
    let chunk0 = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 0,
        end_offset: 10240,
        data: Bytes::from(vec![0xAA; 10240]),
    };
    sink.write_block(&sequencer, &chunk0)
        .expect("write chunk 0");
    assert_eq!(sink.completed_ranges(), &[(0, 10240)]);
    assert_eq!(sink.total_bytes_received(), 10240);

    // Write chunk 1: [10240, 20480) - adjacent, should merge with chunk 0
    let chunk1 = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 10240,
        end_offset: 20480,
        data: Bytes::from(vec![0xBB; 10240]),
    };
    sink.write_block(&sequencer, &chunk1)
        .expect("write chunk 1");
    assert_eq!(sink.completed_ranges(), &[(0, 20480)]);
    assert_eq!(sink.total_bytes_received(), 20480);

    // Write chunk out-of-order: [50000, 60000)
    let chunk_disjoint = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 50000,
        end_offset: 60000,
        data: Bytes::from(vec![0xCC; 10000]),
    };
    sink.write_block(&sequencer, &chunk_disjoint)
        .expect("write disjoint chunk");
    assert_eq!(sink.completed_ranges(), &[(0, 20480), (50000, 60000)]);
    assert_eq!(sink.total_bytes_received(), 30480);

    // Fill gap: [20480, 50000) - merges both intervals into [0, 60000)
    let chunk_gap = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 20480,
        end_offset: 50000,
        data: Bytes::from(vec![0xDD; 29520]),
    };
    sink.write_block(&sequencer, &chunk_gap).expect("fill gap");
    assert_eq!(sink.completed_ranges(), &[(0, 60000)]);
    assert_eq!(sink.total_bytes_received(), 60000);
    assert!(sink.has_range(0, 60000));
    assert!(sink.has_range(10000, 20000));
    assert!(!sink.has_range(0, 70000));
}

#[test]
fn test_part_sink_duplicate_and_overlapping_chunk_rejection() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("dup_test.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();

    let mut sink =
        Ed2kPartFileSink::create_new(part_path, hash, TEST_FILE_SIZE).expect("create new sink");

    let chunk = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 10000,
        end_offset: 20000,
        data: Bytes::from(vec![0x11; 10000]),
    };
    sink.write_block(&sequencer, &chunk).expect("first write");

    // Exact duplicate
    let err_exact = sink.write_block(&sequencer, &chunk).unwrap_err();
    assert!(matches!(
        err_exact,
        Ed2kSequencerError::DuplicateBlock {
            start: 10000,
            end: 20000
        }
    ));

    // Overlapping at head
    let overlap_head = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 5000,
        end_offset: 15000,
        data: Bytes::from(vec![0x22; 10000]),
    };
    let err_head = sink.write_block(&sequencer, &overlap_head).unwrap_err();
    assert!(matches!(
        err_head,
        Ed2kSequencerError::DuplicateBlock {
            start: 5000,
            end: 15000
        }
    ));

    // Overlapping at tail
    let overlap_tail = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 15000,
        end_offset: 25000,
        data: Bytes::from(vec![0x33; 10000]),
    };
    let err_tail = sink.write_block(&sequencer, &overlap_tail).unwrap_err();
    assert!(matches!(
        err_tail,
        Ed2kSequencerError::DuplicateBlock {
            start: 15000,
            end: 25000
        }
    ));
}

#[test]
fn test_part_sink_cross_part_boundary_rejection() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("boundary_test.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();

    let mut sink =
        Ed2kPartFileSink::create_new(part_path, hash, TEST_FILE_SIZE).expect("create new sink");

    // Part 0 ends at 9,728,000. Try writing a chunk that crosses this boundary.
    let cross_chunk = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 9_727_000,
        end_offset: 9_728_001,
        data: Bytes::from(vec![0xFF; 1001]),
    };
    let err = sink.write_block(&sequencer, &cross_chunk).unwrap_err();
    assert!(matches!(
        err,
        Ed2kSequencerError::CrossPartBoundary {
            part_boundary: 9_728_000,
            ..
        }
    ));
}

#[test]
fn test_part_sink_atomic_journal_survives_reopen() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("resume_test.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();

    // 1. Initial session: write 20 full blocks (20 * 184,320 = 3,686,400 bytes)
    let blocks_20_len = 20 * ED2K_BLOCK_SIZE as u64;
    {
        let mut sink = Ed2kPartFileSink::create_new(part_path.clone(), hash, TEST_FILE_SIZE)
            .expect("create sink");

        let part0_blocks = sequencer.plan_part_blocks(0).expect("plan part 0");
        assert_eq!(part0_blocks.len(), 53);

        for block in &part0_blocks[0..20] {
            let chunk_len = (block.end - block.start) as usize;
            let chunk = Ed2kSendingPart {
                file_hash: hash,
                start_offset: block.start,
                end_offset: block.end,
                data: Bytes::from(vec![0x42; chunk_len]),
            };
            sink.write_block(&sequencer, &chunk).expect("write block");
        }

        sink.flush_and_sync().expect("flush and sync");
        assert_eq!(sink.total_bytes_received(), blocks_20_len);
        assert_eq!(sink.completed_ranges(), &[(0, blocks_20_len)]);
        // Sink is dropped here, closing file handles
    }

    // 2. Re-open / resume session
    {
        let resumed_sink =
            Ed2kPartFileSink::open_or_resume(part_path.clone(), hash, TEST_FILE_SIZE)
                .expect("resume sink");

        assert_eq!(resumed_sink.total_bytes_received(), blocks_20_len);
        assert_eq!(resumed_sink.completed_ranges(), &[(0, blocks_20_len)]);
        assert!(resumed_sink.has_range(0, blocks_20_len));
        assert!(!resumed_sink.has_range(0, blocks_20_len + 1));
    }
}

#[test]
fn test_sequencer_plan_missing_blocks_computes_exact_gaps() {
    let sequencer = make_sequencer();
    let all_blocks = sequencer.plan_part_blocks(0).expect("plan part 0");
    assert_eq!(all_blocks.len(), 53);

    // Simulate 20 completed blocks: [0, 3,686,400)
    let completed_ranges = vec![(0, 20 * ED2K_BLOCK_SIZE as u64)];

    let missing = sequencer
        .plan_missing_blocks(0, &completed_ranges)
        .expect("plan missing blocks");

    // Must return exactly 33 blocks (indices 20..52)
    assert_eq!(missing.len(), 33);
    assert_eq!(missing[0].start, 20 * ED2K_BLOCK_SIZE as u32);
    assert_eq!(missing[0].end, 21 * ED2K_BLOCK_SIZE as u32);
    assert_eq!(missing[32].start, 52 * ED2K_BLOCK_SIZE as u32);
    assert_eq!(missing[32].end, ED2K_PART_SIZE as u32);

    // Verify zero overlap with blocks 0..19
    for block in &missing {
        assert!(block.start >= 20 * ED2K_BLOCK_SIZE as u32);
        assert!(block.end <= ED2K_PART_SIZE as u32);
    }

    // If all 53 blocks are completed:
    let all_completed = vec![(0, ED2K_PART_SIZE as u64)];
    let missing_empty = sequencer
        .plan_missing_blocks(0, &all_completed)
        .expect("plan missing blocks when complete");
    assert!(
        missing_empty.is_empty(),
        "all blocks complete means 0 missing blocks"
    );
}

#[test]
fn test_part_sink_hash_and_size_mismatch_rejection_on_resume() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("mismatch_test.part");
    let hash = test_file_hash();

    // Create valid sink
    let sink =
        Ed2kPartFileSink::create_new(part_path.clone(), hash, TEST_FILE_SIZE).expect("create sink");
    drop(sink);

    // Attempt to open with different hash
    let wrong_hash = [0x00; 16];
    let err_hash = Ed2kPartFileSink::open_or_resume(part_path.clone(), wrong_hash, TEST_FILE_SIZE)
        .unwrap_err();
    assert!(matches!(err_hash, Ed2kSequencerError::HashMismatch { .. }));

    // Attempt to open with different size
    let wrong_size = TEST_FILE_SIZE + 1000;
    let err_size = Ed2kPartFileSink::open_or_resume(part_path, hash, wrong_size).unwrap_err();
    assert!(matches!(err_size, Ed2kSequencerError::Storage(_)));
}

#[test]
fn test_part_sink_compute_part_md4_streams_from_disk_matching_oracle() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("md4_test.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();

    let mut sink =
        Ed2kPartFileSink::create_new(part_path, hash, TEST_FILE_SIZE).expect("create new sink");

    // Part 0 is 9,728,000 bytes. Generate deterministic synthetic pattern.
    let mut part0_bytes = vec![0u8; ED2K_PART_SIZE];
    for (i, byte) in part0_bytes.iter_mut().enumerate() {
        *byte = ((i ^ (i >> 8)) & 0xFF) as u8;
    }

    // Compute direct in-memory MD4 oracle
    let mut oracle_hasher = Md4::new();
    oracle_hasher.update(&part0_bytes);
    let expected_md4: [u8; 16] = oracle_hasher.finalize().into();

    // Incomplete part check: before writing, computing MD4 must fail
    let incomplete_err = sink.compute_part_md4(0, &sequencer).unwrap_err();
    assert!(matches!(incomplete_err, Ed2kSequencerError::Storage(_)));
    assert!(!sink
        .is_part_complete(0, &sequencer)
        .expect("part complete check"));

    // Write all 53 blocks of Part 0 into sink
    let part0_blocks = sequencer.plan_part_blocks(0).expect("plan part 0");
    for block in &part0_blocks {
        let b_start = block.start as usize;
        let b_end = block.end as usize;
        let chunk = Ed2kSendingPart {
            file_hash: hash,
            start_offset: block.start,
            end_offset: block.end,
            data: Bytes::copy_from_slice(&part0_bytes[b_start..b_end]),
        };
        sink.write_block(&sequencer, &chunk).expect("write block");
    }

    assert!(sink
        .is_part_complete(0, &sequencer)
        .expect("part complete check"));
    assert_eq!(sink.total_bytes_received(), ED2K_PART_SIZE as u64);

    // Compute MD4 via streaming from disk in 64 KiB buffers
    let disk_md4 = sink
        .compute_part_md4(0, &sequencer)
        .expect("compute part md4");
    assert_eq!(
        disk_md4, expected_md4,
        "in-disk streamed MD4 must match oracle byte-for-byte"
    );
}

#[tokio::test]
async fn test_download_blocks_pipelined_with_resume_and_interruption() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("pipelined_resume_test.part");
    let hash = test_file_hash();
    let sequencer = make_sequencer();

    // Prepare 5 blocks (5 * 184,320 = 921,600 bytes) of known synthetic data
    let total_test_bytes = 5 * ED2K_BLOCK_SIZE;
    let mut file_payload = vec![0u8; total_test_bytes];
    for (i, byte) in file_payload.iter_mut().enumerate() {
        *byte = ((i ^ 0x5A) % 251) as u8;
    }

    let test_peer = Ed2kTestPeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    test_peer
        .add_shared_file_data(hash, Bytes::from(file_payload.clone()))
        .await;
    let (peer_handle, stop_signal) = test_peer.spawn_service();

    let user_hash = [0x55; 16];
    let part0_blocks = sequencer.plan_part_blocks(0).expect("plan part 0");
    let target_blocks = &part0_blocks[0..5];

    // Session 1: Download with sliding window (max 3 in-flight) but stop after exactly 2 blocks
    {
        let (mut client, _info) = Ed2kLoopbackClient::connect_and_handshake(
            peer_addr,
            user_hash,
            0xC0A80001,
            39735,
            vec![],
            0x7F000001,
            4661,
            Duration::from_millis(500),
        )
        .await
        .expect("handshake session 1");

        let mut sink = Ed2kPartFileSink::create_new(part_path.clone(), hash, TEST_FILE_SIZE)
            .expect("create sink");

        let completed = client
            .download_blocks_pipelined(
                &sequencer,
                target_blocks,
                &mut sink,
                3,
                Some(2),
                Duration::from_secs(2),
            )
            .await
            .expect("pipelined download session 1");

        assert_eq!(
            completed, 2,
            "session 1 must stop after completing exactly 2 blocks"
        );
        assert_eq!(sink.total_bytes_received(), 2 * ED2K_BLOCK_SIZE as u64);
        assert_eq!(sink.completed_ranges(), &[(0, 2 * ED2K_BLOCK_SIZE as u64)]);

        // Abrupt interruption: close client
        client.close().await;
        // sink is dropped here
    }

    // Verify on-disk intermediate state
    let intermediate_sink =
        Ed2kPartFileSink::open_or_resume(part_path.clone(), hash, TEST_FILE_SIZE)
            .expect("open intermediate sink");
    assert_eq!(
        intermediate_sink.total_bytes_received(),
        2 * ED2K_BLOCK_SIZE as u64
    );
    assert_eq!(
        intermediate_sink.completed_ranges(),
        &[(0, 2 * ED2K_BLOCK_SIZE as u64)]
    );
    drop(intermediate_sink);

    // Session 2: Resume download, compute missing blocks (blocks 2..5)
    {
        let mut resumed_sink =
            Ed2kPartFileSink::open_or_resume(part_path.clone(), hash, TEST_FILE_SIZE)
                .expect("resume sink in session 2");

        let missing = sequencer
            .plan_missing_blocks(0, resumed_sink.completed_ranges())
            .expect("plan missing blocks");

        // Out of target_blocks (0..5), missing should contain blocks 2..5
        let missing_for_target: Vec<_> = missing
            .into_iter()
            .filter(|b| b.end <= 5 * ED2K_BLOCK_SIZE as u32)
            .collect();
        assert_eq!(
            missing_for_target.len(),
            3,
            "must have 3 missing blocks remaining"
        );
        assert_eq!(missing_for_target[0].start, 2 * ED2K_BLOCK_SIZE as u32);

        let (mut client2, _info2) = Ed2kLoopbackClient::connect_and_handshake(
            peer_addr,
            user_hash,
            0xC0A80001,
            39735,
            vec![],
            0x7F000001,
            4661,
            Duration::from_millis(500),
        )
        .await
        .expect("handshake session 2");

        let completed2 = client2
            .download_blocks_pipelined(
                &sequencer,
                &missing_for_target,
                &mut resumed_sink,
                3,
                None,
                Duration::from_secs(2),
            )
            .await
            .expect("pipelined download session 2");

        assert_eq!(
            completed2, 3,
            "session 2 must complete all remaining 3 blocks"
        );
        assert_eq!(resumed_sink.total_bytes_received(), total_test_bytes as u64);
        assert_eq!(
            resumed_sink.completed_ranges(),
            &[(0, total_test_bytes as u64)]
        );

        client2.close().await;
    }

    // Verify on-disk file payload matches synthetic fixture byte-by-byte
    let disk_bytes = fs::read(&part_path).expect("read part file from disk");
    assert_eq!(
        &disk_bytes[0..total_test_bytes],
        &file_payload[..],
        "disk payload after resume must match fixture byte-for-byte"
    );

    stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
    peer_handle.abort();
}

fn generate_test_fixture_bytes() -> Vec<u8> {
    let mut data = Vec::with_capacity(TEST_FILE_SIZE as usize);
    for i in 0..TEST_FILE_SIZE as usize {
        data.push((i % 256) as u8);
    }
    data
}

#[test]
fn test_part1_residual_byte_write_and_full_file_completion() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("full_completion.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();
    let fixture = generate_test_fixture_bytes();

    let mut sink =
        Ed2kPartFileSink::create_new(part_path.clone(), hash, TEST_FILE_SIZE).expect("create sink");

    // Write all of Part 0 (9,728,000 bytes) in standard 180 KiB blocks
    let part0_blocks = sequencer.plan_part_blocks(0).expect("plan part 0");
    for block in part0_blocks {
        let chunk = Ed2kSendingPart {
            file_hash: hash,
            start_offset: block.start,
            end_offset: block.end,
            data: Bytes::copy_from_slice(&fixture[block.start as usize..block.end as usize]),
        };
        sink.write_block(&sequencer, &chunk).expect("write block");
    }

    assert_eq!(sink.total_bytes_received(), 9_728_000);
    assert!(sink
        .is_part_complete(0, &sequencer)
        .expect("part 0 complete"));
    assert!(!sink
        .is_part_complete(1, &sequencer)
        .expect("part 1 complete"));
    assert!(!sink.is_file_complete(&sequencer));

    // Plan and write Part 1: exactly 1 residual block [9,728,000, 9,728,001)
    let part1_blocks = sequencer.plan_part_blocks(1).expect("plan part 1");
    assert_eq!(part1_blocks.len(), 1, "Part 1 has exactly 1 residual block");
    assert_eq!(part1_blocks[0].start, 9_728_000);
    assert_eq!(part1_blocks[0].end, 9_728_001);

    let residual_chunk = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 9_728_000,
        end_offset: 9_728_001,
        data: Bytes::from_static(&[0x00]),
    };
    sink.write_block(&sequencer, &residual_chunk)
        .expect("write residual block");

    assert_eq!(sink.total_bytes_received(), TEST_FILE_SIZE);
    assert_eq!(sink.completed_ranges(), &[(0, TEST_FILE_SIZE)]);
    assert!(sink.is_part_complete(0, &sequencer).expect("part 0"));
    assert!(sink.is_part_complete(1, &sequencer).expect("part 1"));
    assert!(sink.is_file_complete(&sequencer));
}

#[test]
fn test_multipart_root_ed2k_hash_computation() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("hash_verification.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();
    let fixture = generate_test_fixture_bytes();

    let mut sink =
        Ed2kPartFileSink::create_new(part_path, hash, TEST_FILE_SIZE).expect("create sink");

    // Write Part 0 blocks
    for block in sequencer.plan_part_blocks(0).expect("plan part 0") {
        let chunk = Ed2kSendingPart {
            file_hash: hash,
            start_offset: block.start,
            end_offset: block.end,
            data: Bytes::copy_from_slice(&fixture[block.start as usize..block.end as usize]),
        };
        sink.write_block(&sequencer, &chunk).expect("write part 0");
    }

    // Write Part 1 residual block
    let part1_chunk = Ed2kSendingPart {
        file_hash: hash,
        start_offset: 9_728_000,
        end_offset: 9_728_001,
        data: Bytes::from_static(&[0x00]),
    };
    sink.write_block(&sequencer, &part1_chunk)
        .expect("write part 1");

    // Verify individual part MD4 digests
    let part0_md4 = sink.compute_part_md4(0, &sequencer).expect("part 0 MD4");
    assert_eq!(
        hex::encode(part0_md4),
        "240568858a65f85133d858f538d17467",
        "Part 0 MD4 must match oracle"
    );

    let part1_md4 = sink.compute_part_md4(1, &sequencer).expect("part 1 MD4");
    assert_eq!(
        hex::encode(part1_md4),
        "47c61a0fa8738ba77308a8a600f88e4b",
        "Part 1 MD4 must match MD4([0x00])"
    );

    // Verify root multi-part eD2K hash
    let root_hash = sink
        .compute_file_ed2k_hash(&sequencer)
        .expect("root eD2K hash");
    assert_eq!(
        root_hash, hash,
        "Root multi-part hash must match sealed file hash"
    );
    assert_eq!(hex::encode(root_hash), "695b81b1727666818988524bd7fc823c");
}

#[test]
fn test_two_phase_promotion_crash_recovery_and_cleanup() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("two_phase.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();
    let fixture = generate_test_fixture_bytes();

    let mut sink =
        Ed2kPartFileSink::create_new(part_path.clone(), hash, TEST_FILE_SIZE).expect("create sink");

    // Fill entire file
    for p in 0..sequencer.total_parts() {
        for block in sequencer.plan_part_blocks(p).expect("plan part") {
            let chunk = Ed2kSendingPart {
                file_hash: hash,
                start_offset: block.start,
                end_offset: block.end,
                data: Bytes::copy_from_slice(&fixture[block.start as usize..block.end as usize]),
            };
            sink.write_block(&sequencer, &chunk).expect("write chunk");
        }
    }

    assert_eq!(sink.status(), Ed2kPartDownloadStatus::Downloading);

    // Phase 1: Mark verified pending promotion
    let verified_hash = sink
        .mark_verified_pending_promotion(&sequencer)
        .expect("mark verified pending promotion");
    assert_eq!(verified_hash, hash);
    assert_eq!(
        sink.status(),
        Ed2kPartDownloadStatus::VerifiedPendingPromotion
    );

    // Verify durable state on disk
    let resume_path = sink.resume_file_path().to_path_buf();
    assert!(resume_path.exists());
    let journal = mule_protocol::ed2k::Ed2kPartResumeState::load_from_file(&resume_path)
        .expect("load journal");
    assert_eq!(
        journal.status,
        Ed2kPartDownloadStatus::VerifiedPendingPromotion
    );

    // Simulate abrupt crash: drop sink
    drop(sink);

    // Recovery on reboot: open_or_resume reloads in VerifiedPendingPromotion state
    let mut resumed_sink =
        Ed2kPartFileSink::open_or_resume(part_path.clone(), hash, TEST_FILE_SIZE)
            .expect("reopen sink after crash");
    assert_eq!(
        resumed_sink.status(),
        Ed2kPartDownloadStatus::VerifiedPendingPromotion,
        "resumed sink must retain VerifiedPendingPromotion state without contacting peer"
    );

    // Phase 2: Execute promotion to downloads directory
    let downloads_dir = dir.path().join("downloads");
    let promoted_path = resumed_sink
        .promote_to_destination(&downloads_dir, "tier3_promoted_fixture.bin")
        .expect("promote to destination");

    assert!(promoted_path.exists(), "promoted file must exist on disk");
    assert_eq!(
        fs::metadata(&promoted_path).expect("metadata").len(),
        TEST_FILE_SIZE,
        "promoted file size must match target"
    );

    // Verify payload on disk byte-for-byte
    let disk_bytes = fs::read(&promoted_path).expect("read promoted file");
    assert_eq!(
        &disk_bytes, &fixture,
        "promoted content must match fixture byte-for-byte"
    );

    // Verify cleanup
    assert!(
        !part_path.exists(),
        ".part file must be removed after promotion"
    );
    assert!(
        !resume_path.exists(),
        ".resume.json journal must be deleted after promotion"
    );
    assert_eq!(resumed_sink.status(), Ed2kPartDownloadStatus::Promoted);
}

#[test]
fn test_promotion_collision_protection() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("collision.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();
    let fixture = generate_test_fixture_bytes();

    let mut sink =
        Ed2kPartFileSink::create_new(part_path.clone(), hash, TEST_FILE_SIZE).expect("create sink");

    for p in 0..sequencer.total_parts() {
        for block in sequencer.plan_part_blocks(p).expect("plan part") {
            let chunk = Ed2kSendingPart {
                file_hash: hash,
                start_offset: block.start,
                end_offset: block.end,
                data: Bytes::copy_from_slice(&fixture[block.start as usize..block.end as usize]),
            };
            sink.write_block(&sequencer, &chunk).expect("write chunk");
        }
    }

    sink.mark_verified_pending_promotion(&sequencer)
        .expect("verify");

    let dest_dir = dir.path().join("downloads");
    fs::create_dir_all(&dest_dir).expect("create dest dir");
    let existing_dest = dest_dir.join("existing_target.bin");
    fs::write(&existing_dest, b"pre-existing unyielding file").expect("write existing");

    // Promotion must fail safely when target already exists
    let err = sink
        .promote_to_destination(&dest_dir, "existing_target.bin")
        .expect_err("must refuse to overwrite existing file");

    match err {
        Ed2kSequencerError::Storage(msg) => {
            assert!(
                msg.contains("already exists"),
                "error message must mention collision: {msg}"
            );
        }
        other => panic!("expected Storage error, got {other:?}"),
    }

    // Assert source .part and pre-existing file remain intact
    assert!(part_path.exists(), ".part file must remain untouched");
    assert_eq!(
        fs::read(&existing_dest).expect("read"),
        b"pre-existing unyielding file"
    );
}

#[test]
fn test_promotion_refusal_when_incomplete() {
    let dir = tempdir().expect("tempdir");
    let part_path = dir.path().join("incomplete.part");
    let sequencer = make_sequencer();
    let hash = test_file_hash();

    let mut sink =
        Ed2kPartFileSink::create_new(part_path.clone(), hash, TEST_FILE_SIZE).expect("create sink");

    // Sink has 0 bytes downloaded
    let dest_dir = dir.path().join("downloads");
    let promo_err = sink
        .promote_to_destination(&dest_dir, "premature.bin")
        .expect_err("must refuse promotion when Downloading");

    match promo_err {
        Ed2kSequencerError::Storage(msg) => {
            assert!(
                msg.contains("VerifiedPendingPromotion"),
                "must require VerifiedPendingPromotion: {msg}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let verify_err = sink
        .mark_verified_pending_promotion(&sequencer)
        .expect_err("must refuse verification when incomplete");

    match verify_err {
        Ed2kSequencerError::Storage(msg) => {
            assert!(
                msg.contains("incomplete"),
                "must report file is incomplete: {msg}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
