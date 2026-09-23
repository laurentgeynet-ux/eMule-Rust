// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use mule_core::kad::{KadContact, KadId};
use mule_filter::IpFilter;
use mule_format::kad::{NodesDatFile, MAX_NODES_DAT_SIZE};
use mule_network_policy::{classify_ip, IpCategory, NetworkPolicyManager, NetworkProfile};
use std::net::IpAddr;

use super::types::{KadContactEntryDto, NodesDatInspectReport};

pub fn inspect_nodes_dat(
    raw_bytes: &[u8],
    policy_mgr: &NetworkPolicyManager,
    ip_filter: &IpFilter,
) -> Result<NodesDatInspectReport, String> {
    if raw_bytes.len() > MAX_NODES_DAT_SIZE {
        return Err(format!(
            "File size {} exceeds maximum allowed {}",
            raw_bytes.len(),
            MAX_NODES_DAT_SIZE
        ));
    }

    let nodes_file = NodesDatFile::parse(std::io::Cursor::new(raw_bytes))
        .map_err(|e| format!("Failed to parse nodes.dat: {e}"))?;

    let profile = policy_mgr.profile();
    let mut valid_count = 0;
    let mut policy_rejected = 0;
    let mut filter_rejected = 0;
    let mut sample_contacts = Vec::new();

    for entry in &nodes_file.contacts {
        let ip = IpAddr::V4(entry.ip);

        // Policy check
        let category = classify_ip(ip);
        let policy_allowed = match profile {
            NetworkProfile::LocalLab => category == IpCategory::Loopback,
            NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                category == IpCategory::PublicRoutable
            }
        };

        if !policy_allowed {
            policy_rejected += 1;
            continue;
        }

        // Filter check
        if ip_filter.check_ip(ip).is_some() {
            filter_rejected += 1;
            continue;
        }

        valid_count += 1;
        if sample_contacts.len() < 20 {
            let tcp_port = if entry.tcp_port != 0 {
                Some(entry.tcp_port)
            } else {
                None
            };
            sample_contacts.push(KadContactEntryDto {
                kad_id: KadId(entry.client_id).to_hex(),
                ip: entry.ip.to_string(),
                udp_port: entry.udp_port,
                tcp_port,
                version: entry.version,
                verified: entry.verified,
                fail_count: 0,
                last_seen_unix_ms: 0,
            });
        }
    }

    Ok(NodesDatInspectReport {
        version: nodes_file.version,
        declared_count: nodes_file.contacts.len(),
        valid_contacts_count: valid_count,
        rejected_by_policy_count: policy_rejected,
        rejected_by_filter_count: filter_rejected,
        sample_contacts,
    })
}

pub fn parse_valid_nodes_contacts(
    raw_bytes: &[u8],
    policy_mgr: &NetworkPolicyManager,
    ip_filter: &IpFilter,
) -> Result<Vec<KadContact>, String> {
    let _ = inspect_nodes_dat(raw_bytes, policy_mgr, ip_filter)?;
    let nodes_file = NodesDatFile::parse(std::io::Cursor::new(raw_bytes))
        .map_err(|e| format!("Failed to parse nodes.dat: {e}"))?;

    let profile = policy_mgr.profile();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut imported_contacts = Vec::new();

    for entry in nodes_file.contacts {
        let ip = IpAddr::V4(entry.ip);
        let category = classify_ip(ip);
        let policy_allowed = match profile {
            NetworkProfile::LocalLab => category == IpCategory::Loopback,
            NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                category == IpCategory::PublicRoutable
            }
        };

        if !policy_allowed || ip_filter.check_ip(ip).is_some() {
            continue;
        }

        let tcp_port = if entry.tcp_port != 0 {
            Some(entry.tcp_port)
        } else {
            None
        };
        let contact = KadContact {
            kad_id: KadId(entry.client_id),
            ip: entry.ip,
            udp_port: entry.udp_port,
            tcp_port,
            version: entry.version,
            verified: entry.verified,
            fail_count: 0,
            first_seen_unix_ms: now_ms,
            last_seen_unix_ms: now_ms,
        };
        imported_contacts.push(contact);
    }

    Ok(imported_contacts)
}
