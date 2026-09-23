// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Swarm evaluation tick, hash validation, and connection scheduling.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mule_core::model::{DownloadStatus, PeerSourceStatus};
use mule_protocol::ed2k::GetSourcesRequest;

use crate::services::downloads::types::DownloadSnapshot;
use crate::services::server_session::ServerConnectionStatus;
use crate::services::swarm_allocator::{FileDemand, SwarmAllocationConfig, SwarmAllocator};

use super::types::{
    SwarmCoordinator, HEARTBEAT_INTERVAL, MAX_CONNECT_PER_TICK, SOURCE_REQUERY_INTERVAL,
};

impl SwarmCoordinator {
    /// Evaluates all active downloads, runs source discovery, and connects to peers.
    pub async fn tick(&mut self) {
        // Periodic consolidated system heartbeat (every 60s)
        if self.last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            self.last_heartbeat = Instant::now();
            self.emit_heartbeat().await;
            // eMule SavePartFile cadence: persist .part.met companions for
            // in-progress downloads so a crash never loses more than ~60 s
            // of gap state.
            let dl_guard = self.state.downloads.lock().await;
            dl_guard.save_active_part_mets();
        }

        // 0. Decay stale download rates (reset frozen speed to 0 if inactive > 2s)
        let decayed_snapshots = {
            let mut dl_guard = self.state.downloads.lock().await;
            dl_guard.decay_stale_rates()
        };
        for mut snap in decayed_snapshots {
            snap.pending_ranges = self
                .state
                .peer_session
                .try_pending_ranges(&snap.ed2k_hash);
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
                                    if let Ok(snap) = dl_guard.update_status(
                                        &id,
                                        mule_core::model::DownloadStatus::Failed,
                                    ) {
                                        state_clone.broadcast_download_update(snap);
                                    }
                                }
                            }
                        } else {
                            // Global MD4 mismatch: if the per-part hashset is known,
                            // identify corrupted parts and evict only those ranges
                            // for re-download (eMule CPartFile corrupted-part recovery)
                            // instead of failing the whole file.
                            let expected_part_hashes = {
                                let dl_guard = state_clone.downloads.lock().await;
                                dl_guard.get_part_hashes_for_download(&id)
                            };

                            let mut corrupted: Vec<u32> = Vec::new();
                            let mut verified_ok: Vec<u32> = Vec::new();
                            if !expected_part_hashes.is_empty() {
                                for (i, expected_ph) in expected_part_hashes.iter().enumerate() {
                                    match computed.part_hashes.get(i) {
                                        Some(actual) if actual == expected_ph => {
                                            verified_ok.push(i as u32)
                                        }
                                        _ => corrupted.push(i as u32),
                                    }
                                }
                            }

                            if corrupted.is_empty() {
                                let (covered_bytes, missing_ranges) = {
                                    let dl_guard = state_clone.downloads.lock().await;
                                    (
                                        dl_guard.get_covered_len(&id).unwrap_or(0),
                                        dl_guard.get_missing_ranges(&id).unwrap_or_default(),
                                    )
                                };
                                let disk_len =
                                    std::fs::metadata(&temp_path).map(|m| m.len()).unwrap_or(0);
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
                                if let Ok(snap) = dl_guard
                                    .update_status(&id, mule_core::model::DownloadStatus::Failed)
                                {
                                    state_clone.broadcast_download_update(snap);
                                }
                            } else {
                                tracing::warn!(
                                    "Download {id} root hash mismatch: {} corrupted part(s) {:?} out of {} — evicting for re-download",
                                    corrupted.len(),
                                    corrupted,
                                    expected_part_hashes.len()
                                );
                                let mut dl_guard = state_clone.downloads.lock().await;
                                for part_idx in verified_ok {
                                    let _ = dl_guard.db_mut().mark_part_verified(&id, part_idx);
                                }
                                match dl_guard.evict_corrupted_parts(&id, &corrupted) {
                                    Ok(snap) => {
                                        state_clone.broadcast_download_update(snap);
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to evict corrupted parts for download {id}: {e}");
                                        if let Ok(snap) = dl_guard.update_status(
                                            &id,
                                            mule_core::model::DownloadStatus::Failed,
                                        ) {
                                            state_clone.broadcast_download_update(snap);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::error!(
                            "Download {id} hash reader failed for {}: {e}",
                            temp_path.display()
                        );
                        let mut dl_guard = state_clone.downloads.lock().await;
                        let already_finalized = target_path.exists()
                            && std::fs::metadata(&target_path)
                                .map(|m| m.len())
                                .unwrap_or(0)
                                == total_size;
                        if already_finalized {
                            tracing::info!(
                                "Download {id} target file already exists in {}; marking Completed",
                                target_path.display()
                            );
                            if let Ok(snap) = dl_guard
                                .update_status(&id, mule_core::model::DownloadStatus::Completed)
                            {
                                state_clone.broadcast_download_update(snap);
                            }
                        } else if let Ok(snap) =
                            dl_guard.update_status(&id, mule_core::model::DownloadStatus::Failed)
                        {
                            state_clone.broadcast_download_update(snap);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Download {id} spawn_blocking failed: {e}");
                        let mut dl_guard = state_clone.downloads.lock().await;
                        if let Ok(snap) =
                            dl_guard.update_status(&id, mule_core::model::DownloadStatus::Failed)
                        {
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

        // 1b. Automatic server failover: if the eD2K server connection dropped,
        // pick another enabled known server after a backoff (eMule DeadServer
        // retry semantics, bounded so a dead server list cannot spin).
        self.maybe_reconnect_server().await;

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
                            | PeerSourceStatus::Allowed => true,
                            PeerSourceStatus::Failed => {
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
                let remaining_parts = remaining_bytes.div_ceil(9_728_000) as usize;
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

        // Per-file source caps (eMule `.part.met` FT_MAXSOURCES /
        // `CPartFile::GetMaxSources`): when present, the per-file limit
        // overrides the global `max_peers_per_file` for quota, requery and
        // purge-threshold decisions.
        let file_max_sources: std::collections::HashMap<String, usize> = active_downloads
            .iter()
            .filter_map(|d| {
                if d.ed2k_hash.is_empty() {
                    return None;
                }
                let cap = d
                    .max_sources
                    .map(|v| v as usize)
                    .unwrap_or(alloc_config.max_peers_per_file);
                Some((d.ed2k_hash.clone(), cap))
            })
            .collect();

        // A4AF ("Ask For Another File"): a starved download may reclaim a
        // peer address currently connected to a lower-priority file that is
        // already above its minimum peer count. eMule multiplexes files per
        // connection; our sessions are mono-file, so we release the session
        // and let the starved file dial the address on the next tick.
        // A4AF event pass every tick: sources that just reported NNP are
        // candidates for immediate swap to other files (eMule triggers
        // `SwapToAnotherFile` on NNP/FNF, not only on the periodic pass).
        self.a4af_event_pass(&active_downloads, now_ms).await;

        // Full A4AF rebalance at eMule's `ProcessA4AFClients` cadence (~8 min).
        if self.last_a4af_full.elapsed() >= super::types::A4AF_FULL_INTERVAL {
            self.last_a4af_full = Instant::now();
            self.a4af_rebalance(&demands, &alloc_config, now_ms)
                .await;
        }

        // Client-UDP keep-alives: OP_REASKFILEPING toward sources queued on
        // a remote peer that announced an eMule UDP port — deduplicated at
        // ~21 min cadence (CUpDownClient::UDPReaskForDownload). eMule only
        // spends a UDP reask when the last connection attempt is old
        // (> ~20 min): skip freshly-queued sessions to avoid UDP noise.
        for target in self.state.peer_session.queued_udp_targets().await {
            if let Some(connected_at) = target.connected_at_unix_ms {
                if now_ms.saturating_sub(connected_at) < 20 * 60 * 1000 {
                    continue;
                }
            }
            self.state.client_udp.send_reask_file_ping(&target, 0).await;
        }

        // Réglages effectifs lus une fois par tick (persistés en base —
        // eMule Preferences → Sécurité) : les valeurs `config.*` figées au
        // boot ne reflètent pas les changements via `settings.set`.
        let (obf_enabled, obf_required) = self.state.effective_obfuscation().await;
        let nickname = self.state.current_nickname().await;

        for dl in active_downloads {
            let file_info = {
                let dl_guard = self.state.downloads.lock().await;
                dl_guard.get_download_file_info(&dl.id)
            };

            let (_temp_path, _target_path, hash, size) = match file_info {
                Some(info) => info,
                None => continue,
            };

            let hash_hex = hex::encode(hash.0);

            let mut candidate_sources = {
                let dl_guard = self.state.downloads.lock().await;
                dl_guard.list_peer_sources(&hash_hex).unwrap_or_default()
            };

            candidate_sources.sort_by(|a, b| {
                a.fail_count
                    .cmp(&b.fail_count)
                    .then_with(|| b.last_seen_unix_ms.cmp(&a.last_seen_unix_ms))
            });

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

            // Match official eMule: if file has already reached its max sources
            // limit (per-file FT_MAXSOURCES or the global cap), do not waste
            // server and Kad bandwidth re-querying.
            let file_max = file_max_sources
                .get(&hash_hex)
                .copied()
                .unwrap_or(alloc_config.max_peers_per_file);
            if should_query && candidate_sources.len() < file_max {
                self.last_source_query.insert(hash_hex.clone(), now);

                if self.state.server_session.snapshot().await.status
                    == ServerConnectionStatus::Connected
                {
                    let req =
                        GetSourcesRequest::with_obfuscation(hash.0, size, obf_enabled);
                    if let Err(e) = self.state.server_session.get_sources(req).await {
                        tracing::debug!("Swarm coordinator get_sources failed for {hash_hex}: {e}");
                    } else {
                        tracing::info!(
                            "Swarm coordinator requested sources from eD2K server for file {hash_hex} (obfuscated={})",
                            obf_enabled
                        );
                    }
                }

                // UDP source query (OP_GLOBGETSOURCES2) toward the connected
                // server — its UDP port is TCP+4 (eMule convention).
                // Fire-and-forget; answers arrive as OP_GLOBFOUNDSOURCES.
                if let Some(addr_str) = self.state.server_session.snapshot().await.server_address {
                    if let Ok(mut udp_addr) = addr_str.parse::<SocketAddr>() {
                        udp_addr.set_port(udp_addr.port().saturating_add(4));
                        self.state
                            .server_udp
                            .send_glob_get_sources(udp_addr, &hash.0, size)
                            .await;
                    }
                }

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

            let (client_id, server_ip, server_port, server_is_high_id) = {
                let s_snap = self.state.server_session.snapshot().await;
                if s_snap.status == ServerConnectionStatus::Connected {
                    let cid = s_snap.client_id.unwrap_or(0);
                    if cid == 0 {
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
                    (cid, s_ip, s_port, s_snap.is_high_id)
                } else {
                    // eMule fonctionne en Kad-only : sans serveur le hello
                    // annonce client_id = 0 (même convention que le chemin
                    // manuel `sources.connect`). `server_is_high_id=false`
                    // filtre d'office les callbacks serveur et les sources
                    // LowID, injoignables sans serveur.
                    (0u32, 0u32, 0u16, false)
                }
            };

            // eMule `CPartFile::Process`: near the per-file source cap
            // (>= 80%), periodically drop sources that cannot serve us —
            // unreachable LowIDs, NNP-flagged and repeatedly failed sources.
            if self
                .last_source_purge
                .get(&hash_hex)
                .is_none_or(|t| t.elapsed() >= super::types::SOURCE_PURGE_INTERVAL)
            {
                self.last_source_purge.insert(hash_hex.clone(), now);
                self.purge_excess_sources(
                    &hash_hex,
                    &candidate_sources,
                    file_max_sources
                        .get(&hash_hex)
                        .copied()
                        .unwrap_or(alloc_config.max_peers_per_file),
                    server_is_high_id,
                    now_ms,
                )
                .await;
            }

            let swarm_snap = self
                .state
                .peer_session
                .swarm_snapshot(Some(&hash_hex))
                .await;
            let active_count = swarm_snap.active_peer_count;

            let target_quota = quotas
                .get(&hash_hex)
                .copied()
                .unwrap_or(0)
                .min(file_max_sources.get(&hash_hex).copied().unwrap_or(usize::MAX));
            if active_count >= target_quota {
                continue;
            }

            let needed = (target_quota - active_count).min(MAX_CONNECT_PER_TICK);
            if needed == 0 {
                continue;
            }

            let mut connected_this_tick = 0;
            let mut skipped_cooldown = 0usize;
            let mut skipped_connected = 0usize;
            let total_candidates = candidate_sources.len();
            for src in candidate_sources {
                if connected_this_tick >= needed {
                    break;
                }

                match src.status {
                    PeerSourceStatus::Candidate
                    | PeerSourceStatus::Tried
                    | PeerSourceStatus::Allowed
                    | PeerSourceStatus::Failed => {}
                    _ => continue,
                }

                if let Some(cooldown_until) = src.cooldown_until_unix_ms {
                    if cooldown_until > now_ms {
                        skipped_cooldown += 1;
                        continue;
                    }
                }

                let already_connected = swarm_snap
                    .peers
                    .iter()
                    .any(|p| p.peer_address.as_deref() == Some(&src.address));
                if already_connected {
                    skipped_connected += 1;
                    continue;
                }

                // LowID source: unreachable directly — ask the connected
                // server to relay a callback (eMule `OP_CALLBACKREQUEST`).
                // The target then connects inbound; the inbound listener
                // matches it by client_id and adopts it as a download session.
                if let Some(lowid_str) = src.address.strip_prefix("lowid:") {
                    if !server_is_high_id {
                        // Callbacks are useless while we are LowID ourselves:
                        // the target cannot connect back to us.
                        continue;
                    }
                    let Ok(lowid) = lowid_str.parse::<u32>() else {
                        continue;
                    };
                    if self.state.peer_session.has_pending_callback(lowid) {
                        continue;
                    }
                    match self.state.server_session.request_callback(lowid).await {
                        Ok(()) => {
                            tracing::info!(
                                "Requested server callback to LowID source {lowid} for {file_hash}",
                                file_hash = hash_hex
                            );
                            self.state.peer_session.register_pending_callback(
                                lowid,
                                hash_hex.clone(),
                                5 * 60 * 1000,
                            );
                            let mut dl_guard = self.state.downloads.lock().await;
                            let _ = dl_guard.update_peer_source_status(
                                &hash_hex,
                                &src.address,
                                PeerSourceStatus::Tried,
                                None,
                                Some(now_ms + 5 * 60 * 1000),
                            );
                            connected_this_tick += 1;
                        }
                        Err(e) => {
                            tracing::debug!("Callback request for LowID {lowid} failed: {e}");
                        }
                    }
                    continue;
                }

                // Kad LowID source: reachable only through its published
                // buddy. Send KADEMLIA_CALLBACK_REQ to the buddy node; the
                // LowID client then connects inbound to our TCP port and is
                // adopted by remote IP (pending_kad_callbacks).
                if let Some(ip_str) = src.address.strip_prefix("kadcb:") {
                    let Ok(src_ip) = ip_str.parse::<std::net::IpAddr>() else {
                        continue;
                    };
                    let Some(buddy_bytes) = src.buddy_id_bytes() else {
                        continue;
                    };
                    if self.state.peer_session.has_pending_kad_callback(src_ip) {
                        continue;
                    }
                    let buddy_id = mule_core::kad::KadId(buddy_bytes);
                    let Some(buddy) = self.state.kad.find_contact(&buddy_id).await else {
                        tracing::debug!(
                            "Kad buddy {buddy_id:?} for LowID source {src_ip} not in routing table"
                        );
                        continue;
                    };
                    let Ok(file_id_bytes) = hex::decode(&hash_hex) else {
                        continue;
                    };
                    let mut file_id = [0u8; 16];
                    file_id.copy_from_slice(&file_id_bytes);
                    let buddy_addr =
                        SocketAddr::new(std::net::IpAddr::V4(buddy.ip), buddy.udp_port);
                    match self
                        .state
                        .kad
                        .udp()
                        .send_callback_req(
                            buddy_addr,
                            &buddy_bytes,
                            &file_id,
                            self.state.ed2k_port(),
                        )
                        .await
                    {
                        Ok(()) => {
                            tracing::info!(
                                "Requested Kad buddy callback to LowID source {src_ip} via {buddy_addr} for {hash_hex}"
                            );
                            self.state.peer_session.register_pending_kad_callback(
                                src_ip,
                                hash_hex.clone(),
                                5 * 60 * 1000,
                            );
                            let mut dl_guard = self.state.downloads.lock().await;
                            let _ = dl_guard.update_peer_source_status(
                                &hash_hex,
                                &src.address,
                                PeerSourceStatus::Tried,
                                None,
                                Some(now_ms + 5 * 60 * 1000),
                            );
                            connected_this_tick += 1;
                        }
                        Err(e) => {
                            tracing::debug!("Kad buddy callback request for {src_ip} failed: {e}");
                        }
                    }
                    continue;
                }

                let peer_addr: SocketAddr = match src.address.parse() {
                    Ok(a) => a,
                    Err(_) => continue,
                };

                let permit = match self.state.network_policy.request_tcp_connect(
                    peer_addr,
                    mule_network_policy::ConnectionPurpose::Ed2kPeer,
                ) {
                    Ok(p) => p,
                    Err(e) => {
                        // Refus permanent (adresse invalide / ipfilter) : la
                        // source ne sera jamais dialable — la marquer Rejected
                        // au lieu de la retenter à chaque tick (eMule
                        // DeadSourceList). Les refus transitoires (rate-limit,
                        // budget) conservent le backoff normal.
                        if matches!(
                            e,
                            mule_network_policy::PolicyError::AddressViolation(_)
                                | mule_network_policy::PolicyError::IpFiltered(..)
                        ) {
                            tracing::info!(
                                "[SOURCE_SCHED] Source {} for {hash_hex} permanently rejected: {e}",
                                src.address
                            );
                            let mut dl_guard = self.state.downloads.lock().await;
                            let _ = dl_guard.update_peer_source_status(
                                &hash_hex,
                                &src.address,
                                PeerSourceStatus::Rejected,
                                Some(&e.to_string()),
                                None,
                            );
                        } else {
                            tracing::debug!("Permit denied for peer {peer_addr}: {e}");
                        }
                        continue;
                    }
                };

                let user_hash = self.state.identity.lock().await.user_hash();
                let sx_env = self.state.sx_env.clone();
                let dl_ref = Arc::clone(&self.state.downloads);
                let file_hash_str = hash_hex.clone();
                let ps = self.state.peer_session.clone();
                let client_port = self.state.ed2k_port();
                let kad_port = self.state.kad_port();

                let target_user_hash = src.user_hash_bytes();
                let peer_crypt_options = src.crypt_options;
                let obfu_pad_max = self.state.config.obfuscation_padding_max;
                let nickname = nickname.clone();

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
                            nickname,
                            user_hash,
                            dl_ref,
                            client_id,
                            server_ip,
                            server_port,
                            kad_port,
                            target_user_hash,
                            peer_crypt_options,
                            obf_enabled,
                            obf_required,
                            obfu_pad_max,
                            sx_env,
                        )
                        .await
                    {
                        tracing::debug!("Peer connection to {peer_addr} failed: {e}");
                    }
                });

                connected_this_tick += 1;
            }

            if connected_this_tick > 0 {
                tracing::info!(
                    "[SOURCE_SCHED] File {hash_hex}: dialed {connected_this_tick} new sources this tick \
                     (candidates={total_candidates}, slots_needed={needed}, active={active_count}, \
                     skipped: cooldown={skipped_cooldown}, already_connected={skipped_connected})"
                );
            } else if total_candidates > 0 {
                tracing::debug!(
                    "[SOURCE_SCHED] File {hash_hex}: no source dialed this tick \
                     (candidates={total_candidates}, slots_needed={needed}, active={active_count}, \
                     skipped: cooldown={skipped_cooldown}, already_connected={skipped_connected})"
                );
            }

            if (active_count > 0 || connected_this_tick > 0) && dl.status.to_lowercase() == "queued"
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

    /// Automatic server failover. While disconnected, periodically picks an
    /// enabled known server (excluding the one that just dropped and any
    /// address attempted within `SERVER_FAIL_COOLDOWN`) and reconnects.
    /// Bounded by `SERVER_RECONNECT_INTERVAL` between attempts.
    async fn maybe_reconnect_server(&mut self) {
        use super::types::{SERVER_FAIL_COOLDOWN, SERVER_RECONNECT_INTERVAL};

        let snap = self.state.server_session.snapshot().await;
        match snap.status {
            ServerConnectionStatus::Connected => {
                self.last_connected_server = snap.server_address;
                return;
            }
            ServerConnectionStatus::Connecting | ServerConnectionStatus::Disconnecting => {
                return;
            }
            ServerConnectionStatus::Disconnected => {}
        }

        if let Some(last) = self.last_server_reconnect {
            if last.elapsed() < SERVER_RECONNECT_INTERVAL {
                return;
            }
        }

        // Pick an enabled, non-blocked server not attempted recently; prefer
        // a different server than the one that just dropped.
        let candidates = {
            let dl_guard = self.state.downloads.lock().await;
            dl_guard
                .list_servers()
                .unwrap_or_default()
                .into_iter()
                .filter(|s| {
                    s.filter_status == mule_core::model::ServerFilterStatus::Allowed
                        && s.address.parse::<SocketAddr>().is_ok()
                        && self
                            .server_attempts
                            .get(&s.address)
                            .is_none_or(|t| t.elapsed() >= SERVER_FAIL_COOLDOWN)
                })
                .map(|s| (s.address, s.name, s.preference))
                .collect::<Vec<_>>()
        };
        if candidates.is_empty() {
            return;
        }

        let pick = candidates
            .iter()
            .filter(|(addr, _, _)| Some(addr) != self.last_connected_server.as_ref())
            .max_by_key(|(_, _, pref)| *pref)
            .or_else(|| candidates.iter().max_by_key(|(_, _, pref)| *pref));
        let Some((address, name, _)) = pick else {
            return;
        };

        let addr: SocketAddr = match address.parse() {
            Ok(a) => a,
            Err(_) => return,
        };
        let permit = match self
            .state
            .network_policy
            .request_tcp_connect(addr, mule_network_policy::ConnectionPurpose::Ed2kServer)
        {
            Ok(p) => p,
            Err(e) => {
                tracing::debug!("Server failover: permit denied for {addr}: {e}");
                return;
            }
        };

        let user_hash = self.state.identity.lock().await.user_hash();
        let ed2k_port = self.state.ed2k_port();
        let nickname = self.state.current_nickname().await;
        tracing::info!("Server failover: connecting to {name} ({addr})");
        self.last_server_reconnect = Some(Instant::now());
        self.server_attempts.insert(address.clone(), Instant::now());
        let _ = self
            .state
            .server_session
            .connect(
                addr,
                Some(name.clone()),
                permit,
                10000,
                15000,
                ed2k_port,
                nickname,
                user_hash,
            )
            .await;
    }

    /// Emits a consolidated periodic system heartbeat summarizing the entire daemon status.
    pub(crate) async fn emit_heartbeat(&self) {
        // 1. Server info
        let srv_snap = self.state.server_session.snapshot().await;
        let srv_str = match srv_snap.status {
            ServerConnectionStatus::Connected => {
                let name = srv_snap.server_name.as_deref().unwrap_or("Server");
                let addr = srv_snap.server_address.as_deref().unwrap_or("");
                let client_id = srv_snap
                    .client_id
                    .map(|id| format!("{id}"))
                    .unwrap_or_else(|| "N/A".into());
                format!("Connected to {name} ({addr}, ID: {client_id})")
            }
            ServerConnectionStatus::Connecting => "Connecting...".to_string(),
            ServerConnectionStatus::Disconnected => "Disconnected".to_string(),
            _ => format!("{:?}", srv_snap.status),
        };

        // 2. Kad info
        let kad_snap = self.state.kad.get_status().await;
        let kad_str = if kad_snap.is_udp_active {
            format!(
                "Running ({} contacts, {}/{} pkts)",
                kad_snap.total_contacts, kad_snap.packets_sent, kad_snap.packets_received
            )
        } else {
            "Inactive".to_string()
        };

        // 3. Downloads info
        let (active_dl_count, other_dl_count, total_dl_rate, total_sources) = {
            let dl_guard = self.state.downloads.lock().await;
            let dls = dl_guard.list_downloads();
            let mut active = 0;
            let mut other = 0;
            let mut rate = 0u64;
            let mut sources = 0;
            for d in dls {
                if d.status.to_lowercase() == "downloading" {
                    active += 1;
                } else {
                    other += 1;
                }
                rate = rate.saturating_add(d.download_rate_bps);
                sources += d.source_count;
            }
            (active, other, rate, sources)
        };
        let swarm_snap = self.state.peer_session.swarm_snapshot(None).await;
        let dl_str = format!(
            "{} active, {} other ({:.1} KB/s, {} peers connected, {} known sources)",
            active_dl_count,
            other_dl_count,
            total_dl_rate as f64 / 1024.0,
            swarm_snap.active_peer_count,
            total_sources
        );

        // 4. Upload info
        let listener_snap = self.state.inbound_listener.snapshot().await;
        let ul_str = format!(
            "{} active ({:.1} KB/s, {} queued)",
            listener_snap.active_uploads,
            listener_snap.upload_rate_bps as f64 / 1024.0,
            listener_snap.queued_uploads
        );

        tracing::info!(
            "[SYSTEM_HEARTBEAT] Server: {} | Kad: {} | DL: {} | UL: {}",
            srv_str,
            kad_str,
            dl_str,
            ul_str
        );
    }

    /// A4AF rebalance pass (eMule `CPartFile` source swapping semantics).
    ///
    /// For every download below `min_peers_per_file`, scan live sessions of
    /// *lower-priority* files that sit above their minimum: if the connected
    /// peer address is a viable candidate for the starved file, release the
    /// session so the starved file can dial it on the next tick.
    /// At most one swap per tick keeps the system stable.
    async fn a4af_rebalance(
        &mut self,
        demands: &[FileDemand],
        alloc_config: &SwarmAllocationConfig,
        now_ms: i64,
    ) {
        for starved in demands {
            if starved.current_active_peers >= alloc_config.min_peers_per_file {
                continue;
            }

            // Candidate addresses for the starved file (viable sources only).
            let wanted: std::collections::HashSet<String> = {
                let dl_guard = self.state.downloads.lock().await;
                dl_guard
                    .list_peer_sources(&starved.file_hash)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|s| match s.status {
                        PeerSourceStatus::Candidate
                        | PeerSourceStatus::Tried
                        | PeerSourceStatus::Allowed => true,
                        PeerSourceStatus::Failed => s
                            .cooldown_until_unix_ms
                            .map(|c| c <= now_ms)
                            .unwrap_or(true),
                        _ => false,
                    })
                    .map(|s| s.address.clone())
                    .collect()
            };
            if wanted.is_empty() {
                continue;
            }

            // Live sessions connected to other files.
            let swarm = self.state.peer_session.swarm_snapshot(None).await;
            let donors: std::collections::HashMap<&str, u32> = demands
                .iter()
                .map(|d| (d.file_hash.as_str(), d.priority_weight))
                .collect();

            for peer in &swarm.peers {
                let Some(donor_hash) = peer.file_hash.as_deref() else {
                    continue;
                };
                let Some(peer_addr) = peer.peer_address.as_deref() else {
                    continue;
                };
                if donor_hash == starved.file_hash || !wanted.contains(peer_addr) {
                    continue;
                }
                let donor_weight = donors.get(donor_hash).copied().unwrap_or(2);
                // Only steal from a strictly lower-priority donor that stays
                // above its minimum peer count.
                if donor_weight >= starved.priority_weight {
                    continue;
                }
                let donor_active = self
                    .state
                    .peer_session
                    .active_peer_count_for_hash(donor_hash)
                    .await;
                if donor_active <= alloc_config.min_peers_per_file {
                    continue;
                }
                tracing::info!(
                    "[A4AF] Swapping source {} from file {} (prio={}) to starved file {} (prio={})",
                    peer_addr,
                    donor_hash,
                    donor_weight,
                    starved.file_hash,
                    starved.priority_weight
                );
                let _ = self
                    .state
                    .peer_session
                    .disconnect_peer(Some(peer_addr))
                    .await;
                return; // One swap per tick.
            }
        }
    }

    /// Conditional source purge (eMule `CPartFile::Process` slow timers).
    ///
    /// Only runs when the file's known-source count reaches ~80% of its
    /// `maxSources` (`FT_MAXSOURCES` or the global per-file cap). Purged:
    /// - `lowid:` sources we cannot call back (we are LowID ourselves);
    /// - `kadcb:` sources without a buddy id (unreachable);
    /// - sources flagged "no needed parts" (NNP) — their part mask covers
    ///   nothing we still need;
    /// - sources that failed repeatedly (fail_count >= 3).
    ///
    /// Purge removes at most `count - threshold` rows per pass.
    async fn purge_excess_sources(
        &mut self,
        file_hash: &str,
        sources: &[mule_core::model::PeerSource],
        file_max: usize,
        server_is_high_id: bool,
        now_ms: i64,
    ) {
        if file_max == 0 || sources.len() < file_max * 4 / 5 {
            return;
        }
        let budget = sources.len() - file_max * 4 / 5;
        if budget == 0 {
            return;
        }

        let mut removed = 0usize;
        for s in sources {
            if removed >= budget {
                break;
            }
            if s.status == PeerSourceStatus::Connected {
                continue;
            }
            let nnp = s
                .last_error
                .as_deref()
                .map(|e| e.to_lowercase().contains("no needed parts"))
                .unwrap_or(false);
            let unreachable_lowid = (s.address.starts_with("lowid:") && !server_is_high_id)
                || (s.address.starts_with("kadcb:") && s.buddy_id.is_none());
            let repeatedly_failed =
                s.status == PeerSourceStatus::Failed && s.fail_count >= 3;
            if !(nnp || unreachable_lowid || repeatedly_failed) {
                continue;
            }
            let why = if nnp {
                "NNP"
            } else if unreachable_lowid {
                "unreachable LowID"
            } else {
                "repeated failures"
            };
            let mut dl_guard = self.state.downloads.lock().await;
            match dl_guard.delete_peer_source(file_hash, &s.address) {
                Ok(true) => {
                    removed += 1;
                    tracing::info!(
                        "[SOURCE_PURGE] {file_hash}: dropped {why} source {} (swarm over 80% of maxSources={file_max})",
                        s.address
                    );
                }
                Ok(false) => {}
                Err(e) => {
                    tracing::debug!("[SOURCE_PURGE] delete {} failed: {e}", s.address)
                }
            }
        }
        if removed > 0 {
            tracing::info!(
                "[SOURCE_PURGE] {file_hash}: removed {removed} sources at {}% of maxSources ({}/{})",
                sources.len() * 100 / file_max.max(1),
                sources.len(),
                file_max
            );
        }
        let _ = now_ms;
    }

    /// Event-driven A4AF (eMule `CUpDownClient` NNP/FNF swap triggers).
    ///
    /// Scans sources recently flagged "no needed parts" (NNP): the source is
    /// useless for the file it served but may hold parts for another active
    /// download. When the same address is a viable candidate of another
    /// file, its cooldown is cleared so the dialer picks it up immediately
    /// (bounded to one address per file per tick).
    async fn a4af_event_pass(&mut self, downloads: &[DownloadSnapshot], now_ms: i64) {
        let hashes: Vec<String> = downloads
            .iter()
            .filter(|d| !d.ed2k_hash.is_empty())
            .map(|d| d.ed2k_hash.clone())
            .collect();
        if hashes.len() < 2 {
            return;
        }

        // Sources per file, loaded once.
        let mut per_file: std::collections::HashMap<String, Vec<mule_core::model::PeerSource>> =
            std::collections::HashMap::new();
        {
            let dl_guard = self.state.downloads.lock().await;
            for h in &hashes {
                per_file.insert(h.clone(), dl_guard.list_peer_sources(h).unwrap_or_default());
            }
        }

        // Addresses that just reported NNP/FNF, indexed by the file(s) that
        // flagged them. The flag is per-(file, address): a source that said
        // "no needed parts" for file A must never have its cooldown cleared
        // on file A — only on other files where it might be useful.
        let mut nnp_files_by_addr: std::collections::HashMap<
            String,
            std::collections::HashSet<String>,
        > = std::collections::HashMap::new();
        for (hash, srcs) in &per_file {
            for s in srcs {
                let flagged = s
                    .last_error
                    .as_deref()
                    .map(|e| {
                        let e = e.to_lowercase();
                        e.contains("no needed parts") || e.contains("file not found")
                    })
                    .unwrap_or(false);
                if flagged {
                    nnp_files_by_addr
                        .entry(s.address.clone())
                        .or_default()
                        .insert(hash.clone());
                }
            }
        }
        if nnp_files_by_addr.is_empty() {
            return;
        }

        // Clear the cooldown of the same address on *other* files so the
        // scheduler can dial it right away (eMule `SwapToAnotherFile`).
        for (hash, srcs) in &per_file {
            for s in srcs {
                let flagging_files = match nnp_files_by_addr.get(&s.address) {
                    Some(f) => f,
                    None => continue,
                };
                if flagging_files.contains(hash) {
                    continue;
                }
                let viable = matches!(
                    s.status,
                    PeerSourceStatus::Candidate
                        | PeerSourceStatus::Tried
                        | PeerSourceStatus::Allowed
                ) || (s.status == PeerSourceStatus::Failed
                    && s.cooldown_until_unix_ms.is_some_and(|c| c <= now_ms));
                if !viable {
                    continue;
                }
                let cooling = s
                    .cooldown_until_unix_ms
                    .is_some_and(|c| c > now_ms);
                if !cooling {
                    continue;
                }
                let mut dl_guard = self.state.downloads.lock().await;
                if dl_guard
                    .reset_peer_source_cooldown(hash, &s.address)
                    .unwrap_or(false)
                {
                    tracing::info!(
                        "[A4AF] Event swap: source {} freed by another file, reactivating for {hash}",
                        s.address
                    );
                }
                break; // one event swap per file per tick
            }
        }
    }
}
