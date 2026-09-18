//! Swarm Coordinator Service.
//!
//! Autonomous background worker that continuously orchestrates:
//! 1. Source discovery: queries connected eD2K server (OP_GETSOURCES) and Kad (search_sources).
//! 2. Swarm peer connection management: selects candidate peer sources from the database,
//!    requests network policy permits, and establishes peer sessions up to MAX_PEERS_PER_DOWNLOAD.
//! 3. Lifecycle state propagation: automatically moves downloads from Queued/Waiting to Downloading.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mule_core::model::{DownloadStatus, PeerSourceStatus};
use mule_protocol::ed2k::GetSourcesRequest;

use crate::rpc::events::DownloadSnapshot;
use crate::services::server_session::ServerConnectionStatus;
use crate::services::swarm_allocator::{FileDemand, SwarmAllocationConfig, SwarmAllocator};
use crate::state::AppState;

/// Interval between coordinator evaluation ticks.
const COORDINATOR_TICK_INTERVAL: Duration = Duration::from_secs(3);

/// Minimum elapsed time before re-querying sources for the same file hash.
const SOURCE_REQUERY_INTERVAL: Duration = Duration::from_secs(300);

/// Maximum number of new peer connections to initiate per file per tick (burst prevention).
const MAX_CONNECT_PER_TICK: usize = 5;

