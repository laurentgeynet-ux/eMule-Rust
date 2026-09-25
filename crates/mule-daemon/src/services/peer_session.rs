// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Swarm eD2K Peer Connection & In-flight Block Manager (Dual-source Multi-Peer).
//!
//! Manages active TCP peer connections per download and globally.
//! Enforces network policy, token bucket rate limits, instant kill-switch abortion,
//! bounded wire framing decoding (OP_HELLO, OP_HELLO_ANSWER, OP_FILESTATUSREQ, OP_FILESTATUS, OP_REQUESTPARTS, OP_SENDINGPART),
//! granular timeouts (connect, handshake, status, queue, first_data, inactivity),
//! bounded exponential backoff cooldown per source, and non-overlapping block allocation.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, info, warn, Instrument};

use mule_core::model::{PeerFailureReason, PeerSourceStatus};
use mule_network_policy::SocketPermit;

use crate::error::DaemonError;
use crate::services::downloads::DownloadService;

pub mod a4af;
pub mod change_client_id;
pub mod connection;
pub mod events;
pub mod file_request;
pub mod handshake;
pub mod source_exchange;
pub mod status_query;
pub mod sui;
pub mod transfer;
pub mod types;

pub use connection::{emit_download_update, emit_event, OP_COMPRESSEDPART_I64, OP_SENDINGPART_I64};
pub use types::{
    calculate_backoff_cooldown_ms, PeerConnectionStatus, PeerSessionSnapshot, PeerTimeouts,
    SwarmStatusSnapshot, MAX_GLOBAL_PEERS, MAX_PEERS_PER_DOWNLOAD, MAX_PEER_READ_BUFFER_SIZE,
};
use types::{PeerSessionHandle, SwarmInnerState};

/// Délai minimal avant de re-dialer une source dont la socket en file vient
/// de mourir quand la deadline de re-ask est déjà passée (garde anti-spin —
/// eMule re-ask immédiatement à `GetTimeUntilReask() == 0`).
const QUEUE_LOSS_REDIAL_FLOOR_MS: i64 = 60_000;

/// Pseudonymiseur de session partagé par les sous-modules `peer_session`
/// : les adresses de pairs externes n'apparaissent jamais en clair dans
/// les fichiers de log persistants — `peer-NN:port` (loopback en clair,
/// inoffensif pour la lab).
static LOG_PSEUDONYMIZER: std::sync::OnceLock<mule_filter::SessionPseudonymizer> =
    std::sync::OnceLock::new();

/// Formate une adresse de pair pour les logs : loopback en clair,
/// adresses externes pseudonymisées de façon déterministe par session.
/// À utiliser à chaque émission de log contenant une adresse distante.
pub fn log_peer_addr(addr: SocketAddr) -> String {
    if addr.ip().is_loopback() {
        addr.to_string()
    } else {
        LOG_PSEUDONYMIZER
            .get_or_init(mule_filter::SessionPseudonymizer::new)
            .pseudonymize_peer(addr)
    }
}

/// Variante serveur : `server-NN:port` pour les endpoints eD2K/Kad UDP
/// côté serveur (table distincte des pairs).
pub fn log_server_addr(addr: SocketAddr) -> String {
    if addr.ip().is_loopback() {
        addr.to_string()
    } else {
        LOG_PSEUDONYMIZER
            .get_or_init(mule_filter::SessionPseudonymizer::new)
            .pseudonymize_server(addr)
    }
}

/// Pour une adresse déjà formatée (`"ip:port"`) — pseudonymisée si
/// parseable, inchangée sinon (`lowid:<id>` n'est pas une IP).
pub fn log_addr_str(s: impl std::fmt::Display) -> String {
    let raw = s.to_string();
    match raw.parse::<SocketAddr>() {
        Ok(addr) => log_peer_addr(addr),
        Err(_) => raw,
    }
}

/// Pour une IP nue sans port — `peer-NN` (partage la table des pairs).
/// Accepte `IpAddr` ou `Ipv4Addr` (`Into<IpAddr>`).
pub fn log_peer_ip(ip: impl Into<std::net::IpAddr>) -> String {
    let ip = ip.into();
    if ip.is_loopback() {
        ip.to_string()
    } else {
        LOG_PSEUDONYMIZER
            .get_or_init(mule_filter::SessionPseudonymizer::new)
            .pseudonymize_ip(ip)
    }
}

/// Thread-safe manager for multi-source eD2K peer connections.
#[derive(Clone)]
pub struct PeerSessionManager {
    inner: Arc<Mutex<SwarmInnerState>>,
    event_tx: broadcast::Sender<String>,
    sequence: Arc<AtomicU64>,
    timeouts: PeerTimeouts,
    kad: Arc<std::sync::RwLock<Option<Arc<crate::services::KadService>>>>,
    identity: Arc<std::sync::RwLock<Option<Arc<Mutex<crate::services::IdentityService>>>>>,
    sui_stats: Arc<std::sync::RwLock<Arc<crate::services::identity::SuiStats>>>,
    /// LowID → (file_hash_hex, expiry unix ms) for pending server-relayed
    /// callbacks (`OP_CALLBACKREQUEST`). Entries expire after 5 minutes.
    pending_callbacks: Arc<std::sync::Mutex<std::collections::HashMap<u32, (String, i64)>>>,
    /// Peer IP → (file_hash_hex, expiry unix ms) for pending Kad buddy
    /// callbacks (`KADEMLIA_CALLBACK_REQ`). The LowID source connects
    /// inbound; matched in the inbound listener by the remote IP.
    pending_kad_callbacks:
        Arc<std::sync::Mutex<std::collections::HashMap<std::net::IpAddr, (String, i64)>>>,
    /// Global bandwidth rate limiters shared with the network policy —
    /// the transfer loop paces received data through `download_bucket`.
    rate_limiters: Arc<std::sync::RwLock<Option<Arc<mule_network_policy::PolicyRateLimiters>>>>,
}

