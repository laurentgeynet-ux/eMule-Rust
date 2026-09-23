// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Swarm coordinator constants and internal state structure.

use crate::state::AppState;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Interval between coordinator evaluation ticks.
pub const COORDINATOR_TICK_INTERVAL: Duration = Duration::from_secs(3);

/// Minimum elapsed time before re-querying sources for the same file hash.
pub const SOURCE_REQUERY_INTERVAL: Duration = Duration::from_secs(300);

/// Maximum number of new peer connections to initiate per file per tick
/// (anti-burst par fichier). Le pacing global reste assuré par
/// `ResourceBudgets.rate_limit_per_min` (240/min = eMule
/// `MaxConnectionsPerFiveSeconds` 20 ≈ 4/s) appliqué à chaque
/// `request_tcp_connect` — 12/tick à 3 s ≈ 4/s, donc un fichier seul peut
/// consommer tout le rythme eMule sans le dépasser.
pub const MAX_CONNECT_PER_TICK: usize = 12;

/// Interval between periodic system heartbeats (60 seconds).
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);

/// Minimum delay between two automatic server (re)connect attempts.
pub const SERVER_RECONNECT_INTERVAL: Duration = Duration::from_secs(60);

/// Per-address backoff before retrying a server that failed to connect.
pub const SERVER_FAIL_COOLDOWN: Duration = Duration::from_secs(300);

/// Minimum interval between excess-source purge passes per file
/// (eMule `CPartFile::Process` slow timers run every ~30-60s).
pub const SOURCE_PURGE_INTERVAL: Duration = Duration::from_secs(60);

/// Full A4AF rebalance cadence (eMule `ProcessA4AFClients` ~8 min).
/// Event-driven NNP swaps still run on every tick.
pub const A4AF_FULL_INTERVAL: Duration = Duration::from_secs(8 * 60);

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
    /// Last excess-source purge pass per file hash (eMule 80% rule).
    pub(crate) last_source_purge: HashMap<String, Instant>,
    /// Last full A4AF rebalance pass (~8 min eMule cadence).
    pub(crate) last_a4af_full: Instant,
}
