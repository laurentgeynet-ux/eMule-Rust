// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Server UDP service (`UDPSocket`, eMule 0.72a).
//!
//! eMule uses a *separate* UDP socket (from the client UDP socket) for
//! global server queries. This service binds an ephemeral UDP port and
//! sends `OP_GLOBGETSOURCES2` to each connected server's UDP port
//! (conventionally TCP port + 4 for Lugdunum servers), then ingests
//! `OP_GLOBFOUNDSOURCES` / `OP_GLOBSERVSTATRES` replies.

use crate::services::peer_session::log_server_addr;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

use mule_core::model::{PeerSource, PeerSourceOrigin, PeerSourceStatus};
use mule_network_policy::{
    classify_ip, ConnectionPurpose, IpCategory, NetworkPolicyManager, NetworkProfile,
};
use mule_protocol::ed2k::server_udp::{self, ServerUdpMessage};

use crate::services::downloads::DownloadService;

/// Snapshot of the server UDP service state.
#[derive(Debug, Clone)]
pub struct ServerUdpSnapshot {
    /// Whether the UDP socket is bound.
    pub is_active: bool,
    /// Local bind address, when bound.
    pub bind_address: Option<String>,
}

struct ServerUdpInner {
    socket: Option<Arc<UdpSocket>>,
    bind_addr: Option<SocketAddr>,
    listen_task: Option<JoinHandle<()>>,
    /// Per-server UDP capabilities learned from `OP_GLOBSERVSTATRES`,
    /// keyed by the server's UDP address (IP + TCP+4). Mirrors eMule's
    /// `CServer::GetUDPFlags()` storage.
    server_udp_flags: std::collections::HashMap<SocketAddr, u32>,
    /// Challenges sent in pending `OP_GLOBSERVSTATREQ` pings.
    pending_statreq: std::collections::HashMap<SocketAddr, u32>,
    /// eMule `CServer::lastdescpingedcout` — incremented on each accepted
    /// `GLOBSERVSTATRES`; an `OP_SERVER_DESC_REQ` is sent while `< 2` and
    /// the counter resets at 2 (so roughly every other stat ping asks for
    /// a description, `UDPSocket.cpp:420-437`).
    desc_ping_counts: std::collections::HashMap<SocketAddr, u8>,
    /// eMule `CServer::m_uDescReqChallenge` — challenge of the pending
    /// `OP_SERVER_DESC_REQ`, validated on new-format `DESC_RES`.
    desc_req_challenges: std::collections::HashMap<SocketAddr, u32>,
}

/// Canal d'événements broadcast + compteur de séquence partagé.
type EventChannel = Arc<
    std::sync::RwLock<
        Option<(
            tokio::sync::broadcast::Sender<String>,
            Arc<std::sync::atomic::AtomicU64>,
        )>,
    >,
>;

/// Shared server UDP service (cloneable handle).
#[derive(Clone)]
pub struct ServerUdpService {
    policy_mgr: NetworkPolicyManager,
    downloads: Arc<std::sync::RwLock<Option<Arc<Mutex<DownloadService>>>>>,
    event_channel: EventChannel,
    inner: Arc<RwLock<ServerUdpInner>>,
}

impl ServerUdpService {
    /// Creates a new server UDP service.
    pub fn new(policy_mgr: NetworkPolicyManager) -> Self {
        Self {
            policy_mgr,
            downloads: Arc::new(std::sync::RwLock::new(None)),
            event_channel: Arc::new(std::sync::RwLock::new(None)),
            inner: Arc::new(RwLock::new(ServerUdpInner {
                socket: None,
                bind_addr: None,
                listen_task: None,
                server_udp_flags: std::collections::HashMap::new(),
                pending_statreq: std::collections::HashMap::new(),
                desc_ping_counts: std::collections::HashMap::new(),
                desc_req_challenges: std::collections::HashMap::new(),
            })),
        }
    }

    /// Injects the download service (source ingestion).
    pub fn set_downloads(&self, downloads: Arc<Mutex<DownloadService>>) {
        *self.downloads.write().unwrap() = Some(downloads);
    }

