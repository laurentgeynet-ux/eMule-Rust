// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! UPnP types, status snapshot, and IP classification utilities.

use igd_next::PortMappingProtocol;
use mule_network_policy::validator::{classify_ip, IpCategory};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{SystemTime, UNIX_EPOCH};

/// Snapshot of UPnP status and active port mappings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct UpnpStatusSnapshot {
    /// Whether UPnP is enabled in settings.
    pub enabled: bool,
    /// Whether a UPnP gateway router was discovered.
    pub available: bool,
    /// Address of the gateway device if found.
    pub gateway_addr: Option<String>,
    /// External WAN IP reported by the gateway.
    pub external_ip: Option<String>,
    /// Whether the TCP port is currently mapped on the gateway.
    pub tcp_mapped: bool,
    /// Whether the UDP port is currently mapped on the gateway.
    pub udp_mapped: bool,
    /// TCP port requested / mapped.
    pub tcp_port: u16,
    /// UDP port requested / mapped.
    pub udp_port: u16,
    /// Whether the gateway is behind Carrier-Grade NAT (CGNAT / 100.64.x.x).
    pub cgnat_detected: bool,
    /// Last error message if any.
    pub last_error: Option<String>,
    /// Timestamp of last refresh (milliseconds since epoch).
    pub updated_at_unix_ms: u64,
}

/// Returns current unix milliseconds.
pub fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Checks if an address is a valid public routable WAN IP.
pub fn is_valid_wan_ip(ip: IpAddr) -> bool {
    classify_ip(ip) == IpCategory::PublicRoutable
}

/// Checks if an address falls in the CGNAT range (100.64.0.0/10, RFC 6598).
pub fn is_cgnat_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            octets[0] == 100 && (octets[1] & 0xC0) == 64
        }
        IpAddr::V6(_) => false,
    }
}

/// Checks if an address is a valid local network gateway address (anti-rogue-IGD check).
pub fn is_valid_gateway_addr(addr: SocketAddr) -> bool {
    let cat = classify_ip(addr.ip());
    cat == IpCategory::PrivateLan || cat == IpCategory::Loopback
}

/// Discovers the local IPv4 address used to reach the default gateway.
pub fn detect_primary_local_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

/// Discovers the local IPv4 address used to communicate with the given gateway.
pub fn detect_local_ipv4_for_gateway(gateway_addr: SocketAddr) -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(gateway_addr).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

/// Result of inspecting an existing IGD port mapping rule on the router.
#[derive(Debug, PartialEq, Eq)]
pub enum MappingOwnership {
    /// The existing rule on the gateway is identical and already maps to our exact internal endpoint and description.
    ExactMatch,
    /// The existing rule belongs to eMule-Rust on this machine, but targets an old local IP or port (stale lease to update).
    StaleLease,
    /// The rule belongs to another application or another host on the LAN. Must NOT be touched.
    ForeignConflict,
    /// No matching rule found for this external port / protocol.
    NotFound,
}

/// Ancien préfixe de description (« EMule-Rust », avant le renommage en
/// « eMule-Rust ») : les baux créés par les versions antérieures doivent
/// rester reconnus comme nous appartenant pour être mis à jour ou supprimés.
const LEGACY_DESCRIPTION_PREFIX: &str = "EMule-Rust";

/// Inspects whether a specific port mapping entry matches our expected criteria.
pub fn inspect_mapping_entry(
    entry_protocol: PortMappingProtocol,
    entry_external_port: u16,
    entry_internal_client: &str,
    entry_internal_port: u16,
    entry_description: &str,
    target_protocol: PortMappingProtocol,
    target_external_port: u16,
    expected_local: SocketAddrV4,
    description_prefix: &str,
) -> Option<MappingOwnership> {
    if entry_protocol != target_protocol || entry_external_port != target_external_port {
        return None;
    }

    let expected_client_ip = expected_local.ip().to_string();
    let expected_internal_port = expected_local.port();
    let is_emule_desc = entry_description.starts_with(description_prefix)
        || entry_description.starts_with(LEGACY_DESCRIPTION_PREFIX);

    if is_emule_desc {
        if entry_internal_client == expected_client_ip
            && entry_internal_port == expected_internal_port
        {
            Some(MappingOwnership::ExactMatch)
        } else {
            Some(MappingOwnership::StaleLease)
        }
    } else {
        Some(MappingOwnership::ForeignConflict)
    }
}
