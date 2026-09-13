//! Tracking and correlation of in-flight block requests.

use std::collections::HashMap;
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

/// Tracks and validates client block requests currently in flight.
#[derive(Debug, Clone, Default)]
pub struct InFlightTracker {
    pending: HashMap<([u8; 16], u64), u32>,
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

        self.pending.insert(key, req.length);
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
            Some(expected) => {
                if expected as usize != actual_len {
                    // Re-insert on mismatch to keep state consistent for failure reporting
                    self.pending.insert(key, expected);
                    Err(InFlightError::LengthMismatch {
                        start_offset,
                        expected,
                        actual: actual_len,
                    })
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Clears all pending requests.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}