pub struct SwarmCoordinator {
    state: AppState,
    last_source_query: HashMap<String, Instant>,
}

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

    /// Evaluates all active downloads, runs source discovery, and connects to peers.
    pub async fn tick(&mut self) {
        // 0. Decay stale download rates (reset frozen speed to 0 if inactive > 2s)
        let decayed_snapshots = {
            let mut dl_guard = self.state.downloads.lock().await;
            dl_guard.decay_stale_rates()
        };
        for snap in decayed_snapshots {
            crate::services::peer_session::emit_download_update(
                &self.state.event_tx,
                &self.state.sequence,
                &snap,
            );
        }

        // 1. Auto-verify and finalize any completed downloads
        let pending_verifications = {
            let mut dl_guard = self.state.downloads.lock().await;
            dl_guard.get_pending_verifications()
        };

        for (id, temp_path, target_path, expected_hash, total_size) in pending_verifications {
            let state_clone = self.state.clone();
            tokio::spawn(async move {
                tracing::info!(
                    "Starting eD2K hash verification for download {id} ({total_size} bytes, temp_path={})...",
                    temp_path.display()
                );
                let temp_path_for_hashing = temp_path.clone();
                let hash_res = tokio::task::spawn_blocking(move || {
                    let mut file = std::fs::File::open(&temp_path_for_hashing)?;
                    mule_format::ed2k::hash_reader(&mut file)
                })
                .await;

                match hash_res {
                    Ok(Ok(computed)) => {
                        if computed.root_hash.0 == expected_hash {
                            let mut dl_guard = state_clone.downloads.lock().await;
                            match dl_guard.finalize_download(&id) {
                                Ok(snap) => {
                                    state_clone.broadcast_download_update(snap.clone());
                                    tracing::info!(
                                        "Download {id} ('{}') successfully verified eD2K hash ({}) and finalized to {}",
                                        snap.file_name,
                                        hex::encode(expected_hash),
                                        target_path.display()
                                    );
                                }
                                Err(e) => {
                                    tracing::error!("Failed to finalize download {id}: {e}");
                                    if let Ok(snap) = dl_guard.update_status(&id, mule_core::model::DownloadStatus::Failed) {
                                        state_clone.broadcast_download_update(snap);
                                    }
                                }
                            }
                        } else {
                            let (covered_bytes, missing_ranges) = {
                                let dl_guard = state_clone.downloads.lock().await;
                                (
                                    dl_guard.get_covered_len(&id).unwrap_or(0),
                                    dl_guard.get_missing_ranges(&id).unwrap_or_default(),
                                )
                            };
                            let disk_len = std::fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0);
                            tracing::error!(
                                "Download {id} hash mismatch: expected {}, got {}. File size: expected={} bytes, on-disk={} bytes, covered_in_ranges={} bytes. Missing ranges: {:?}",
                                hex::encode(expected_hash),
                                hex::encode(computed.root_hash.0),
                                total_size,
                                disk_len,
                                covered_bytes,
                                missing_ranges
                            );
                            let mut dl_guard = state_clone.downloads.lock().await;
                            if let Ok(snap) = dl_guard.update_status(&id, mule_core::model::DownloadStatus::Failed) {
                                state_clone.broadcast_download_update(snap);
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::error!("Download {id} hash reader failed for {}: {e}", temp_path.display());
                        let mut dl_guard = state_clone.downloads.lock().await;
                        let already_finalized = target_path.exists()
                            && std::fs::metadata(&target_path).map(|m| m.len()).unwrap_or(0) == total_size;
                        if already_finalized {
                            tracing::info!("Download {id} target file already exists in {}; marking Completed", target_path.display());
                            if let Ok(snap) = dl_guard.update_status(&id, mule_core::model::DownloadStatus::Completed) {
                                state_clone.broadcast_download_update(snap);
                            }
                        } else if let Ok(snap) = dl_guard.update_status(&id, mule_core::model::DownloadStatus::Failed) {
                            state_clone.broadcast_download_update(snap);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Download {id} spawn_blocking failed: {e}");
                        let mut dl_guard = state_clone.downloads.lock().await;
                        if let Ok(snap) = dl_guard.update_status(&id, mule_core::model::DownloadStatus::Failed) {
                            state_clone.broadcast_download_update(snap);
                        }
                    }
                }
            });
        }

        // 1. Skip if network is disabled by policy
        if self.state.network_policy.state() == mule_network_policy::NetworkState::Disabled {
            return;
        }

        // 2. Fetch list of active/queued downloads
        let active_downloads: Vec<DownloadSnapshot> = {
            let dl_guard = self.state.downloads.lock().await;
            dl_guard
                .list_downloads()
                .into_iter()
                .filter(|d| {
                    let s = d.status.to_lowercase();
                    s == "queued"
                        || s == "downloading"
                        || s == "waitingforsources"
                        || s == "waiting_for_sources"
                })
                .collect()
        };

        if active_downloads.is_empty() {
            return;
        }

        let now = Instant::now();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        // Build demands and compute dynamic swarm quotas using Water-filling allocator
        let mut demands = Vec::with_capacity(active_downloads.len());
        for dl in &active_downloads {
            let file_info = {
                let dl_guard = self.state.downloads.lock().await;
                dl_guard.get_download_file_info(&dl.id)
            };
            if let Some((_, _, hash, _)) = file_info {
                let hash_hex = hex::encode(hash.0);
                let viable_sources = {
                    let dl_guard = self.state.downloads.lock().await;
                    dl_guard
                        .list_peer_sources(&hash_hex)
                        .unwrap_or_default()
                        .into_iter()
                        .filter(|s| match s.status {
                            PeerSourceStatus::Candidate
                            | PeerSourceStatus::Tried
                            | PeerSourceStatus::Allowed
                            | PeerSourceStatus::Failed => {
                                if let Some(cooldown) = s.cooldown_until_unix_ms {
                                    cooldown <= now_ms
                                } else {
                                    true
                                }
                            }
                            _ => false,
                        })
                        .count()
                };

                let remaining_bytes = dl.total_bytes.saturating_sub(dl.received_bytes);
                let remaining_parts = ((remaining_bytes + 9_727_999) / 9_728_000) as usize;
                let active_peers = self
                    .state
                    .peer_session
                    .active_peer_count_for_hash(&hash_hex)
                    .await;

                demands.push(FileDemand::new(
                    hash_hex,
                    &dl.priority,
                    viable_sources,
                    remaining_parts,
                    active_peers,
                ));
            }
        }

        let alloc_config = {
            let dl_guard = self.state.downloads.lock().await;
            let max_g = dl_guard
                .get_setting("max_global_download_peers")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(self.state.config.max_global_download_peers);
            let max_f = dl_guard
                .get_setting("max_peers_per_download")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(self.state.config.max_peers_per_download);
            let min_f = dl_guard
                .get_setting("min_peers_per_download")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(self.state.config.min_peers_per_download);
            SwarmAllocationConfig {
                max_global_peers: max_g,
                max_peers_per_file: max_f,
                min_peers_per_file: min_f,
            }
        };
        let quotas = SwarmAllocator::compute_quotas(&demands, &alloc_config);

        for dl in active_downloads {
            // Get detailed file info (hash and total size)
            let file_info = {
                let dl_guard = self.state.downloads.lock().await;
                dl_guard.get_download_file_info(&dl.id)
            };

            let (_temp_path, _target_path, hash, size) = match file_info {
                Some(info) => info,
                None => continue,
            };

            let hash_hex = hex::encode(hash.0);

            // List candidate sources recorded for this file
            let mut candidate_sources = {
                let dl_guard = self.state.downloads.lock().await;
                dl_guard.list_peer_sources(&hash_hex).unwrap_or_default()
            };

            // Sort candidate sources: prefer 0 fail_count first, then most recently seen
            candidate_sources.sort_by(|a, b| {
                a.fail_count
                    .cmp(&b.fail_count)
                    .then_with(|| b.last_seen_unix_ms.cmp(&a.last_seen_unix_ms))
            });

            // Dynamic source discovery interval:
            // - If 0 sources: 30s
            // - If < 15 sources: 60s (aggressive discovery while sources are scarce)
            // - If >= 15 sources: 300s (standard 5-minute maintenance)
            let query_interval = if candidate_sources.is_empty() {
                Duration::from_secs(30)
            } else if candidate_sources.len() < 15 {
                Duration::from_secs(60)
            } else {
                SOURCE_REQUERY_INTERVAL
            };

            let should_query = match self.last_source_query.get(&hash_hex) {
                Some(last) => now.duration_since(*last) >= query_interval,
                None => true,
            };

            if should_query {
                self.last_source_query.insert(hash_hex.clone(), now);

                // Query connected eD2K server
                if self.state.server_session.snapshot().await.status
                    == ServerConnectionStatus::Connected
                {
                    let req = GetSourcesRequest::with_obfuscation(
                        hash.0,
                        size,
                        self.state.config.obfuscation_enabled,
                    );
                    if let Err(e) = self.state.server_session.get_sources(req).await {
                        tracing::debug!("Swarm coordinator get_sources failed for {hash_hex}: {e}");
                    } else {
                        tracing::info!(
                            "Swarm coordinator requested sources from eD2K server for file {hash_hex} (obfuscated={})",
                            self.state.config.obfuscation_enabled
                        );
                    }
                }

                // Query Kad if enabled in current network profile
                if self.state.network_policy.profile().allows_udp_kad() {
                    let kad = Arc::clone(&self.state.kad);
                    let h = hash_hex.clone();
                    tracing::info!(
                        "Swarm coordinator triggering Kad search_sources for file {hash_hex} (known sources in DB: {})",
                        candidate_sources.len()
                    );
                    tokio::spawn(async move {
                        let _ = kad.search_sources(&h, size).await;
                    });
                }
            }

            // 4. Swarm peer connection management
            //
            // Guard: resolve client_id BEFORE initiating any peer connection.
            // Sending OP_HELLO with client_id=0 (server=0:0) causes immediate rejection
            // by remote peers ("closed connection during handshake"). This happens at
            // daemon startup when the SwarmCoordinator resumes persisted downloads before
            // the eD2K server login completes. We skip peer connections this tick and
            // retry on the next tick (3s later) once the server is connected and has
            // assigned a valid client_id. Source discovery (eD2K + Kad) is unaffected.
            let (client_id, server_ip, server_port) = {
                let s_snap = self.state.server_session.snapshot().await;
                if s_snap.status == ServerConnectionStatus::Connected {
                    let cid = s_snap.client_id.unwrap_or(0);
                    if cid == 0 {
                        // Connected but OP_IDCHANGE not yet received — skip peer connections.
                        tracing::debug!(
                            "Skipping peer connections for {hash_hex}: server connected but client_id not yet assigned"
                        );
                        continue;
                    }
                    let (s_ip, s_port) = if let Some(ref addr_str) = s_snap.server_address {
                        if let Ok(sock_addr) = addr_str.parse::<std::net::SocketAddr>() {
                            match sock_addr.ip() {
                                std::net::IpAddr::V4(v4) => {
                                    (u32::from_le_bytes(v4.octets()), sock_addr.port())
                                }
                                std::net::IpAddr::V6(_) => (0u32, sock_addr.port()),
                            }
                        } else {
                            (0u32, 0u16)
                        }
                    } else {
                        (0u32, 0u16)
                    };
                    (cid, s_ip, s_port)
                } else {
                    // eD2K server not connected — skip peer connections this tick.
                    tracing::debug!(
                        "Skipping peer connections for {hash_hex}: no eD2K server connection (client_id would be 0)"
                    );
                    continue;
                }
            };

            let swarm_snap = self
                .state
                .peer_session
                .swarm_snapshot(Some(&hash_hex))
                .await;
            let active_count = swarm_snap.active_peer_count;

            let target_quota = *quotas.get(&hash_hex).unwrap_or(&0);
            if active_count >= target_quota {
                continue;
            }

            let needed = (target_quota - active_count).min(MAX_CONNECT_PER_TICK);
            if needed == 0 {
                continue;
            }

            let mut connected_this_tick = 0;
            for src in candidate_sources {
                if connected_this_tick >= needed {
                    break;
                }

                // Only pick viable candidates (including Failed whose backoff cooldown expired)
                match src.status {
                    PeerSourceStatus::Candidate
                    | PeerSourceStatus::Tried
                    | PeerSourceStatus::Allowed
                    | PeerSourceStatus::Failed => {}
                    _ => continue,
                }

                // Check cooldown timer
                if let Some(cooldown_until) = src.cooldown_until_unix_ms {
                    if cooldown_until > now_ms {
                        continue;
                    }
                }

                // Skip if already in the active sessions
                let already_connected = swarm_snap
                    .peers
                    .iter()
                    .any(|p| p.peer_address.as_deref() == Some(&src.address));
                if already_connected {
                    continue;
                }

                let peer_addr: SocketAddr = match src.address.parse() {
                    Ok(a) => a,
                    Err(_) => continue,
                };

                // Request TCP connect permit from NetworkPolicyManager
                let permit = match self.state.network_policy.request_tcp_connect(
                    peer_addr,
                    mule_network_policy::ConnectionPurpose::Ed2kPeer,
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::debug!("Permit denied for peer {peer_addr}: {e}");
                        continue;
                    }
                };

                // Spawn peer session connection
                let user_hash = self.state.identity.lock().await.user_hash();
                let dl_ref = Arc::clone(&self.state.downloads);
                let file_hash_str = hash_hex.clone();
                let ps = self.state.peer_session.clone();
                let client_port = self.state.config.ed2k_port;
                let kad_port = self.state.config.kad_port;

                let target_user_hash = src.user_hash_bytes();
                let peer_crypt_options = src.crypt_options;
                let obfu_enabled = self.state.config.obfuscation_enabled;
                let obfu_required = self.state.config.obfuscation_required;
                let obfu_pad_max = self.state.config.obfuscation_padding_max;

                tracing::info!(
                    "Swarm coordinator initiating peer connection to {peer_addr} for {file_hash_str} (quota={target_quota}, active={active_count}, obfu_target_hash={:?}, crypt_opts=0x{:02x})",
                    target_user_hash.is_some(),
                    peer_crypt_options
                );
                tokio::spawn(async move {
                    if let Err(e) = ps
                        .connect(
                            &file_hash_str,
                            peer_addr,
                            permit,
                            client_port,
                            "Emule-Sion".to_string(),
                            user_hash,
                            dl_ref,
                            client_id,
                            server_ip,
                            server_port,
                            kad_port,
                            target_user_hash,
                            peer_crypt_options,
                            obfu_enabled,
                            obfu_required,
                            obfu_pad_max,
                        )
                        .await
                    {
                        tracing::debug!("Peer connection to {peer_addr} failed: {e}");
                    }
                });

                connected_this_tick += 1;
            }

            // Transition status to Downloading if connections were made or exist
            if (active_count > 0 || connected_this_tick > 0)
                && dl.status.to_lowercase() == "queued"
            {
                let mut dl_guard = self.state.downloads.lock().await;
                if let Ok(updated_snap) =
                    dl_guard.update_status(&dl.id, DownloadStatus::Downloading)
                {
                    self.state.broadcast_download_update(updated_snap);
                }
            }
        }
    }
}
