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
                            Ok(ServerUdpMessage::GlobServStatRes { users, files }) => {
                                tracing::debug!(
                                    "Server UDP: GlobServStatRes from {peer_addr} (users={users}, files={files})"
                                );
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
                                tracing::debug!(
                                    "Server UDP: ignoring opcode 0x{opcode:02X} from {peer_addr}"
                                );
                            }
                            Err(e) => {
                                tracing::debug!("Server UDP: decode error from {peer_addr}: {e}");
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

    /// Sends `OP_GLOBSEARCHREQ2` to a server's UDP port. `search_payload` is
    /// the raw payload of an encoded `SearchQuery` packet (eMule reuses the
    /// TCP search packet bytes verbatim, `SearchResultsWnd.cpp:296`).
    pub async fn send_glob_search(
        &self,
        server_udp_addr: SocketAddr,
        search_payload: &[u8],
    ) -> bool {
        let socket = { self.inner.read().await.socket.clone() };
        let Some(socket) = socket else {
            return false;
        };
        let pkt = server_udp::encode_glob_search_req2(search_payload);
        match socket.send_to(&pkt, server_udp_addr).await {
            Ok(_) => true,
            Err(e) => {
                tracing::debug!("Server UDP: send_glob_search to {server_udp_addr} failed: {e}");
                false
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