    /// Injects the daemon event channel so `OP_GLOBSEARCHRES` answers can be
    /// surfaced as `server.searchResult` notifications like TCP results.
    pub fn set_event_channel(
        &self,
        event_tx: tokio::sync::broadcast::Sender<String>,
        sequence: Arc<std::sync::atomic::AtomicU64>,
    ) {
        *self.event_channel.write().unwrap() = Some((event_tx, sequence));
    }

    /// Starts the server UDP socket on an ephemeral port of `bind_ip`.
    pub async fn start(&self, bind_ip: std::net::IpAddr) -> Result<SocketAddr, String> {
        let mut inner = self.inner.write().await;
        if let Some(addr) = inner.bind_addr {
            if inner.socket.is_some() {
                return Ok(addr);
            }
        }

        let bind_addr = SocketAddr::new(bind_ip, 0);
        let _permit = self
            .policy_mgr
            .request_tcp_listen(bind_addr, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied server UDP bind on {bind_addr}: {e}"))?;

        let socket = UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| format!("Failed to bind server UDP socket to {bind_addr}: {e}"))?;
        let local_addr = socket
            .local_addr()
            .map_err(|e| format!("Failed to get local addr: {e}"))?;

        let socket_arc = Arc::new(socket);
        inner.socket = Some(socket_arc.clone());
        inner.bind_addr = Some(local_addr);

        let downloads_arc = self.downloads.clone();
        let event_channel = self.event_channel.clone();
        let policy_mgr_clone = self.policy_mgr.clone();
        let socket_for_task = socket_arc.clone();
        let inner_for_task = self.inner.clone();

        inner.listen_task = Some(tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                match socket_for_task.recv_from(&mut buf).await {
                    Ok((len, peer_addr)) => {
                        if len < 2 {
                            continue;
                        }
                        // Policy-based IP classification (same rules as Kad UDP).
                        let cat = classify_ip(peer_addr.ip());
                        let policy_allowed = match policy_mgr_clone.profile() {
                            NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                            NetworkProfile::PublicEd2kPreview
                            | NetworkProfile::PublicEd2kKadPreview => {
                                cat == IpCategory::PublicRoutable
                            }
                        };
                        if !policy_allowed {
                            continue;
                        }
                        match server_udp::decode_server_udp(&buf[..len]) {
                            Ok(ServerUdpMessage::GlobFoundSources(entries)) => {
                                let responder_ip = match peer_addr.ip() {
                                    std::net::IpAddr::V4(v4) => u32::from_le_bytes(v4.octets()),
                                    _ => 0,
                                };
                                handle_glob_found_sources(&downloads_arc, entries, responder_ip)
                                    .await;
                            }
                            Ok(ServerUdpMessage::GlobServStatRes {
                                challenge,
                                users,
                                files,
                                udp_flags,
                                tcp_obfuscation_port,
                                udp_obfuscation_port,
                            }) => {
                                // Only accept answers to challenges we issued
                                // (eMule `GetChallenge()` check) — unsolicited
                                // stat replies are dropped.
                                let accepted = {
                                    let mut inr = inner_for_task.write().await;
                                    match inr.pending_statreq.get(&peer_addr) {
                                        Some(&c) if c == challenge => {
                                            inr.server_udp_flags.insert(peer_addr, udp_flags);
                                            inr.pending_statreq.remove(&peer_addr);
                                            true
                                        }
                                        _ => false,
                                    }
                                };
                                if accepted {
                                    tracing::info!(
                                        "Server UDP: GlobServStatRes from {server} (users={users}, files={files}, udp_flags=0x{udp_flags:08X}, obf_tcp={tcp_obfuscation_port}, obf_udp={udp_obfuscation_port})"
                                    , server = log_server_addr(peer_addr));
                                    // eMule UDPSocket.cpp:418-437 — after a
                                    // stat answer, ask for the server
                                    // description while count < 2 (the
                                    // counter resets at 2, so the request
                                    // goes out on ~every other ping).
                                    maybe_send_desc_req(
                                        &socket_for_task,
                                        &inner_for_task,
                                        peer_addr,
                                    )
                                    .await;
                                } else {
                                    tracing::debug!(
                                        "Server UDP: ignoring unsolicited/mismatched GlobServStatRes from {server} (challenge={challenge})"
                                    , server = log_server_addr(peer_addr));
                                }
                            }
                            Ok(ServerUdpMessage::GlobSearchRes(payload)) => {
                                // UDP 0x99 has NO u32 count prefix — bare
                                // entry stream until packet end.
                                match server_udp::decode_glob_search_res(
                                    &payload,
                                    &mule_protocol::ed2k::Ed2kLimits::for_server(),
                                ) {
                                    Ok(sr) => {
                                        tracing::info!(
                                            "Server UDP: {} global search result(s) from {server}",
                                            sr.files.len(),
                                            server = log_server_addr(peer_addr)
                                        );
                                        let chan = event_channel.read().unwrap().clone();
                                        if let Some((tx, seq)) = chan {
                                            let dl = downloads_arc.read().unwrap().clone();
                                            let cancelled = super::server_session::events::search_result_cancelled_flags(&dl, &sr).await;
                                            super::server_session::events::emit_search_results(
                                                &tx, &seq, &sr, &cancelled,
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        tracing::info!(
                                            "Server UDP: malformed GlobSearchRes from {server}: {e}"
                                        , server = log_server_addr(peer_addr));
                                    }
                                }
                            }
                            Ok(ServerUdpMessage::ServerDescRes {
                                challenge,
                                name,
                                description,
                                dyn_ip,
                                version,
                            }) => {
                                handle_server_desc_res(
                                    &downloads_arc,
                                    &inner_for_task,
                                    peer_addr,
                                    challenge,
                                    name,
                                    description,
                                    dyn_ip,
                                    version,
                                )
                                .await;
                            }
                            Ok(ServerUdpMessage::Unknown { opcode }) => {
                                tracing::info!(
                                    "Server UDP: unknown opcode 0x{opcode:02X} from {server}",
                                    server = log_server_addr(peer_addr)
                                );
                            }
                            Err(e) => {
                                tracing::info!(
                                    "Server UDP: decode error from {server}: {e}",
                                    server = log_server_addr(peer_addr)
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Server UDP: recv_from error: {e}");
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                }
            }
        }));

        Ok(local_addr)
    }

    /// Sends `OP_GLOBGETSOURCES2` to a server's UDP port for `file_hash`.
    /// `server_udp_addr` is conventionally `server_tcp_addr` with port + 4.
    pub async fn send_glob_get_sources(
        &self,
        server_udp_addr: SocketAddr,
        file_hash: &[u8; 16],
        file_size: u64,
    ) -> bool {
        let socket = { self.inner.read().await.socket.clone() };
        let Some(socket) = socket else {
            return false;
        };
        let size32 = file_size.min(u32::MAX as u64) as u32;
        let pkt = server_udp::encode_glob_get_sources2(file_hash, size32);
        match socket.send_to(&pkt, server_udp_addr).await {
            Ok(_) => true,
            Err(e) => {
                tracing::debug!(
                    "Server UDP: send_glob_get_sources to {server} failed: {e}",
                    server = log_server_addr(server_udp_addr)
                );
                false
            }
        }
    }

    /// Sends `OP_GLOBSERVSTATREQ` to learn the server's UDP capability flags
    /// (`SRV_UDPFLG_*`) and obfuscation ports — the answer is a
    /// `GLOBSERVSTATRES` whose challenge must echo the one we send.
    pub async fn send_servstat_req(&self, server_udp_addr: SocketAddr) -> bool {
        let (socket, challenge) = {
            let mut inner = self.inner.write().await;
            let Some(socket) = inner.socket.clone() else {
                return false;
            };
            let challenge: u32 = rand::random();
            inner.pending_statreq.insert(server_udp_addr, challenge);
            (socket, challenge)
        };
        let pkt = server_udp::encode_glob_servstat_req(challenge);
        match socket.send_to(&pkt, server_udp_addr).await {
            Ok(_) => true,
            Err(e) => {
                tracing::debug!(
                    "Server UDP: send_servstat_req to {server} failed: {e}",
                    server = log_server_addr(server_udp_addr)
                );
                false
            }
        }
    }

    /// Sends a global UDP search to a server's UDP port, choosing the wire
    /// form like eMule `SearchResultsWnd.cpp:277-310`:
    /// `OP_GLOBSEARCHREQ3` when the server advertised both
    /// `SRV_UDPFLG_EXT_GETFILES` and `SRV_UDPFLG_LARGEFILES`, otherwise
    /// `OP_GLOBSEARCHREQ2`. When the flags are still unknown, a
    /// `GLOBSERVSTATREQ` ping is sent first so a later search can upgrade.
    /// `search_payload` is the raw payload of an encoded `SearchQuery`
    /// packet (eMule reuses the TCP search bytes verbatim).
    ///
    /// Returns the wire form actually sent (`"REQ3"`/`"REQ2"`) so the RPC
    /// layer can report it for testing, `None` when nothing was sent.
    pub async fn send_glob_search(
        &self,
        server_udp_addr: SocketAddr,
        search_payload: &[u8],
    ) -> Option<&'static str> {
        let (socket, flags) = {
            let inner = self.inner.read().await;
            let socket = inner.socket.clone()?;
            (
                socket,
                inner.server_udp_flags.get(&server_udp_addr).copied(),
            )
        };
        let Some(flags) = flags else {
            // Flags unknown — probe the server first, then send REQ1, the
            // lowest common denominator (eMule sends GLOBSEARCHREQ 0x98 to
            // servers without EXT_GETFILES; REQ2 would be ignored there).
            self.send_servstat_req(server_udp_addr).await;
            let pkt = server_udp::encode_glob_search_req(search_payload);
            return match socket.send_to(&pkt, server_udp_addr).await {
                Ok(_) => Some("REQ1"),
                Err(e) => {
                    tracing::debug!(
                        "Server UDP: send_glob_search to {server} failed: {e}",
                        server = log_server_addr(server_udp_addr)
                    );
                    None
                }
            };
        };

        // eMule SearchResultsWnd.cpp:277-310 — three wire forms:
        // EXT_GETFILES+LARGEFILES → REQ3, EXT_GETFILES → REQ2, else REQ1.
        let ext_getfiles = flags & server_udp::SRV_UDPFLG_EXT_GETFILES != 0;
        let (pkt, form) = if ext_getfiles && flags & server_udp::SRV_UDPFLG_LARGEFILES != 0 {
            (server_udp::encode_glob_search_req3(search_payload), "REQ3")
        } else if ext_getfiles {
            (server_udp::encode_glob_search_req2(search_payload), "REQ2")
        } else {
            (server_udp::encode_glob_search_req(search_payload), "REQ1")
        };
        tracing::debug!(
            "Server UDP: GLOBSEARCH{form} to {server} (udp_flags=0x{flags:08X})",
            server = log_server_addr(server_udp_addr)
        );
        match socket.send_to(&pkt, server_udp_addr).await {
            Ok(_) => Some(form),
            Err(e) => {
                tracing::debug!(
                    "Server UDP: send_glob_search to {server} failed: {e}",
                    server = log_server_addr(server_udp_addr)
                );
                None
            }
        }
    }

    /// Stops the socket and receive task.
    pub async fn stop(&self) {
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.listen_task.take() {
            task.abort();
        }
        inner.socket = None;
        inner.bind_addr = None;
    }

    /// Status snapshot.
    pub async fn snapshot(&self) -> ServerUdpSnapshot {
        let inner = self.inner.read().await;
        ServerUdpSnapshot {
            is_active: inner.socket.is_some(),
            bind_address: inner.bind_addr.map(|a| a.to_string()),
        }
    }
}

/// Sends `OP_SERVER_DESC_REQ` to `server_udp_addr` following eMule's
/// `LastDescPingedCount` cycle (`UDPSocket.cpp:418-437`): each accepted
/// stat answer increments the counter; while it is `< 2` a challenged
/// description request goes out, and the counter resets at 2. The
/// challenge — `(rand16 << 16) | 0xF0FF` — is stored so the new-format
/// answer can be validated (`CServer::m_uDescReqChallenge`).
async fn maybe_send_desc_req(
    socket: &UdpSocket,
    inner: &RwLock<ServerUdpInner>,
    server_udp_addr: SocketAddr,
) {
    let challenge = {
        let mut inr = inner.write().await;
        let count = inr.desc_ping_counts.entry(server_udp_addr).or_insert(0);
        *count += 1;
        if *count >= 2 {
            *count = 0;
            return;
        }
        let challenge = server_udp::make_desc_req_challenge(rand::random::<u16>());
        inr.desc_req_challenges.insert(server_udp_addr, challenge);
        challenge
    };
    let pkt = server_udp::encode_server_desc_req(challenge);
    if let Err(e) = socket.send_to(&pkt, server_udp_addr).await {
        tracing::debug!(
            "Server UDP: send_server_desc_req to {server} failed: {e}",
            server = log_server_addr(server_udp_addr)
        );
    }
}

/// Applies an `OP_SERVER_DESC_RES` to the matching `Ed2kServer` record
/// (`UDPSocket.cpp:438-496` + `CServerList::GetServerByIPUDP` — the server
/// is matched on `ip` with `udp_port == tcp_port + 4`).
///
/// New-format answers are only applied when their challenge echoes a
/// pending request (unsolicited ones are dropped like eMule). Old-format
/// answers carry no challenge and are applied directly. `dyn_ip` is kept
/// only when it is a DNS name — eMule rejects bare dotted-quad strings
/// via `inet_addr() == INADDR_NONE`.
#[allow(clippy::too_many_arguments)]
async fn handle_server_desc_res(
    downloads: &Arc<std::sync::RwLock<Option<Arc<Mutex<DownloadService>>>>>,
    inner: &RwLock<ServerUdpInner>,
    peer_addr: SocketAddr,
    challenge: Option<u32>,
    name: Option<String>,
    description: Option<String>,
    dyn_ip: Option<String>,
    version: Option<String>,
) {
    if let Some(challenge) = challenge {
        let mut inr = inner.write().await;
        match inr.desc_req_challenges.get(&peer_addr) {
            Some(&c) if c == challenge => {
                inr.desc_req_challenges.remove(&peer_addr);
            }
            _ => {
                tracing::debug!(
                    "Server UDP: ignoring unsolicited/mismatched ServerDescRes from {server} (challenge={challenge:#010X})",
                    server = log_server_addr(peer_addr)
                );
                return;
            }
        }
    }

    let downloads = downloads.read().unwrap().clone();
    let Some(dl_service) = downloads else {
        return;
    };
    let mut dl_guard = dl_service.lock().await;
    let Ok(servers) = dl_guard.list_servers() else {
        return;
    };
    // GetServerByIPUDP: same IP and udp_port == tcp_port + 4 (the
    // dedicated-obfuscation-port alternative is not modelled — we do not
    // track per-server obfuscation ports).
    let ip_str = peer_addr.ip().to_string();
    let Some(mut server) = servers
        .into_iter()
        .find(|s| s.ip == ip_str && u32::from(s.port) + 4 == u32::from(peer_addr.port()))
    else {
        tracing::debug!(
            "Server UDP: ServerDescRes from unknown server {server}",
            server = log_server_addr(peer_addr)
        );
        return;
    };

    if let Some(name) = name {
        server.name = name;
    }
    if let Some(description) = description {
        server.description = description;
    }
    if let Some(dyn_ip) = dyn_ip.filter(|d| d.parse::<std::net::Ipv4Addr>().is_err()) {
        server.dyn_ip = Some(dyn_ip);
    }
    if let Some(version) = version {
        server.version = Some(version);
    }
    server.updated_at_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    if let Err(e) = dl_guard.upsert_servers(&[server]) {
        tracing::debug!("Server UDP: failed to persist server description: {e}");
    }
}

/// Ingests `OP_GLOBFOUNDSOURCES` entries into the peer source DB —
/// same filtering as the TCP `FoundSources` path (invalid IPs rejected,
/// LowID sources kept as `lowid:<id>` for server callback). The responder's
/// IP is recorded as the sources' server (eMule `m_dwServerIP`) so the
/// dialer can tell local-server LowIDs from foreign ones.
async fn handle_glob_found_sources(
    downloads: &Arc<std::sync::RwLock<Option<Arc<Mutex<DownloadService>>>>>,
    entries: Vec<server_udp::GlobFoundSourcesEntry>,
    responder_ip: u32,
) {
    let downloads = downloads.read().unwrap().clone();
    let Some(dl_service) = downloads else {
        return;
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    for entry in entries {
        let file_hash_hex = hex::encode(entry.file_hash);
        let mut discovered = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for (ip, port) in entry.sources {
            if ip == 0 {
                continue;
            }
            let addr_str = if ip < 16_777_216 {
                format!("lowid:{ip}")
            } else {
                if port == 0 {
                    continue;
                }
                let ip_addr = std::net::Ipv4Addr::from(ip.to_le_bytes());
                if ip_addr.is_broadcast() || ip_addr.is_multicast() {
                    continue;
                }
                format!("{ip_addr}:{port}")
            };
            if !seen.insert(addr_str.clone()) {
                continue;
            }
            discovered.push(PeerSource {
                file_hash: file_hash_hex.clone(),
                address: addr_str,
                ip,
                port,
                origin: PeerSourceOrigin::Server,
                discovered_at_unix_ms: now_ms,
                last_seen_unix_ms: now_ms,
                status: PeerSourceStatus::Candidate,
                fail_count: 0,
                last_error: None,
                cooldown_until_unix_ms: None,
                user_hash: None,
                crypt_options: 0,
                buddy_id: None,
                software: None,
                availability: None,
                // Serveur répondant (IP seule — son port TCP est inconnu du
                // canal UDP ; la localité se compare alors sur l'IP).
                server_ip: responder_ip,
                server_port: 0,
                udp_port: 0,
            });
        }

        if !discovered.is_empty() {
            tracing::info!(
                "Server UDP GlobFoundSources for {file_hash_hex}: {} sources accepted",
                discovered.len()
            );
            let mut dl_guard = dl_service.lock().await;
            let _ = dl_guard.upsert_peer_sources(&file_hash_hex, &discovered);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mule_network_policy::ResourceBudgets;
    use std::net::Ipv4Addr;
    use std::time::Duration;

    fn service() -> ServerUdpService {
        ServerUdpService::new(NetworkPolicyManager::new(
            NetworkProfile::LocalLab,
            ResourceBudgets::default(),
        ))
    }

    /// Receives the next datagram on `recv` and returns its opcode byte.
    async fn next_opcode(recv: &UdpSocket) -> u8 {
        let mut buf = [0u8; 2048];
        tokio::time::timeout(Duration::from_secs(2), recv.recv_from(&mut buf))
            .await
            .expect("no datagram received")
            .expect("recv failed");
        assert_eq!(buf[0], mule_protocol::ed2k::protocol::PR_ED2K);
        buf[1]
    }

    #[tokio::test]
    async fn glob_search_uses_req3_when_server_supports_it() {
        let svc = service();
        svc.start(Ipv4Addr::LOCALHOST.into()).await.unwrap();
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr().unwrap();

        svc.inner.write().await.server_udp_flags.insert(
            server_addr,
            server_udp::SRV_UDPFLG_EXT_GETFILES | server_udp::SRV_UDPFLG_LARGEFILES,
        );

        let form = svc.send_glob_search(server_addr, &[0xAA, 0xBB]).await;
        assert_eq!(form, Some("REQ3"));
        assert_eq!(next_opcode(&server).await, 0x90);
    }

    #[tokio::test]
    async fn glob_search_falls_back_to_req2_without_largefiles() {
        let svc = service();
        svc.start(Ipv4Addr::LOCALHOST.into()).await.unwrap();
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr().unwrap();

        svc.inner
            .write()
            .await
            .server_udp_flags
            .insert(server_addr, server_udp::SRV_UDPFLG_EXT_GETFILES);

        let form = svc.send_glob_search(server_addr, &[0xAA]).await;
        assert_eq!(form, Some("REQ2"));
        assert_eq!(next_opcode(&server).await, 0x92);
    }

    #[tokio::test]
    async fn glob_search_probes_unknown_server_then_sends_req1() {
        let svc = service();
        svc.start(Ipv4Addr::LOCALHOST.into()).await.unwrap();
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr().unwrap();

        let form = svc.send_glob_search(server_addr, &[0xAA]).await;
        assert_eq!(form, Some("REQ1"));
        // First datagram: GLOBSERVSTATREQ probe (0x96), second: REQ1 (0x98).
        assert_eq!(next_opcode(&server).await, 0x96);
        assert_eq!(next_opcode(&server).await, 0x98);
    }

    #[tokio::test]
    async fn glob_search_sends_req1_when_server_lacks_ext_getfiles() {
        let svc = service();
        svc.start(Ipv4Addr::LOCALHOST.into()).await.unwrap();
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr().unwrap();

        svc.inner
            .write()
            .await
            .server_udp_flags
            .insert(server_addr, server_udp::SRV_UDPFLG_LARGEFILES);

        let form = svc.send_glob_search(server_addr, &[0xAA]).await;
        assert_eq!(form, Some("REQ1"));
        assert_eq!(next_opcode(&server).await, 0x98);
    }

    /// Receives a datagram and returns `(payload, sender)`.
    async fn recv_pkt(recv: &UdpSocket) -> (Vec<u8>, SocketAddr) {
        let mut buf = [0u8; 4096];
        let (len, from) = tokio::time::timeout(Duration::from_secs(2), recv.recv_from(&mut buf))
            .await
            .expect("no datagram received")
            .expect("recv failed");
        (buf[..len].to_vec(), from)
    }

    /// Answers the pending `GLOBSERVSTATREQ` from `server` with a minimal
    /// stat answer echoing its challenge.
    async fn answer_statreq(server: &UdpSocket) -> SocketAddr {
        let (pkt, daemon) = recv_pkt(server).await;
        assert_eq!(pkt[0], mule_protocol::ed2k::protocol::PR_ED2K);
        assert_eq!(pkt[1], mule_protocol::ed2k::opcodes::OP_GLOBSERVSTATREQ);
        let mut res = vec![
            mule_protocol::ed2k::protocol::PR_ED2K,
            mule_protocol::ed2k::opcodes::OP_GLOBSERVSTATRES,
        ];
        res.extend_from_slice(&pkt[2..6]); // challenge echo
        res.extend_from_slice(&42u32.to_le_bytes()); // users
        res.extend_from_slice(&4242u32.to_le_bytes()); // files
        server.send_to(&res, daemon).await.unwrap();
        daemon
    }

    #[tokio::test]
    async fn desc_req_after_accepted_statres_and_res_updates_record() {
        let svc = service();
        svc.start(Ipv4Addr::LOCALHOST.into()).await.unwrap();
        let server = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server.local_addr().unwrap();

        // Wire a DownloadService holding the server record: the UDP peer
        // resolves to tcp_port = udp_port - 4 (GetServerByIPUDP).
        let tmp = tempfile::tempdir().unwrap();
        let mut dl = DownloadService::in_memory(tmp.path().join("temp")).unwrap();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let tcp_port = server_addr.port() - 4;
        dl.upsert_servers(&[mule_core::model::Ed2kServer {
            address: format!("127.0.0.1:{tcp_port}"),
            ip: "127.0.0.1".into(),
            port: tcp_port,
            name: String::new(),
            description: String::new(),
            ping: 0,
            preference: 0,
            failed_count: 0,
            dyn_ip: None,
            version: None,
            filter_status: mule_core::model::ServerFilterStatus::Allowed,
            last_seen_unix_ms: None,
            created_at_unix_ms: now_ms,
            updated_at_unix_ms: now_ms,
        }])
        .unwrap();
        svc.set_downloads(Arc::new(Mutex::new(dl)));

        // 1st stat ping → DESC_REQ expected (count 1 < 2).
        assert!(svc.send_servstat_req(server_addr).await);
        answer_statreq(&server).await;
        let (pkt, _) = recv_pkt(&server).await;
        assert_eq!(pkt[1], server_udp::OP_SERVER_DESC_REQ);
        let desc_challenge = u32::from_le_bytes(pkt[2..6].try_into().unwrap());
        assert_eq!(
            desc_challenge & 0xFFFF,
            u32::from(server_udp::INV_SERV_DESC_LEN)
        );
        assert_eq!(
            svc.inner
                .read()
                .await
                .desc_req_challenges
                .get(&server_addr)
                .copied(),
            Some(desc_challenge)
        );

        // Unsolicited new-format answer with a wrong challenge is dropped.
        let mut wrong = vec![
            mule_protocol::ed2k::protocol::PR_ED2K,
            server_udp::OP_SERVER_DESC_RES,
        ];
        // Flip one high bit — keeps the INV_SERV_DESC_LEN marker so the
        // packet is a valid new-format RES, but never equals the pending
        // challenge.
        wrong.extend_from_slice(&(desc_challenge ^ 0x0001_0000).to_le_bytes());
        wrong.extend_from_slice(&1u32.to_le_bytes());
        wrong.push(mule_protocol::ed2k::tags::TAGTYPE_STRING | 0x80);
        wrong.push(server_udp::ST_SERVERNAME);
        wrong.extend_from_slice(&4u16.to_le_bytes());
        wrong.extend_from_slice(b"WRNG");
        server
            .send_to(&wrong, svc.inner.read().await.bind_addr.unwrap())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Matching challenge → the record is updated.
        let mut res = vec![
            mule_protocol::ed2k::protocol::PR_ED2K,
            server_udp::OP_SERVER_DESC_RES,
        ];
        res.extend_from_slice(&desc_challenge.to_le_bytes());
        res.extend_from_slice(&2u32.to_le_bytes());
        for (tag_id, val) in [
            (server_udp::ST_SERVERNAME, "Loopback Lugdunum"),
            (server_udp::ST_DYNIP, "loop.example.org"),
        ] {
            res.push(mule_protocol::ed2k::tags::TAGTYPE_STRING | 0x80);
            res.push(tag_id);
            res.extend_from_slice(&(val.len() as u16).to_le_bytes());
            res.extend_from_slice(val.as_bytes());
        }
        server
            .send_to(&res, svc.inner.read().await.bind_addr.unwrap())
            .await
            .unwrap();

        // Wait for the update to land.
        let downloads = svc.downloads.read().unwrap().clone().unwrap();
        for _ in 0..50 {
            let updated = {
                let guard = downloads.lock().await;
                guard.list_servers().unwrap()[0].name == "Loopback Lugdunum"
            };
            if updated {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let guard = downloads.lock().await;
        let rec = &guard.list_servers().unwrap()[0];
        assert_eq!(rec.name, "Loopback Lugdunum");
        assert_eq!(rec.dyn_ip.as_deref(), Some("loop.example.org"));
        drop(guard);

        // 2nd stat ping → counter hits 2, resets: NO desc_req.
        assert!(svc.send_servstat_req(server_addr).await);
        answer_statreq(&server).await;
        let mut buf = [0u8; 256];
        let timed_out = tokio::time::timeout(
            Duration::from_millis(300),
            server.recv_from(&mut buf),
        )
        .await;
        assert!(timed_out.is_err(), "unexpected datagram on reset cycle");
    }
}
