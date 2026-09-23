// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Request and block in-flight trackers for eD2K sessions.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::types::Ed2kSessionError;

/// Metadata for an in-flight file status request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStatusRequest {
    /// 16-byte MD4 file hash requested.
    pub file_hash: [u8; 16],
    /// Instant when the request was dispatched.
    pub sent_at: Instant,
}

/// Bounded tracking structure for pending `OP_FILESTATUSREQ` exchanges.
#[derive(Debug, Clone)]
pub struct FileStatusTracker {
    pending: HashMap<[u8; 16], FileStatusRequest>,
    max_pending: usize,
}

impl Default for FileStatusTracker {
    fn default() -> Self {
        Self::new(16)
    }
}

impl FileStatusTracker {
    /// Creates a new tracker with a maximum in-flight capacity.
    pub fn new(max_pending: usize) -> Self {
        Self {
            pending: HashMap::with_capacity(max_pending.min(32)),
            max_pending,
        }
    }

    /// Registers a newly sent file status request.
    pub fn register_request(
        &mut self,
        file_hash: [u8; 16],
        now: Instant,
    ) -> Result<(), Ed2kSessionError> {
        if self.pending.contains_key(&file_hash) {
            return Err(Ed2kSessionError::DuplicateFileStatusRequest(hex::encode(
                file_hash,
            )));
        }
        if self.pending.len() >= self.max_pending {
            return Err(Ed2kSessionError::TooManyPendingRequests(self.max_pending));
        }
        self.pending.insert(
            file_hash,
            FileStatusRequest {
                file_hash,
                sent_at: now,
            },
        );
        Ok(())
    }

    /// Validates an incoming response and removes the pending entry.
    pub fn validate_and_remove(
        &mut self,
        file_hash: &[u8; 16],
    ) -> Result<FileStatusRequest, Ed2kSessionError> {
        self.pending
            .remove(file_hash)
            .ok_or_else(|| Ed2kSessionError::UnsolicitedFileStatus(hex::encode(file_hash)))
    }

    /// Checks if a request is currently pending for the specified hash.
    pub fn is_pending(&self, file_hash: &[u8; 16]) -> bool {
        self.pending.contains_key(file_hash)
    }

    /// Returns the number of currently pending requests.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Removes and returns hashes of all requests that exceeded the timeout threshold.
    pub fn expire_timed_out(&mut self, timeout: Duration, now: Instant) -> Vec<[u8; 16]> {
        let mut expired = Vec::new();
        self.pending.retain(|hash, req| {
            if now.saturating_duration_since(req.sent_at) >= timeout {
                expired.push(*hash);
                false
            } else {
                true
            }
        });
        expired
    }

    /// Clears all pending requests (e.g. on disconnection).
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

/// Default maximum concurrent in-flight block requests per peer (8).
pub const MAX_IN_FLIGHT_BLOCK_REQUESTS: usize = 8;

/// Composite key uniquely identifying an in-flight eD2K block request.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct BlockRequestKey {
    /// 16-byte MD4 eD2K file hash.
    pub file_hash: [u8; 16],
    /// Absolute start byte offset.
    pub start: u64,
    /// Absolute end byte offset (exclusive).
    pub end: u64,
}

impl BlockRequestKey {
    /// Creates a new `BlockRequestKey`.
    pub fn new(file_hash: [u8; 16], start: u64, end: u64) -> Self {
        Self {
            file_hash,
            start,
            end,
        }
    }

    /// Length in bytes of the requested block range.
    #[inline]
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// Returns true if start >= end.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }
}

/// Information tracking an active in-flight block request.
#[derive(Clone, Debug)]
pub struct PendingBlockRequest {
    /// Unique composite key.
    pub key: BlockRequestKey,
    /// Timestamp when this request was issued.
    pub requested_at: Instant,
    /// Retry attempt counter.
    pub attempt: u16,
    /// Byte ranges received so far for this request.
    pub received_ranges: Vec<(u64, u64)>,
}

/// Bounded tracking structure for pending `OP_REQUESTPARTS` exchanges.
#[derive(Debug, Clone)]
pub struct BlockRequestTracker {
    pending: HashMap<BlockRequestKey, PendingBlockRequest>,
    max_pending: usize,
}

impl Default for BlockRequestTracker {
    fn default() -> Self {
        Self::new(MAX_IN_FLIGHT_BLOCK_REQUESTS)
    }
}

impl BlockRequestTracker {
    /// Creates a new tracker with a maximum in-flight capacity.
    pub fn new(max_pending: usize) -> Self {
        Self {
            pending: HashMap::with_capacity(max_pending.min(32)),
            max_pending,
        }
    }

