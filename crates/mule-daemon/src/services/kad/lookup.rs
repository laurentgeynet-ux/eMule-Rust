// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Iterative Kad `find_node` convergence shared by search and publish paths.
//!
//! eMule only honors a publish/search request when the target key lies within
//! `SEARCHTOLERANCE` of the answering node's KadID — and drops the packet
//! silently otherwise. Publishing straight from the local routing table
//! therefore fails ~always when the table is sparse or stale: the "closest"
//! local contacts are usually still out of tolerance, and dead contacts
//! cannot answer at all. Converging with `find_node` first yields live,
//! actually-close contacts.

use std::collections::{BTreeMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;

use mule_core::kad::{KBucketTable, KadContact, KadId};
use mule_filter::IpFilter;
use mule_network_policy::{classify_ip, IpCategory, NetworkPolicyManager, NetworkProfile};

use crate::services::kad_udp::KadUdpService;

/// Iteratively queries `find_node` toward `target` starting from the closest
/// known contacts, until no closer unqueried contact remains or the early
/// convergence criterion (eMule `SEARCHTOLERANCE`) is met.
///
/// Returns the converged candidate pool sorted by XOR distance to `target`
/// (closest first). Responsive contacts are marked seen/verified in the
/// routing table; unresponsive ones accumulate failures and may be evicted.
pub async fn converge_closest_contacts(
    target: &KadId,
    routing_table: &Mutex<KBucketTable>,
    policy_mgr: &NetworkPolicyManager,
    ip_filter: &Arc<IpFilter>,
    udp: &Arc<KadUdpService>,
    max_rounds: usize,
) -> Vec<KadContact> {
    let initial_closest = {
        let table = routing_table.lock().await;
        table.find_closest_verified(target, 32)
    };

    let mut candidates: BTreeMap<KadId, KadContact> = BTreeMap::new();
    for c in initial_closest {
        let dist = c.kad_id.xor_distance(target);
        candidates.insert(dist, c);
    }

    let mut queried_nodes = HashSet::new();
    const ALPHA: usize = 5;

    for iter in 0..max_rounds {
        let unqueried: Vec<_> = candidates
            .values()
            .filter(|c| !queried_nodes.contains(&SocketAddr::new(IpAddr::V4(c.ip), c.udp_port)))
            .take(ALPHA)
            .cloned()
            .collect();

        if unqueried.is_empty() {
            break;
        }

        // Early stop once enough close candidates sit inside SEARCHTOLERANCE.
        let within_tolerance_count = candidates
            .values()
            .take(16)
            .filter(|c| c.kad_id.is_within_search_tolerance(target))
            .count();
        if within_tolerance_count >= 12 && iter >= 2 {
            break;
        }

        let mut find_futs = Vec::new();
        for contact in &unqueried {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            let receiver_id = contact.kad_id;
            queried_nodes.insert(addr);
            let udp_c = Arc::clone(udp);
            let target_id = *target;
            find_futs.push(async move {
                match tokio::time::timeout(
                    Duration::from_millis(2500),
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
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        {
            let mut table = routing_table.lock().await;
            for (contact, res) in unqueried.iter().zip(find_results.iter()) {
                if res.is_ok() {
                    table.mark_seen(&contact.kad_id, now_ms);
                } else if table.mark_failed(&contact.kad_id) {
                    tracing::debug!(
                        "Kad: contact {}:{} evicted after repeated unresponsive queries",
                        contact.ip,
                        contact.udp_port
                    );
                }
            }
        }

        let mut new_contacts = Vec::new();
        let profile = policy_mgr.profile();
        for contacts in find_results.into_iter().flatten() {
            for c in contacts {
                let cat = classify_ip(IpAddr::V4(c.ip));
                let policy_allowed = match profile {
                    NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                    NetworkProfile::PublicEd2kPreview
                    | NetworkProfile::PublicEd2kKadPreview => cat == IpCategory::PublicRoutable,
                };
                if policy_allowed && ip_filter.check_ip(IpAddr::V4(c.ip)).is_none() {
                    // eMule semantics: a contact learned from a routing
                    // response is *not* IP-verified — verification requires
                    // it answering one of our requests (mark_seen) or the
                    // HELLO_RES_ACK handshake. Keep it unverified; it may
                    // still be queried and promoted on response.
                    new_contacts.push(c);
                }
            }
        }

        if !new_contacts.is_empty() {
            let mut table = routing_table.lock().await;
            for c in &new_contacts {
                table.insert_or_update(c.clone());
                let dist = c.kad_id.xor_distance(target);
                candidates.insert(dist, c.clone());
            }
        }
    }

    candidates.into_values().collect()
}
