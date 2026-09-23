// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenBucketError {
    #[error("Rate limit wait was cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitsConfig {
    /// Download rate limit in bytes per second (0 = unlimited)
    pub download_bytes_per_sec: u64,
    /// Upload rate limit in bytes per second (0 = unlimited)
    pub upload_bytes_per_sec: u64,
    /// Connection creation rate limit in connections per minute (0 = unlimited)
    pub connections_per_minute: u32,
    /// Server search and interaction requests per second (0 = unlimited)
    pub server_requests_per_sec: u32,
    /// Kad/UDP packets per second (0 = unlimited)
    pub kad_packets_per_sec: u32,
}

impl Default for RateLimitsConfig {
    fn default() -> Self {
        Self {
            download_bytes_per_sec: 0,   // unlimited
            upload_bytes_per_sec: 0,     // unlimited
            connections_per_minute: 120, // 2 per second max by default
            server_requests_per_sec: 10,
            kad_packets_per_sec: 50,
        }
    }
}

/// Token bucket for rate limiting bytes or operations.
/// Supports burst allowance, dynamic refill, and cancellation.
pub struct TokenBucket {
    rate_per_sec: AtomicU64,
    burst_capacity: AtomicU64,
    state: Mutex<BucketState>,
}

struct BucketState {
    tokens: f64,
    last_update: Instant,
}

impl TokenBucket {
    pub fn new(rate_per_sec: u64, burst_capacity: u64) -> Self {
        Self {
            rate_per_sec: AtomicU64::new(rate_per_sec),
            burst_capacity: AtomicU64::new(burst_capacity),
            state: Mutex::new(BucketState {
                tokens: burst_capacity as f64,
                last_update: Instant::now(),
            }),
        }
    }

    pub fn set_rate(&self, rate_per_sec: u64, burst_capacity: u64) {
        self.rate_per_sec.store(rate_per_sec, Ordering::Relaxed);
        self.burst_capacity.store(burst_capacity, Ordering::Relaxed);
        let mut state = self.state.lock().unwrap();
        state.last_update = Instant::now();
        if state.tokens > burst_capacity as f64 {
            state.tokens = burst_capacity as f64;
        }
    }

    pub fn rate_per_sec(&self) -> u64 {
        self.rate_per_sec.load(Ordering::Relaxed)
    }

    pub fn burst_capacity(&self) -> u64 {
        self.burst_capacity.load(Ordering::Relaxed)
    }

    /// Tries to acquire `count` tokens immediately without waiting.
    /// Returns `true` if tokens were acquired, `false` otherwise.
    pub fn try_acquire(&self, count: u64) -> bool {
        let rate = self.rate_per_sec.load(Ordering::Relaxed);
        if rate == 0 {
            // 0 means unlimited
            return true;
        }

        let mut state = self.state.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_update).as_secs_f64();
        let burst = self.burst_capacity.load(Ordering::Relaxed) as f64;

        state.tokens = (state.tokens + elapsed * (rate as f64)).min(burst);
        state.last_update = now;

        if state.tokens >= count as f64 {
            state.tokens -= count as f64;
            true
        } else {
            false
        }
    }

    /// Asynchronously acquires `count` tokens, sleeping if necessary.
    /// If the `cancel` callback or cancellation token fires, returns `Err(TokenBucketError::Cancelled)`.
    pub async fn acquire_tokens<F>(
        &self,
        count: u64,
        is_cancelled: F,
    ) -> Result<(), TokenBucketError>
    where
        F: Fn() -> bool,
    {
        let rate = self.rate_per_sec.load(Ordering::Relaxed);
        if rate == 0 {
            return Ok(());
        }

        loop {
            if is_cancelled() {
                return Err(TokenBucketError::Cancelled);
            }

            let wait_duration = {
                let mut state = self.state.lock().unwrap();
                let now = Instant::now();
                let elapsed = now.duration_since(state.last_update).as_secs_f64();
                let burst = self.burst_capacity.load(Ordering::Relaxed) as f64;

                state.tokens = (state.tokens + elapsed * (rate as f64)).min(burst);
                state.last_update = now;

                if state.tokens >= count as f64 {
                    state.tokens -= count as f64;
                    return Ok(());
                }

                // Needed tokens
                let needed = (count as f64) - state.tokens;
                let wait_secs = needed / (rate as f64);
                Duration::from_secs_f64(wait_secs.clamp(0.005, 2.0))
            };

            // Sleep in small increments or for wait_duration
            tokio::select! {
                _ = tokio::time::sleep(wait_duration) => {
                    // loop around and recheck
                }
            }
        }
    }
}