impl PeerSessionManager {
    /// Creates a new `PeerSessionManager`.
    pub fn new(event_tx: broadcast::Sender<String>, sequence: Arc<AtomicU64>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SwarmInnerState {
                sessions: HashMap::new(),
                in_flight_blocks: Vec::new(),
                last_session: None,
                max_peers_per_download: MAX_PEERS_PER_DOWNLOAD,
                max_global_peers: MAX_GLOBAL_PEERS,
                peer_udp: HashMap::new(),
                queued_peers: HashMap::new(),
            })),
            event_tx,
            sequence,
            timeouts: PeerTimeouts::default(),
            kad: Arc::new(std::sync::RwLock::new(None)),
            identity: Arc::new(std::sync::RwLock::new(None)),
            sui_stats: Arc::new(std::sync::RwLock::new(Arc::new(
                crate::services::identity::SuiStats::default(),
            ))),
            pending_callbacks: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            pending_kad_callbacks: Arc::new(
                std::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            rate_limiters: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Injects the shared network-policy rate limiters (global download
    /// bandwidth scheduler for peer transfer loops).
    pub fn set_rate_limiters(&self, limiters: Arc<mule_network_policy::PolicyRateLimiters>) {
        *self.rate_limiters.write().unwrap() = Some(limiters);
    }

    fn rate_limiters(&self) -> Option<Arc<mule_network_policy::PolicyRateLimiters>> {
        self.rate_limiters.read().unwrap().clone()
    }

    /// Registers a pending LowID callback: after `OP_CALLBACKREQUEST` was sent
    /// to the server, the target LowID client (`lowid`) is expected to connect
    /// inbound within `ttl_ms`. Matched in the inbound listener by the peer's
    /// `client_id` from `OP_HELLO`.
    pub fn register_pending_callback(&self, lowid: u32, file_hash: String, ttl_ms: i64) {
        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
            + ttl_ms;
        let mut map = self.pending_callbacks.lock().unwrap();
        map.retain(|_, (_, exp)| *exp > expiry - ttl_ms);
        map.insert(lowid, (file_hash, expiry));
    }

    /// Consumes a pending callback registration for `lowid`, returning the
    /// associated file hash if still fresh.
    pub fn take_pending_callback(&self, lowid: u32) -> Option<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let mut map = self.pending_callbacks.lock().unwrap();
        map.retain(|_, (_, exp)| *exp > now);
        map.remove(&lowid).map(|(h, _)| h)
    }

    /// Returns true if a pending (unexpired) callback exists for `lowid`.
    pub fn has_pending_callback(&self, lowid: u32) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let mut map = self.pending_callbacks.lock().unwrap();
        map.retain(|_, (_, exp)| *exp > now);
        map.contains_key(&lowid)
    }

    /// Registers a pending Kad buddy callback: after `KADEMLIA_CALLBACK_REQ`
    /// was relayed to the buddy, the LowID source (at `ip`) is expected to
    /// connect inbound within `ttl_ms`.
    pub fn register_pending_kad_callback(
        &self,
        ip: std::net::IpAddr,
        file_hash: String,
        ttl_ms: i64,
    ) {
        let expiry = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
            + ttl_ms;
        let mut map = self.pending_kad_callbacks.lock().unwrap();
        map.retain(|_, (_, exp)| *exp > expiry - ttl_ms);
        map.insert(ip, (file_hash, expiry));
    }

    /// Consumes a pending Kad buddy callback registration for `ip`.
    pub fn take_pending_kad_callback(&self, ip: std::net::IpAddr) -> Option<String> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let mut map = self.pending_kad_callbacks.lock().unwrap();
        map.retain(|_, (_, exp)| *exp > now);
        map.remove(&ip).map(|(h, _)| h)
    }

    /// Returns true if a pending (unexpired) Kad buddy callback exists for `ip`.
    pub fn has_pending_kad_callback(&self, ip: std::net::IpAddr) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let mut map = self.pending_kad_callbacks.lock().unwrap();
        map.retain(|_, (_, exp)| *exp > now);
        map.contains_key(&ip)
    }

    /// Sets the shared SUI anomaly counters (identity.status `sui` object).
    pub fn set_sui_stats(&self, stats: Arc<crate::services::identity::SuiStats>) {
        *self.sui_stats.write().unwrap() = stats;
    }

    /// Sets the identity service used for outbound eMule Secure User
    /// Identification (RSA-384 signing of peer challenges).
    pub fn set_identity(&self, identity: Arc<Mutex<crate::services::IdentityService>>) {
        *self.identity.write().unwrap() = Some(identity);
    }

    fn identity(&self) -> Option<Arc<Mutex<crate::services::IdentityService>>> {
        self.identity.read().unwrap().clone()
    }

    pub fn with_limits(mut self, max_peers_per_download: usize, max_global_peers: usize) -> Self {
        self.inner = Arc::new(Mutex::new(SwarmInnerState {
            sessions: HashMap::new(),
            in_flight_blocks: Vec::new(),
            last_session: None,
            max_peers_per_download,
            max_global_peers,
            peer_udp: HashMap::new(),
            queued_peers: HashMap::new(),
        }));
        self
    }

    pub fn set_kad_service(&self, kad: Arc<crate::services::KadService>) {
        let mut g = self.kad.write().unwrap();
        *g = Some(kad);
    }

    pub fn set_kad(&self, kad: Arc<crate::services::KadService>) {
        self.set_kad_service(kad);
    }

    pub async fn set_swarm_limits(&self, max_per_file: usize, max_global: usize) {
        let mut g = self.inner.lock().await;
        g.max_peers_per_download = max_per_file;
        g.max_global_peers = max_global;
    }

    pub fn kad_service(&self) -> Option<Arc<crate::services::KadService>> {
        let g = self.kad.read().unwrap();
        g.clone()
    }

    /// UDP endpoints of sessions currently `Queued` on a remote peer that
    /// announced an eMule UDP port — keep-alive targets for
    /// `OP_REASKFILEPING` (`CUpDownClient::UDPReaskForDownload`).
    /// Peers we are logically queued on — eMule `UDPReaskForDownload`
    /// requires `!(socket && socket->IsConnected())` (`DownloadClient.cpp:
    /// 1344`) : the ping is only meaningful once the queue socket died
    /// (`DS_ONQUEUE` persists with `socket == NULL`), so live sessions are
    /// excluded and the state comes from `queued_peers`.
    pub async fn queued_udp_targets(&self) -> Vec<crate::services::client_udp::UdpReaskTarget> {
        let mut g = self.inner.lock().await;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        // Drop entries whose re-ask deadline is long past — the source was
        // purged or re-dialed without re-queueing.
        g.queued_peers.retain(|_, q| {
            q.next_tcp_reask_unix_ms
                .map(|d| {
                    d > now_ms
                        - 2 * crate::services::peer_session::status_query::FILE_REASK_INTERVAL
                            .as_millis() as i64
                })
                .unwrap_or(true)
        });
        g.queued_peers
            .iter()
            // Live TCP session → eMule would not emit the UDP ping.
            .filter(|(addr, _)| !g.sessions.contains_key(*addr))
            .filter_map(|(addr_str, q)| {
                let (udp_port, ver) = g.peer_udp.get(addr_str)?;
                // eMule `UDPReaskForDownload` requires `GetUDPVersion() != 0`.
                if *ver == 0 {
                    return None;
                }
                let sock_addr = addr_str.parse::<std::net::SocketAddr>().ok()?;
                let hash_bytes = hex::decode(&q.file_hash).ok()?;
                if hash_bytes.len() != 16 {
                    return None;
                }
                let mut file_hash = [0u8; 16];
                file_hash.copy_from_slice(&hash_bytes);
                Some(crate::services::client_udp::UdpReaskTarget {
                    addr: std::net::SocketAddr::new(sock_addr.ip(), *udp_port),
                    file_hash,
                    connected_at_unix_ms: q.connected_at_unix_ms,
                    next_tcp_reask_unix_ms: q.next_tcp_reask_unix_ms,
                })
            })
            .collect()
    }

    /// eMule `m_bReaskPending` — `CUpDownClient::AskForDownload`
    /// (`DownloadClient.cpp:185-206`): the TCP re-ask of a LowID peer that
    /// is *also waiting in our upload queue* is deferred by one cycle so
    /// the peer can reach us with its own upload re-ask. Marks the
    /// `queued_peers` entry for `ip` (keyed `"ip:port"`) and pushes its
    /// re-ask deadline to `next_reask_unix_ms`.
    ///
    /// Returns `false` when no queue entry exists for `ip`+`file_hash`
    /// (eMule `GetLastAskedTime() > 0` — a never-contacted source is
    /// dialed immediately) or when the flag is already set (the deferral
    /// fires once, like `!m_bReaskPending`).
    pub async fn defer_reask_for_queued_peer(
        &self,
        ip: std::net::IpAddr,
        file_hash: &str,
        next_reask_unix_ms: i64,
    ) -> bool {
        let prefix = format!("{ip}:");
        let mut g = self.inner.lock().await;
        let mut marked = false;
        for (addr, q) in g.queued_peers.iter_mut() {
            if !addr.starts_with(&prefix)
                || !q.file_hash.eq_ignore_ascii_case(file_hash)
                || q.reask_pending
            {
                continue;
            }
            q.reask_pending = true;
            q.next_tcp_reask_unix_ms = Some(next_reask_unix_ms);
            marked = true;
        }
        marked
    }

    /// Consomme le flag `m_bReaskPending` — eMule `ConnectionEstablished`
    /// (`BaseClient.cpp:1552`): quand le pair différé ouvre une connexion
    /// vers nous, sa requête de fichier part immédiatement. Retourne
    /// l'état de file conservé (hash + deadline) pour que l'appelant
    /// déclenche le callback.
    pub async fn take_reask_pending(
        &self,
        ip: std::net::IpAddr,
    ) -> Option<crate::services::peer_session::types::QueuedPeerState> {
        let prefix = format!("{ip}:");
        let mut g = self.inner.lock().await;
        let mut found = None;
        for (addr, q) in g.queued_peers.iter_mut() {
            if addr.starts_with(&prefix) && q.reask_pending {
                q.reask_pending = false;
                found = Some(q.clone());
            }
        }
        found
    }

    /// Applies a queue rank learned via `OP_REASKACK` to the queued
    /// session of that peer (matched by IP, like
    /// `GetDownloadClientByIP_UDP`).
    pub async fn apply_udp_queue_rank(&self, ip: &std::net::IpAddr, rank: u32) {
        let ip_str = ip.to_string();
        let mut g = self.inner.lock().await;
        for s in g.sessions.values_mut() {
            let matches = s
                .snapshot
                .peer_address
                .as_ref()
                .and_then(|a| a.rsplit_once(':'))
                .map(|(host, _)| host == ip_str)
                .unwrap_or(false);
            if matches && s.snapshot.status == PeerConnectionStatus::Queued {
                s.snapshot.queue_rank = Some(rank);
                // eMule `UDPReaskACK` → `SetRemoteQueueFull(false)`.
                s.snapshot.remote_queue_full = false;
                emit_event(&self.event_tx, &self.sequence, &s.snapshot);
                tracing::info!(
                    "Peer {peer}: queue rank #{rank} updated via OP_REASKACK (UDP)",
                    peer = log_peer_ip(*ip)
                );
            }
        }
    }

    /// Applies `OP_QUEUEFULL` to the queued session of that peer — eMule
    /// `SetRemoteQueueFull(true)` + `UDPReaskACK(0)` (rank cleared to 0).
    pub async fn apply_udp_queue_full(&self, ip: &std::net::IpAddr) {
        let ip_str = ip.to_string();
        let mut g = self.inner.lock().await;
        for s in g.sessions.values_mut() {
            let matches = s
                .snapshot
                .peer_address
                .as_ref()
                .and_then(|a| a.rsplit_once(':'))
                .map(|(host, _)| host == ip_str)
                .unwrap_or(false);
            if matches && s.snapshot.status == PeerConnectionStatus::Queued {
                s.snapshot.remote_queue_full = true;
                s.snapshot.queue_rank = Some(0);
                emit_event(&self.event_tx, &self.sequence, &s.snapshot);
                tracing::info!(
                    "Peer {peer}: remote queue full via OP_QUEUEFULL (UDP)",
                    peer = log_peer_ip(*ip)
                );
            }
        }
    }

    /// Applies `OP_FILENOTFOUND` to the queued session of that peer —
    /// eMule `UDPReaskFNF` removes the download client for that file.
    pub async fn apply_udp_file_not_found(&self, ip: &std::net::IpAddr) {
        let ip_str = ip.to_string();
        let mut g = self.inner.lock().await;
        for s in g.sessions.values_mut() {
            let matches = s
                .snapshot
                .peer_address
                .as_ref()
                .and_then(|a| a.rsplit_once(':'))
                .map(|(host, _)| host == ip_str)
                .unwrap_or(false);
            if matches && s.snapshot.status == PeerConnectionStatus::Queued {
                s.snapshot.status = PeerConnectionStatus::Disconnected;
                s.snapshot.failure_reason = Some(mule_core::model::PeerFailureReason::FileNotFound);
                s.snapshot.last_error = Some("peer answered OP_FILENOTFOUND (UDP)".to_string());
                emit_event(&self.event_tx, &self.sequence, &s.snapshot);
                tracing::info!(
                    "Peer {peer}: queued session dropped via OP_FILENOTFOUND (UDP)",
                    peer = log_peer_ip(*ip)
                );
            }
        }
    }

    pub async fn active_peer_count_for_hash(&self, file_hash: &str) -> usize {
        let g = self.inner.lock().await;
        g.sessions
            .values()
            .filter(|s| s.snapshot.file_hash.as_deref() == Some(file_hash))
            .count()
    }

    /// Checks if any peer sessions are currently active.
    pub async fn is_active(&self) -> bool {
        let g = self.inner.lock().await;
        !g.sessions.is_empty()
    }

    /// Disconnects the default peer session (first active) for backward compatibility.
    pub async fn disconnect(&self) -> Result<PeerSessionSnapshot, DaemonError> {
        self.disconnect_peer(None).await
    }

    /// Spawns a background task for connecting to a peer and downloading blocks.
    pub async fn connect_peer(
        &self,
        file_hash: String,
        address: String,
        downloads: Arc<Mutex<DownloadService>>,
        policy_mgr: mule_network_policy::NetworkPolicyManager,
        client_port: u16,
        nickname: String,
        user_hash: [u8; 16],
        client_id: u32,
        server_ip: u32,
        server_port: u16,
        kad_port: u16,
        target_user_hash: Option<[u8; 16]>,
        peer_crypt_options: u8,
        obfuscation_enabled: bool,
        obfuscation_required: bool,
        obfuscation_padding_max: u8,
    ) -> Result<PeerSessionSnapshot, DaemonError> {
        let target_addr: SocketAddr = address.parse().map_err(|e| {
            DaemonError::InvalidParams(format!("Invalid peer address '{address}': {e}"))
        })?;

        // Network Policy Check
        let permit = policy_mgr
            .request_tcp_connect(
                target_addr,
                mule_network_policy::ConnectionPurpose::Ed2kPeer,
            )
            .map_err(|e| {
                DaemonError::Business(format!(
                    "Network policy rejected connection to {target_addr}: {e}"
                ))
            })?;

        self.connect(
            &file_hash,
            target_addr,
            permit,
            client_port,
            nickname,
            user_hash,
            downloads,
            client_id,
            server_ip,
            server_port,
            kad_port,
            target_user_hash,
            peer_crypt_options,
            obfuscation_enabled,
            obfuscation_required,
            obfuscation_padding_max,
            source_exchange::SxEnv::new(policy_mgr, user_hash),
        )
        .await
    }

    /// Primary connect method using an already acquired permit.
    pub async fn connect(
        &self,
        file_hash: &str,
        target_addr: SocketAddr,
        permit: SocketPermit,
        client_port: u16,
        nickname: String,
        user_hash: [u8; 16],
        downloads: Arc<Mutex<DownloadService>>,
        client_id: u32,
        server_ip: u32,
        server_port: u16,
        kad_port: u16,
        target_user_hash: Option<[u8; 16]>,
        peer_crypt_options: u8,
        obfuscation_enabled: bool,
        obfuscation_required: bool,
        obfuscation_padding_max: u8,
        sx_env: source_exchange::SxEnv,
    ) -> Result<PeerSessionSnapshot, DaemonError> {
        let address = target_addr.to_string();
        let dl_hash_bytes = hex::decode(file_hash)
            .map_err(|e| DaemonError::InvalidParams(format!("Invalid file hash hex: {e}")))?;
        if dl_hash_bytes.len() != 16 {
            return Err(DaemonError::InvalidParams(
                "File hash must be exactly 16 bytes".into(),
            ));
        }
        let mut dl_hash_arr = [0u8; 16];
        dl_hash_arr.copy_from_slice(&dl_hash_bytes);

        // Enforce max active peers
        {
            let g = self.inner.lock().await;
            g.check_session_slot(file_hash, &address)?;
        }

        let cancel_token = permit.cancellation_token().clone();

        let initial_snapshot = PeerSessionSnapshot {
            peer_address: Some(address.clone()),
            file_hash: Some(file_hash.to_string()),
            status: PeerConnectionStatus::Connecting,
            transferred_bytes: 0,
            transfer_rate_bps: 0,
            current_part: None,
            queue_rank: None,
            remote_queue_full: false,
            last_error: None,
            failure_reason: None,
            connected_at_unix_ms: None,
            next_tcp_reask_unix_ms: None,
            obfuscated: false,
        };

        // Spawn async connection handler
        let inner_clone = Arc::clone(&self.inner);
        let event_tx_clone = self.event_tx.clone();
        let seq_clone = Arc::clone(&self.sequence);
        let address_str = address.clone();
        let file_hash_str = file_hash.to_string();
        let timeouts = self.timeouts;
        let kad_svc_opt = self.kad_service();
        let identity_opt = self.identity();
        let sui_stats = self.sui_stats.read().unwrap().clone();
        let rate_limiters_opt = self.rate_limiters();

        // Span de session : `peer` pseudonymisé est joint à tous les
        // événements de la tâche dans le JSONL (corrélation sans IP brute).
        let log_addr = log_peer_addr(target_addr);
        let session_span = tracing::info_span!("peer_session", peer = %log_addr);
        let join_handle = tokio::spawn(
            async move {
                info!("Starting peer connection to {log_addr}");
                let res = connection::run_peer_connection(
                    file_hash_str.clone(),
                    dl_hash_arr,
                    address_str.clone(),
                    target_addr,
                    client_port,
                    nickname,
                    user_hash,
                    Arc::clone(&downloads),
                    cancel_token,
                    Arc::clone(&inner_clone),
                    event_tx_clone.clone(),
                    seq_clone.clone(),
                    timeouts,
                    client_id,
                    server_ip,
                    server_port,
                    kad_port,
                    kad_svc_opt,
                    target_user_hash,
                    peer_crypt_options,
                    obfuscation_enabled,
                    obfuscation_required,
                    obfuscation_padding_max,
                    identity_opt,
                    sui_stats,
                    rate_limiters_opt,
                    sx_env,
                )
                .await;

                let mut final_status = PeerConnectionStatus::Disconnected;
                let mut err_msg = None;
                let mut failure_reason = None;

                match res {
                    Ok(bytes) => {
                        info!(
                            "Peer connection to {} completed cleanly. Total transferred: {} bytes",
                            log_addr, bytes
                        );
                        final_status = PeerConnectionStatus::Disconnected;
                    }
                    Err(e) => {
                        // Churn normal du swarm (sources mortes, timeouts, file pleine) -> debug ;
                        // seules les anomalies d'intégrité restent en warn.
                        match e.reason {
                            PeerFailureReason::HashMismatch
                            | PeerFailureReason::InvalidRange
                            | PeerFailureReason::CorruptedData => {
                                warn!("Peer connection to {} ended with error: {:?}", log_addr, e);
                            }
                            _ => {
                                debug!("Peer connection to {} ended with error: {:?}", log_addr, e);
                            }
                        }
                        err_msg = Some(e.message.clone());
                        failure_reason = Some(e.reason);
                    }
                }

                // Cleanup session and free in-flight blocks
                let (final_snapshot, outcome_hash, queued_deadline) = {
                    let mut g = inner_clone.lock().await;
                    g.in_flight_blocks.retain(|b| b.peer_id != address_str);

                    if let Some(mut session) = g.sessions.remove(&address_str) {
                        // eMule `DS_ONQUEUE` survives ANY non-terminal
                        // disconnect (`BaseClient.cpp:1126-1135` : seuls
                        // DS_CONNECTED/DS_REQHASHSET → deadsource, les autres
                        // états — ONQUEUE compris — restent en file logique).
                        // L'entrée `queued_peers` n'est donc supprimée que
                        // pour les issues qui changent l'état logique :
                        // FNF → source rejetée, NNP → DS_NONEEDEDPARTS.
                        let terminal = matches!(
                            failure_reason,
                            Some(
                                PeerFailureReason::FileNotFound | PeerFailureReason::NoNeededParts
                            )
                        );
                        if terminal {
                            g.queued_peers.remove(&address_str);
                        }
                        let deadline = if session.snapshot.status == PeerConnectionStatus::Queued {
                            session.snapshot.next_tcp_reask_unix_ms
                        } else {
                            None
                        };
                        session.snapshot.status = final_status;
                        session.snapshot.transfer_rate_bps = 0;
                        session.snapshot.last_error = err_msg.clone();
                        session.snapshot.failure_reason = failure_reason;
                        g.last_session = Some(session.snapshot.clone());
                        // After an A4AF socket swap the session served another
                        // file — the outcome belongs to the file actually
                        // requested last (eMule `RemoveSource` on `m_reqfile`),
                        // not the originally-dialed one.
                        let outcome = session.snapshot.file_hash.clone();
                        (Some(session.snapshot), outcome, deadline)
                    } else {
                        (None, None, None)
                    }
                };

                if let Some(snap) = final_snapshot {
                    connection::emit_event(&event_tx_clone, &seq_clone, &snap);
                }

                // Update SQLite PeerSource record (cooldown, fail_count, status)
                {
                    let mut dl_svc = downloads.lock().await;
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    let outcome_hash = outcome_hash.as_deref().unwrap_or(&file_hash_str);

                    // Real consecutive-failure count (the UPDATE increments it on
                    // Failed) so the backoff actually escalates instead of being
                    // stuck at the first step.
                    let next_fail_count = dl_svc
                        .list_peer_sources(outcome_hash)
                        .unwrap_or_default()
                        .into_iter()
                        .find(|s| s.address == address_str)
                        .map(|s| s.fail_count + 1)
                        .unwrap_or(1);

                    let (new_status, new_cooldown) = if let Some(reason) = failure_reason {
                        match reason {
                            PeerFailureReason::FileNotFound => (PeerSourceStatus::Rejected, None),
                            // eMule "No Needed Parts" (DS_NONEEDEDPARTS) : la
                            // source est conservée et redemandée au cycle
                            // doublé `FILEREASKTIME * 2` = 58 min
                            // (`DownloadClient.cpp:1888`, `PartFile.cpp:2276`).
                            PeerFailureReason::NoNeededParts => {
                                (PeerSourceStatus::Allowed, Some(now_ms + 58 * 60 * 1000))
                            }
                            _ => {
                                if let Some(deadline) = queued_deadline {
                                    // Standard eMule queue behavior: the TCP
                                    // socket died (remote `CheckTimeOut` or our
                                    // own idle drop) while `DS_ONQUEUE` — the
                                    // client stays logically queued and is
                                    // re-asked at the FILEREASKTIME deadline
                                    // (`GetTimeUntilReask`), with a small floor
                                    // against immediate re-dial spin.
                                    (
                                        PeerSourceStatus::Allowed,
                                        Some(deadline.max(now_ms + QUEUE_LOSS_REDIAL_FLOOR_MS)),
                                    )
                                } else {
                                    let cd = calculate_backoff_cooldown_ms(next_fail_count);
                                    (PeerSourceStatus::Failed, Some(now_ms + cd))
                                }
                            }
                        }
                    } else {
                        (PeerSourceStatus::Candidate, None)
                    };

                    let _ = dl_svc.update_peer_source_status(
                        outcome_hash,
                        &address_str,
                        new_status,
                        err_msg.as_deref(),
                        new_cooldown,
                    );
                }
            }
            .instrument(session_span),
        );

        let session_handle = PeerSessionHandle {
            snapshot: initial_snapshot.clone(),
            abort_handle: join_handle.abort_handle(),
            _permit: Some(permit),
        };

        {
            let mut g = self.inner.lock().await;
            g.sessions.insert(address.clone(), session_handle);
            g.last_session = Some(initial_snapshot.clone());
        }

        connection::emit_event(&self.event_tx, &self.sequence, &initial_snapshot);

        Ok(initial_snapshot)
    }

    /// Adopts an accepted inbound connection that resulted from a LowID
    /// server callback (`OP_CALLBACKREQUEST`). The peer has already sent
    /// `OP_HELLO` and received our `OP_HELLOANSWER`; this runs the client-role
    /// status/queue/transfer pipeline on it.
    /// `source_address` is the stored peer-source key (`lowid:<id>`) used for
    /// status bookkeeping, distinct from the real socket `peer_addr`.
    #[allow(clippy::too_many_arguments)]
    pub async fn accept_callback(
        &self,
        stream: crate::services::obfuscation_stream::ObfuscatedStream<tokio::net::TcpStream>,
        peer_addr: SocketAddr,
        file_hash: &str,
        source_address: String,
        sx_caps: source_exchange::PeerSxCaps,
        peer_supports_aich: bool,
        peer_sec_ident: u8,
        peer_ext_req: u8,
        downloads: Arc<Mutex<DownloadService>>,
        cancel_token: mule_network_policy::CancellationToken,
        sx_env: source_exchange::SxEnv,
        sx_local: source_exchange::SxLocalIds,
    ) -> Result<PeerSessionSnapshot, DaemonError> {
        let address = peer_addr.to_string();
        let dl_hash_bytes = hex::decode(file_hash)
            .map_err(|e| DaemonError::InvalidParams(format!("Invalid file hash hex: {e}")))?;
        if dl_hash_bytes.len() != 16 {
            return Err(DaemonError::InvalidParams(
                "File hash must be exactly 16 bytes".into(),
            ));
        }
        let mut dl_hash_arr = [0u8; 16];
        dl_hash_arr.copy_from_slice(&dl_hash_bytes);

        {
            let g = self.inner.lock().await;
            g.check_session_slot(file_hash, &address)?;
        }

        let initial_snapshot = PeerSessionSnapshot {
            peer_address: Some(address.clone()),
            file_hash: Some(file_hash.to_string()),
            status: PeerConnectionStatus::Handshaking,
            transferred_bytes: 0,
            transfer_rate_bps: 0,
            current_part: None,
            queue_rank: None,
            remote_queue_full: false,
            last_error: None,
            failure_reason: None,
            connected_at_unix_ms: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            ),
            next_tcp_reask_unix_ms: None,
            obfuscated: stream.is_obfuscated(),
        };

        let inner_clone = Arc::clone(&self.inner);
        let event_tx_clone = self.event_tx.clone();
        let seq_clone = Arc::clone(&self.sequence);
        let address_str = address.clone();
        let file_hash_str = file_hash.to_string();
        let timeouts = self.timeouts;
        let identity_opt = self.identity();
        let sui_stats = self.sui_stats.read().unwrap().clone();
        let rate_limiters_opt = self.rate_limiters();

        let log_addr = log_peer_addr(peer_addr);
        let session_span = tracing::info_span!("peer_session", peer = %log_addr);
        let join_handle = tokio::spawn(
            async move {
            info!(
                "Starting callback peer session to {} for {}",
                log_addr, file_hash_str
            );
            let res = connection::run_callback_session(
                stream,
                peer_addr,
                file_hash_str.clone(),
                dl_hash_arr,
                address_str.clone(),
                sx_caps,
                peer_supports_aich,
                peer_sec_ident,
                peer_ext_req,
                Arc::clone(&downloads),
                cancel_token,
                Arc::clone(&inner_clone),
                event_tx_clone.clone(),
                seq_clone.clone(),
                timeouts,
                identity_opt,
                sui_stats,
                rate_limiters_opt,
                sx_env,
                sx_local,
            )
            .await;

            let mut final_status = PeerConnectionStatus::Disconnected;
            let mut err_msg = None;
            let mut failure_reason = None;

            match res {
                Ok(bytes) => {
                    info!("Callback peer session to {} completed cleanly. Total transferred: {} bytes", log_addr, bytes);
                    final_status = PeerConnectionStatus::Disconnected;
                }
                Err(e) => {
                    warn!(
                        "Callback peer session to {} ended with error: {:?}",
                        log_addr, e
                    );
                    err_msg = Some(e.message.clone());
                    failure_reason = Some(e.reason);
                }
            }

            let (final_snapshot, outcome_hash) = {
                let mut g = inner_clone.lock().await;
                g.in_flight_blocks.retain(|b| b.peer_id != address_str);

                if let Some(mut session) = g.sessions.remove(&address_str) {
                    session.snapshot.status = final_status;
                    session.snapshot.transfer_rate_bps = 0;
                    session.snapshot.last_error = err_msg.clone();
                    session.snapshot.failure_reason = failure_reason;
                    g.last_session = Some(session.snapshot.clone());
                    // Post-A4AF : l'issue s'écrit sur le fichier servi en
                    // dernier (`m_reqfile` eMule), pas sur le hash dialé.
                    let outcome = session.snapshot.file_hash.clone();
                    (Some(session.snapshot), outcome)
                } else {
                    (None, None)
                }
            };

            if let Some(snap) = final_snapshot {
                connection::emit_event(&event_tx_clone, &seq_clone, &snap);
            }

            // Update the `lowid:<id>` peer-source record.
            {
                let mut dl_svc = downloads.lock().await;
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let outcome_hash = outcome_hash.as_deref().unwrap_or(&file_hash_str);

                let next_fail_count = dl_svc
                    .list_peer_sources(outcome_hash)
                    .unwrap_or_default()
                    .into_iter()
                    .find(|s| s.address == source_address)
                    .map(|s| s.fail_count + 1)
                    .unwrap_or(1);

                let (new_status, new_cooldown) = if let Some(reason) = failure_reason {
                    match reason {
                        PeerFailureReason::FileNotFound => (PeerSourceStatus::Rejected, None),
                        // eMule DS_NONEEDEDPARTS : source conservée, reask
                        // doublé `FILEREASKTIME * 2` = 58 min.
                        PeerFailureReason::NoNeededParts => {
                            (PeerSourceStatus::Allowed, Some(now_ms + 58 * 60 * 1000))
                        }
                        _ => {
                            if err_msg.as_deref() == Some("Disconnected in queue") {
                                (PeerSourceStatus::Allowed, Some(now_ms + 10 * 60 * 1000))
                            } else {
                                let cd = calculate_backoff_cooldown_ms(next_fail_count);
                                (PeerSourceStatus::Failed, Some(now_ms + cd))
                            }
                        }
                    }
                } else {
                    (PeerSourceStatus::Candidate, None)
                };

                let _ = dl_svc.update_peer_source_status(
                    outcome_hash,
                    &source_address,
                    new_status,
                    err_msg.as_deref(),
                    new_cooldown,
                );
            }
        }
        .instrument(session_span));

        let session_handle = PeerSessionHandle {
            snapshot: initial_snapshot.clone(),
            abort_handle: join_handle.abort_handle(),
            _permit: None,
        };

        {
            let mut g = self.inner.lock().await;
            g.sessions.insert(address.clone(), session_handle);
            g.last_session = Some(initial_snapshot.clone());
        }

        connection::emit_event(&self.event_tx, &self.sequence, &initial_snapshot);

        Ok(initial_snapshot)
    }

    /// Disconnects an active peer session by address and aborts in-flight operations.
    pub async fn disconnect_peer(
        &self,
        address: Option<&str>,
    ) -> Result<PeerSessionSnapshot, DaemonError> {
        let mut g = self.inner.lock().await;
        let addr_to_remove = if let Some(a) = address {
            a.to_string()
        } else if let Some(first_key) = g.sessions.keys().next().cloned() {
            first_key
        } else {
            return Err(DaemonError::Business(
                "No active peer session to disconnect".into(),
            ));
        };

        if let Some(session) = g.sessions.remove(&addr_to_remove) {
            session.abort_handle.abort();
            g.in_flight_blocks.retain(|b| b.peer_id != addr_to_remove);

            let mut final_snap = session.snapshot;
            final_snap.status = PeerConnectionStatus::Disconnected;
            final_snap.transfer_rate_bps = 0;
            g.last_session = Some(final_snap.clone());

            connection::emit_event(&self.event_tx, &self.sequence, &final_snap);
            Ok(final_snap)
        } else {
            Err(DaemonError::Business(format!(
                "No active peer session found for '{addr_to_remove}'"
            )))
        }
    }

    /// Insère une session factice pour les tests — la tâche associée est
    /// un pending sans socket ; `abort_handle` la termine proprement.
    #[cfg(test)]
    pub async fn insert_test_session(&self, snapshot: PeerSessionSnapshot) {
        let key = snapshot
            .peer_address
            .clone()
            .unwrap_or_else(|| "test-peer".to_string());
        let jh = tokio::spawn(std::future::pending::<()>());
        self.inner.lock().await.sessions.insert(
            key,
            types::PeerSessionHandle {
                snapshot,
                abort_handle: jh.abort_handle(),
                _permit: None,
            },
        );
    }

    /// Returns a single session snapshot for backward compatibility.
    pub async fn snapshot(&self) -> PeerSessionSnapshot {
        let g = self.inner.lock().await;
        if let Some(s) = g.sessions.values().next() {
            s.snapshot.clone()
        } else if let Some(last) = &g.last_session {
            last.clone()
        } else {
            PeerSessionSnapshot::default()
        }
    }

    /// Returns the aggregated swarm snapshot, optionally filtered by file hash.
    pub async fn swarm_snapshot(&self, file_hash: Option<&str>) -> SwarmStatusSnapshot {
        let g = self.inner.lock().await;
        let peers: Vec<PeerSessionSnapshot> = g
            .sessions
            .values()
            .filter(|s| {
                if let Some(fh) = file_hash {
                    s.snapshot.file_hash.as_deref() == Some(fh)
                } else {
                    true
                }
            })
            .map(|s| s.snapshot.clone())
            .collect();

        let in_flight = if file_hash.is_some() {
            let active_peer_addrs: std::collections::HashSet<_> = peers
                .iter()
                .filter_map(|p| p.peer_address.clone())
                .collect();
            g.in_flight_blocks
                .iter()
                .filter(|b| active_peer_addrs.contains(&b.peer_id))
                .cloned()
                .collect()
        } else {
            g.in_flight_blocks.clone()
        };

        SwarmStatusSnapshot {
            file_hash: file_hash.map(|s| s.to_string()),
            active_peer_count: peers.len(),
            peers,
            in_flight_blocks: in_flight,
        }
    }

    /// Byte ranges of blocks currently requested but not yet received for a
    /// file (eMule `requestedblocks_list` — painted yellow on the status
    /// bar). Empty when the file has no active session.
    pub(crate) fn pending_ranges_locked(g: &SwarmInnerState, file_hash: &str) -> Vec<(u64, u64)> {
        g.in_flight_blocks
            .iter()
            .filter(|b| {
                g.sessions
                    .get(&b.peer_id)
                    .is_some_and(|s| s.snapshot.file_hash.as_deref() == Some(file_hash))
            })
            .map(|b| (b.start, b.end))
            .collect()
    }

    /// Non-blocking variant for sync emit paths (`broadcast_download_update`
    /// runs without async context): returns empty when the swarm state is
    /// contended — the next event refreshes it.
    pub fn try_pending_ranges(&self, file_hash: &str) -> Vec<(u64, u64)> {
        self.inner
            .try_lock()
            .map(|g| Self::pending_ranges_locked(&g, file_hash))
            .unwrap_or_default()
    }

    /// Async variant for RPC handlers (`downloads.get`).
    pub async fn pending_ranges_for_file(&self, file_hash: &str) -> Vec<(u64, u64)> {
        let g = self.inner.lock().await;
        Self::pending_ranges_locked(&g, file_hash)
    }

    /// All in-flight ranges grouped by file hash — one lock for
    /// `downloads.list`.
    pub async fn pending_ranges_by_file(&self) -> HashMap<String, Vec<(u64, u64)>> {
        let g = self.inner.lock().await;
        let mut out: HashMap<String, Vec<(u64, u64)>> = HashMap::new();
        for b in &g.in_flight_blocks {
            if let Some(hash) = g
                .sessions
                .get(&b.peer_id)
                .and_then(|s| s.snapshot.file_hash.clone())
            {
                out.entry(hash).or_default().push((b.start, b.end));
            }
        }
        out
    }

    /// Clears any cached sessions and in-flight blocks.
    pub async fn clear(&self) {
        let mut g = self.inner.lock().await;
        for (_, session) in g.sessions.drain() {
            session.abort_handle.abort();
        }
        g.in_flight_blocks.clear();
        g.last_session = None;
    }
}

