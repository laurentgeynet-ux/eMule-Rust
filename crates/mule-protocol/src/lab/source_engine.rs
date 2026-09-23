// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Secure in-memory test file source engine for laboratory loopback transfers.

use std::collections::HashMap;

use bytes::Bytes;

use super::messages::{LabMessage, LabRejectReason};
use crate::limits::MAX_BLOCK_BYTES;

/// A fixture file hosted by the laboratory source engine.
#[derive(Debug, Clone)]
pub struct HostedFile {
    /// 16-byte eD2K file hash.
    pub file_hash: [u8; 16],
    /// Total file size in bytes.
    pub file_size: u64,
    /// Display name.
    pub name: String,
    /// In-memory binary data payload.
    pub data: Bytes,
}

/// In-memory source server for laboratory testing.
///
/// Disallows filesystem path inputs from the network and serves strictly
/// from an internal registry indexed by 16-byte hashes.
#[derive(Debug, Clone, Default)]
pub struct LabSourceEngine {
    files: HashMap<[u8; 16], HostedFile>,
}

impl LabSourceEngine {
    /// Creates a new empty `LabSourceEngine`.
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Registers a fixture file into the source engine.
    pub fn host_file(&mut self, file_hash: [u8; 16], name: impl Into<String>, data: Bytes) {
        let file_size = data.len() as u64;
        self.files.insert(
            file_hash,
            HostedFile {
                file_hash,
                file_size,
                name: name.into(),
                data,
            },
        );
    }

    /// Returns the hashes of all hosted files.
    pub fn hosted_hashes(&self) -> Vec<[u8; 16]> {
        self.files.keys().copied().collect()
    }

    /// Generates a `FileOffer` message for the specified hash if hosted.
    pub fn offer_file(&self, file_hash: &[u8; 16]) -> Option<LabMessage> {
        self.files.get(file_hash).map(|f| LabMessage::FileOffer {
            file_hash: f.file_hash,
            file_size: f.file_size,
            name: f.name.clone(),
        })
    }

    /// Serves a block request safely according to protocol bounds.
    pub fn serve_block(&self, file_hash: &[u8; 16], start_offset: u64, length: u32) -> LabMessage {
        let file = match self.files.get(file_hash) {
            Some(f) => f,
            None => {
                return LabMessage::BlockRejected {
                    file_hash: *file_hash,
                    start_offset,
                    reason: LabRejectReason::FileNotFound,
                }
            }
        };

        if length == 0 || (length as usize) > MAX_BLOCK_BYTES {
            return LabMessage::BlockRejected {
                file_hash: *file_hash,
                start_offset,
                reason: LabRejectReason::InvalidRange,
            };
        }

        let end_offset = match start_offset.checked_add(length as u64) {
            Some(end) => end,
            None => {
                return LabMessage::BlockRejected {
                    file_hash: *file_hash,
                    start_offset,
                    reason: LabRejectReason::InvalidRange,
                }
            }
        };

        if end_offset > file.file_size {
            return LabMessage::BlockRejected {
                file_hash: *file_hash,
                start_offset,
                reason: LabRejectReason::InvalidRange,
            };
        }

        let start = start_offset as usize;
        let end = end_offset as usize;
        let block_slice = file.data.slice(start..end);

        LabMessage::BlockData {
            file_hash: *file_hash,
            start_offset,
            data: block_slice,
        }
    }
}
