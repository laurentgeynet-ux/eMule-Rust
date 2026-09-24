// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Swarm Coordinator Service.
//!
//! Autonomous background worker that continuously orchestrates:
//! 1. Source discovery: queries connected eD2K server (OP_GETSOURCES) and Kad (search_sources).
//! 2. Swarm peer connection management: selects candidate peer sources from the database,
//!    requests network policy permits, and establishes peer sessions up to MAX_PEERS_PER_DOWNLOAD.
//! 3. Lifecycle state propagation: automatically moves downloads from Queued/Waiting to Downloading.

pub mod strategy;
pub mod types;

use std::collections::HashMap;
use std::time::Instant;
pub use types::{
    SwarmCoordinator, SwarmLimits, COORDINATOR_TICK_INTERVAL, HEARTBEAT_INTERVAL,
    MAX_CONNECT_PER_TICK,
};

use crate::state::AppState;

impl SwarmCoordinator {
    /// Spawns the autonomous swarm coordinator loop as a background task.
    pub fn start(state: AppState) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut coordinator = Self {
                state,
                last_source_query_tcp: HashMap::new(),
                last_source_query_udp: HashMap::new(),
                kad_source_search: HashMap::new(),
                last_kad_file_request: Instant::now(),
                last_heartbeat: Instant::now(),
                server_attempts: HashMap::new(),
                last_server_reconnect: None,
                last_connected_server: None,
                last_source_purge: HashMap::new(),
                last_a4af_full: Instant::now(),
            };
            coordinator.run_loop().await;
        })
    }

    /// Primary execution loop listening for ticks or daemon shutdown.
    /// Le kill switch n'arrête pas la boucle : `tick()` court-circuite
    /// tant que l'état réseau est `Disabled`, puis reprend seul à
    /// `network.enable` (le token annulé serait sinon définitif).
    pub async fn run_loop(&mut self) {
        let shutdown_notify = self.state.shutdown_notify.clone();
        let mut interval = tokio::time::interval(COORDINATOR_TICK_INTERVAL);

        tracing::info!("SwarmCoordinator background task started");

        loop {
            tokio::select! {
                _ = shutdown_notify.notified() => {
                    tracing::info!("SwarmCoordinator stopped by shutdown notification");
                    break;
                }
                _ = interval.tick() => {
                    self.tick().await;
                }
            }
        }
    }
}
