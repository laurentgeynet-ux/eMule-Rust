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

    let initial_closest = {
        let table = routing_table.lock().await;
        table.find_closest(&target_kad_id, 32)
    };

    if initial_closest.is_empty() {
        tracing::warn!(
            "Kad search_sources for {}: routing table is empty (0 contacts)! Import nodes.dat or bootstrap to find sources.",
            file_hash_hex
        );
        return Ok(Vec::new());
    }

    let mut candidates: std::collections::BTreeMap<KadId, KadContact> =
        std::collections::BTreeMap::new();
    for c in initial_closest {
        let dist = c.kad_id.xor_distance(&target_kad_id);
        candidates.insert(dist, c);
    }

    tracing::info!(
        "Kad search_sources: performing iterative find_node convergence for target hash {} (initial pool: {} contacts)...",
        file_hash_hex,
        candidates.len()
    );

    let mut queried_nodes = std::collections::HashSet::new();
    const MAX_CONVERGENCE_ROUNDS: usize = 6;
    const ALPHA: usize = 5;

    for iter in 0..MAX_CONVERGENCE_ROUNDS {
        let unqueried: Vec<_> = candidates
            .values()
            .filter(|c| !queried_nodes.contains(&SocketAddr::new(IpAddr::V4(c.ip), c.udp_port)))
            .take(ALPHA)
            .cloned()
            .collect();

        if unqueried.is_empty() {
            tracing::debug!("Kad search_sources: convergence reached after {} rounds (no closer unqueried contacts)", iter);
            break;
        }

        // Check if top candidates already satisfy SEARCHTOLERANCE (0x01000000)
        let within_tolerance_count = candidates
            .values()
            .take(16)
            .filter(|c| c.kad_id.is_within_search_tolerance(&target_kad_id))
            .count();

        if within_tolerance_count >= 12 && iter >= 2 {
            tracing::info!(
                "Kad search_sources: early convergence after {} rounds ({} candidates within SEARCHTOLERANCE)",
                iter,
                within_tolerance_count
            );
            break;
        }

        let mut find_futs = Vec::new();
        for contact in unqueried {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            let receiver_id = contact.kad_id;
            queried_nodes.insert(addr);
            let udp_c = Arc::clone(udp);
            let target_id = target_kad_id;
            find_futs.push(async move {
                match tokio::time::timeout(
                    std::time::Duration::from_millis(2500),
                    udp_c.send_find_node(addr, &target_id, &receiver_id),
                )
                .await
                {
                    Ok(res) => res,
                    Err(_) => Err("FindNode round timeout (2.5s)".to_string()),
                }
            });
        }

        let find_results = futures_util::future::join_all(find_futs).await;
        let mut new_contacts = Vec::new();
        let profile = policy_mgr.profile();
        for res in find_results {
            if let Ok(contacts) = res {
                for mut c in contacts {
                    let cat = classify_ip(IpAddr::V4(c.ip));
                    let policy_allowed = match profile {
                        NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                        NetworkProfile::PublicEd2kPreview
                        | NetworkProfile::PublicEd2kKadPreview => cat == IpCategory::PublicRoutable,
                    };
                    if policy_allowed && ip_filter.check_ip(IpAddr::V4(c.ip)).is_none() {
                        c.verified = true;
                        new_contacts.push(c);
                    }
                }
            }
        }

        if !new_contacts.is_empty() {
            let mut table = routing_table.lock().await;
            for c in &new_contacts {
                table.insert_or_update(c.clone());
                let dist = c.kad_id.xor_distance(&target_kad_id);
                candidates.insert(dist, c.clone());
            }
            tracing::debug!(
                "Kad search_sources: convergence round {} found {} closer contacts (candidate pool now {}, closest chunk0=0x{:08x})",
                iter + 1,
                new_contacts.len(),
                candidates.len(),
                candidates.keys().next().map_or(0, |d| d.dword(0))
            );
        }
    }

    let search_targets: Vec<KadContact> = candidates.values().take(16).cloned().collect();
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
    for contact in search_targets {
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

                    if w.tcp_port == 0 {
                        continue;
                    }

                    let peer_addr_str = format!("{}:{}", ip, w.tcp_port);
                    let ip_u32 = u32::from_le_bytes(w.ip);

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
