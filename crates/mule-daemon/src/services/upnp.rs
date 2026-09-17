//! UPnP IGD (Internet Gateway Device) Service.
//!
//! Automatically discovers UPnP-enabled gateway routers/boxes on the local network,
//! resolves external WAN IP, and manages port mapping leases for eD2K TCP and Kad UDP.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use igd_next::{search_gateway, AddPortError, Gateway, PortMappingProtocol, SearchOptions};
use mule_network_policy::validator::{classify_ip, IpCategory};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Snapshot of UPnP status and active port mappings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
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
    /// The gateway is reachable and ports are mapped, but the WAN IP is not
    /// publicly routable — internet connectivity via eD2K/Kad may be limited.
    pub cgnat_detected: bool,
    /// Last error message if any.
    pub last_error: Option<String>,
    /// Timestamp of last refresh (milliseconds since epoch).
    pub updated_at_unix_ms: u64,
}

impl Default for UpnpStatusSnapshot {
    fn default() -> Self {
        Self {
            enabled: false,
            available: false,
            gateway_addr: None,
            external_ip: None,
            tcp_mapped: false,
            udp_mapped: false,
            tcp_port: 0,
            udp_port: 0,
            cgnat_detected: false,
            last_error: None,
            updated_at_unix_ms: 0,
        }
    }
}

fn current_unix_ms() -> u64 {
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
/// CGNAT means the ISP uses Carrier-Grade NAT: the gateway IS reachable and
/// UPnP port mapping works on the LAN, but internet-facing connectivity is
/// limited since the "external" IP is not publicly routable.
pub fn is_cgnat_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            // 100.64.0.0/10 — RFC 6598 shared address space (CGNAT)
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
/// `connect()` on a UDP socket does NOT send any packet over the network,
/// but queries the OS kernel routing table to select the primary outgoing network interface.
fn detect_primary_local_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:53").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

/// Discovers the local IPv4 address used to communicate with the given gateway.
fn detect_local_ipv4_for_gateway(gateway_addr: SocketAddr) -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(gateway_addr).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
        _ => None,
    }
}

const MAX_MAPPING_ENTRIES_TO_INSPECT: u32 = 100;

/// Result of inspecting an existing IGD port mapping rule on the router.
#[derive(Debug, PartialEq, Eq)]
pub enum MappingOwnership {
    /// The existing rule on the gateway is identical and already maps to our exact internal endpoint and description.
    ExactMatch,
    /// The existing rule belongs to Emule-Sion on this machine, but targets an old local IP or port (stale lease to update).
    StaleLease,
    /// The rule belongs to another application or another host on the LAN. Must NOT be touched.
    ForeignConflict,
    /// No matching rule found for this external port / protocol.
    NotFound,
}

/// Inspects whether a specific port mapping entry matches our expected criteria.
/// Pure function convenient for unit testing without live network fixtures.
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
    let is_emule_desc = entry_description.starts_with(description_prefix);

    if is_emule_desc {
        if entry_internal_client == expected_client_ip && entry_internal_port == expected_internal_port {
            Some(MappingOwnership::ExactMatch)
        } else {
            Some(MappingOwnership::StaleLease)
        }
    } else {
        Some(MappingOwnership::ForeignConflict)
    }
}

/// Inspects the gateway's generic port mapping table to assess ownership of an existing rule.
fn check_existing_mapping(
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
                // Normal end of IGD table
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

/// Discovers the UPnP gateway, trying first the primary default-route interface,
/// then falling back to wildcard `0.0.0.0:0` broadcast.
fn discover_gateway_with_fallback() -> Result<Gateway, String> {
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
            search_gateway(fallback_options)
                .map_err(|fallback_err| format!("UPnP discovery failed (primary: {primary_err}, fallback: {fallback_err})"))
        }
    }
}