#[cfg(test)]
mod tests {
    use super::types::{PeerSessionHandle, PeerSessionSnapshot, QueuedPeerState};
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn manager() -> PeerSessionManager {
        let (tx, _rx) = broadcast::channel(1);
        PeerSessionManager::new(tx, Arc::new(AtomicU64::new(0)))
    }

    const PEER: &str = "85.25.13.7:4662";

    fn queue_entry(file_hash: &str) -> QueuedPeerState {
        QueuedPeerState {
            file_hash: file_hash.to_string(),
            connected_at_unix_ms: Some(1_000),
            next_tcp_reask_unix_ms: Some(i64::MAX / 2),
            reask_pending: false,
        }
    }

    /// eMule `UDPReaskForDownload` — le ping ne part que quand la socket est
    /// morte : l'état `DS_ONQUEUE` vient de `queued_peers`, pas de la session.
    #[tokio::test]
    async fn queued_udp_targets_come_from_logical_queue_state() {
        let mgr = manager();
        let file_hash = "aa".repeat(16);
        {
            let mut g = mgr.inner.lock().await;
            g.queued_peers
                .insert(PEER.to_string(), queue_entry(&file_hash));
            g.peer_udp.insert(PEER.to_string(), (4672, 4));
        }
        let targets = mgr.queued_udp_targets().await;
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].addr,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::new(85, 25, 13, 7)), 4672)
        );
        assert_eq!(hex::encode(targets[0].file_hash), file_hash);
        assert_eq!(targets[0].connected_at_unix_ms, Some(1_000));
    }

    /// Session TCP vivante → `!(socket && socket->IsConnected())` : eMule
    /// n'émettrait pas le ping, la cible est exclue.
    #[tokio::test]
    async fn queued_udp_targets_excluded_while_session_alive() {
        let mgr = manager();
        let file_hash = "bb".repeat(16);
        {
            let mut g = mgr.inner.lock().await;
            g.queued_peers
                .insert(PEER.to_string(), queue_entry(&file_hash));
            g.peer_udp.insert(PEER.to_string(), (4672, 4));
            let task = tokio::spawn(async { std::future::pending::<()>().await });
            g.sessions.insert(
                PEER.to_string(),
                PeerSessionHandle {
                    snapshot: PeerSessionSnapshot {
                        status: PeerConnectionStatus::Queued,
                        ..Default::default()
                    },
                    abort_handle: task.abort_handle(),
                    _permit: None,
                },
            );
        }
        assert!(mgr.queued_udp_targets().await.is_empty());
    }

    /// `GetUDPVersion() == 0` → pas de ping (eMule exige une version UDP).
    #[tokio::test]
    async fn queued_udp_targets_requires_udp_version() {
        let mgr = manager();
        {
            let mut g = mgr.inner.lock().await;
            g.queued_peers
                .insert(PEER.to_string(), queue_entry(&"cc".repeat(16)));
            g.peer_udp.insert(PEER.to_string(), (4672, 0));
        }
        assert!(mgr.queued_udp_targets().await.is_empty());
    }

    /// eMule `m_bReaskPending` (`DownloadClient.cpp:193`) : le différé se
    /// pose une fois sur l'entrée `queued_peers` du pair (clé `ip:port`),
    /// pousse la deadline, puis est consommé par `take_reask_pending`
    /// (`ConnectionEstablished`, `BaseClient.cpp:1552`).
    #[tokio::test]
    async fn reask_pending_defer_and_consume() {
        let mgr = manager();
        let ip: IpAddr = "85.25.13.7".parse().unwrap();
        let file_hash = "ee".repeat(16);
        {
            let mut g = mgr.inner.lock().await;
            g.queued_peers
                .insert(PEER.to_string(), queue_entry(&file_hash));
        }

        // Jamais sollicité ou mauvais hash → pas de différé
        // (`GetLastAskedTime() > 0` exige un cycle de file déjà vécu).
        assert!(
            !mgr.defer_reask_for_queued_peer(ip, &"ff".repeat(16), 5_000)
                .await
        );

        // Premier différé → flag posé, deadline poussée.
        assert!(mgr.defer_reask_for_queued_peer(ip, &file_hash, 9_999).await);
        {
            let g = mgr.inner.lock().await;
            let q = g.queued_peers.get(PEER).unwrap();
            assert!(q.reask_pending);
            assert_eq!(q.next_tcp_reask_unix_ms, Some(9_999));
        }

        // `!m_bReaskPending` — le différé ne se repose pas : le cycle
        // suivant procède normalement.
        assert!(!mgr.defer_reask_for_queued_peer(ip, &file_hash, 9_999).await);

        // Consommation à la connexion entrante du pair.
        let taken = mgr.take_reask_pending(ip).await.expect("pending entry");
        assert_eq!(taken.file_hash, file_hash);
        assert!(!mgr.inner.lock().await.queued_peers[PEER].reask_pending);
        // Le flag est consumé — un second take ne renvoie rien.
        assert!(mgr.take_reask_pending(ip).await.is_none());
    }

    /// Entrée dont la deadline de re-ask est dépassée depuis plus de deux
    /// cycles → purgée à la lecture (source morte sans re-queue).
    #[tokio::test]
    async fn queued_udp_targets_prunes_long_expired_entries() {
        let mgr = manager();
        let stale = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
            - 3 * crate::services::peer_session::status_query::FILE_REASK_INTERVAL.as_millis()
                as i64;
        {
            let mut g = mgr.inner.lock().await;
            g.queued_peers.insert(
                PEER.to_string(),
                QueuedPeerState {
                    file_hash: "dd".repeat(16),
                    connected_at_unix_ms: Some(1_000),
                    next_tcp_reask_unix_ms: Some(stale),
                    reask_pending: false,
                },
            );
            g.peer_udp.insert(PEER.to_string(), (4672, 4));
        }
        assert!(mgr.queued_udp_targets().await.is_empty());
        assert!(mgr.inner.lock().await.queued_peers.is_empty());
    }
}
