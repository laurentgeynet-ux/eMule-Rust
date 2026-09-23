// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Tracking and correlation of in-flight block requests.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::limits::MAX_IN_FLIGHT_REQUESTS;
use crate::traits::BlockRequest;

/// Errors arising from in-flight request tracking violations.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum InFlightError {
    /// In-flight table has reached maximum concurrent capacity (8).
    #[error("Maximum in-flight requests limit ({limit}) reached")]
    CapacityReached {
        /// Active in-flight count.
        limit: usize,
    },

    /// An identical block request is already pending.
    #[error("Duplicate block request for offset {start_offset}")]
    DuplicateRequest {
        /// Start offset in file.
        start_offset: u64,
    },

    /// Received block was never requested.
    #[error("Unsolicited block data received for offset {start_offset}")]
    UnsolicitedBlock {
        /// Start offset in file.
        start_offset: u64,
    },

    /// Received rejection was never requested.
    #[error("Unsolicited block rejection received for offset {start_offset}")]
    UnsolicitedRejection {
        /// Start offset in file.
        start_offset: u64,
    },

    /// Received block length does not match the requested size.
    #[error("Block length mismatch for offset {start_offset}: expected {expected}, got {actual}")]
    LengthMismatch {
        /// Start offset in file.
        start_offset: u64,
        /// Expected byte length.
        expected: u32,
        /// Actual byte length.
        actual: usize,
    },
}

#[derive(Debug, Clone)]
struct InFlightEntry {
    length: u32,
    requested_at: Instant,
}

/// Tracks and validates client block requests currently in flight.
#[derive(Debug, Clone, Default)]
pub struct InFlightTracker {
    pending: HashMap<([u8; 16], u64), InFlightEntry>,
}

impl InFlightTracker {
    /// Creates a new empty `InFlightTracker`.
    pub fn new() -> Self {
        Self {
            pending: HashMap::with_capacity(MAX_IN_FLIGHT_REQUESTS),
        }
    }

    /// Returns true if a new block request may be issued.
    pub fn can_request(&self) -> bool {
        self.pending.len() < MAX_IN_FLIGHT_REQUESTS
    }

    /// Returns the number of currently active in-flight requests.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Returns true if there are no pending requests.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Registers an outgoing block request.
    pub fn record_request(&mut self, req: &BlockRequest) -> Result<(), InFlightError> {
        if self.pending.len() >= MAX_IN_FLIGHT_REQUESTS {
            return Err(InFlightError::CapacityReached {
                limit: MAX_IN_FLIGHT_REQUESTS,
            });
        }

        let key = (req.file_hash, req.start_offset);
        if self.pending.contains_key(&key) {
            return Err(InFlightError::DuplicateRequest {
                start_offset: req.start_offset,
            });
        }

        self.pending.insert(
            key,
            InFlightEntry {
                length: req.length,
                requested_at: Instant::now(),
            },
        );
        Ok(())
    }

    /// Validates an incoming block against pending requests, and consumes it if valid.
    pub fn validate_and_remove(
        &mut self,
        file_hash: &[u8; 16],
        start_offset: u64,
        actual_len: usize,
    ) -> Result<(), InFlightError> {
        let key = (*file_hash, start_offset);
        match self.pending.remove(&key) {
            None => Err(InFlightError::UnsolicitedBlock { start_offset }),
            Some(entry) => {
                if entry.length as usize != actual_len {
                    // Re-insert on mismatch to keep state consistent for failure reporting
                    self.pending.insert(key, entry.clone());
                    Err(InFlightError::LengthMismatch {
                        start_offset,
                        expected: entry.length,
                        actual: actual_len,
                    })
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Handles an incoming block rejection, freeing capacity and returning the cancelled request.
    pub fn handle_rejection(
        &mut self,
        file_hash: &[u8; 16],
        start_offset: u64,
    ) -> Result<BlockRequest, InFlightError> {
        let key = (*file_hash, start_offset);
        match self.pending.remove(&key) {
            None => Err(InFlightError::UnsolicitedRejection { start_offset }),
            Some(entry) => Ok(BlockRequest::new(*file_hash, start_offset, entry.length)),
        }
    }

    /// Identifies and extracts expired block requests exceeding `timeout`, restoring capacity.
    pub fn reap_timeouts(&mut self, timeout: Duration) -> Vec<BlockRequest> {
        let now = Instant::now();
        let mut expired_keys = Vec::new();

        for (key, entry) in &self.pending {
            if now.duration_since(entry.requested_at) >= timeout {
                expired_keys.push(*key);
            }
        }

        let mut expired_requests = Vec::with_capacity(expired_keys.len());
        for key in expired_keys {
            if let Some(entry) = self.pending.remove(&key) {
                expired_requests.push(BlockRequest::new(key.0, key.1, entry.length));
            }
        }

        expired_requests
    }

    /// Cancels all pending in-flight requests (e.g. on peer disconnect), restoring full capacity.
    pub fn cancel_all(&mut self) -> Vec<BlockRequest> {
        let mut requests = Vec::with_capacity(self.pending.len());
        for (key, entry) in self.pending.drain() {
            requests.push(BlockRequest::new(key.0, key.1, entry.length));
        }
        requests
    }

    /// Clears all pending requests.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}
