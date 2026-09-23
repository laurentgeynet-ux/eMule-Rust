// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use mule_core::kad::KadContact;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KadStatusSnapshot {
    pub local_id: String,
    pub total_contacts: usize,
    pub total_replacements: usize,
    pub db_contacts_count: usize,
    pub active_buckets: usize,
    pub network_profile: String,
    pub is_udp_active: bool,
    pub udp_bind_address: Option<String>,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub active_transactions: usize,
    /// UDP firewall reachability: "unverified" | "testing" | "open" | "firewalled".
    pub udp_firewall_status: String,
    /// True once a KADEMLIA2_FIREWALLUDP check produced a conclusive result.
    pub udp_firewall_verified: bool,
    /// NAT-mapped extern Kad UDP port learned from a probe, if any.
    pub extern_udp_port: Option<u16>,
    /// UDP port peers should use to reach us on Kad (intern or extern).
    pub effective_udp_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodesDatInspectReport {
    pub version: u32,
    pub declared_count: usize,
    pub valid_contacts_count: usize,
    pub rejected_by_policy_count: usize,
    pub rejected_by_filter_count: usize,
    pub sample_contacts: Vec<KadContactEntryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KadContactEntryDto {
    pub kad_id: String,
    pub ip: String,
    pub udp_port: u16,
    pub tcp_port: Option<u16>,
    pub version: u8,
    pub verified: bool,
    pub fail_count: u32,
    pub last_seen_unix_ms: u64,
}

impl From<&KadContact> for KadContactEntryDto {
    fn from(c: &KadContact) -> Self {
        Self {
            kad_id: c.kad_id.to_hex(),
            ip: c.ip.to_string(),
            udp_port: c.udp_port,
            tcp_port: c.tcp_port,
            version: c.version,
            verified: c.verified,
            fail_count: c.fail_count,
            last_seen_unix_ms: c.last_seen_unix_ms,
        }
    }
}
