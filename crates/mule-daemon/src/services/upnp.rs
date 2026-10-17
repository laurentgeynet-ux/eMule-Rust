// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! UPnP IGD (Internet Gateway Device) Service.
//!
//! Automatically discovers UPnP-enabled gateway routers/boxes on the local network,
//! resolves external WAN IP, and manages port mapping leases for eD2K TCP and Kad UDP.

pub mod discovery;
pub mod mapping;
pub mod types;

use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::sync::Arc;
use std::time::Duration;

use igd_next::{Gateway, PortMappingProtocol};
use tokio::sync::RwLock;
use tracing::{info, warn};

use discovery::discover_gateway_with_fallback;
use mapping::{add_port_safe, check_existing_mapping};
pub use types::{
    current_unix_ms, detect_local_ipv4_for_gateway, detect_primary_local_ipv4,
    inspect_mapping_entry, is_cgnat_ip, is_valid_gateway_addr, is_valid_wan_ip, MappingOwnership,
    UpnpStatusSnapshot,
};

/// Service managing UPnP IGD lifecycle.
#[derive(Debug, Clone)]
pub struct UpnpService {
    snapshot: Arc<RwLock<UpnpStatusSnapshot>>,
    cached_gateway: Arc<RwLock<Option<Gateway>>>,
}

impl UpnpService {
    /// Creates a new UPnP service.
    pub fn new(enabled: bool, tcp_port: u16, udp_port: u16) -> Self {
        let snap = UpnpStatusSnapshot {
            enabled,
            tcp_port,
            udp_port,
            ..Default::default()
        };
        Self {
            snapshot: Arc::new(RwLock::new(snap)),
            cached_gateway: Arc::new(RwLock::new(None)),
        }
    }

    /// Returns a current status snapshot.
    pub async fn snapshot(&self) -> UpnpStatusSnapshot {
        self.snapshot.read().await.clone()
    }

    /// Enables or disables UPnP.
    /// Updates the mapped TCP/UDP ports (eMule client ports changed at
    /// runtime) so refreshes/remaps use the current values.
    pub async fn set_ports(&self, tcp_port: u16, udp_port: u16) {
        let mut snap = self.snapshot.write().await;
        snap.tcp_port = tcp_port;
        snap.udp_port = udp_port;
    }

    pub async fn set_enabled(&self, enabled: bool) {
        let mut snap = self.snapshot.write().await;
        snap.enabled = enabled;
        snap.updated_at_unix_ms = current_unix_ms();
    }

    /// Sets a manual external IP override (fallback when UPnP is unavailable).
    pub async fn set_manual_external_ip(&self, ip: Option<String>) {
        let mut snap = self.snapshot.write().await;
        snap.external_ip = ip;
        snap.updated_at_unix_ms = current_unix_ms();
    }

