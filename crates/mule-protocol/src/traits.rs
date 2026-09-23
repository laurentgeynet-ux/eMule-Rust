// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! High-level asynchronous traits decoupling the wire protocol engine from
//! underlying storage (`mule-core`) and piece scheduling logic.

use std::future::Future;
use std::pin::Pin;

use bytes::Bytes;

use crate::error::ProtocolError;
use crate::types::PeerId;

/// A contiguous slice of file data received from a peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedBlock {
    /// 16-byte eD2K file hash.
    pub file_hash: [u8; 16],
    /// Absolute byte offset within the file where this block begins.
    pub start_offset: u64,
    /// Binary data payload for this block.
    pub data: Bytes,
}

impl ReceivedBlock {
    /// Creates a new `ReceivedBlock`.
    pub fn new(file_hash: [u8; 16], start_offset: u64, data: Bytes) -> Self {
        Self {
            file_hash,
            start_offset,
            data,
        }
    }

    /// End byte offset (exclusive: `start_offset + data.len()`).
    #[inline]
    pub fn end_offset(&self) -> u64 {
        self.start_offset + self.data.len() as u64
    }

    /// Length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns true if this block has empty data.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// The result returned by a `DownloadSink` after persisting a received block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockWriteOutcome {
    /// Number of bytes successfully committed to storage.
    pub bytes_written: usize,
    /// Indicates whether this block completed an eD2K part (9.5 MB) boundary.
    pub part_completed: bool,
    /// Part index that completed, if applicable.
    pub completed_part_index: Option<u32>,
    /// Indicates whether this write completed the entire target file.
    pub file_completed: bool,
}

/// Abstract storage sink for writing downloaded peer blocks.
///
/// Implementors handle the physical disk writes, sparse `.part` file updates,
/// and hash integrity checks without coupling to the network protocol layer.
pub trait DownloadSink: Send + Sync {
    /// Persists a downloaded block into storage.
    fn write_block(
        &self,
        block: ReceivedBlock,
    ) -> Pin<Box<dyn Future<Output = Result<BlockWriteOutcome, ProtocolError>> + Send + '_>>;
}

/// Request for a single block of data from a peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRequest {
    /// 16-byte eD2K file hash being requested.
    pub file_hash: [u8; 16],
    /// Absolute byte offset where the requested block begins.
    pub start_offset: u64,
    /// Requested block length in bytes (standard is 184,320 bytes = 180 KiB).
    pub length: u32,
}

impl BlockRequest {
    /// Creates a new `BlockRequest`.
    pub fn new(file_hash: [u8; 16], start_offset: u64, length: u32) -> Self {
        Self {
            file_hash,
            start_offset,
            length,
        }
    }

    /// End byte offset (exclusive: `start_offset + length`).
    #[inline]
    pub fn end_offset(&self) -> u64 {
        self.start_offset + self.length as u64
    }
}

/// State update describing a remote peer's status and parts availability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceStateUpdate {
    /// Peer identifier.
    pub peer_id: PeerId,
    /// 16-byte eD2K file hash.
    pub file_hash: [u8; 16],
    /// Bitmask of part availability (true = peer has this part).
    pub available_parts: Vec<bool>,
    /// Optional remote upload queue rank / position.
    pub queue_rank: Option<u32>,
}

/// Abstract planner responsible for download scheduling, piece selection,
/// and managing requests across active peers.
pub trait DownloadPlanner: Send + Sync {
    /// Retrieves the next block request to send to a specific peer, if any block is eligible.
    fn next_request(
        &self,
        peer_id: &PeerId,
    ) -> Pin<Box<dyn Future<Output = Option<BlockRequest>> + Send + '_>>;

    /// Notifies the planner of an update in remote peer state (parts bitmask, queue position, etc.).
    fn on_source_state(
        &self,
        update: SourceStateUpdate,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}
