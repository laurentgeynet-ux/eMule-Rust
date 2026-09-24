// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;

use mule_core::kad::{KBucketTable, KadContact, KadId};
use mule_core::model::{PeerSource, PeerSourceOrigin, PeerSourceStatus};
use mule_filter::IpFilter;
use mule_network_policy::{classify_ip, IpCategory, NetworkPolicyManager, NetworkProfile};

use crate::services::downloads::DownloadService;
use crate::services::kad_udp::KadUdpService;

pub async fn search_sources_iterative(
    file_hash_hex: &str,
    file_size: u64,
    routing_table: &Mutex<KBucketTable>,
    downloads: &Arc<Mutex<DownloadService>>,
    policy_mgr: &NetworkPolicyManager,
    ip_filter: &Arc<IpFilter>,
    udp: &Arc<KadUdpService>,
    in_flight_searches: &Arc<Mutex<std::collections::HashSet<String>>>,
) -> Result<Vec<PeerSource>, String> {
    let f_key = file_hash_hex.to_lowercase();
    {
        let mut in_flight = in_flight_searches.lock().await;
        if in_flight.contains(&f_key) {
            tracing::info!(
                "Kad search_sources already in flight for file {}, coalescing duplicate request",
                f_key
            );
            return Ok(Vec::new());
        }
        in_flight.insert(f_key.clone());
    }

    struct InFlightSearchGuard {
        set: Arc<Mutex<std::collections::HashSet<String>>>,
        key: String,
    }
    impl Drop for InFlightSearchGuard {
        fn drop(&mut self) {
            let s = Arc::clone(&self.set);
            let k = self.key.clone();
            tokio::spawn(async move {
                let mut guard = s.lock().await;
                guard.remove(&k);
            });
        }
    }
    let _guard = InFlightSearchGuard {
        set: Arc::clone(in_flight_searches),
        key: f_key,
    };

    let target_kad_id = KadId::from_hash_hex(file_hash_hex)
        .ok_or_else(|| format!("Invalid target file hash hex: {file_hash_hex}"))?;

    {
        let table = routing_table.lock().await;
        if table.find_closest(&target_kad_id, 1).is_empty() {
            tracing::warn!(
                "Kad search_sources for {}: routing table is empty (0 contacts)! Import nodes.dat or bootstrap to find sources.",
                file_hash_hex
            );
            return Ok(Vec::new());
        }
    }

    tracing::info!(
        "Kad search_sources: performing iterative find_node convergence for target hash {}...",
        file_hash_hex
    );

    const MAX_CONVERGENCE_ROUNDS: usize = 6;
    let candidates: Vec<KadContact> = super::lookup::converge_closest_contacts(
        &target_kad_id,
        routing_table,
        policy_mgr,
        ip_filter,
        udp,
        MAX_CONVERGENCE_ROUNDS,
    )
    .await;

    let search_targets: Vec<KadContact> = candidates.into_iter().take(16).collect();
    let within_tol = search_targets
        .iter()
        .filter(|c| c.kad_id.is_within_search_tolerance(&target_kad_id))
        .count();

    tracing::info!(
        "Kad starting search_sources for file {} (size={}): querying {} closest converged Kad contacts ({} within SEARCHTOLERANCE)",
        file_hash_hex,
        file_size,
        search_targets.len(),
        within_tol
    );

    let mut discovered_sources = Vec::new();
    let profile = policy_mgr.profile();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut futs = Vec::with_capacity(search_targets.len());
    for contact in &search_targets {
        let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
        let udp_c = Arc::clone(udp);
        let target_hash = target_kad_id.0;
        let f_hex = file_hash_hex.to_string();
        futs.push(async move {
            tracing::debug!(
                "Kad sending SearchSources request for {} to {}",
                f_hex,
                addr
            );
            let res = udp_c
                .send_search_sources(addr, &target_hash, file_size)
                .await;
            (addr, res)
        });
    }

    let results = futures_util::future::join_all(futs).await;
    {
        // Les timeouts SearchSources ne comptent PAS comme échec du contact :
        // eMule ne répond rien quand le nœud n'indexe pas le fichier
        // (SendValidSourceResult silencieux) — le nœud peut être sain.
        // Seule une réponse confirme la vivacité (mark_seen).
        let mut table = routing_table.lock().await;
        for (contact, (_, res)) in search_targets.iter().zip(results.iter()) {
            if res.is_ok() {
                table.mark_seen(&contact.kad_id, now_ms);
            }
        }
    }
    for (addr, res) in results {
        match res {
            Ok(wires) => {
                tracing::info!(
                    "Kad received {} source candidates from node {}",
                    wires.len(),
                    addr
                );
                for w in wires {
                    let ip = Ipv4Addr::from(w.ip);
                    let ip_addr = IpAddr::V4(ip);

                    let cat = classify_ip(ip_addr);
                    let policy_allowed = match profile {
                        NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                        NetworkProfile::PublicEd2kPreview
                        | NetworkProfile::PublicEd2kKadPreview => cat == IpCategory::PublicRoutable,
                    };

                    if !policy_allowed {
                        continue;
                    }
                    if let Some(rule) = ip_filter.check_ip(ip_addr) {
                        tracing::debug!(
                            "[IP_FILTER] Discarded Kad source candidate {} for {} (rule: {})",
                            ip_addr,
                            file_hash_hex,
                            rule.description
                        );
                        continue;
                    }

                    // LowID Kad source (tcp_port == 0): only reachable through a
                    // buddy callback (KADEMLIA_CALLBACK_REQ). Keep it only if a
                    // buddy ID was published; the peer's own IP is used to adopt
                    // the incoming callback connection.
                    let (peer_addr_str, buddy_hex) = if w.tcp_port == 0 {
                        match w.buddy_id {
                            Some(bid) => (format!("kadcb:{ip}"), Some(KadId(bid).to_hash_hex())),
                            None => continue,
                        }
                    } else {
                        (format!("{}:{}", ip, w.tcp_port), None)
                    };
                    let ip_u32 = u32::from_le_bytes(w.ip);

                    // eMule déduplique les sources par adresse IP:port : la même
                    // source peut être rapportée par plusieurs contacts Kad.
                    if discovered_sources
                        .iter()
                        .any(|s: &PeerSource| s.address == peer_addr_str)
                    {
                        continue;
                    }

                    discovered_sources.push(PeerSource {
                        file_hash: file_hash_hex.to_lowercase(),
                        address: peer_addr_str,
                        ip: ip_u32,
                        port: w.tcp_port,
                        origin: PeerSourceOrigin::Kad,
                        discovered_at_unix_ms: now_ms as i64,
                        last_seen_unix_ms: now_ms as i64,
                        status: PeerSourceStatus::Candidate,
                        fail_count: 0,
                        last_error: None,
                        cooldown_until_unix_ms: None,
                        user_hash: Some(KadId(w.kad_id).to_hash_hex()),
                        crypt_options: w.encryption,
                        buddy_id: buddy_hex,
                        software: None,
                        availability: None,
                        // Kad results carry no server context (buddy path).
                        server_ip: 0,
                        server_port: 0,
                    });
                }
            }
            Err(e) => {
                tracing::debug!(
                    "Kad search_sources to {} timed out or node does not index file: {}",
                    addr,
                    e
                );
            }
        }
    }

    if !discovered_sources.is_empty() {
        let mut dl = downloads.lock().await;
        let upserted = dl
            .upsert_peer_sources(file_hash_hex, &discovered_sources)
            .unwrap_or(0);
        let total_swarm = dl
            .find_by_ed2k_hash(file_hash_hex)
            .map(|d| d.source_count)
            .unwrap_or(0);
        tracing::info!(
            "[KAD_SEARCH] SearchSources for file {} finished: {} candidate responses -> {} new unique sources persisted (total swarm: {} sources)",
            file_hash_hex,
            discovered_sources.len(),
            upserted,
            total_swarm
        );
    } else {
        tracing::info!(
            "[KAD_SEARCH] SearchSources for file {} finished: 0 sources found from queried contacts",
            file_hash_hex
        );
    }

    Ok(discovered_sources)
}
