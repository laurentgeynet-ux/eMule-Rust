// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Gateway discovery and SSDP searching logic.

use igd_next::{search_gateway, Gateway, SearchOptions};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tracing::debug;

use super::types::detect_primary_local_ipv4;

/// Discovers the UPnP gateway, trying first the primary default-route interface,
/// then falling back to wildcard `0.0.0.0:0` broadcast.
pub fn discover_gateway_with_fallback() -> Result<Gateway, String> {
    let primary_bind = detect_primary_local_ipv4()
        .map(|v4| SocketAddr::V4(SocketAddrV4::new(v4, 0)))
        .unwrap_or_else(|| SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)));

    let primary_options = SearchOptions {
        bind_addr: primary_bind,
        timeout: Some(Duration::from_secs(4)),
        ..Default::default()
    };

    match search_gateway(primary_options) {
        Ok(gw) => Ok(gw),
        Err(primary_err) => {
            debug!("Primary SSDP discovery on {primary_bind} failed: {primary_err}; trying fallback 0.0.0.0");
            let fallback_options = SearchOptions {
                bind_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
                timeout: Some(Duration::from_secs(4)),
                ..Default::default()
            };
            search_gateway(fallback_options).map_err(|fallback_err| {
                format!("UPnP discovery failed (primary: {primary_err}, fallback: {fallback_err})")
            })
        }
    }
}
