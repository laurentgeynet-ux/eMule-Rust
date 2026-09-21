use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;

use mule_core::kad::{extract_kad_keywords, hash_kad_keyword, KBucketTable, KadContact, KadId};
use mule_filter::IpFilter;
use mule_network_policy::{classify_ip, IpCategory, NetworkPolicyManager, NetworkProfile};

use crate::services::kad_udp::KadUdpService;

pub async fn search_keywords_iterative(
    term: &str,
    min_size: Option<u64>,
    max_size: Option<u64>,
    routing_table: &Mutex<KBucketTable>,
    policy_mgr: &NetworkPolicyManager,
    ip_filter: &Arc<IpFilter>,
    udp: &Arc<KadUdpService>,
) -> Result<Vec<serde_json::Value>, String> {
    let (norm_word, keyword_kad_id) = extract_kad_keywords(term)
        .into_iter()
        .next()
        .unwrap_or_else(|| hash_kad_keyword(term));

    let initial_closest = {
        let table = routing_table.lock().await;
        table.find_closest(&keyword_kad_id, 32)
    };

    if initial_closest.is_empty() {
        tracing::warn!(
            "Kad search_keywords for '{}' (norm='{}'): routing table is empty (0 contacts)",
            term,
            norm_word
        );
        return Ok(Vec::new());
    }

    let mut candidates: std::collections::BTreeMap<KadId, KadContact> =
        std::collections::BTreeMap::new();
    for c in initial_closest {
        let dist = c.kad_id.xor_distance(&keyword_kad_id);
        candidates.insert(dist, c);
    }

    tracing::info!(
        "Kad search_keywords: starting search for '{}' (hash={}): performing iterative find_node convergence (initial pool: {} contacts)...",
        term,
        keyword_kad_id.to_hex(),
        candidates.len()
    );

    let mut queried_find_nodes = std::collections::HashSet::new();
    const MAX_CONVERGENCE_ROUNDS: usize = 6;
    const ALPHA: usize = 5;

    // 1. Iterative find_node step: query closest unqueried contacts to converge towards the keyword ID
    for iter in 0..MAX_CONVERGENCE_ROUNDS {
        let unqueried: Vec<_> = candidates
            .values()
            .filter(|c| {
                !queried_find_nodes.contains(&SocketAddr::new(IpAddr::V4(c.ip), c.udp_port))
            })
            .take(ALPHA)
            .cloned()
            .collect();

        if unqueried.is_empty() {
            tracing::debug!("Kad search_keywords: convergence reached after {} rounds (no closer unqueried contacts)", iter);
            break;
        }

        // Check if top candidates already satisfy SEARCHTOLERANCE (0x01000000)
        let within_tolerance_count = candidates
            .values()
            .take(16)
            .filter(|c| c.kad_id.is_within_search_tolerance(&keyword_kad_id))
            .count();

        if within_tolerance_count >= 12 && iter >= 2 {
            tracing::info!(
                "Kad search_keywords: early convergence after {} rounds ({} candidates within SEARCHTOLERANCE)",
                iter,
                within_tolerance_count
            );
            break;
        }

        let mut find_futs = Vec::new();
        for contact in unqueried {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            let receiver_id = contact.kad_id;
            queried_find_nodes.insert(addr);
            let udp_c = Arc::clone(udp);
            let target_id = keyword_kad_id;
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
        for contacts in find_results.into_iter().flatten() {
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

        if !new_contacts.is_empty() {
            let mut table = routing_table.lock().await;
            for c in &new_contacts {
                table.insert_or_update(c.clone());
                let dist = c.kad_id.xor_distance(&keyword_kad_id);
                candidates.insert(dist, c.clone());
            }
            tracing::debug!(
                "Kad search_keywords: find_node convergence round {} discovered {} new contacts (candidate pool now {}, closest chunk0=0x{:08x})",
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
        .filter(|c| c.kad_id.is_within_search_tolerance(&keyword_kad_id))
        .count();

    tracing::info!(
        "Kad search_keywords: querying {} converged contacts for keyword '{}' ({} within SEARCHTOLERANCE)...",
        search_targets.len(),
        term,
        within_tol
    );

    let mut discovered_entries: Vec<serde_json::Value> = Vec::new();
    let mut seen_hashes = std::collections::HashSet::new();
    let profile = policy_mgr.profile();

    let mut futs = Vec::with_capacity(search_targets.len());
    for contact in search_targets {
        let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
        let cat = classify_ip(IpAddr::V4(contact.ip));
        let policy_allowed = match profile {
            NetworkProfile::LocalLab => cat == IpCategory::Loopback,
            NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                cat == IpCategory::PublicRoutable
            }
        };

        if !policy_allowed || ip_filter.check_ip(IpAddr::V4(contact.ip)).is_some() {
            continue;
        }

        let udp_c = Arc::clone(udp);
        let target_kw = keyword_kad_id.0;
        futs.push(async move {
            let res = udp_c.send_search_keyword(addr, &target_kw, 0).await;
            (addr, res)
        });
    }

    let results = futures_util::future::join_all(futs).await;
    for (addr, res) in results {
        match res {
            Ok(wires) => {
                tracing::info!(
                    "Kad received {} keyword search entries from node {}",
                    wires.len(),
                    addr
                );
                for w in wires {
                    let file_hash_hex = KadId(w.file_hash).to_hash_hex();
                    if let Some(min) = min_size {
                        if w.file_size < min {
                            continue;
                        }
                    }
                    if let Some(max) = max_size {
                        if w.file_size > max {
                            continue;
                        }
                    }

                    if seen_hashes.insert(file_hash_hex.clone()) {
                        discovered_entries.push(serde_json::json!({
                            "fileHash": file_hash_hex,
                            "fileName": w.file_name,
                            "fileSize": w.file_size,
                            "sourceCount": w.source_count,
                        }));
                    }
                }
            }
            Err(e) => {
                tracing::debug!(
                    "Kad search_keyword to {} timed out or node does not index keyword: {}",
                    addr,
                    e
                );
            }
        }
    }

    tracing::info!(
        "Kad search_keywords for '{}' finished: found {} unique file results",
        term,
        discovered_entries.len()
    );

    Ok(discovered_entries)
}

/// Publishes keyword entries for a file to the closest Kad contacts.
pub async fn publish_keyword(
    file_hash_hex: &str,
    file_name: &str,
    file_size: u64,
    routing_table: &Mutex<KBucketTable>,
    udp: &Arc<KadUdpService>,
) -> Result<usize, String> {
    let file_hash_kad_id = KadId::from_hash_hex(file_hash_hex)
        .ok_or_else(|| format!("Invalid file hash hex: {file_hash_hex}"))?;

    let keywords = extract_kad_keywords(file_name);
    if keywords.is_empty() {
        return Ok(0);
    }

    let mut total_published = 0;
    for (_word, kw_kad_id) in keywords {
        let closest_contacts = {
            let table = routing_table.lock().await;
            table.find_closest(&kw_kad_id, 20)
        };

        for contact in closest_contacts {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            let entries = vec![mule_protocol::kad::KadKeywordEntryWire {
                file_hash: file_hash_kad_id.0,
                file_name: file_name.to_string(),
                file_size,
                source_count: 1,
            }];
            if let Ok(status) = udp.send_publish_keyword(addr, &kw_kad_id.0, &entries).await {
                if status == 0 {
                    total_published += 1;
                }
            }
        }
    }

    Ok(total_published)
}
