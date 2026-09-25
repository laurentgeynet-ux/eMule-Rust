// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Swarm evaluation tick, hash validation, and connection scheduling.

use crate::services::peer_session::log_addr_str;
use crate::services::peer_session::log_peer_addr;
use crate::services::peer_session::log_peer_ip;
use crate::services::peer_session::log_server_addr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use mule_core::model::{DownloadStatus, PeerSourceOrigin, PeerSourceStatus};
use mule_protocol::ed2k::GetSourcesRequest;

use crate::services::downloads::types::DownloadSnapshot;
use crate::services::server_session::ServerConnectionStatus;

use super::types::{
    max_source_per_file_soft, max_source_per_file_udp, A4afFileState, KadSourceSearchState,
    SwarmCoordinator, SwarmLimits, HEARTBEAT_INTERVAL, KADEMLIA_ASK_INTERVAL,
    KADEMLIA_REASK_INTERVAL, KAD_MAX_FILE_SEARCHES, KAD_MAX_TOTAL_SEARCHES, MAX_CONNECT_PER_TICK,
    SERVER_REASK_INTERVAL, UDP_SERVER_REASK_INTERVAL,
};

/// eMule `PartFile.cpp:2291` — `UDPReaskForDownload` fires while the
/// source's next TCP file re-ask is less than 2 minutes away
/// (`GetTimeUntilReask() < MIN2MS(2)`).
const UDP_REASK_WINDOW_MS: i64 = 2 * 60 * 1_000;
/// …but still more than 1 second away (`GetTimeUntilReask() > SEC2MS(1)`)
/// — the UDP reply needs ~1 min to arrive before the TCP re-ask, and the
/// window prevents pinging a source whose TCP re-ask already fired.
const UDP_REASK_MIN_LEAD_MS: i64 = 1_000;
/// …and only when the last connection attempt is at least 20 min old
/// (`curTick >= GetLastTriedToConnectTime() + MIN2MS(20)`).
const UDP_REASK_MIN_CONNECT_AGE_MS: i64 = 20 * 60 * 1_000;

/// eMule `CPartFile::Process` (`PartFile.cpp:2291-2294`): a `DS_ONQUEUE`
/// source gets one UDP re-ask per cycle, inside the `(1 s, 2 min)` window
/// before its TCP re-ask, when the connection is ≥ 20 min old and we are
/// not firewalled (a NAT would drop the reply — eMule gates on
/// `!theApp.IsFirewalled()`).
fn should_send_udp_reask(
    target: &crate::services::client_udp::UdpReaskTarget,
    now_ms: i64,
    is_firewalled: bool,
) -> bool {
    if is_firewalled {
        return false;
    }
    let Some(next_reask) = target.next_tcp_reask_unix_ms else {
        return false;
    };
    let until_reask = next_reask.saturating_sub(now_ms);
    if until_reask <= UDP_REASK_MIN_LEAD_MS || until_reask >= UDP_REASK_WINDOW_MS {
        return false;
    }
    if let Some(connected_at) = target.connected_at_unix_ms {
        if now_ms.saturating_sub(connected_at) < UDP_REASK_MIN_CONNECT_AGE_MS {
            return false;
        }
    }
    true
}

impl SwarmCoordinator {
    /// eMule `CemuleApp::IsFirewalled` — unreachable from the outside:
    /// neither a HighID on the connected server nor an open Kad UDP
    /// socket. Firewalled clients skip UDP reasks entirely.
    async fn is_firewalled(&self) -> bool {
        let s_snap = self.state.server_session.snapshot().await;
        let server_high_id =
            s_snap.status == ServerConnectionStatus::Connected && s_snap.is_high_id;
        let udp_snap = self.state.kad.udp().get_snapshot().await;
        let fw_status = {
            let tracker = self.state.kad.udp().firewall_tracker();
            let fw = tracker.lock().await;
            fw.status
        };
        !(server_high_id
            || (udp_snap.is_active
                && fw_status == crate::services::kad_udp::types::UdpFirewallStatus::Open))
    }