    /// Returns true if the tracker has capacity for more requests.
    #[inline]
    pub fn can_request(&self) -> bool {
        self.pending.len() < self.max_pending
    }

    /// Returns the number of active in-flight block requests.
    #[inline]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns true if there are no pending block requests.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Checks if an exact request key is currently pending.
    pub fn is_pending(&self, key: &BlockRequestKey) -> bool {
        self.pending.contains_key(key)
    }

    /// Checks if any pending request for the given file hash overlaps [start..end].
    pub fn has_overlap(&self, file_hash: &[u8; 16], start: u64, end: u64) -> bool {
        self.pending
            .keys()
            .any(|k| &k.file_hash == file_hash && k.start < end && start < k.end)
    }

    /// Registers a newly sent block request.
    pub fn register_request(
        &mut self,
        key: BlockRequestKey,
        now: Instant,
    ) -> Result<(), Ed2kSessionError> {
        if key.start >= key.end {
            return Err(Ed2kSessionError::InvalidBlockRequest(format!(
                "start offset {} must be strictly less than end offset {}",
                key.start, key.end
            )));
        }

        if self.pending.len() >= self.max_pending {
            return Err(Ed2kSessionError::TooManyPendingBlockRequests(
                self.max_pending,
            ));
        }

        if self.has_overlap(&key.file_hash, key.start, key.end) {
            return Err(Ed2kSessionError::DuplicateBlockRequest(format!(
                "{}:{}-{}",
                hex::encode(key.file_hash),
                key.start,
                key.end
            )));
        }

        self.pending.insert(
            key.clone(),
            PendingBlockRequest {
                key,
                requested_at: now,
                attempt: 1,
                received_ranges: Vec::new(),
            },
        );

        Ok(())
    }

    /// Validates an incoming chunk response and removes the pending entry when fully received.
    pub fn validate_and_remove(
        &mut self,
        file_hash: &[u8; 16],
        start: u64,
        end: u64,
    ) -> Result<PendingBlockRequest, Ed2kSessionError> {
        if start >= end {
            return Err(Ed2kSessionError::UnsolicitedBlock(format!(
                "{}:{}-{}",
                hex::encode(file_hash),
                start,
                end
            )));
        }

        let matching_key = self
            .pending
            .keys()
            .find(|k| &k.file_hash == file_hash && start >= k.start && end <= k.end)
            .cloned();

        let key = matching_key.ok_or_else(|| {
            Ed2kSessionError::UnsolicitedBlock(format!(
                "{}:{}-{}",
                hex::encode(file_hash),
                start,
                end
            ))
        })?;

        let is_complete = {
            let req = match self.pending.get_mut(&key) {
                Some(r) => r,
                None => {
                    return Err(Ed2kSessionError::UnsolicitedBlock(format!(
                        "{}:{}-{}",
                        hex::encode(file_hash),
                        start,
                        end
                    )));
                }
            };

            if req
                .received_ranges
                .iter()
                .any(|&(s, e)| s == start && e == end)
            {
                return Err(Ed2kSessionError::DuplicateBlock(format!(
                    "{}:{}-{}",
                    hex::encode(file_hash),
                    start,
                    end
                )));
            }

            req.received_ranges.push((start, end));
            let total_received: u64 = req.received_ranges.iter().map(|&(s, e)| e - s).sum();
            total_received >= key.len()
        };

        if is_complete {
            self.pending.remove(&key).ok_or_else(|| {
                Ed2kSessionError::UnsolicitedBlock(format!(
                    "{}:{}-{}",
                    hex::encode(file_hash),
                    start,
                    end
                ))
            })
        } else {
            self.pending.get(&key).cloned().ok_or_else(|| {
                Ed2kSessionError::UnsolicitedBlock(format!(
                    "{}:{}-{}",
                    hex::encode(file_hash),
                    start,
                    end
                ))
            })
        }
    }

    /// Removes and returns all requests that exceeded the timeout threshold.
    pub fn reap_timeouts(&mut self, timeout: Duration, now: Instant) -> Vec<PendingBlockRequest> {
        let mut expired = Vec::new();
        self.pending.retain(|_, req| {
            if now.saturating_duration_since(req.requested_at) >= timeout {
                expired.push(req.clone());
                false
            } else {
                true
            }
        });
        expired
    }

    /// Clears all in-flight requests and returns them.
    pub fn cancel_all(&mut self) -> Vec<PendingBlockRequest> {
        self.pending.drain().map(|(_, req)| req).collect()
    }
}
