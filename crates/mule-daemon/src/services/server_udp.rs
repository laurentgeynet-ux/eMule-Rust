//! Server UDP service (`UDPSocket`, eMule 0.72a).
//!
//! eMule uses a *separate* UDP socket (from the client UDP socket) for
//! global server queries. This service binds an ephemeral UDP port and
//! sends `OP_GLOBGETSOURCES2` to each connected server's UDP port
//! (conventionally TCP port + 4 for Lugdunum servers), then ingests
//! `OP_GLOBFOUNDSOURCES` / `OP_GLOBSERVSTATRES` replies.

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
}

/// Shared server UDP service (cloneable handle).
#[derive(Clone)]
pub struct ServerUdpService {
    policy_mgr: NetworkPolicyManager,
    downloads: Arc<std::sync::RwLock<Option<Arc<Mutex<DownloadService>>>>>,
    event_channel: Arc<
        std::sync::RwLock<
            Option<(
                tokio::sync::broadcast::Sender<String>,
                Arc<std::sync::atomic::AtomicU64>,
            )>,
        >,
    >,
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
                                handle_glob_found_sources(&downloads_arc, entries).await;
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
                                        "Server UDP: GlobServStatRes from {peer_addr} (users={users}, files={files}, udp_flags=0x{udp_flags:08X}, obf_tcp={tcp_obfuscation_port}, obf_udp={udp_obfuscation_port})"
                                    );
                                } else {
                                    tracing::debug!(
                                        "Server UDP: ignoring unsolicited/mismatched GlobServStatRes from {peer_addr} (challenge={challenge})"
                                    );
                                }
                            }
                            Ok(ServerUdpMessage::GlobSearchRes(payload)) => {
                                // Same entry layout as OP_SEARCHRESULT — decode
                                // through the TCP codec and emit searchResult events.
                                match mule_protocol::ed2k::decode_server_response(
                                    mule_protocol::PR_ED2K,
                                    mule_protocol::ed2k::OP_SEARCH_RESULT,
                                    &payload,
                                    &mule_protocol::ed2k::Ed2kLimits::for_server(),
                                ) {
                                    Ok(mule_protocol::ed2k::ServerResponse::SearchResult(sr)) => {
                                        tracing::info!(
                                            "Server UDP: {} global search result(s) from {peer_addr}",
                                            sr.files.len()
                                        );
                                        let chan = event_channel.read().unwrap().clone();
                                        if let Some((tx, seq)) = chan {
                                            super::server_session::events::emit_search_results(
                                                &tx, &seq, &sr,
                                            );
                                        }
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        tracing::debug!(
                                            "Server UDP: malformed GlobSearchRes from {peer_addr}: {e}"
                                        );
                                    }
                                }
                            }
                            Ok(ServerUdpMessage::Unknown { opcode }) => {
                                tracing::info!(
                                    "Server UDP: unknown opcode 0x{opcode:02X} from {peer_addr}"
                                );
                            }
                            Err(e) => {
                                tracing::info!("Server UDP: decode error from {peer_addr}: {e}");
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
                    "Server UDP: send_glob_get_sources to {server_udp_addr} failed: {e}"
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
                tracing::debug!("Server UDP: send_servstat_req to {server_udp_addr} failed: {e}");
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
            let Some(socket) = inner.socket.clone() else {
                return None;
            };
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
                        "Server UDP: send_glob_search to {server_udp_addr} failed: {e}"
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
            "Server UDP: GLOBSEARCH{form} to {server_udp_addr} (udp_flags=0x{flags:08X})"
        );
        match socket.send_to(&pkt, server_udp_addr).await {
            Ok(_) => Some(form),
            Err(e) => {
                tracing::debug!("Server UDP: send_glob_search to {server_udp_addr} failed: {e}");
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

/// Ingests `OP_GLOBFOUNDSOURCES` entries into the peer source DB —
/// same filtering as the TCP `FoundSources` path (invalid IPs rejected,
/// LowID sources kept as `lowid:<id>` for server callback).
async fn handle_glob_found_sources(
    downloads: &Arc<std::sync::RwLock<Option<Arc<Mutex<DownloadService>>>>>,
    entries: Vec<server_udp::GlobFoundSourcesEntry>,
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
}