    /// Evaluates all active downloads, runs source discovery, and connects to peers.
    pub async fn tick(&mut self) {
        // Periodic consolidated system heartbeat (every 60s)
        if self.last_heartbeat.elapsed() >= HEARTBEAT_INTERVAL {
            self.last_heartbeat = Instant::now();
            self.emit_heartbeat().await;
            // eMule SavePartFile cadence: persist .part.met companions for
            // in-progress downloads so a crash never loses more than ~60 s
            // of gap state.
            let mut dl_guard = self.state.downloads.lock().await;
            dl_guard.save_active_part_mets();
        }

        // 0. Decay stale download rates (reset frozen speed to 0 if inactive > 2s)
        let decayed_snapshots = {
            let mut dl_guard = self.state.downloads.lock().await;
            // Backstop de persistance : fsync des `.part` sales quand le
            // flux de blocs s'est interrompu entre deux `inject_range`
            // (la cadence principale vit dans le chemin de réception —
            // `PART_PERSIST_INTERVAL`).
            dl_guard.flush_dirty_downloads();
            dl_guard.decay_stale_rates()
        };
        for mut snap in decayed_snapshots {
            snap.pending_ranges = self.state.peer_session.try_pending_ranges(&snap.ed2k_hash);
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
                                let evict_ok = match dl_guard.evict_corrupted_parts(&id, &corrupted)
                                {
                                    Ok(snap) => {
                                        state_clone.broadcast_download_update(snap);
                                        true
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to evict corrupted parts for download {id}: {e}");
                                        false
                                    }
                                };
                                // eMule `CorruptionBlackBox::EvaluateData` a pu
                                // condamner des expéditeurs : déconnexion +
                                // rejet de leurs sources.
                                let banned = dl_guard.drain_corruption_bans(&id);
                                drop(dl_guard);
                                for addr in banned {
                                    tracing::warn!(
                                        "[CORRUPTION_BLACKBOX] Banning {addr} for download {id} (>32% corrupted data)"
                                    );
                                    let mut dl_guard = state_clone.downloads.lock().await;
                                    let _ = dl_guard.update_peer_source_status(
                                        &hex::encode(expected_hash),
                                        &addr,
                                        mule_core::model::PeerSourceStatus::Rejected,
                                        Some("corruption blackbox: exceeded ban threshold"),
                                        None,
                                    );
                                    drop(dl_guard);
                                    let _ =
                                        state_clone.peer_session.disconnect_peer(Some(&addr)).await;
                                }
                                if !evict_ok {
                                    let mut dl_guard = state_clone.downloads.lock().await;
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

        // État A4AF par fichier (eMule `ProcessA4AFClients`) : poids de
        // priorité et pairs actifs — plus aucun quota d'allocation n'est
        // calculé, chaque fichier connecte ses sources jusqu'à son plafond.
        // `source_count` alimente `UpdateAutoDownPriority` (priorité « auto »).
        let mut a4af_states = Vec::with_capacity(active_downloads.len());
        for dl in &active_downloads {
            if dl.ed2k_hash.is_empty() {
                continue;
            }
            let source_count = {
                let dl_guard = self.state.downloads.lock().await;
                dl_guard
                    .list_peer_sources(&dl.ed2k_hash)
                    .map(|v| v.len())
                    .unwrap_or(0)
            };
            let active_peers = self
                .state
                .peer_session
                .active_peer_count_for_hash(&dl.ed2k_hash)
                .await;
            a4af_states.push(A4afFileState::new(
                dl.ed2k_hash.clone(),
                &dl.priority,
                source_count,
                active_peers,
            ));
        }

        let limits = {
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
            SwarmLimits {
                max_global_peers: max_g,
                max_peers_per_file: max_f,
                min_peers_per_file: min_f,
            }
        };

        // eMule `CListenSocket::TooManySockets` : la limite globale de
        // connexions borne les nouvelles tentatives de ce tick, quel que
        // soit le fichier — premier arrivé, premier servi.
        let mut connect_budget = limits.max_global_peers.saturating_sub(
            self.state
                .peer_session
                .swarm_snapshot(None)
                .await
                .active_peer_count,
        );

        // Per-file source caps (eMule `.part.met` FT_MAXSOURCES /
        // `CPartFile::GetMaxSources`): when present, the per-file limit
        // overrides the global `max_peers_per_file` for connect, requery
        // and purge-threshold decisions.
        let file_max_sources: std::collections::HashMap<String, usize> = active_downloads
            .iter()
            .filter_map(|d| {
                if d.ed2k_hash.is_empty() {
                    return None;
                }
                let cap = d
                    .max_sources
                    .map(|v| v as usize)
                    .unwrap_or(limits.max_peers_per_file);
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
            self.a4af_rebalance(&a4af_states, &limits, now_ms).await;
        }

        // Client-UDP keep-alives — eMule `CPartFile::Process`
        // (`PartFile.cpp:2291-2294`): the `OP_REASKFILEPING` goes out once
        // per cycle, inside the `(1 s, 2 min)` window before the queued
        // session's TCP re-ask (`next_tcp_reask_unix_ms`), only when the
        // connection is ≥ 20 min old and we are not firewalled. The
        // service-level dedup (`UDP_REASK_INTERVAL`, pending-ack map)
        // remains as a backstop against repeated sends inside the window.
        let is_firewalled = self.is_firewalled().await;
        for target in self.state.peer_session.queued_udp_targets().await {
            if should_send_udp_reask(&target, now_ms, is_firewalled) {
                self.state.client_udp.send_reask_file_ping(&target, 0).await;
            }
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

            // eMule `CPartFile::Process` + `CDownloadQueue::Process`
            // (Opcodes.h) : chaque canal de découverte a sa propre cadence
            // et son propre seuil — pas de fenêtre adaptative.
            //   TCP OP_GETSOURCES      : SERVERREASKTIME (15 min), si
            //       sources < GetMaxSourcePerFileSoft (90 % du cap, ≤750)
            //   UDP OP_GLOBGETSOURCES2 : UDPSERVERREASKTIME (30 min), si
            //       sources < GetMaxSourcePerFileUDP (75 % du cap, ≤50)
            //   Kad search_sources     : KADEMLIAREASKTIME (1 h) *
            //       m_TotalSearchesKad (≤7), même seuil que l'UDP, plus
            //       KADEMLIATOTALFILE recherches simultanées et
            //       KADEMLIAASKTIME entre deux démarrages.
            let source_count = candidate_sources.len();
            let file_max = file_max_sources
                .get(&hash_hex)
                .copied()
                .unwrap_or(limits.max_peers_per_file);

            let tcp_due = self
                .last_source_query_tcp
                .get(&hash_hex)
                .is_none_or(|t| now.duration_since(*t) >= SERVER_REASK_INTERVAL);
            if tcp_due
                && source_count < max_source_per_file_soft(file_max)
                && self.state.server_session.snapshot().await.status
                    == ServerConnectionStatus::Connected
            {
                self.last_source_query_tcp.insert(hash_hex.clone(), now);
                let req = GetSourcesRequest::with_obfuscation(hash.0, size, obf_enabled);
                if let Err(e) = self.state.server_session.get_sources(req).await {
                    tracing::debug!("Swarm coordinator get_sources failed for {hash_hex}: {e}");
                } else {
                    tracing::info!(
                        "Swarm coordinator requested sources from eD2K server for file {hash_hex} (obfuscated={})",
                        obf_enabled
                    );
                }
            }

            let udp_due = self
                .last_source_query_udp
                .get(&hash_hex)
                .is_none_or(|t| now.duration_since(*t) >= UDP_SERVER_REASK_INTERVAL);
            if udp_due && source_count < max_source_per_file_udp(file_max) {
                // UDP source query (OP_GLOBGETSOURCES2) toward the connected
                // server — its UDP port is TCP+4 (eMule convention).
                // Fire-and-forget; answers arrive as OP_GLOBFOUNDSOURCES.
                if let Some(addr_str) = self.state.server_session.snapshot().await.server_address {
                    if let Ok(mut udp_addr) = addr_str.parse::<SocketAddr>() {
                        udp_addr.set_port(udp_addr.port().saturating_add(4));
                        self.last_source_query_udp.insert(hash_hex.clone(), now);
                        self.state
                            .server_udp
                            .send_glob_get_sources(udp_addr, &hash.0, size)
                            .await;
                    }
                }
            }

            let kad_due = self
                .kad_source_search
                .get(&hash_hex)
                .is_none_or(|s| now >= s.next_reask);
            if kad_due
                && source_count < max_source_per_file_udp(file_max)
                && self.state.network_policy.profile().allows_udp_kad()
                && self.state.kad.in_flight_file_searches().await < KAD_MAX_FILE_SEARCHES
                && now.duration_since(self.last_kad_file_request) >= KADEMLIA_ASK_INTERVAL
            {
                let total_searches = self
                    .kad_source_search
                    .get(&hash_hex)
                    .map(|s| (s.total_searches + 1).min(KAD_MAX_TOTAL_SEARCHES))
                    .unwrap_or(1);
                self.kad_source_search.insert(
                    hash_hex.clone(),
                    KadSourceSearchState {
                        next_reask: now + KADEMLIA_REASK_INTERVAL * total_searches,
                        total_searches,
                    },
                );
                self.last_kad_file_request = now;

                let kad = Arc::clone(&self.state.kad);
                let h = hash_hex.clone();
                tracing::info!(
                    "Swarm coordinator triggering Kad search_sources for file {hash_hex} (known sources in DB: {source_count}, search #{total_searches})",
                );
                tokio::spawn(async move {
                    let _ = kad.search_sources(&h, size).await;
                });
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

            // eMule `CemuleApp::CanDoCallback` (Emule.cpp:1067) :
            // la joignabilité des sources LowID dépend de notre propre
            // état — HighID serveur OU Kad connecté non firewalled — et,
            // quand nous sommes LowID, du serveur sur lequel vit la source
            // (`IsLocalServer`). `kad_open` = `CKademlia::IsConnected() &&
            // !IsFirewalled()` : socket UDP actif + test firewall positif.
            let cb_ctx = {
                let udp_snap = self.state.kad.udp().get_snapshot().await;
                let fw_status = {
                    let tracker = self.state.kad.udp().firewall_tracker();
                    let fw = tracker.lock().await;
                    fw.status
                };
                super::lowid_callback::CallbackCtx {
                    server_connected: client_id != 0,
                    server_high_id: server_is_high_id,
                    our_server_ip: server_ip,
                    our_server_port: server_port,
                    kad_open: udp_snap.is_active
                        && fw_status == crate::services::kad_udp::types::UdpFirewallStatus::Open,
                }
            };

            // eMule `CanAddSource` refuse les sources `lowid:` tant que
            // `IsFirewalled()` (ni HighID eD2K ni Kad ouvert) — le flag est
            // propagé au funnel d'ingestion `upsert_peer_sources` pour tous
            // les canaux de découverte (serveur TCP/UDP, SX, inbound).
            {
                let mut dl_guard = self.state.downloads.lock().await;
                dl_guard.set_lowid_source_acceptance(cb_ctx.server_high_id || cb_ctx.kad_open);
            }

            // eMule `CPartFile::Process`: near the per-file source cap
            // (>= 80%), periodically drop sources that cannot serve us —
            // unreachable LowIDs, NNP-flagged and repeatedly failed sources.
            if self
                .last_source_purge
                .get(&hash_hex)
                .is_none_or(|t| t.elapsed() >= super::types::SOURCE_PURGE_INTERVAL)
            {
                self.last_source_purge.insert(hash_hex.clone(), now);
                self.purge_excess_sources(&hash_hex, &candidate_sources, file_max, &cb_ctx, now_ms)
                    .await;
            }

            let swarm_snap = self
                .state
                .peer_session
                .swarm_snapshot(Some(&hash_hex))
                .await;
            let active_count = swarm_snap.active_peer_count;

            // eMule : pas de quota par fichier — chaque fichier connecte
            // ses sources viables jusqu'à son plafond (`GetMaxSources` /
            // `maxsourceperfile`). Seule la limite globale de connexions
            // (`connect_budget`, eMule `maxconnections`) et le pacing par
            // tick bornent les nouvelles tentatives.
            let needed = file_max
                .saturating_sub(active_count)
                .min(connect_budget)
                .min(MAX_CONNECT_PER_TICK);
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

                // eMule `AddSources` : les clients bannis (corruption,
                // `IsBannedClient`) ne sont jamais re-dialés — la source est
                // marquée `Rejected` pour ne pas revenir en candidat.
                let source_banned = {
                    let dl_guard = self.state.downloads.lock().await;
                    dl_guard.is_source_banned(&src.address)
                };
                if source_banned {
                    let mut dl_guard = self.state.downloads.lock().await;
                    let _ = dl_guard.update_peer_source_status(
                        &hash_hex,
                        &src.address,
                        PeerSourceStatus::Rejected,
                        Some("banned client"),
                        None,
                    );
                    continue;
                }

                // LowID source: unreachable directly — ask the connected
                // server to relay a callback (eMule `OP_CALLBACKREQUEST`).
                // The target then connects inbound; the inbound listener
                // matches it by client_id and adopts it as a download session.
                if let Some(lowid_str) = src.address.strip_prefix("lowid:") {
                    let Ok(lowid) = lowid_str.parse::<u32>() else {
                        continue;
                    };
                    // eMule `CemuleApp::CanDoCallback` — si ni notre ID
                    // eD2K ni Kad ne permettent le connect-back, la source
                    // reste parquée (eMule `DS_LOWTOLOWIP`, reprise
                    // possible dès que notre état change).
                    if !super::lowid_callback::can_do_callback(&src, &cb_ctx) {
                        continue;
                    }
                    // eMule `CanAddSource` : notre propre LowID répercuté par
                    // le serveur connecté ne doit pas être rappelé (les LowIDs
                    // ne sont uniques que par serveur — limité aux sources
                    // d'origine serveur comme chez eMule).
                    if lowid == client_id && src.origin == PeerSourceOrigin::Server {
                        continue;
                    }
                    // `OP_CALLBACKREQUEST` ne joindra la LowID que si elle
                    // vit sur notre serveur (`IsLocalServer`) — aucune autre
                    // option eMule (direct callback, buddy) n'est portée par
                    // les sources `lowid:`. Sans option, eMule supprime la
                    // source (`Disconnected("No Callback Option")`).
                    if !super::lowid_callback::has_server_callback_option(&src, &cb_ctx) {
                        let mut dl_guard = self.state.downloads.lock().await;
                        let _ = dl_guard.update_peer_source_status(
                            &hash_hex,
                            &src.address,
                            PeerSourceStatus::Rejected,
                            Some("LowID on a foreign server — no callback path"),
                            None,
                        );
                        continue;
                    }
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
                    // Le pair va tenter de se connecter à notre port TCP :
                    // même condition `CanDoCallback` que les sources
                    // `lowid:` (HighID serveur ou Kad ouvert).
                    if !super::lowid_callback::can_do_callback(&src, &cb_ctx) {
                        continue;
                    }
                    if self.state.peer_session.has_pending_kad_callback(src_ip) {
                        continue;
                    }

                    // eMule `AskForDownload` (`DownloadClient.cpp:185-206`) :
                    // source LowID déjà sollicitée (entrée `queued_peers` =
                    // `GetLastAskedTime() > 0`) et présente dans notre file
                    // d'upload (`US_ONUPLOADQUEUE`) → `m_bReaskPending` : le
                    // re-ask est différé d'un cycle pour laisser le pair
                    // composer vers nous à son propre re-ask d'upload. Le
                    // flag est consommé à sa connexion entrante.
                    if self
                        .state
                        .inbound_listener
                        .upload_manager()
                        .waiting_rank_by_ip(&src_ip)
                        .await
                        .is_some()
                    {
                        let next_cycle = now_ms
                            + crate::services::peer_session::status_query::FILE_REASK_INTERVAL
                                .as_millis() as i64;
                        if self
                            .state
                            .peer_session
                            .defer_reask_for_queued_peer(src_ip, &hash_hex, next_cycle)
                            .await
                        {
                            let mut dl_guard = self.state.downloads.lock().await;
                            let _ = dl_guard.update_peer_source_status(
                                &hash_hex,
                                &src.address,
                                PeerSourceStatus::Tried,
                                Some("reask deferred — peer waits in our upload queue"),
                                Some(next_cycle),
                            );
                            continue;
                        }
                    }

                    // eMule `TryToConnect` step 4 (`BaseClient.cpp:1403`) —
                    // le callback UDP direct précède le relais buddy : la
                    // source est TCP-firewalled mais UDP-ouverte et a
                    // annoncé le bit dans son byte d'options publié
                    // (`connect-options & 0x08`, OtherFunctions.cpp:4049).
                    if src.crypt_options
                        & mule_protocol::ed2k::client_udp::CONNECT_OPT_DIRECT_CALLBACK
                        != 0
                        && src.udp_port != 0
                    {
                        let (obf_enabled, obf_required) = self.state.effective_obfuscation().await;
                        // `GetMyConnectOptions(true, false)` : crypt flags
                        // seuls, le bit direct-callback reste à 0.
                        let conn_opts = (obf_enabled as u8)
                            | ((obf_enabled as u8) << 1)
                            | ((obf_required as u8) << 2);
                        let our_hash = self.state.identity.lock().await.user_hash();
                        let target = SocketAddr::new(src_ip, src.udp_port);
                        if self
                            .state
                            .client_udp
                            .send_direct_callback_req(
                                target,
                                self.state.ed2k_port(),
                                &our_hash,
                                conn_opts,
                            )
                            .await
                        {
                            tracing::info!(
                                "Sent OP_DIRECTCALLBACKREQ to LowID source {peer}:{port} for {hash_hex}",
                                peer = log_peer_ip(src_ip),
                                port = src.udp_port,
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
                            continue;
                        }
                        // Pas de socket UDP → l'option n'existe pas,
                        // on retombe sur le relais buddy.
                    }

                    let Some(buddy_bytes) = src.buddy_id_bytes() else {
                        continue;
                    };
                    let buddy_id = mule_core::kad::KadId(buddy_bytes);
                    let Some(buddy) = self.state.kad.find_contact(&buddy_id).await else {
                        tracing::debug!(
                            "Kad buddy {buddy_id:?} for LowID source {peer} not in routing table",
                            peer = log_peer_ip(src_ip)
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
                                "Requested Kad buddy callback to LowID source {peer} via {buddy} for {hash_hex}"
                            , peer = log_peer_ip(src_ip), buddy = log_peer_addr(buddy_addr));
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
                            tracing::debug!(
                                "Kad buddy callback request for {peer} failed: {e}",
                                peer = log_peer_ip(src_ip)
                            );
                        }
                    }
                    continue;
                }

                let peer_addr: SocketAddr = match src.address.parse() {
                    Ok(a) => a,
                    Err(_) => continue,
                };

                // eMule `CanAddSource` : ne jamais dialer notre propre
                // endpoint — quand le serveur nous a donné un HighID, notre
                // client_id EST notre IP publique ; une source `ip:port`
                // identique est notre propre annonce répercutée.
                if server_is_high_id {
                    if let std::net::IpAddr::V4(v4) = peer_addr.ip() {
                        if u32::from_le_bytes(v4.octets()) == client_id
                            && peer_addr.port() == self.state.ed2k_port()
                        {
                            continue;
                        }
                    }
                }

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
                            tracing::debug!(
                                "Permit denied for peer {peer}: {e}",
                                peer = log_peer_addr(peer_addr)
                            );
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

                tracing::debug!(
                    "Swarm coordinator initiating peer connection to {peer} for {file_hash_str} (max={file_max}, active={active_count}, obfu_target_hash={:?}, crypt_opts=0x{:02x})",
                    target_user_hash.is_some(),
                    peer_crypt_options
                , peer = log_peer_addr(peer_addr));
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
                        tracing::debug!(
                            "Peer connection to {peer} failed: {e}",
                            peer = log_peer_addr(peer_addr)
                        );
                    }
                });

                connected_this_tick += 1;
            }

            if connected_this_tick > 0 {
                tracing::debug!(
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

            // Chaque tentative de ce fichier consomme le budget global de
            // connexions (eMule `maxconnections` — `TooManySockets`).
            connect_budget = connect_budget.saturating_sub(connected_this_tick);

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
                tracing::debug!(
                    "Server failover: permit denied for {server}: {e}",
                    server = log_server_addr(addr)
                );
                return;
            }
        };

        let user_hash = self.state.identity.lock().await.user_hash();
        let ed2k_port = self.state.ed2k_port();
        let nickname = self.state.current_nickname().await;
        tracing::info!(
            "Server failover: connecting to {name} ({server})",
            server = log_server_addr(addr)
        );
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
        let (active_dl_count, other_dl_count, total_dl_rate, total_sources, dl_session_bytes, banned_count) = {
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
            (
                active,
                other,
                rate,
                sources,
                dl_guard.session_downloaded_bytes(),
                dl_guard.banned_source_count(),
            )
        };
        let swarm_snap = self.state.peer_session.swarm_snapshot(None).await;
        let dl_str = format!(
            "{} active, {} other ({:.1} KB/s, {} peers connected, {} known sources, session {:.1} MB)",
            active_dl_count,
            other_dl_count,
            total_dl_rate as f64 / 1024.0,
            swarm_snap.active_peer_count,
            total_sources,
            dl_session_bytes as f64 / (1024.0 * 1024.0),
        );

        // 4. Upload info
        let listener_snap = self.state.inbound_listener.snapshot().await;
        let ul_session_bytes = self
            .state
            .inbound_listener
            .upload_manager()
            .total_uploaded_bytes();
        let ul_str = format!(
            "{} active ({:.1} KB/s, {} queued, session {:.1} MB, {} banned)",
            listener_snap.active_uploads,
            listener_snap.upload_rate_bps as f64 / 1024.0,
            listener_snap.queued_uploads,
            ul_session_bytes as f64 / (1024.0 * 1024.0),
            banned_count,
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
        states: &[A4afFileState],
        limits: &SwarmLimits,
        now_ms: i64,
    ) {
        for starved in states {
            if starved.current_active_peers >= limits.min_peers_per_file {
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
            let donors: std::collections::HashMap<&str, u32> = states
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
                if donor_active <= limits.min_peers_per_file {
                    continue;
                }
                tracing::info!(
                    "[A4AF] Swapping source {} from file {} (prio={}) to starved file {} (prio={})",
                    log_addr_str(peer_addr),
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
    /// - `lowid:`/`kadcb:` sources we cannot call back at all
    ///   (`!CanDoCallback` — eMule `DS_LOWTOLOWIP`, purgées à 80 %);
    /// - `kadcb:` sources without a buddy id (unreachable);
    /// - sources flagged "no needed parts" (NNP) — their part mask covers
    ///   nothing we still need;
    /// - sources that failed repeatedly (fail_count >= 3);
    /// - sessions flagged `remote_queue_full` (`OP_QUEUEFULL` reçu sur le
    ///   re-ask UDP) — eMule `DS_ONQUEUE && IsRemoteQueueFull()` : ces
    ///   sources sont purgées en priorité quand la srclist atteint 80 % de
    ///   `GetMaxSources` (`PartFile.cpp:2285-2295`, cadence 1 min).
    ///
    /// Purge removes at most `count - threshold + 1` rows per pass —
    /// eMule descends sous le seuil (`GetSourceCount() >= 4/5` → retire
    /// encore à égalité), l'état final est `threshold - 1`.
    async fn purge_excess_sources(
        &mut self,
        file_hash: &str,
        sources: &[mule_core::model::PeerSource],
        file_max: usize,
        cb_ctx: &super::lowid_callback::CallbackCtx,
        now_ms: i64,
    ) {
        if file_max == 0 || sources.len() < file_max * 4 / 5 {
            return;
        }
        // eMule purge tant que `GetSourceCount() >= GetMaxSources()*4/5` —
        // l'état final est `seuil - 1` (à 8/10 il retire 1 source, pas 0).
        let budget = sources.len() - file_max * 4 / 5 + 1;

        // Sessions live marquées remote-queue-full — eMule garde le flag
        // sur le client (`m_bRemoteQueueFull`), nous sur le snapshot de
        // session. Correspondance adresse exacte puis IP seule (les sources
        // `lowid:`/`kadcb:` ont un peer_address ip:port différent de leur
        // adresse source).
        let rqfs = self
            .state
            .peer_session
            .swarm_snapshot(Some(file_hash))
            .await;
        let mut rqf_addrs: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut rqf_ips: std::collections::HashSet<std::net::IpAddr> =
            std::collections::HashSet::new();
        for p in &rqfs.peers {
            if !p.remote_queue_full {
                continue;
            }
            if let Some(addr) = &p.peer_address {
                rqf_addrs.insert(addr.clone());
                if let Ok(sa) = addr.parse::<std::net::SocketAddr>() {
                    rqf_ips.insert(sa.ip());
                }
            }
        }
        let rqf_source = |s: &&mule_core::model::PeerSource| {
            rqf_addrs.contains(&s.address)
                || crate::services::downloads::source_addr_ip(&s.address)
                    .is_some_and(|ip| rqf_ips.contains(&ip))
        };

        let mut removed = 0usize;
        for s in sources {
            if removed >= budget {
                break;
            }
            let nnp = s
                .last_error
                .as_deref()
                .map(|e| e.to_lowercase().contains("no needed parts"))
                .unwrap_or(false);
            let unreachable_lowid = (s.address.starts_with("lowid:")
                || s.address.starts_with("kadcb:"))
                && (!super::lowid_callback::can_do_callback(s, cb_ctx)
                    || (s.address.starts_with("kadcb:") && s.buddy_id.is_none()));
            let repeatedly_failed = s.status == PeerSourceStatus::Failed && s.fail_count >= 3;
            let remote_queue_full = rqf_source(&s);
            if !(nnp || unreachable_lowid || repeatedly_failed || remote_queue_full) {
                continue;
            }
            // eMule purge aussi les DS_ONQUEUE connectés quand ils sont
            // RemoteQueueFull — le statut Connected ne les protège pas.
            if s.status == PeerSourceStatus::Connected && !remote_queue_full {
                continue;
            }
            let why = if remote_queue_full {
                "remote queue full"
            } else if nnp {
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
                    // eMule `RemoveSource` détruit le download client —
                    // terminer la session live associée si elle existe
                    // (clé de session = peer_address réel, pas l'adresse
                    // source `lowid:`/`kadcb:`).
                    let session_addr = rqfs.peers.iter().find_map(|p| {
                        let pa = p.peer_address.as_ref()?;
                        let src_ip = crate::services::downloads::source_addr_ip(&s.address)?;
                        let pa_ip = crate::services::downloads::source_addr_ip(pa)?;
                        (p.remote_queue_full && (pa == &s.address || pa_ip == src_ip))
                            .then(|| pa.clone())
                    });
                    if let Some(addr) = session_addr {
                        let _ = self.state.peer_session.disconnect_peer(Some(&addr)).await;
                    }
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
                let cooling = s.cooldown_until_unix_ms.is_some_and(|c| c > now_ms);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::peer_session::{PeerConnectionStatus, PeerSessionSnapshot};
    use crate::services::swarm_coordinator::types::SwarmCoordinator;
    use mule_core::model::PeerSource;
    use std::collections::HashMap;

    fn make_source(file_hash: &str, addr: &str) -> PeerSource {
        PeerSource {
            file_hash: file_hash.to_string(),
            address: addr.to_string(),
            ip: 0,
            port: 0,
            origin: PeerSourceOrigin::Server,
            discovered_at_unix_ms: 0,
            last_seen_unix_ms: 0,
            status: PeerSourceStatus::Candidate,
            fail_count: 0,
            last_error: None,
            cooldown_until_unix_ms: None,
            user_hash: None,
            crypt_options: 0,
            buddy_id: None,
            software: None,
            availability: None,
            server_ip: 0,
            server_port: 0,
            udp_port: 0,
        }
    }

    /// État daemon minimal en mémoire — `mule_test_support` ne peut pas
    /// servir ici : ses helpers renvoient l'`AppState` de la crate externe
    /// (double version du type en unit-test).
    fn test_state(dir: &std::path::Path, hash_hex: &str) -> crate::state::AppState {
        let config = crate::config::DaemonConfig::new(dir.join("data"), "test-token")
            .with_workdir_mode(crate::workdir::WorkDirMode::Portable);
        let mut downloads =
            crate::services::downloads::DownloadService::in_memory(dir.join("temp")).unwrap();
        let link = format!("ed2k://|file|f.bin|10000000|{hash_hex}|/");
        downloads
            .add_ed2k_link(
                &link,
                &dir.join("downloads"),
                mule_core::storage::part_file::AllocationPolicy::None,
                None,
            )
            .unwrap();
        crate::state::AppState::new(config, downloads)
    }

    /// Contexte « LowID sans Kad » — `can_do_callback` faux pour toute
    /// source (équivalent de l'ancien `server_is_high_id = false`).
    fn test_cb_ctx() -> super::super::lowid_callback::CallbackCtx {
        super::super::lowid_callback::CallbackCtx {
            server_connected: true,
            server_high_id: false,
            our_server_ip: 0,
            our_server_port: 0,
            kad_open: false,
        }
    }

    fn coordinator(state: crate::state::AppState) -> SwarmCoordinator {
        SwarmCoordinator {
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
        }
    }

    /// eMule `CPartFile::Process` (`PartFile.cpp:2285-2295`) : à ≥80 % de
    /// `GetMaxSources`, une source `DS_ONQUEUE && IsRemoteQueueFull` est
    /// purgée — la session live est terminée et la ligne source supprimée,
    /// alors qu'une source connectée sans flag survit.
    #[tokio::test]
    async fn purge_evicts_remote_queue_full_sources_at_80_percent() {
        let dir = tempfile::tempdir().unwrap();
        let hash = "0123456789abcdef0123456789abcdef".to_string();
        let state = test_state(dir.path(), &hash);
        let file_max = 10usize;
        // 8 sources sur un cap de 10 → 80 %, seuil eMule atteint.
        let sources: Vec<PeerSource> = (0..8)
            .map(|i| make_source(&hash, &format!("10.0.0.{i}:49662")))
            .collect();
        {
            let mut dl = state.downloads.lock().await;
            dl.upsert_peer_sources(&hash, &sources).unwrap();
        }

        // Session live queued + remote_queue_full sur la source 10.0.0.3.
        state
            .peer_session
            .insert_test_session(PeerSessionSnapshot {
                status: PeerConnectionStatus::Queued,
                file_hash: Some(hash.clone()),
                peer_address: Some("10.0.0.3:49662".to_string()),
                remote_queue_full: true,
                ..Default::default()
            })
            .await;

        let mut coord = coordinator(state.clone());
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        coord
            .purge_excess_sources(&hash, &sources, file_max, &test_cb_ctx(), now_ms)
            .await;

        // La source RQF est supprimée de la liste persistée.
        let remaining = state
            .downloads
            .lock()
            .await
            .list_peer_sources(&hash)
            .unwrap();
        assert_eq!(remaining.len(), 7);
        assert!(!remaining.iter().any(|s| s.address == "10.0.0.3:49662"));
        // Sa session live est terminée (eMule `RemoveSource` détruit le
        // download client).
        let snap = state.peer_session.swarm_snapshot(Some(&hash)).await;
        assert!(snap.peers.is_empty());
    }

    /// Sous le seuil de 80 %, aucune purge n'a lieu même avec une session
    /// remote-queue-full.
    #[tokio::test]
    async fn purge_keeps_remote_queue_full_below_80_percent() {
        let dir = tempfile::tempdir().unwrap();
        let hash = "0123456789abcdef0123456789abcdef".to_string();
        let state = test_state(dir.path(), &hash);
        let sources: Vec<PeerSource> = (0..5)
            .map(|i| make_source(&hash, &format!("10.0.1.{i}:49662")))
            .collect();
        {
            let mut dl = state.downloads.lock().await;
            dl.upsert_peer_sources(&hash, &sources).unwrap();
        }
        state
            .peer_session
            .insert_test_session(PeerSessionSnapshot {
                status: PeerConnectionStatus::Queued,
                file_hash: Some(hash.clone()),
                peer_address: Some("10.0.1.3:49662".to_string()),
                remote_queue_full: true,
                ..Default::default()
            })
            .await;

        let mut coord = coordinator(state.clone());
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        coord
            .purge_excess_sources(&hash, &sources, 10, &test_cb_ctx(), now_ms)
            .await;

        let remaining = state
            .downloads
            .lock()
            .await
            .list_peer_sources(&hash)
            .unwrap();
        assert_eq!(remaining.len(), 5);
        let snap = state.peer_session.swarm_snapshot(Some(&hash)).await;
        assert_eq!(snap.peers.len(), 1);
    }

    fn udp_target(
        connected_at_unix_ms: Option<i64>,
        next_tcp_reask_unix_ms: Option<i64>,
    ) -> crate::services::client_udp::UdpReaskTarget {
        crate::services::client_udp::UdpReaskTarget {
            addr: "10.0.0.9:49672".parse().unwrap(),
            file_hash: [7u8; 16],
            connected_at_unix_ms,
            next_tcp_reask_unix_ms,
        }
    }

    /// eMule `PartFile.cpp:2291-2294` — bornes de la fenêtre UDP : le ping
    /// part uniquement quand le re-ask TCP arrive dans `(1 s, 2 min)`,
    /// la connexion a ≥ 20 min et nous ne sommes pas firewalled.
    #[test]
    fn udp_reask_window_boundaries() {
        let now_ms = 1_000_000_000i64;
        let old_conn = Some(now_ms - super::UDP_REASK_MIN_CONNECT_AGE_MS);
        // Deadline pile à 2 min → hors fenêtre (eMule exige < MIN2MS(2)).
        assert!(!super::should_send_udp_reask(
            &udp_target(old_conn, Some(now_ms + super::UDP_REASK_WINDOW_MS)),
            now_ms,
            false,
        ));
        // 1 ms sous la borne haute → dans la fenêtre.
        assert!(super::should_send_udp_reask(
            &udp_target(old_conn, Some(now_ms + super::UDP_REASK_WINDOW_MS - 1)),
            now_ms,
            false,
        ));
        // Deadline pile à 1 s → hors fenêtre (eMule exige > SEC2MS(1)).
        assert!(!super::should_send_udp_reask(
            &udp_target(old_conn, Some(now_ms + super::UDP_REASK_MIN_LEAD_MS)),
            now_ms,
            false,
        ));
        // 1 ms au-dessus de la borne basse → dans la fenêtre.
        assert!(super::should_send_udp_reask(
            &udp_target(old_conn, Some(now_ms + super::UDP_REASK_MIN_LEAD_MS + 1)),
            now_ms,
            false,
        ));
        // Deadline déjà dépassée → jamais de ping.
        assert!(!super::should_send_udp_reask(
            &udp_target(old_conn, Some(now_ms - 1)),
            now_ms,
            false,
        ));
    }

    /// eMule exige `curTick >= LastTriedToConnectTime + MIN2MS(20)` : une
    /// session en file depuis moins de 20 min ne déclenche pas de ping.
    #[test]
    fn udp_reask_requires_old_connection() {
        let now_ms = 1_000_000_000i64;
        let in_window = Some(now_ms + 60_000);
        // Connexion de 19 min → refusé ; pile 20 min → accepté
        // (eMule utilise >=).
        assert!(!super::should_send_udp_reask(
            &udp_target(
                Some(now_ms - super::UDP_REASK_MIN_CONNECT_AGE_MS + 1),
                in_window,
            ),
            now_ms,
            false,
        ));
        assert!(super::should_send_udp_reask(
            &udp_target(
                Some(now_ms - super::UDP_REASK_MIN_CONNECT_AGE_MS),
                in_window,
            ),
            now_ms,
            false,
        ));
        // `connected_at` inconnu → permissif (le champ est toujours rempli
        // pour une vraie session ; absent = pas de preuve de fraîcheur).
        assert!(super::should_send_udp_reask(
            &udp_target(None, in_window),
            now_ms,
            false,
        ));
        // Pas de deadline publiée (session à peine en file) → pas de ping.
        assert!(!super::should_send_udp_reask(
            &udp_target(Some(now_ms - super::UDP_REASK_MIN_CONNECT_AGE_MS), None,),
            now_ms,
            false,
        ));
    }

    /// eMule `!theApp.IsFirewalled()` — un client injoignable n'émet pas
    /// de re-ask UDP (le NAT jetterait la réponse).
    #[test]
    fn udp_reask_skipped_when_firewalled() {
        let now_ms = 1_000_000_000i64;
        let target = udp_target(
            Some(now_ms - super::UDP_REASK_MIN_CONNECT_AGE_MS),
            Some(now_ms + 60_000),
        );
        assert!(!super::should_send_udp_reask(&target, now_ms, true));
    }
}
