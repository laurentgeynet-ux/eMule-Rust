// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

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

    {
        let table = routing_table.lock().await;
        if table.find_closest(&keyword_kad_id, 1).is_empty() {
            tracing::warn!(
                "Kad search_keywords for '{}' (norm='{}'): routing table is empty (0 contacts)",
                term,
                norm_word
            );
            return Ok(Vec::new());
        }
    }

    tracing::info!(
        "Kad search_keywords: starting search for '{}' (hash={}): performing iterative find_node convergence...",
        term,
        keyword_kad_id.to_hex()
    );

    const MAX_CONVERGENCE_ROUNDS: usize = 6;
    let candidates = super::lookup::converge_closest_contacts(
        &keyword_kad_id,
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
///
/// eMule nodes silently drop a `PUBLISH_KEY_REQ` whose keyword hash lies
/// outside `SEARCHTOLERANCE` of their KadID, and only live nodes answer at
/// all — so the publish targets are first converged with an iterative
/// `find_node` lookup, then restricted to in-tolerance contacts.
pub async fn publish_keyword(
    file_hash_hex: &str,
    file_name: &str,
    file_size: u64,
    routing_table: &Mutex<KBucketTable>,
    policy_mgr: &NetworkPolicyManager,
    ip_filter: &Arc<IpFilter>,
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
        let candidates = super::lookup::converge_closest_contacts(
            &kw_kad_id,
            routing_table,
            policy_mgr,
            ip_filter,
            udp,
            4,
        )
        .await;

        // Only nodes within SEARCHTOLERANCE will index the keyword and reply;
        // fall back to the closest converged contacts when none qualify.
        let mut targets: Vec<KadContact> = candidates
            .iter()
            .filter(|c| c.kad_id.is_within_search_tolerance(&kw_kad_id))
            .take(10)
            .cloned()
            .collect();
        if targets.is_empty() {
            targets = candidates.into_iter().take(5).collect();
        }

        let entries = vec![mule_protocol::kad::KadKeywordEntryWire {
            file_hash: file_hash_kad_id.0,
            file_name: file_name.to_string(),
            file_size,
            source_count: 1,
        }];

        let mut futs = Vec::with_capacity(targets.len());
        for contact in targets {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            let udp_c = Arc::clone(udp);
            let kw = kw_kad_id.0;
            let entries_c = entries.clone();
            futs.push(async move { udp_c.send_publish_keyword(addr, &kw, &entries_c).await });
        }
        for res in futures_util::future::join_all(futs).await {
            if let Ok(0) = res {
                total_published += 1;
            }
        }
    }

    Ok(total_published)
}