    /// Spawns a background task that periodically refreshes UPnP port mappings before leases expire.
    pub fn start_periodic_refresh(
        self: Arc<Self>,
        interval: Duration,
        cancel_token: mule_network_policy::CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.tick().await;
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("UPnP periodic refresh loop cancelled");
                        break;
                    }
                    _ = ticker.tick() => {
                        let (enabled, tcp_port, udp_port) = {
                            let snap = self.snapshot.read().await;
                            (snap.enabled, snap.tcp_port, snap.udp_port)
                        };
                        if enabled {
                            info!("Renewing UPnP port mapping lease (TCP {tcp_port}, UDP {udp_port})");
                            let _ = self.map_ports(tcp_port, udp_port).await;
                        }
                    }
                }
            }
        })
    }

    /// Discovers gateway and maps the ports.
    pub async fn map_ports(
        &self,
        tcp_port: u16,
        udp_port: u16,
    ) -> Result<UpnpStatusSnapshot, String> {
        {
            let mut snap = self.snapshot.write().await;
            snap.tcp_port = tcp_port;
            snap.udp_port = udp_port;
            snap.last_error = None;
            snap.updated_at_unix_ms = current_unix_ms();
        }

        const MAX_ATTEMPTS: u32 = 3;
        let mut last_err = String::new();

        for attempt in 0..MAX_ATTEMPTS {
            if attempt > 0 {
                let delay_secs = 2u64 << (attempt - 1);
                warn!(
                    "UPnP: attempt {}/{MAX_ATTEMPTS} failed, retrying in {delay_secs}s (cause: {last_err})",
                    attempt
                );
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            }

            let cached_gw = self.cached_gateway.read().await.clone();

            let join_handle = tokio::task::spawn_blocking(
                move || -> Result<(Gateway, IpAddr, Ipv4Addr, bool, bool, bool), String> {
                    let gateway = match cached_gw {
                        Some(gw) => gw,
                        None => discover_gateway_with_fallback()?,
                    };

                    if !is_valid_gateway_addr(gateway.addr) {
                        return Err(format!(
                            "Discovered UPnP gateway has invalid non-LAN address: {}",
                            gateway.addr
                        ));
                    }

                    let ext_ip = gateway
                        .get_external_ip()
                        .map_err(|e| format!("Failed to retrieve external WAN IP: {e}"))?;

                    let cgnat = is_cgnat_ip(ext_ip);

                    if !is_valid_wan_ip(ext_ip) && !cgnat {
                        return Err(format!(
                            "Gateway reported invalid non-public WAN IP: {ext_ip}"
                        ));
                    }

                    if cgnat {
                        warn!(
                            "UPnP: CGNAT detected — WAN IP {ext_ip} is in the RFC 6598 shared address space. eD2K/Kad internet connectivity may be limited."
                        );
                    }

                    let local_ip =
                        detect_local_ipv4_for_gateway(gateway.addr).ok_or_else(|| {
                            "Failed to detect local IPv4 address for gateway".to_string()
                        })?;

                    let local_tcp_addr = SocketAddrV4::new(local_ip, tcp_port);
                    let local_udp_addr = SocketAddrV4::new(local_ip, udp_port);

                    let tcp_ok = add_port_safe(
                        &gateway,
                        PortMappingProtocol::TCP,
                        tcp_port,
                        local_tcp_addr,
                        3600,
                        "eMuleRust eD2K TCP",
                    );

                    let udp_ok = add_port_safe(
                        &gateway,
                        PortMappingProtocol::UDP,
                        udp_port,
                        local_udp_addr,
                        3600,
                        "eMuleRust Kad UDP",
                    );

                    if !tcp_ok && !udp_ok {
                        return Err(format!(
                            "Failed to map both TCP {tcp_port} and UDP {udp_port} ports on gateway"
                        ));
                    }

                    Ok((gateway, ext_ip, local_ip, tcp_ok, udp_ok, cgnat))
                },
            );

            let res = match tokio::time::timeout(Duration::from_secs(15), join_handle).await {
                Ok(Ok(inner_res)) => inner_res,
                Ok(Err(join_err)) => Err(format!("UPnP task join error: {join_err}")),
                Err(_) => Err("UPnP gateway discovery timed out after 15 seconds".to_string()),
            };

            match res {
                Ok((gw, ext_ip, _local_ip, tcp_ok, udp_ok, cgnat)) => {
                    let mut snap = self.snapshot.write().await;
                    snap.available = true;
                    snap.gateway_addr = Some(gw.addr.to_string());
                    snap.external_ip = Some(ext_ip.to_string());
                    snap.tcp_mapped = tcp_ok;
                    snap.udp_mapped = udp_ok;
                    snap.cgnat_detected = cgnat;
                    snap.last_error = if cgnat {
                        Some(
                            "CGNAT détecté : votre FAI utilise un NAT partagé (100.64.x.x). Les ports sont mappés localement mais la connectivité eD2K/Kad depuis internet peut être limitée."
                                .to_string(),
                        )
                    } else if !tcp_ok || !udp_ok {
                        Some("Partial UPnP port mapping: some ports failed to map".to_string())
                    } else {
                        None
                    };
                    snap.updated_at_unix_ms = current_unix_ms();
                    *self.cached_gateway.write().await = Some(gw);
                    info!(
                        "UPnP setup succeeded on attempt {}/{MAX_ATTEMPTS}: Gateway {}, External IP {}, TCP: {}, UDP: {}, CGNAT: {}",
                        attempt + 1,
                        snap.gateway_addr.as_deref().unwrap_or(""),
                        ext_ip,
                        tcp_ok,
                        udp_ok,
                        cgnat,
                    );
                    return Ok(snap.clone());
                }
                Err(e) => {
                    last_err = e;
                    *self.cached_gateway.write().await = None;
                }
            }
        }

        let mut snap = self.snapshot.write().await;
        snap.available = false;
        snap.cgnat_detected = false;
        snap.tcp_mapped = false;
        snap.udp_mapped = false;
        snap.last_error = Some(format!("{last_err} (après {MAX_ATTEMPTS} tentatives)"));
        snap.updated_at_unix_ms = current_unix_ms();
        warn!("UPnP setup failed after {MAX_ATTEMPTS} attempts: {last_err}");
        Err(last_err)
    }

    /// Removes previously mapped ports from the gateway if verified to still belong to eMuleRust.
    pub async fn unmap_ports(&self) -> Result<(), String> {
        let (gw_opt, tcp_port, udp_port, tcp_mapped, udp_mapped) = {
            let snap = self.snapshot.read().await;
            let gw = self.cached_gateway.read().await.clone();
            (
                gw,
                snap.tcp_port,
                snap.udp_port,
                snap.tcp_mapped,
                snap.udp_mapped,
            )
        };

        if !tcp_mapped && !udp_mapped {
            return Ok(());
        }

        if let Some(gateway) = gw_opt {
            tokio::task::spawn_blocking(move || {
                unmap_gateway_blocking(&gateway, tcp_port, udp_port, tcp_mapped, udp_mapped)
            })
            .await
            .map_err(|e| format!("UPnP unmap task join error: {e}"))?;
        }

        let mut snap = self.snapshot.write().await;
        snap.tcp_mapped = false;
        snap.udp_mapped = false;
        snap.updated_at_unix_ms = current_unix_ms();
        Ok(())
    }

    /// Best-effort unmap for process shutdown, hard-bounded at 500 ms.
    ///
    /// The gateway SOAP calls run on a detached `std::thread` — never on
    /// `spawn_blocking`: the Tokio runtime waits for all blocking-pool
    /// threads at shutdown, so a `spawn_blocking` still executing a slow
    /// gateway call would keep the process alive past exit (making the old
    /// outer `timeout()` ineffective). A detached thread is dropped with
    /// the process, and the oneshot channel lets us await its completion
    /// without occupying a blocking-pool slot.
    pub async fn unmap_ports_for_shutdown(&self) {
        const SHUTDOWN_UNMAP_BUDGET: Duration = Duration::from_millis(500);

        let (gw_opt, tcp_port, udp_port, tcp_mapped, udp_mapped) = {
            let snap = self.snapshot.read().await;
            let gw = self.cached_gateway.read().await.clone();
            (
                gw,
                snap.tcp_port,
                snap.udp_port,
                snap.tcp_mapped,
                snap.udp_mapped,
            )
        };

        if !tcp_mapped && !udp_mapped {
            return;
        }

        if let Some(gateway) = gw_opt {
            let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
            std::thread::spawn(move || {
                unmap_gateway_blocking(&gateway, tcp_port, udp_port, tcp_mapped, udp_mapped);
                let _ = done_tx.send(());
            });
            let _ = tokio::time::timeout(SHUTDOWN_UNMAP_BUDGET, done_rx).await;
        }
    }
}

