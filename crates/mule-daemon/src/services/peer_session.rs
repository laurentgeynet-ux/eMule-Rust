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
use tracing::{info, warn};

use mule_core::model::{PeerFailureReason, PeerSourceStatus};
use mule_network_policy::SocketPermit;

use crate::error::DaemonError;
use crate::services::downloads::DownloadService;

pub mod connection;
pub mod events;
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
    pub async fn queued_udp_targets(&self) -> Vec<crate::services::client_udp::UdpReaskTarget> {
        let g = self.inner.lock().await;
        g.sessions
            .values()
            .filter(|s| s.snapshot.status == PeerConnectionStatus::Queued)
            .filter_map(|s| {
                let addr_str = s.snapshot.peer_address.as_ref()?;
                let (udp_port, _ver) = g.peer_udp.get(addr_str)?;
                let sock_addr = addr_str.parse::<std::net::SocketAddr>().ok()?;
                let hash_hex = s.snapshot.file_hash.as_ref()?;
                let hash_bytes = hex::decode(hash_hex).ok()?;
                if hash_bytes.len() != 16 {
                    return None;
                }
                let mut file_hash = [0u8; 16];
                file_hash.copy_from_slice(&hash_bytes);
                Some(crate::services::client_udp::UdpReaskTarget {
                    addr: std::net::SocketAddr::new(sock_addr.ip(), *udp_port),
                    file_hash,
                })
            })
            .collect()
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
                emit_event(&self.event_tx, &self.sequence, &s.snapshot);
                tracing::info!("Peer {ip}: queue rank #{rank} updated via OP_REASKACK (UDP)");
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
            if g.sessions.len() >= g.max_global_peers {
                return Err(DaemonError::Business(
                    "Global peer connection limit reached".into(),
                ));
            }
            let file_peers_count = g
                .sessions
                .values()
                .filter(|s| s.snapshot.file_hash.as_deref() == Some(file_hash))
                .count();
            if file_peers_count >= g.max_peers_per_download {
                return Err(DaemonError::Business(
                    "Maximum concurrent peer connections reached for this download".into(),
                ));
            }
            if g.sessions.contains_key(&address) {
                return Err(DaemonError::Business(
                    "Peer session is already running".into(),
                ));
            }
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
            last_error: None,
            failure_reason: None,
            connected_at_unix_ms: None,
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

        let join_handle = tokio::spawn(async move {
            info!("Starting peer connection to {}", target_addr);
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
            )
            .await;

            let mut final_status = PeerConnectionStatus::Disconnected;
            let mut err_msg = None;
            let mut failure_reason = None;

            match res {
                Ok(bytes) => {
                    info!(
                        "Peer connection to {} completed cleanly. Total transferred: {} bytes",
                        address_str, bytes
                    );
                    final_status = PeerConnectionStatus::Disconnected;
                }
                Err(e) => {
                    warn!(
                        "Peer connection to {} ended with error: {:?}",
                        address_str, e
                    );
                    err_msg = Some(e.message.clone());
                    failure_reason = Some(e.reason);
                }
            }

            // Cleanup session and free in-flight blocks
            let final_snapshot = {
                let mut g = inner_clone.lock().await;
                g.in_flight_blocks.retain(|b| b.peer_id != address_str);

                if let Some(mut session) = g.sessions.remove(&address_str) {
                    session.snapshot.status = final_status;
                    session.snapshot.transfer_rate_bps = 0;
                    session.snapshot.last_error = err_msg.clone();
                    session.snapshot.failure_reason = failure_reason;
                    g.last_session = Some(session.snapshot.clone());
                    Some(session.snapshot)
                } else {
                    None
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

                let (new_status, new_cooldown) = if let Some(reason) = failure_reason {
                    match reason {
                        PeerFailureReason::FileNotFound => (PeerSourceStatus::Rejected, None),
                        _ => {
                            if err_msg.as_deref() == Some("Disconnected in queue") {
                                // Standard eMule queue behavior: remote peer returned queue rank and closed TCP socket.
                                // Keep status as Allowed (not Failed) with a 10-minute re-ask cooldown.
                                (PeerSourceStatus::Allowed, Some(now_ms + 10 * 60 * 1000))
                            } else {
                                let cd = calculate_backoff_cooldown_ms(1);
                                (PeerSourceStatus::Failed, Some(now_ms + cd))
                            }
                        }
                    }
                } else {
                    (PeerSourceStatus::Candidate, None)
                };

                let _ = dl_svc.update_peer_source_status(
                    &file_hash_str,
                    &address_str,
                    new_status,
                    err_msg.as_deref(),
                    new_cooldown,
                );
            }
        });

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
        peer_supports_sx: bool,
        peer_supports_sx2: bool,
        peer_supports_aich: bool,
        peer_sec_ident: u8,
        downloads: Arc<Mutex<DownloadService>>,
        cancel_token: mule_network_policy::CancellationToken,
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
            if g.sessions.len() >= g.max_global_peers {
                return Err(DaemonError::Business(
                    "Global peer connection limit reached".into(),
                ));
            }
            let file_peers_count = g
                .sessions
                .values()
                .filter(|s| s.snapshot.file_hash.as_deref() == Some(file_hash))
                .count();
            if file_peers_count >= g.max_peers_per_download {
                return Err(DaemonError::Business(
                    "Maximum concurrent peer connections reached for this download".into(),
                ));
            }
            if g.sessions.contains_key(&address) {
                return Err(DaemonError::Business(
                    "Peer session is already running".into(),
                ));
            }
        }

        let initial_snapshot = PeerSessionSnapshot {
            peer_address: Some(address.clone()),
            file_hash: Some(file_hash.to_string()),
            status: PeerConnectionStatus::Handshaking,
            transferred_bytes: 0,
            transfer_rate_bps: 0,
            current_part: None,
            queue_rank: None,
            last_error: None,
            failure_reason: None,
            connected_at_unix_ms: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            ),
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

        let join_handle = tokio::spawn(async move {
            info!(
                "Starting callback peer session to {} for {}",
                peer_addr, file_hash_str
            );
            let res = connection::run_callback_session(
                stream,
                peer_addr,
                file_hash_str.clone(),
                dl_hash_arr,
                address_str.clone(),
                peer_supports_sx,
                peer_supports_sx2,
                peer_supports_aich,
                peer_sec_ident,
                Arc::clone(&downloads),
                cancel_token,
                Arc::clone(&inner_clone),
                event_tx_clone.clone(),
                seq_clone.clone(),
                timeouts,
                identity_opt,
                sui_stats,
                rate_limiters_opt,
            )
            .await;

            let mut final_status = PeerConnectionStatus::Disconnected;
            let mut err_msg = None;
            let mut failure_reason = None;

            match res {
                Ok(bytes) => {
                    info!("Callback peer session to {} completed cleanly. Total transferred: {} bytes", address_str, bytes);
                    final_status = PeerConnectionStatus::Disconnected;
                }
                Err(e) => {
                    warn!(
                        "Callback peer session to {} ended with error: {:?}",
                        address_str, e
                    );
                    err_msg = Some(e.message.clone());
                    failure_reason = Some(e.reason);
                }
            }

            let final_snapshot = {
                let mut g = inner_clone.lock().await;
                g.in_flight_blocks.retain(|b| b.peer_id != address_str);

                if let Some(mut session) = g.sessions.remove(&address_str) {
                    session.snapshot.status = final_status;
                    session.snapshot.transfer_rate_bps = 0;
                    session.snapshot.last_error = err_msg.clone();
                    session.snapshot.failure_reason = failure_reason;
                    g.last_session = Some(session.snapshot.clone());
                    Some(session.snapshot)
                } else {
                    None
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

                let (new_status, new_cooldown) = if let Some(reason) = failure_reason {
                    match reason {
                        PeerFailureReason::FileNotFound => (PeerSourceStatus::Rejected, None),
                        _ => {
                            if err_msg.as_deref() == Some("Disconnected in queue") {
                                (PeerSourceStatus::Allowed, Some(now_ms + 10 * 60 * 1000))
                            } else {
                                let cd = calculate_backoff_cooldown_ms(1);
                                (PeerSourceStatus::Failed, Some(now_ms + cd))
                            }
                        }
                    }
                } else {
                    (PeerSourceStatus::Candidate, None)
                };

                let _ = dl_svc.update_peer_source_status(
                    &file_hash_str,
                    &source_address,
                    new_status,
                    err_msg.as_deref(),
                    new_cooldown,
                );
            }
        });

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