/// Attempts to map a port safely and idempotently.
/// If an identical Emule-Sion rule already exists, returns true without disturbing active connections.
fn add_port_safe(
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
        SocketAddr::V4(local_addr),
        lease_duration,
        description,
    ) {
        Ok(_) => {
            info!("UPnP mapped {proto_name} port {port} -> {local_addr}");
            true
        }
        Err(AddPortError::PortInUse) => {
            // IGD error 718 (ConflictInMappingEntry). Verify ownership before modifying.
            match check_existing_mapping(gateway, protocol, port, local_addr, "Emule-Sion") {
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
                        SocketAddr::V4(local_addr),
                        lease_duration,
                        description,
                    ) {
                        Ok(_) => {
                            info!("UPnP re-mapped {proto_name} port {port} after stale lease cleanup");
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
            ticker.tick().await; // skip initial immediate tick
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
    ///
    /// Retries up to [`MAX_UPNP_ATTEMPTS`] times with exponential backoff before giving up,
    /// mimicking libtorrent's `resend_request` strategy. This makes discovery resilient to
    /// SSDP packet loss and to box firmwares that respond slowly on the first try.
    pub async fn map_ports(&self, tcp_port: u16, udp_port: u16) -> Result<UpnpStatusSnapshot, String> {
        {
            let mut snap = self.snapshot.write().await;
            snap.tcp_port = tcp_port;
            snap.udp_port = udp_port;
            snap.last_error = None;
            snap.updated_at_unix_ms = current_unix_ms();
        }

        /// Maximum number of discovery+mapping attempts before reporting failure.
        const MAX_ATTEMPTS: u32 = 3;

        let mut last_err = String::new();

        for attempt in 0..MAX_ATTEMPTS {
            // Exponential backoff between retries: 2 s after attempt 1, 4 s after attempt 2.
            // No delay before the first attempt (attempt == 0).
            if attempt > 0 {
                let delay_secs = 2u64 << (attempt - 1); // 2, 4, 8 …
                warn!(
                    "UPnP: attempt {}/{MAX_ATTEMPTS} failed, retrying in {delay_secs}s \
                     (cause: {last_err})",
                    attempt
                );
                tokio::time::sleep(Duration::from_secs(delay_secs)).await;
            }

            let cached_gw = self.cached_gateway.read().await.clone();

            // Run UPnP discovery / port mapping in blocking thread pool with a 15 s hard timeout.
            let join_handle = tokio::task::spawn_blocking(
                move || -> Result<(Gateway, IpAddr, Ipv4Addr, bool, bool, bool), String> {
                    let gateway = match cached_gw {
                        Some(gw) => gw,
                        None => discover_gateway_with_fallback()?,
                    };

                    // Anti-Rogue-IGD: gateway must be on private LAN or loopback.
                    if !is_valid_gateway_addr(gateway.addr) {
                        return Err(format!(
                            "Discovered UPnP gateway has invalid non-LAN address: {}",
                            gateway.addr
                        ));
                    }

                    let ext_ip = gateway
                        .get_external_ip()
                        .map_err(|e| format!("Failed to retrieve external WAN IP: {e}"))?;

                    // Detect CGNAT (RFC 6598 — 100.64.0.0/10).
                    let cgnat = is_cgnat_ip(ext_ip);

                    if !is_valid_wan_ip(ext_ip) && !cgnat {
                        return Err(format!(
                            "Gateway reported invalid non-public WAN IP: {ext_ip}"
                        ));
                    }

                    if cgnat {
                        warn!(
                            "UPnP: CGNAT detected — WAN IP {ext_ip} is in the RFC 6598 shared \
                             address space. eD2K/Kad internet connectivity may be limited."
                        );
                    }

                    let local_ip = detect_local_ipv4_for_gateway(gateway.addr)
                        .ok_or_else(|| "Failed to detect local IPv4 address for gateway".to_string())?;

                    let local_tcp_addr = SocketAddrV4::new(local_ip, tcp_port);
                    let local_udp_addr = SocketAddrV4::new(local_ip, udp_port);

                    // Lease 3600 seconds (1 hour) with ownership-verified conflict recovery
                    let tcp_ok = add_port_safe(
                        &gateway,
                        PortMappingProtocol::TCP,
                        tcp_port,
                        local_tcp_addr,
                        3600,
                        "Emule-Sion eD2K TCP",
                    );

                    let udp_ok = add_port_safe(
                        &gateway,
                        PortMappingProtocol::UDP,
                        udp_port,
                        local_udp_addr,
                        3600,
                        "Emule-Sion Kad UDP",
                    );

                    // If both port mappings failed completely, treat as an error to trigger retry
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
                    // ── Success ──────────────────────────────────────────────────────
                    let mut snap = self.snapshot.write().await;
                    snap.available = true;
                    snap.gateway_addr = Some(gw.addr.to_string());
                    snap.external_ip = Some(ext_ip.to_string());
                    snap.tcp_mapped = tcp_ok;
                    snap.udp_mapped = udp_ok;
                    snap.cgnat_detected = cgnat;
                    snap.last_error = if cgnat {
                        Some(
                            "CGNAT détecté : votre FAI utilise un NAT partagé (100.64.x.x). \
                             Les ports sont mappés localement mais la connectivité eD2K/Kad \
                             depuis internet peut être limitée."
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
                        "UPnP setup succeeded on attempt {}/{MAX_ATTEMPTS}: \
                         Gateway {}, External IP {}, TCP: {}, UDP: {}, CGNAT: {}",
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
                    // ── Failure — prepare next retry ────────────────────────────────
                    last_err = e;
                    *self.cached_gateway.write().await = None;
                }
            }
        }

        // ── All attempts exhausted ────────────────────────────────────────────────
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

    /// Removes previously mapped ports from the gateway if verified to still belong to Emule-Sion.
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
                let local_ip_opt = detect_local_ipv4_for_gateway(gateway.addr);

                if tcp_mapped {
                    let should_remove = match local_ip_opt {
                        Some(lip) => match check_existing_mapping(
                            &gateway,
                            PortMappingProtocol::TCP,
                            tcp_port,
                            SocketAddrV4::new(lip, tcp_port),
                            "Emule-Sion",
                        ) {
                            MappingOwnership::ExactMatch => true,
                            MappingOwnership::StaleLease => {
                                warn!("Skipped unmapping TCP port {tcp_port}: entry is an old/stale lease from another IP or port; letting it expire naturally");
                                false
                            }
                            MappingOwnership::ForeignConflict => {
                                warn!("Skipped unmapping TCP port {tcp_port}: entry belongs to another application/host");
                                false
                            }
                            MappingOwnership::NotFound => false,
                        },
                        None => {
                            warn!("Skipped unmapping TCP port {tcp_port}: unable to determine local endpoint for ownership verification");
                            false
                        }
                    };

                    if should_remove {
                        let _ = gateway.remove_port(PortMappingProtocol::TCP, tcp_port);
                        info!("UPnP unmapped TCP port {tcp_port}");
                    }
                }

                if udp_mapped {
                    let should_remove = match local_ip_opt {
                        Some(lip) => match check_existing_mapping(
                            &gateway,
                            PortMappingProtocol::UDP,
                            udp_port,
                            SocketAddrV4::new(lip, udp_port),
                            "Emule-Sion",
                        ) {
                            MappingOwnership::ExactMatch => true,
                            MappingOwnership::StaleLease => {
                                warn!("Skipped unmapping UDP port {udp_port}: entry is an old/stale lease from another IP or port; letting it expire naturally");
                                false
                            }
                            MappingOwnership::ForeignConflict => {
                                warn!("Skipped unmapping UDP port {udp_port}: entry belongs to another application/host");
                                false
                            }
                            MappingOwnership::NotFound => false,
                        },
                        None => {
                            warn!("Skipped unmapping UDP port {udp_port}: unable to determine local endpoint for ownership verification");
                            false
                        }
                    };

                    if should_remove {
                        let _ = gateway.remove_port(PortMappingProtocol::UDP, udp_port);
                        info!("UPnP unmapped UDP port {udp_port}");
                    }
                }
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
}

