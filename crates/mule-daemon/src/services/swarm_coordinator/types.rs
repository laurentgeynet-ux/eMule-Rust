//! Swarm coordinator constants and internal state structure.

use crate::state::AppState;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Interval between coordinator evaluation ticks.
pub const COORDINATOR_TICK_INTERVAL: Duration = Duration::from_secs(3);

/// Minimum elapsed time before re-querying sources for the same file hash.
pub const SOURCE_REQUERY_INTERVAL: Duration = Duration::from_secs(300);

/// Maximum number of new peer connections to initiate per file per tick (burst prevention).
pub const MAX_CONNECT_PER_TICK: usize = 5;

/// Interval between periodic system heartbeats (60 seconds).
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);

/// Minimum delay between two automatic server (re)connect attempts.
pub const SERVER_RECONNECT_INTERVAL: Duration = Duration::from_secs(60);

/// Per-address backoff before retrying a server that failed to connect.
pub const SERVER_FAIL_COOLDOWN: Duration = Duration::from_secs(300);

/// Swarm coordinator orchestrating peer connections and background downloads.
pub struct SwarmCoordinator {
    pub(crate) state: AppState,
    pub(crate) last_source_query: HashMap<String, Instant>,
    pub(crate) last_heartbeat: Instant,
    /// Last connect attempt per server address (failover backoff).
    pub(crate) server_attempts: HashMap<String, Instant>,
    /// Last time an automatic server connect was attempted.
    pub(crate) last_server_reconnect: Option<Instant>,
    /// Server address that was connected before the last drop (excluded
    /// from the immediate failover pick).
    pub(crate) last_connected_server: Option<String>,
}
