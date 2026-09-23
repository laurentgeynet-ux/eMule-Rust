// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! UPnP port mapping and unmapping lease operations.

use igd_next::{AddPortError, Gateway, PortMappingProtocol};
use std::net::SocketAddrV4;
use tracing::{debug, info, warn};

use super::types::{inspect_mapping_entry, MappingOwnership};

const MAX_MAPPING_ENTRIES_TO_INSPECT: u32 = 100;

/// Inspects the gateway's generic port mapping table to assess ownership of an existing rule.
pub fn check_existing_mapping(
    gateway: &Gateway,
    protocol: PortMappingProtocol,
    external_port: u16,
    expected_local: SocketAddrV4,
    description_prefix: &str,
) -> MappingOwnership {
    for index in 0..MAX_MAPPING_ENTRIES_TO_INSPECT {
        match gateway.get_generic_port_mapping_entry(index) {
            Ok(entry) => {
                if let Some(ownership) = inspect_mapping_entry(
                    entry.protocol,
                    entry.external_port,
                    &entry.internal_client,
                    entry.internal_port,
                    &entry.port_mapping_description,
                    protocol,
                    external_port,
                    expected_local,
                    description_prefix,
                ) {
                    return ownership;
                }
            }
            Err(igd_next::GetGenericPortMappingEntryError::SpecifiedArrayIndexInvalid) => {
                break;
            }
            Err(e) => {
                debug!("IGD port mapping table inspection error at index {index}: {e}");
                break;
            }
        }
    }
    MappingOwnership::NotFound
}

/// Attempts to map a port safely and idempotently.
pub fn add_port_safe(
    gateway: &Gateway,
    protocol: PortMappingProtocol,
    port: u16,
    local_addr: SocketAddrV4,
    lease_duration: u32,
    description: &str,
) -> bool {
    let proto_name = match protocol {
        PortMappingProtocol::TCP => "TCP",
        PortMappingProtocol::UDP => "UDP",
    };

    match gateway.add_port(
        protocol,
        port,
        std::net::SocketAddr::V4(local_addr),
        lease_duration,
        description,
    ) {
        Ok(_) => {
            info!("UPnP mapped {proto_name} port {port} -> {local_addr}");
            true
        }
        Err(AddPortError::PortInUse) => {
            match check_existing_mapping(gateway, protocol, port, local_addr, "EMule-Rust") {
                MappingOwnership::ExactMatch => {
                    info!("UPnP {proto_name} port {port} already correctly mapped to {local_addr} (idempotent keep)");
                    true
                }
                MappingOwnership::StaleLease => {
                    info!("Removing our stale UPnP lease for {proto_name} port {port} and updating mapping");
                    let _ = gateway.remove_port(protocol, port);
                    match gateway.add_port(
                        protocol,
                        port,
                        std::net::SocketAddr::V4(local_addr),
                        lease_duration,
                        description,
                    ) {
                        Ok(_) => {
                            info!(
                                "UPnP re-mapped {proto_name} port {port} after stale lease cleanup"
                            );
                            true
                        }
                        Err(retry_err) => {
                            warn!("UPnP retry failed for {proto_name} port {port}: {retry_err}");
                            false
                        }
                    }
                }
                MappingOwnership::ForeignConflict => {
                    warn!(
                        "UPnP {proto_name} port {port} is already in use by another application/host on the router. Refusing to overwrite."
                    );
                    false
                }
                MappingOwnership::NotFound => {
                    warn!(
                        "UPnP gateway reported 718 PortInUse for {proto_name} port {port}, but no entry was found in table. Refusing destructive removal."
                    );
                    false
                }
            }
        }
        Err(e) => {
            warn!("UPnP failed to map {proto_name} port {port}: {e}");
            false
        }
    }
}
