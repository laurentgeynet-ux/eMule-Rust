//! Swarm coordinator constants and internal state structure.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use crate::state::AppState;

/// Interval between coordinator evaluation ticks.
pub const COORDINATOR_TICK_INTERVAL: Duration = Duration::from_secs(3);

/// Minimum elapsed time before re-querying sources for the same file hash.
pub const SOURCE_REQUERY_INTERVAL: Duration = Duration::from_secs(300);

/// Maximum number of new peer connections to initiate per file per tick (burst prevention).
pub const MAX_CONNECT_PER_TICK: usize = 5;

/// Swarm coordinator orchestrating peer connections and background downloads.
pub struct SwarmCoordinator {
    pub(crate) state: AppState,
    pub(crate) last_source_query: HashMap<String, Instant>,
}
