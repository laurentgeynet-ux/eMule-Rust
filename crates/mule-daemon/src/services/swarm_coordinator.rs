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
pub use types::{
    SwarmCoordinator, COORDINATOR_TICK_INTERVAL, MAX_CONNECT_PER_TICK,
    SOURCE_REQUERY_INTERVAL,
};

use crate::state::AppState;

impl SwarmCoordinator {
    /// Spawns the autonomous swarm coordinator loop as a background task.
    pub fn start(state: AppState) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut coordinator = Self {
                state,
                last_source_query: HashMap::new(),
            };
            coordinator.run_loop().await;
        })
    }

    /// Primary execution loop listening for ticks, cancellation, or daemon shutdown.
    pub async fn run_loop(&mut self) {
        let cancel_token = self.state.network_policy.cancellation_token();
        let shutdown_notify = self.state.shutdown_notify.clone();
        let mut interval = tokio::time::interval(COORDINATOR_TICK_INTERVAL);

        tracing::info!("SwarmCoordinator background task started");

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    tracing::info!("SwarmCoordinator stopped by cancellation token");
                    break;
                }
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