/// Blocking SOAP ownership-check + removal for both unmap paths.
/// Runs on a blocking thread (`spawn_blocking` or detached `std::thread`) —
/// never on an async executor thread.
fn unmap_gateway_blocking(
    gateway: &Gateway,
    tcp_port: u16,
    udp_port: u16,
    tcp_mapped: bool,
    udp_mapped: bool,
) {
    let local_ip_opt = detect_local_ipv4_for_gateway(gateway.addr);

    for (proto, port, mapped) in [
        (PortMappingProtocol::TCP, tcp_port, tcp_mapped),
        (PortMappingProtocol::UDP, udp_port, udp_mapped),
    ] {
        if !mapped {
            continue;
        }

        let should_remove = match local_ip_opt {
            Some(lip) => match check_existing_mapping(
                gateway,
                proto,
                port,
                SocketAddrV4::new(lip, port),
                "eMuleRust",
            ) {
                MappingOwnership::ExactMatch => true,
                MappingOwnership::StaleLease => {
                    warn!("Skipped unmapping {proto:?} port {port}: entry is an old/stale lease from another IP or port; letting it expire naturally");
                    false
                }
                MappingOwnership::ForeignConflict => {
                    warn!("Skipped unmapping {proto:?} port {port}: entry belongs to another application/host");
                    false
                }
                MappingOwnership::NotFound => false,
            },
            None => {
                warn!("Skipped unmapping {proto:?} port {port}: unable to determine local endpoint for ownership verification");
                false
            }
        };

        if should_remove {
            let _ = gateway.remove_port(proto, port);
            info!("UPnP unmapped {proto:?} port {port}");
        }
    }
}
