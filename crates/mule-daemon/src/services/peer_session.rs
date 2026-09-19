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
            })),
            event_tx,
            sequence,
            timeouts: PeerTimeouts::default(),
            kad: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    pub fn with_limits(mut self, max_peers_per_download: usize, max_global_peers: usize) -> Self {
        self.inner = Arc::new(Mutex::new(SwarmInnerState {
            sessions: HashMap::new(),
            in_flight_blocks: Vec::new(),
            last_session: None,
            max_peers_per_download,
            max_global_peers,
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

    pub async fn active_peer_count_for_hash(&self, file_hash: &str) -> usize {
        let g = self.inner.lock().await;
        g.sessions.values().filter(|s| s.snapshot.file_hash.as_deref() == Some(file_hash)).count()
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
            .request_tcp_connect(target_addr, mule_network_policy::ConnectionPurpose::Ed2kPeer)
            .map_err(|e| DaemonError::Business(format!("Network policy rejected connection to {target_addr}: {e}")))?;

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
        let dl_hash_bytes = hex::decode(file_hash).map_err(|e| {
            DaemonError::InvalidParams(format!("Invalid file hash hex: {e}"))
        })?;
        if dl_hash_bytes.len() != 16 {
            return Err(DaemonError::InvalidParams("File hash must be exactly 16 bytes".into()));
        }
        let mut dl_hash_arr = [0u8; 16];
        dl_hash_arr.copy_from_slice(&dl_hash_bytes);

        // Enforce max active peers
        {
            let g = self.inner.lock().await;
            if g.sessions.len() >= g.max_global_peers {
                return Err(DaemonError::Business("Global peer connection limit reached".into()));
            }
            let file_peers_count = g.sessions.values().filter(|s| s.snapshot.file_hash.as_deref() == Some(file_hash)).count();
            if file_peers_count >= g.max_peers_per_download {
                return Err(DaemonError::Business("Maximum concurrent peer connections reached for this download".into()));
            }
            if g.sessions.contains_key(&address) {
                return Err(DaemonError::Business("Peer session is already running".into()));
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
            ).await;

            let mut final_status = PeerConnectionStatus::Disconnected;
            let mut err_msg = None;
            let mut failure_reason = None;

            match res {
                Ok(bytes) => {
                    info!("Peer connection to {} completed cleanly. Total transferred: {} bytes", address_str, bytes);
                    final_status = PeerConnectionStatus::Disconnected;
                }
                Err(e) => {
                    warn!("Peer connection to {} ended with error: {:?}", address_str, e);
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
                            let cd = calculate_backoff_cooldown_ms(1);
                            (PeerSourceStatus::Failed, Some(now_ms + cd))
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
            _permit: permit,
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
    pub async fn disconnect_peer(&self, address: Option<&str>) -> Result<PeerSessionSnapshot, DaemonError> {
        let mut g = self.inner.lock().await;
        let addr_to_remove = if let Some(a) = address {
            a.to_string()
        } else if let Some(first_key) = g.sessions.keys().next().cloned() {
            first_key
        } else {
            return Err(DaemonError::Business("No active peer session to disconnect".into()));
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
            Err(DaemonError::Business(format!("No active peer session found for '{addr_to_remove}'")))
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
            let active_peer_addrs: std::collections::HashSet<_> = peers.iter().filter_map(|p| p.peer_address.clone()).collect();
            g.in_flight_blocks.iter().filter(|b| active_peer_addrs.contains(&b.peer_id)).cloned().collect()
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
