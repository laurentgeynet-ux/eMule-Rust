//! Swarm eD2K Peer Connection & In-flight Block Manager (Dual-source Multi-Peer).
//!
//! Manages up to 4 active TCP peer connections per download, and up to 20 global peer connections.
//! Enforces network policy, token bucket rate limits, instant kill-switch abortion,
//! bounded wire framing decoding (OP_HELLO, OP_HELLO_ANSWER, OP_FILESTATUSREQ, OP_FILESTATUS, OP_REQUESTPARTS, OP_SENDINGPART),
//! granular timeouts (connect, handshake, status, queue, first_data, inactivity),
//! bounded exponential backoff cooldown per source, and non-overlapping block allocation.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};
use tracing::{info, warn};

use flate2::{Decompress, FlushDecompress, Status};
use mule_core::model::{PeerFailureReason, PeerSourceStatus};
use mule_network_policy::SocketPermit;
use mule_protocol::ed2k::{
    Ed2kAnswerSources, Ed2kDecoder, Ed2kDownloadSequencer, Ed2kEncoder, Ed2kFileStatusReq,
    Ed2kHello, Ed2kRequestParts, Ed2kRequestSources, InFlightBlock, SxSource, OP_ACCEPTUPLOADREQ,
    OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_COMPRESSEDPART, OP_END_OF_DOWNLOAD,
    OP_FILEREQANSNOFILE, OP_FILESTATUS, OP_HELLO, OP_HELLO_ANSWER, OP_QUEUERANKING,
    OP_REQFILENAME, OP_REQFILENAME_ANSWER, OP_REQUESTSOURCES, OP_REQUESTSOURCES2,
    OP_SENDINGPART, OP_SETREQFILEID_EMULE, OP_STARTUPLOADREQ, PR_ED2K, PR_EMULE,
    PR_EMULE_COMPRESSED,
};
use mule_protocol::framing::PacketCodec;

pub const OP_COMPRESSEDPART_I64: u8 = 0xA1;
pub const OP_SENDINGPART_I64: u8 = 0xA2;

use crate::error::JsonRpcError;
use crate::rpc::events::DownloadSnapshot;
use crate::rpc::types::JsonRpcNotification;
use crate::services::downloads::DownloadService;

/// Maximum active peer connections per file hash (hard safety ceiling).
pub const MAX_PEERS_PER_DOWNLOAD: usize = 40;

/// Maximum active peer connections across the entire daemon.
pub const MAX_GLOBAL_PEERS: usize = 50;

/// Maximum read buffer capacity for incoming peer data (256 KiB) to prevent OOM.
pub const MAX_PEER_READ_BUFFER_SIZE: usize = 256 * 1024;

/// Configurable granular timeouts for eD2K peer connections.
#[derive(Debug, Clone, Copy)]
pub struct PeerTimeouts {
    pub connect_timeout: Duration,
    pub handshake_timeout: Duration,
    pub status_timeout: Duration,
    pub queue_timeout: Duration,
    pub first_data_timeout: Duration,
    pub inactivity_timeout: Duration,
}

impl Default for PeerTimeouts {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            handshake_timeout: Duration::from_secs(8),
            status_timeout: Duration::from_secs(20),
            queue_timeout: Duration::from_secs(900),
            first_data_timeout: Duration::from_secs(25),
            inactivity_timeout: Duration::from_secs(25),
        }
    }
}

/// Computes the exponential backoff cooldown in milliseconds based on fail_count.
/// Formule: min(1_200_000, 2^(fail_count - 1) * 60_000) (1m, 2m, 4m, 8m... max 20 min)
pub fn calculate_backoff_cooldown_ms(fail_count: u32) -> i64 {
    if fail_count == 0 {
        return 0;
    }
    let exponent = (fail_count - 1).min(5);
    let base = 60_000i64;
    let factor = 1i64 << exponent;
    (base * factor).min(1_200_000)
}

/// Current lifecycle status of an eD2K peer connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PeerConnectionStatus {
    /// No peer connection active.
    Disconnected,
    /// TCP socket connect in progress.
    Connecting,
    /// Handshake (OP_HELLO / OP_HELLO_ANSWER) in progress.
    Handshaking,
    /// Queued on remote peer (OP_QUEUERANKING / waiting for upload slot).
    Queued,
    /// Active block downloading (OP_REQUESTPARTS / OP_SENDINGPART).
    Transferring,
    /// Teardown in progress.
    Disconnecting,
}

impl PeerConnectionStatus {
    /// String representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Handshaking => "handshaking",
            Self::Queued => "queued",
            Self::Transferring => "transferring",
            Self::Disconnecting => "disconnecting",
        }
    }
}

/// Snapshot of an individual eD2K peer session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeerSessionSnapshot {
    /// Current connection status.
    pub status: PeerConnectionStatus,
    /// Target file hash in hex.
    pub file_hash: Option<String>,
    /// Remote peer address ("ip:port").
    pub peer_address: Option<String>,
    /// Currently requested part index, if transferring.
    pub current_part: Option<u16>,
    /// Transfer speed estimate in bytes/second.
    pub transfer_rate_bps: u64,
    /// Total bytes received in this session.
    pub transferred_bytes: u64,
    /// Queue rank reported by remote peer.
    pub queue_rank: Option<u32>,
    /// Last error message or failure reason.
    pub last_error: Option<String>,
    /// Normalized failure reason, if failed.
    pub failure_reason: Option<PeerFailureReason>,
    /// Timestamp when connection completed (unix ms).
    pub connected_at_unix_ms: Option<i64>,
    /// Whether this peer session is using encrypted protocol obfuscation.
    #[serde(default)]
    pub obfuscated: bool,
}

impl Default for PeerSessionSnapshot {
    fn default() -> Self {
        Self {
            status: PeerConnectionStatus::Disconnected,
            file_hash: None,
            peer_address: None,
            current_part: None,
            transfer_rate_bps: 0,
            transferred_bytes: 0,
            queue_rank: None,
            last_error: None,
            failure_reason: None,
            connected_at_unix_ms: None,
            obfuscated: false,
        }
    }
}

/// Aggregated swarm status for a specific file or globally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwarmStatusSnapshot {
    /// Target file hash if filtered by file.
    pub file_hash: Option<String>,
    /// Total active peer connections.
    pub active_peer_count: usize,
    /// Active individual peer sessions.
    pub peers: Vec<PeerSessionSnapshot>,
    /// All active non-expired in-flight block ranges.
    pub in_flight_blocks: Vec<InFlightBlock>,
}

struct PeerSessionHandle {
    snapshot: PeerSessionSnapshot,
    abort_handle: tokio::task::AbortHandle,
    _permit: SocketPermit,
}

struct SwarmInnerState {
    /// Active sessions indexed by peer address ("ip:port").
    sessions: HashMap<String, PeerSessionHandle>,
    /// Active in-flight blocks allocated to peers.
    in_flight_blocks: Vec<InFlightBlock>,
    /// Most recent peer session snapshot (active or disconnected), preserved for inspections.
    last_session: Option<PeerSessionSnapshot>,
    /// Maximum active peer connections per file hash.
    max_peers_per_download: usize,
    /// Maximum active peer connections across the entire daemon.
    max_global_peers: usize,
}

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

    /// Sets the Kad service reference for auto-bootstrapping discovered peers.
    pub fn set_kad(&self, kad: Arc<crate::services::KadService>) {
        if let Ok(mut k) = self.kad.write() {
            *k = Some(kad);
        }
    }

    /// Sets custom timeouts.
    pub fn with_timeouts(mut self, timeouts: PeerTimeouts) -> Self {
        self.timeouts = timeouts;
        self
    }

    /// Configures initial swarm connection limits.
    pub fn with_limits(self, max_per_download: usize, max_global: usize) -> Self {
        if let Ok(mut guard) = self.inner.try_lock() {
            guard.max_peers_per_download = max_per_download;
            guard.max_global_peers = max_global;
        }
        self
    }

    /// Sets swarm connection limits (for test fixtures or custom configs).
    pub async fn set_swarm_limits(&self, max_per_download: usize, max_global: usize) {
        let mut guard = self.inner.lock().await;
        guard.max_peers_per_download = max_per_download;
        guard.max_global_peers = max_global;
    }

    /// Returns a single session snapshot for backward-compatible single peer inspections.
    pub async fn snapshot(&self) -> PeerSessionSnapshot {
        let guard = self.inner.lock().await;
        guard
            .sessions
            .values()
            .next()
            .map(|h| h.snapshot.clone())
            .or_else(|| guard.last_session.clone())
            .unwrap_or_default()
    }

    /// Returns a full swarm snapshot for a specific file hash, or all active peers.
    pub async fn swarm_snapshot(&self, file_hash: Option<&str>) -> SwarmStatusSnapshot {
        let guard = self.inner.lock().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        let peers: Vec<PeerSessionSnapshot> = guard
            .sessions
            .values()
            .filter(|h| {
                if let Some(hash) = file_hash {
                    h.snapshot.file_hash.as_deref() == Some(hash)
                } else {
                    true
                }
            })
            .map(|h| h.snapshot.clone())
            .collect();

        let in_flight_blocks: Vec<InFlightBlock> = guard
            .in_flight_blocks
            .iter()
            .filter(|ifb| !ifb.is_expired(now))
            .cloned()
            .collect();

        SwarmStatusSnapshot {
            file_hash: file_hash.map(|s| s.to_string()),
            active_peer_count: peers.len(),
            peers,
            in_flight_blocks,
        }
    }

    /// Returns true if at least one peer connection is active.
    pub async fn is_active(&self) -> bool {
        let guard = self.inner.lock().await;
        !guard.sessions.is_empty()
    }

    /// Returns active peer count for a specific file hash.
    pub async fn active_peer_count_for_hash(&self, file_hash: &str) -> usize {
        let guard = self.inner.lock().await;
        guard
            .sessions
            .values()
            .filter(|h| h.snapshot.file_hash.as_deref() == Some(file_hash))
            .count()
    }

    /// Disconnects all active peer sessions or a specific peer address if specified.
    pub async fn disconnect(&self) -> Result<(), JsonRpcError> {
        self.disconnect_peer(None).await
    }

    /// Disconnects a specific peer by address or all active peers if address is None.
    pub async fn disconnect_peer(&self, address: Option<&str>) -> Result<(), JsonRpcError> {
        let mut disconnected_snapshots = Vec::new();
        {
            let mut guard = self.inner.lock().await;
            if let Some(addr) = address {
                if let Some(handle) = guard.sessions.remove(addr) {
                    handle.abort_handle.abort();
                    guard.in_flight_blocks.retain(|b| b.peer_id != addr);
                    disconnected_snapshots.push(PeerSessionSnapshot {
                        status: PeerConnectionStatus::Disconnected,
                        file_hash: handle.snapshot.file_hash,
                        peer_address: Some(addr.to_string()),
                        ..Default::default()
                    });
                }
            } else {
                for (addr, handle) in guard.sessions.drain() {
                    handle.abort_handle.abort();
                    disconnected_snapshots.push(PeerSessionSnapshot {
                        status: PeerConnectionStatus::Disconnected,
                        file_hash: handle.snapshot.file_hash,
                        peer_address: Some(addr),
                        ..Default::default()
                    });
                }
                guard.in_flight_blocks.clear();
            }
        }
        for snap in disconnected_snapshots {
            emit_event(&self.event_tx, &self.sequence, &snap);
        }
        Ok(())
    }

    /// Connects to a specific peer source for a given file hash.
    /// Rejects if limits are exceeded (max 2 per file, max 4 global) or already connected.
    pub async fn connect(
        &self,
        file_hash_hex: &str,
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
    ) -> Result<(), JsonRpcError> {
        let address_str = target_addr.to_string();
        let mut guard = self.inner.lock().await;

        let max_global = guard.max_global_peers;
        if guard.sessions.len() >= max_global {
            return Err(JsonRpcError::business(format!(
                "Global peer connection limit reached ({max_global} max)"
            )));
        }

        let file_active_count = guard
            .sessions
            .values()
            .filter(|h| h.snapshot.file_hash.as_deref() == Some(file_hash_hex))
            .count();

        let max_per_dl = guard.max_peers_per_download;
        if file_active_count >= max_per_dl {
            return Err(JsonRpcError::business(format!(
                "Maximum concurrent peer connections reached for this download ({max_per_dl} max)"
            )));
        }

        if guard.sessions.contains_key(&address_str) {
            return Err(JsonRpcError::business(format!(
                "Already connected to peer {address_str}"
            )));
        }

        let dl_hash_bytes = parse_hex_16(file_hash_hex).ok_or_else(|| {
            JsonRpcError::invalid_params(&format!("Invalid file hash '{file_hash_hex}'"))
        })?;

        let initial_snapshot = PeerSessionSnapshot {
            status: PeerConnectionStatus::Connecting,
            file_hash: Some(file_hash_hex.to_string()),
            peer_address: Some(address_str.clone()),
            current_part: None,
            transfer_rate_bps: 0,
            transferred_bytes: 0,
            queue_rank: None,
            last_error: None,
            failure_reason: None,
            connected_at_unix_ms: None,
            obfuscated: false,
        };

        let inner_clone = Arc::clone(&self.inner);
        let event_tx_clone = self.event_tx.clone();
        let sequence_clone = Arc::clone(&self.sequence);
        let cancel_token = permit.cancellation_token();
        let file_hash_str = file_hash_hex.to_string();
        let timeouts = self.timeouts;
        let addr_for_task = address_str.clone();
        let kad_service = self.kad.read().ok().and_then(|k| k.clone());

        let task = tokio::spawn(async move {
            let result = run_peer_connection(
                file_hash_str.clone(),
                dl_hash_bytes,
                addr_for_task.clone(),
                target_addr,
                client_port,
                nickname,
                user_hash,
                downloads.clone(),
                cancel_token.clone(),
                inner_clone.clone(),
                event_tx_clone.clone(),
                sequence_clone.clone(),
                timeouts,
                client_id,
                server_ip,
                server_port,
                kad_port,
                kad_service,
                target_user_hash,
                peer_crypt_options,
                obfuscation_enabled,
                obfuscation_required,
                obfuscation_padding_max,
            )
            .await;

            // Handle final teardown, cooldown calculation, and status update
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            let (final_status, reason, err_msg) = match result {
                Ok(bytes_received) => {
                    info!("Peer session with {} finished cleanly. Received {} bytes.", target_addr, bytes_received);
                    (PeerSourceStatus::Tried, None, None)
                }
                Err(e) if e.reason == PeerFailureReason::KillSwitchAborted => {
                    info!("Peer session with {} was aborted by Kill Switch / user action without peer penalty.", target_addr);
                    (PeerSourceStatus::Tried, Some(e.reason), Some(e.message))
                }
                Err(e) if e.reason == PeerFailureReason::TimeoutStatus => {
                    info!("Peer session with {} timed out waiting for status/queue (peer may be busy). Gentle retry scheduled.", target_addr);
                    (PeerSourceStatus::Tried, Some(e.reason), Some(e.message))
                }
                Err(e) => {
                    warn!("Peer session with {} ended with failure: [{:?}] {}", target_addr, e.reason, e.message);
                    (PeerSourceStatus::Failed, Some(e.reason), Some(e.message))
                }
            };

            // Update SQLite persistence with fail_count and cooldown
            {
                let mut dl_svc = downloads.lock().await;
                let sources = dl_svc.list_peer_sources(&file_hash_str).unwrap_or_default();
                let current_source = sources.iter().find(|s| s.address == addr_for_task);
                let current_fail_count = current_source.map(|s| s.fail_count).unwrap_or(0);
                let new_fail_count = if final_status == PeerSourceStatus::Failed {
                    current_fail_count + 1
                } else {
                    current_fail_count
                };

                let cooldown_ms = calculate_backoff_cooldown_ms(new_fail_count);
                let cooldown_until = if final_status == PeerSourceStatus::Failed && cooldown_ms > 0 {
                    Some(now + cooldown_ms)
                } else if final_status == PeerSourceStatus::Tried {
                    // For files with scarce sources (< 15), re-ask in 3 minutes (180s) or 45s for status timeouts
                    let reask_ms = if reason == Some(PeerFailureReason::TimeoutStatus) {
                        45 * 1000 // 45 seconds gentle retry for busy peers
                    } else if sources.len() < 15 {
                        3 * 60 * 1000 // 3 minutes for scarce swarms
                    } else {
                        15 * 60 * 1000 // Standard eMule 15 minutes for large swarms
                    };
                    Some(now + reask_ms)
                } else {
                    None
                };

                let _ = dl_svc.update_peer_source_status(
                    &file_hash_str,
                    &addr_for_task,
                    final_status,
                    err_msg.as_deref(),
                    cooldown_until,
                );
            }

            // Remove from swarm sessions and clear any in-flight blocks for this peer
            let final_snapshot = {
                let mut g = inner_clone.lock().await;
                g.sessions.remove(&addr_for_task);
                g.in_flight_blocks.retain(|b| b.peer_id != addr_for_task);
                let snap = PeerSessionSnapshot {
                    status: PeerConnectionStatus::Disconnected,
                    file_hash: Some(file_hash_str),
                    peer_address: Some(addr_for_task),
                    last_error: err_msg,
                    failure_reason: reason,
                    ..Default::default()
                };
                g.last_session = Some(snap.clone());
                snap
            };

            emit_event(&event_tx_clone, &sequence_clone, &final_snapshot);
        });

        guard.sessions.insert(
            address_str,
            PeerSessionHandle {
                snapshot: initial_snapshot.clone(),
                abort_handle: task.abort_handle(),
                _permit: permit,
            },
        );
        drop(guard);

        emit_event(&self.event_tx, &self.sequence, &initial_snapshot);

        Ok(())
    }
}

struct PeerError {
    reason: PeerFailureReason,
    message: String,
}

impl PeerError {
    fn new(reason: PeerFailureReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

async fn run_peer_connection(
    file_hash_str: String,
    dl_hash_bytes: [u8; 16],
    address_str: String,
    target_addr: SocketAddr,
    client_port: u16,
    nickname: String,
    user_hash: [u8; 16],
    downloads: Arc<Mutex<DownloadService>>,
    cancel_token: mule_network_policy::CancellationToken,
    inner: Arc<Mutex<SwarmInnerState>>,
    event_tx: broadcast::Sender<String>,
    sequence: Arc<AtomicU64>,
    timeouts: PeerTimeouts,
    client_id: u32,
    server_ip: u32,
    server_port: u16,
    kad_port: u16,
    kad_service: Option<Arc<crate::services::KadService>>,
    target_user_hash: Option<[u8; 16]>,
    peer_crypt_options: u8,
    obfuscation_enabled: bool,
    obfuscation_required: bool,
    obfuscation_padding_max: u8,
) -> Result<u64, PeerError> {
    // 1. TCP connect with timeout & cancel_token
    let stream_res = tokio::select! {
        _ = cancel_token.cancelled() => {
            return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Cancelled by network policy kill switch"));
        }
        res = tokio::time::timeout(timeouts.connect_timeout, TcpStream::connect(target_addr)) => {
            match res {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("TCP connect refused: {e}"))),
                Err(_) => return Err(PeerError::new(PeerFailureReason::TimeoutConnect, "TCP connect timed out")),
            }
        }
    };

    let tcp_stream = stream_res;
    let connected_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Obfuscation decision:
    let peer_requires_crypt = (peer_crypt_options & 0x04) != 0;
    let peer_supports_crypt = (peer_crypt_options & 0x01) != 0 || (peer_crypt_options & 0x02) != 0;

    let use_obfuscation = if obfuscation_required || peer_requires_crypt {
        if target_user_hash.is_none() {
            return Err(PeerError::new(
                PeerFailureReason::MissingUserHashForObfuscation,
                "Peer requires protocol obfuscation but its user hash is unknown",
            ));
        }
        true
    } else if obfuscation_enabled && peer_supports_crypt && target_user_hash.is_some() {
        true
    } else {
        false
    };

    let mut stream = if use_obfuscation {
        let th = target_user_hash.unwrap();
        tracing::info!(
            "Initiating obfuscated handshake to peer {target_addr} with target user hash {:02X?}",
            &th
        );
        match crate::services::obfuscation_stream::ObfuscatedStream::connect_peer_outgoing(
            tcp_stream,
            &th,
            obfuscation_padding_max,
        )
        .await
        {
            Ok(s) => s,
            Err(e) => {
                if !obfuscation_required && !peer_requires_crypt {
                    tracing::warn!(
                        "Obfuscated handshake to peer {target_addr} failed ({e}), falling back to plaintext connection"
                    );
                    match tokio::time::timeout(timeouts.connect_timeout, TcpStream::connect(target_addr)).await {
                        Ok(Ok(plain_stream)) => {
                            crate::services::obfuscation_stream::ObfuscatedStream::plaintext(plain_stream)
                        }
                        Ok(Err(reconnect_err)) => {
                            return Err(PeerError::new(
                                PeerFailureReason::ConnectRefused,
                                format!("Plaintext fallback connect failed: {reconnect_err}"),
                            ));
                        }
                        Err(_) => {
                            return Err(PeerError::new(
                                PeerFailureReason::TimeoutConnect,
                                "Plaintext fallback connect timed out",
                            ));
                        }
                    }
                } else {
                    tracing::warn!("Obfuscated handshake to peer {target_addr} failed: {e}");
                    return Err(PeerError::new(
                        PeerFailureReason::ObfuscationNegotiationFailed,
                        format!("Protocol obfuscation negotiation failed: {e}"),
                    ));
                }
            }
        }
    } else {
        crate::services::obfuscation_stream::ObfuscatedStream::plaintext(tcp_stream)
    };

    // Update status: Handshaking
    let handshaking_snapshot = {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(&address_str) {
            session.snapshot.status = PeerConnectionStatus::Handshaking;
            session.snapshot.connected_at_unix_ms = Some(connected_ms);
            session.snapshot.obfuscated = stream.is_obfuscated();
            Some(session.snapshot.clone())
        } else {
            None
        }
    };
    if let Some(snap) = handshaking_snapshot {
        emit_event(&event_tx, &sequence, &snap);
    }

    // 2. Send OP_HELLO with full official eMule tags (Name, Version, UDPPorts, EmuleVersion, MiscOptions1, MiscOptions2)
    let encoder = Ed2kEncoder::default();
    let mut tags = Vec::with_capacity(6);

    // 1. CT_NAME (0x01): Nickname
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_NAME),
        value: mule_protocol::ed2k::tags::TagValue::String(nickname),
    });

    // 2. CT_VERSION (0x11): EDONKEYVERSION = 60 (0x3C)
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_VERSION),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(0x3C),
    });

    // 3. CT_EMULE_UDPPORTS (0xF9): (kad_port << 16) | client_port
    let udp_ports = ((kad_port as u32) << 16) | (client_port as u32);
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_UDPPORTS),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(udp_ports),
    });

    // 4. CT_EMULE_VERSION (0xFB): (Mjr << 17) | (Min << 10) | (Upd << 7)
    // For eMule 0.50a: (0 << 17) | (50 << 10) | (0 << 7) = 51200 (0x0000C800)
    let emule_ver: u32 = (0 << 17) | (50 << 10) | (0 << 7);
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_CT_EMULE_VERSION),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(emule_ver),
    });

    // 5. CT_EMULE_MISCOPTIONS1 (0xFA): Standard eMule features
    // nAICHVer = 1 (1 << 29)
    // uUnicodeSupport = 1 (1 << 28)
    // uUdpVer = 4 (4 << 24)
    // uDataCompVer = 1 (1 << 20)
    // uSourceExchange1Ver = 4 (4 << 12)
    // uExtendedRequestsVer = 0 (0 << 8) - use 0 to signal canonical standalone requests
    // uAcceptCommentVer = 1 (1 << 4)
    // uMultiPacket = 0 (0 << 1) - single packet mode
    // uSupportPreview = 1 (1 << 0)
    let misc1: u32 = (1 << 29)
        | (1 << 28)
        | (4 << 24)
        | (1 << 20)
        | (4 << 12)
        | (0 << 8)
        | (1 << 4)
        | (0 << 1)
        | 1;
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS1),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(misc1),
    });

    // 6. CT_EMULE_MISCOPTIONS2 (0xFE):
    // Layout (from eMule BaseClient.cpp):
    //   bit 13: uFileIdentifiers (0)
    //   bit 12: uDirectUDPCallback (not supported yet → 0)
    //   bit 11: uSupportsCaptcha (0)
    //   bit 10: uSupportsSourceEx2
    //   bit  9: uRequiresCryptLayer  ← obfuscation_required
    //   bit  8: uRequestsCryptLayer  ← obfuscation_enabled (we prefer it when available)
    //   bit  7: uSupportsCryptLayer  ← obfuscation_enabled (we can do it)
    //   bit  6: reserved
    //   bit  5: uExtMultiPacket (0)
    //   bit  4: uSupportLargeFiles
    //   bits 3-0: uKadVersion = 2
    //
    // Advertising CryptLayer support correctly is critical: peers that require obfuscation
    // will immediately RST the connection if we claim not to support it (bit 7 = 0).
    let u_supports_crypt = obfuscation_enabled as u32;
    let u_requests_crypt = obfuscation_enabled as u32; // prefer obfuscation when we support it
    let u_requires_crypt = obfuscation_required as u32;
    let misc2: u32 = (0 << 13)
        | (0 << 11)
        | (1 << 10)
        | (u_requires_crypt << 9)
        | (u_requests_crypt << 8)
        | (u_supports_crypt << 7)
        | (0 << 5)
        | (1 << 4)
        | 2;
    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS2),
        value: mule_protocol::ed2k::tags::TagValue::UInt32(misc2),
    });

    let hello = Ed2kHello {
        user_hash,
        client_id,
        tcp_port: client_port,
        tags,
        unknown_tags: vec![],
        server_ip,
        server_port,
    };

    let hello_pkt = encoder.encode_hello(&hello).map_err(|e| {
        PeerError::new(PeerFailureReason::InvalidHandshake, format!("Failed to encode OP_HELLO: {e}"))
    })?;

    let mut out_wire = Vec::with_capacity(6 + hello_pkt.payload.len());
    out_wire.push(hello_pkt.protocol);
    out_wire.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(hello_pkt.opcode);
    out_wire.extend_from_slice(&hello_pkt.payload);

    tracing::info!(
        "Peer {target_addr}: sending OP_HELLO (client_id={client_id}, port={client_port}, server={server_ip}:{server_port}, len={}): {:02X?}",
        out_wire.len(),
        &out_wire[..]
    );

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_HELLO: {e}")));
    }

    // 3. Receive OP_HELLO_ANSWER with Handshake Timeout
    let decoder = Ed2kDecoder::default();
    let codec = PacketCodec::new();
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let mut received_hello_answer = false;
    let mut peer_supports_sx = false;
    let mut peer_supports_sx2 = false;

    let handshake_deadline = tokio::time::Instant::now() + timeouts.handshake_timeout;
    while !received_hello_answer {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Handshake aborted by kill switch"));
            }
            _ = tokio::time::sleep_until(handshake_deadline) => {
                return Err(PeerError::new(PeerFailureReason::TimeoutHandshake, "Handshake timed out waiting for OP_HELLO_ANSWER"));
            }
            read_res = stream.read(&mut raw_buf) => {
                match read_res {
                    Ok(0) => {
                        let buffered_len = read_buf.len();
                        let snippet = if buffered_len > 0 {
                            format!("{:02X?}", &read_buf[..buffered_len.min(32)])
                        } else {
                            "none".to_string()
                        };
                        tracing::warn!(
                            "Peer {target_addr} closed connection during handshake (bytes buffered={buffered_len}: {snippet})"
                        );
                        return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Peer closed connection during handshake"));
                    }
                    Ok(n) => {
                        tracing::debug!(
                            "Peer {target_addr}: read {n} bytes during handshake: {:02X?}",
                            &raw_buf[..n.min(32)]
                        );
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::InvalidHandshake, "Peer read buffer exceeded limit during handshake"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            tracing::debug!(
                                "Peer {target_addr}: decoded packet proto=0x{:02X} opcode=0x{:02X} payload_len={}",
                                packet.protocol,
                                packet.opcode,
                                packet.payload.len()
                            );
                            if packet.protocol == PR_ED2K && (packet.opcode == OP_HELLO || packet.opcode == OP_HELLO_ANSWER) {
                                // Validate handshake packet
                                match decoder.decode_hello_answer(packet.protocol, packet.opcode, &packet.payload, 0) {
                                    Ok(ans) => {
                                        let peer_info = mule_protocol::ed2k::Ed2kPeerInfo::from_hello_answer(&ans);
                                        peer_supports_sx = peer_info.supports_source_exchange();
                                        peer_supports_sx2 = peer_info.supports_source_exchange2();

                                        let mut peer_opts = 0u8;
                                        if peer_info.supports_crypt_layer() { peer_opts |= 0x01; }
                                        if peer_info.requests_crypt_layer() { peer_opts |= 0x02; }
                                        if peer_info.requires_crypt_layer() { peer_opts |= 0x04; }

                                        let hex_user_hash = hex::encode(ans.user_hash);
                                        let updated_src = mule_core::model::PeerSource {
                                            file_hash: file_hash_str.clone(),
                                            address: address_str.clone(),
                                            ip: match target_addr {
                                                SocketAddr::V4(v4) => u32::from_be_bytes(v4.ip().octets()),
                                                _ => 0,
                                             },
                                            port: target_addr.port(),
                                            origin: mule_core::model::PeerSourceOrigin::Manual,
                                            discovered_at_unix_ms: connected_ms,
                                            last_seen_unix_ms: connected_ms,
                                            status: PeerSourceStatus::Candidate,
                                            fail_count: 0,
                                            last_error: None,
                                            cooldown_until_unix_ms: None,
                                            user_hash: Some(hex_user_hash),
                                            crypt_options: peer_opts,
                                        };
                                        let _ = downloads.lock().await.upsert_peer_sources(&file_hash_str, &[updated_src]);

                                        for tag in &ans.tags {
                                            if let mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_UDPPORTS) = tag.name {
                                                if let mule_protocol::ed2k::tags::TagValue::UInt32(ports) = tag.value {
                                                    let peer_kad_port = (ports >> 16) as u16;
                                                    if peer_kad_port > 0 {
                                                        tracing::info!("Peer {target_addr} advertises live Kad UDP port {peer_kad_port}");
                                                        if let Some(ref kad) = kad_service {
                                                            let k_clone = Arc::clone(kad);
                                                            let ip_str = target_addr.ip().to_string();
                                                            tokio::spawn(async move {
                                                                let _ = k_clone.bootstrap_from_node(&ip_str, peer_kad_port).await;
                                                            });
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        return Err(PeerError::new(PeerFailureReason::InvalidHandshake, format!("Invalid OP_HELLO_ANSWER: {e}")));
                                    }
                                }
                                received_hello_answer = true;
                                break;
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error during handshake: {e}"))),
                }
            }
        }
    }

    info!("Peer {} handshake completed. Requesting file status...", target_addr);

    // 4a. Send OP_REQFILENAME (0x58) first, exactly as canonical eMule does in DownloadClient.cpp.
    // Standard eMule and DLP modules require OP_REQFILENAME before OP_SETREQFILEID,
    // otherwise DLP flags the client as an unauthorized crawler and closes connection (10054).
    let file_status_req = Ed2kFileStatusReq {
        file_hash: dl_hash_bytes,
    };
    let req_filename_pkt = encoder.encode_file_status_req_with_opcode(&file_status_req, OP_REQFILENAME).map_err(|e| {
        PeerError::new(PeerFailureReason::InvalidHandshake, format!("Failed to encode OP_REQFILENAME: {e}"))
    })?;

    out_wire.clear();
    out_wire.push(req_filename_pkt.protocol);
    out_wire.extend_from_slice(&(req_filename_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(req_filename_pkt.opcode);
    out_wire.extend_from_slice(&req_filename_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_REQFILENAME: {e}")));
    }

    // 4b. Send OP_SETREQFILEID_EMULE (0x4F) for parts status / availability
    let status_pkt = encoder.encode_file_status_req_with_opcode(&file_status_req, OP_SETREQFILEID_EMULE).map_err(|e| {
        PeerError::new(PeerFailureReason::InvalidHandshake, format!("Failed to encode OP_SETREQFILEID_EMULE: {e}"))
    })?;

    out_wire.clear();
    out_wire.push(status_pkt.protocol);
    out_wire.extend_from_slice(&(status_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(status_pkt.opcode);
    out_wire.extend_from_slice(&status_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_SETREQFILEID_EMULE: {e}")));
    }

    // 4c. Send OP_REQUESTSOURCES / OP_REQUESTSOURCES2 if remote peer supports Source Exchange
    if peer_supports_sx2 || peer_supports_sx {
        let sx_req = if peer_supports_sx2 {
            Ed2kRequestSources::new_v2(dl_hash_bytes)
        } else {
            Ed2kRequestSources::new_v1(dl_hash_bytes)
        };
        if let Ok(sx_pkt) = encoder.encode_request_sources(&sx_req) {
            out_wire.clear();
            out_wire.push(sx_pkt.protocol);
            out_wire.extend_from_slice(&(sx_pkt.frame_len() as u32).to_le_bytes());
            out_wire.push(sx_pkt.opcode);
            out_wire.extend_from_slice(&sx_pkt.payload);
            if let Err(e) = stream.write_all(&out_wire).await {
                tracing::warn!("Failed to send SX request to {target_addr}: {e}");
            } else {
                tracing::info!("Sent OP_REQUESTSOURCES (version {}) to peer {target_addr}", sx_req.version);
            }
        }
    }

    let queued_snapshot = {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(&address_str) {
            session.snapshot.status = PeerConnectionStatus::Queued;
            Some(session.snapshot.clone())
        } else {
            None
        }
    };
    if let Some(snap) = queued_snapshot {
        emit_event(&event_tx, &sequence, &snap);
    }

    // 5. Await OP_FILESTATUS or OP_FILEREQANSNOFILE with Status Timeout
    let status_deadline = tokio::time::Instant::now() + timeouts.status_timeout;
    let mut file_status_opt = None;

    while file_status_opt.is_none() {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Connection aborted by kill switch"));
            }
            _ = tokio::time::sleep_until(status_deadline) => {
                return Err(PeerError::new(PeerFailureReason::TimeoutStatus, "Timed out responding to OP_FILESTATUSREQ"));
            }
            read_res = stream.read(&mut raw_buf) => {
                match read_res {
                    Ok(0) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Peer closed connection while awaiting file status")),
                    Ok(n) => {
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::CorruptedData, "Peer read buffer exceeded limit awaiting file status"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            if packet.protocol == PR_ED2K || packet.protocol == PR_EMULE {
                                if packet.opcode == OP_FILESTATUS {
                                    // Decode and verify file hash
                                    let status_res = decoder.decode_file_status(packet.protocol, packet.opcode, &packet.payload, 0);
                                    match status_res {
                                        Ok(status) => {
                                            if status.file_hash != dl_hash_bytes {
                                                return Err(PeerError::new(
                                                    PeerFailureReason::HashMismatch,
                                                    "OP_FILESTATUS returned unexpected file hash".to_string(),
                                                ));
                                            }
                                            file_status_opt = Some(status);
                                            break;
                                        }
                                        Err(e) => return Err(PeerError::new(PeerFailureReason::CorruptedData, format!("Malformed OP_FILESTATUS: {e}"))),
                                    }
                                } else if packet.opcode == OP_REQFILENAME_ANSWER {
                                    tracing::debug!("Peer {target_addr} sent OP_REQFILENAME_ANSWER (len={})", packet.payload.len());
                                } else if packet.opcode == OP_QUEUERANKING {
                                    let rank = if packet.payload.len() >= 4 {
                                        u32::from_le_bytes(packet.payload[..4].try_into().unwrap_or([0, 0, 0, 0]))
                                    } else if packet.payload.len() >= 2 {
                                        u16::from_le_bytes(packet.payload[..2].try_into().unwrap_or([0, 0])) as u32
                                    } else {
                                        0
                                    };
                                    tracing::info!("Peer {target_addr} responded with queue ranking {rank} during status query");
                                    let queue_update = {
                                        let mut g = inner.lock().await;
                                        if let Some(session) = g.sessions.get_mut(&address_str) {
                                            session.snapshot.queue_rank = Some(rank);
                                            Some(session.snapshot.clone())
                                        } else {
                                            None
                                        }
                                    };
                                    if let Some(snap) = queue_update {
                                        emit_event(&event_tx, &sequence, &snap);
                                    }
                                    file_status_opt = Some(mule_protocol::ed2k::Ed2kFileStatus {
                                        file_hash: dl_hash_bytes,
                                        part_count: 0,
                                        availability_mask: mule_protocol::ed2k::PartAvailabilityMask::from_parts(&[], 0),
                                    });
                                    break;
                                } else if packet.opcode == OP_ACCEPTUPLOADREQ {
                                    tracing::info!("Peer {target_addr} directly accepted upload slot (OP_ACCEPTUPLOADREQ) during status query!");
                                    file_status_opt = Some(mule_protocol::ed2k::Ed2kFileStatus {
                                        file_hash: dl_hash_bytes,
                                        part_count: 0,
                                        availability_mask: mule_protocol::ed2k::PartAvailabilityMask::from_parts(&[], 0),
                                    });
                                    break;
                                } else if packet.opcode == OP_FILEREQANSNOFILE {
                                    return Err(PeerError::new(PeerFailureReason::FileNotFound, "Remote peer does not share this file (OP_FILEREQANSNOFILE)"));
                                } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
                                    && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
                                {
                                    handle_sx_answer(&packet, dl_hash_bytes, &file_hash_str, target_addr, &decoder, &downloads).await;
                                } else if packet.protocol == PR_EMULE
                                    && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
                                {
                                    handle_sx_request(&packet, dl_hash_bytes, &file_hash_str, target_addr, &decoder, &encoder, &downloads, &mut stream).await;
                                }
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error awaiting file status: {e}"))),
                }
            }
        }
    }

    let file_status = file_status_opt.unwrap();

    // Mark source as Connected in database
    {
        let mut dl_svc = downloads.lock().await;
        let _ = dl_svc.update_peer_source_status(
            &file_hash_str,
            &address_str,
            PeerSourceStatus::Connected,
            None,
            None,
        );
    }

    // 6. Send OP_STARTUPLOADREQ (0x54) to enter the upload queue
    info!("Requesting upload slot via OP_STARTUPLOADREQ for file {} from peer {}", file_hash_str, target_addr);
    out_wire.clear();
    out_wire.push(PR_ED2K);
    out_wire.extend_from_slice(&(17u32).to_le_bytes()); // 1 opcode + 16 file_hash
    out_wire.push(OP_STARTUPLOADREQ);
    out_wire.extend_from_slice(&dl_hash_bytes);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_STARTUPLOADREQ: {e}")));
    }

    // 7. Wait in upload queue for OP_ACCEPTUPLOADREQ (0x55) or handle OP_QUEUERANKING (0x60)
    let mut last_queue_activity = tokio::time::Instant::now();
    let mut last_keepalive_sent = tokio::time::Instant::now();
    let keepalive_interval = Duration::from_secs(60);

    'queue_loop: loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Connection aborted by kill switch while in queue"));
            }
            _ = tokio::time::sleep_until(last_queue_activity + timeouts.queue_timeout) => {
                return Err(PeerError::new(PeerFailureReason::TimeoutStatus, "Timed out waiting in queue without updates"));
            }
            _ = tokio::time::sleep_until(last_keepalive_sent + keepalive_interval) => {
                // Keep-alive / periodic re-ask (OP_FILESTATUSREQ)
                let status_pkt = encoder.encode_file_status_req(&file_status_req).map_err(|e| {
                    PeerError::new(PeerFailureReason::CorruptedData, format!("Failed to encode keep-alive: {e}"))
                })?;
                out_wire.clear();
                out_wire.push(status_pkt.protocol);
                out_wire.extend_from_slice(&(status_pkt.frame_len() as u32).to_le_bytes());
                out_wire.push(status_pkt.opcode);
                out_wire.extend_from_slice(&status_pkt.payload);

                if let Err(e) = stream.write_all(&out_wire).await {
                    return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send keep-alive: {e}")));
                }
                last_keepalive_sent = tokio::time::Instant::now();
            }
            read_res = stream.read(&mut raw_buf) => {
                match read_res {
                    Ok(0) => {
                        let rank = inner.lock().await.sessions.get(&address_str).and_then(|s| s.snapshot.queue_rank);
                        if let Some(r) = rank {
                            info!("Peer {target_addr} closed connection while we are in queue (rank {r}). Scheduled re-ask in 15 minutes.");
                            return Ok(0);
                        } else {
                            return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Peer closed connection while in queue"));
                        }
                    }
                    Ok(n) => {
                        last_queue_activity = tokio::time::Instant::now(); // Reset sliding watchdog
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::CorruptedData, "Peer read buffer exceeded limit while in queue"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            if packet.protocol == PR_ED2K && packet.opcode == OP_ACCEPTUPLOADREQ {
                                info!("Upload slot accepted (OP_ACCEPTUPLOADREQ) by peer {}", target_addr);
                                break 'queue_loop;
                            } else if (packet.protocol == PR_ED2K || packet.protocol == PR_EMULE) && packet.opcode == OP_QUEUERANKING {
                                let rank = if packet.payload.len() >= 4 {
                                    u32::from_le_bytes(packet.payload[..4].try_into().unwrap_or([0, 0, 0, 0]))
                                } else if packet.payload.len() >= 2 {
                                    u16::from_le_bytes(packet.payload[..2].try_into().unwrap_or([0, 0])) as u32
                                } else {
                                    0
                                };
                                info!("Queue rank from peer {}: {}", target_addr, rank);
                                let queue_update = {
                                    let mut g = inner.lock().await;
                                    if let Some(session) = g.sessions.get_mut(&address_str) {
                                        session.snapshot.queue_rank = Some(rank);
                                        Some(session.snapshot.clone())
                                    } else {
                                        None
                                    }
                                };
                                if let Some(snap) = queue_update {
                                    emit_event(&event_tx, &sequence, &snap);
                                }
                            } else if packet.protocol == PR_ED2K && packet.opcode == OP_FILESTATUS {
                                // Response to periodic keep-alive / re-ask - consumed normally
                                tracing::debug!("Peer {target_addr}: received keep-alive OP_FILESTATUS response");
                            } else if packet.protocol == PR_ED2K && packet.opcode == OP_FILEREQANSNOFILE {
                                return Err(PeerError::new(PeerFailureReason::FileNotFound, "Remote peer no longer shares this file"));
                            } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
                                && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
                            {
                                handle_sx_answer(&packet, dl_hash_bytes, &file_hash_str, target_addr, &decoder, &downloads).await;
                            } else if packet.protocol == PR_EMULE
                                && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
                            {
                                handle_sx_request(&packet, dl_hash_bytes, &file_hash_str, target_addr, &decoder, &encoder, &downloads, &mut stream).await;
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error while in queue: {e}"))),
                }
            }
        }
    }

    // 8. Multi-source block planner setup
    let (file_size, completed_ranges) = {
        let dl_svc = downloads.lock().await;
        let dl = dl_svc.find_by_ed2k_hash(&file_hash_str).ok_or_else(|| {
            PeerError::new(PeerFailureReason::FileNotFound, "Download not found in local service")
        })?;
        let ranges = dl_svc.get_completed_ranges_by_hash(&file_hash_str);
        (dl.total_bytes, ranges)
    };

    let sequencer = Ed2kDownloadSequencer::new(dl_hash_bytes, file_size, file_status).map_err(|e| {
        PeerError::new(PeerFailureReason::CorruptedData, format!("Sequencer init error: {e}"))
    })?;

    // 9. Plan disjoint block for this peer
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let planned_block_opt = {
        let mut g = inner.lock().await;
        let block_opt = sequencer.plan_next_block_for_peer(
            &address_str,
            &completed_ranges,
            &g.in_flight_blocks,
            now_ms,
        ).map_err(|e| PeerError::new(PeerFailureReason::InvalidRange, format!("Planning error: {e}")))?;

        if let Some((part_idx, block)) = block_opt {
            let ifb = InFlightBlock::new(
                address_str.clone(),
                part_idx,
                block.start as u64,
                block.end as u64,
                now_ms,
                30_000, // 30s deadline
            );
            g.in_flight_blocks.push(ifb);
            Some((part_idx, block))
        } else {
            None
        }
    };

    let (part_idx, req_range) = match planned_block_opt {
        Some(b) => b,
        None => {
            info!("No unassigned/missing blocks available for peer {}", target_addr);
            return Ok(0);
        }
    };

    // Send OP_REQUESTPARTS
    let req_parts = Ed2kRequestParts {
        file_hash: dl_hash_bytes,
        ranges: vec![req_range],
    };

    let req_pkt = encoder.encode_request_parts(&req_parts).map_err(|e| {
        PeerError::new(PeerFailureReason::CorruptedData, format!("Failed to encode OP_REQUESTPARTS: {e}"))
    })?;

    out_wire.clear();
    out_wire.push(req_pkt.protocol);
    out_wire.extend_from_slice(&(req_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(req_pkt.opcode);
    out_wire.extend_from_slice(&req_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_REQUESTPARTS: {e}")));
    }

    let transferring_snapshot = {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(&address_str) {
            session.snapshot.status = PeerConnectionStatus::Transferring;
            session.snapshot.current_part = Some(part_idx);
            Some(session.snapshot.clone())
        } else {
            None
        }
    };
    if let Some(snap) = transferring_snapshot {
        emit_event(&event_tx, &sequence, &snap);
    }

    info!("Requested sub-block [{}, {}) from peer {}", req_range.start, req_range.end, target_addr);

    // 8. Receive OP_SENDINGPART / OP_COMPRESSEDPART with First Data Timeout & Inactivity Timeout
    let mut total_received = 0u64;
    let mut last_activity = tokio::time::Instant::now();
    let mut first_data_received = false;
    let mut last_rate_check = std::time::Instant::now();
    let mut bytes_since_rate_check = 0u64;
    let mut decompressor: Option<Decompress> = None;
    let mut decompressed_offset = req_range.start as u64;
    let mut current_part_idx = part_idx;
    let mut current_req_range = req_range;

    'transfer_loop: loop {
        let timeout_duration = if !first_data_received {
            timeouts.first_data_timeout
        } else {
            timeouts.inactivity_timeout
        };

        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Transfer aborted by kill switch"));
            }
            _ = tokio::time::sleep_until(last_activity + timeout_duration) => {
                if !first_data_received {
                    return Err(PeerError::new(PeerFailureReason::TimeoutFirstData, "Timed out waiting for first data packet"));
                } else {
                    return Err(PeerError::new(PeerFailureReason::TimeoutInactivity, "Transfer stall / inactivity timeout exceeded"));
                }
            }
            read_res = stream.read(&mut raw_buf) => {
                match read_res {
                    Ok(0) => {
                        info!("Peer {} closed stream", target_addr);
                        break;
                    }
                    Ok(n) => {
                        last_activity = tokio::time::Instant::now();
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::CorruptedData, "Peer read buffer exceeded limit during transfer"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            let mut block_finished = false;
                            if packet.protocol == PR_ED2K && packet.opcode == OP_SENDINGPART {
                                first_data_received = true;
                                let part = decoder.decode_sending_part(packet.protocol, packet.opcode, &packet.payload, 0)
                                    .map_err(|e| PeerError::new(PeerFailureReason::CorruptedData, format!("Malformed OP_SENDINGPART: {e}")))?;

                                // Validate file hash
                                if part.file_hash != dl_hash_bytes {
                                    return Err(PeerError::new(PeerFailureReason::HashMismatch, "OP_SENDINGPART returned mismatched file hash"));
                                }

                                // Validate range bounds against the planned block request
                                if part.start_offset < current_req_range.start || part.end_offset > current_req_range.end || part.start_offset >= part.end_offset {
                                    return Err(PeerError::new(PeerFailureReason::InvalidRange, format!("OP_SENDINGPART range out of bounds for requested range: [{}, {}) vs [{}, {})", part.start_offset, part.end_offset, current_req_range.start, current_req_range.end)));
                                }

                                if part.end_offset >= current_req_range.end {
                                    block_finished = true;
                                }

                                let data_len = part.data.len() as u64;
                                total_received += data_len;
                                bytes_since_rate_check += data_len;

                                // Pass received data chunk into download service
                                let is_complete = {
                                    let mut dl_svc = downloads.lock().await;
                                    if let Some(dl) = dl_svc.find_by_ed2k_hash(&file_hash_str) {
                                        if let Ok(snap) = dl_svc.inject_range(
                                            &dl.id,
                                            part.start_offset as u64,
                                            &part.data,
                                        ) {
                                            snap.status.to_lowercase() == "verifying" || snap.progress >= 1.0
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                };

                                // Calculate transfer rate
                                let elapsed = last_rate_check.elapsed();
                                let bps = if elapsed >= Duration::from_millis(500) {
                                    let rate = (bytes_since_rate_check as f64 / elapsed.as_secs_f64()) as u64;
                                    bytes_since_rate_check = 0;
                                    last_rate_check = std::time::Instant::now();
                                    rate
                                } else {
                                    inner.lock().await.sessions.get(&address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0)
                                };

                                let progress_snapshot = {
                                    let mut g = inner.lock().await;
                                    if let Some(session) = g.sessions.get_mut(&address_str) {
                                        session.snapshot.transferred_bytes = total_received;
                                        session.snapshot.transfer_rate_bps = bps;
                                        Some(session.snapshot.clone())
                                    } else {
                                        None
                                    }
                                };
                                if let Some(snap) = progress_snapshot {
                                    emit_event(&event_tx, &sequence, &snap);
                                }

                                if elapsed >= Duration::from_millis(500) || is_complete {
                                    let mut dl_svc = downloads.lock().await;
                                    if let Some(snap) = dl_svc.set_download_rate_by_hash(&file_hash_str, bps) {
                                        emit_download_update(&event_tx, &sequence, &snap);
                                    }
                                }
                            } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED || packet.protocol == PR_ED2K)
                                && (packet.opcode == OP_COMPRESSEDPART || packet.opcode == OP_COMPRESSEDPART_I64)
                            {
                                let is_i64 = packet.opcode == OP_COMPRESSEDPART_I64;
                                let min_len = if is_i64 { 28 } else { 24 };
                                if packet.payload.len() < min_len {
                                    return Err(PeerError::new(PeerFailureReason::CorruptedData, "Compressed part payload too short"));
                                }

                                let mut file_hash = [0u8; 16];
                                file_hash.copy_from_slice(&packet.payload[0..16]);
                                if file_hash != dl_hash_bytes {
                                    return Err(PeerError::new(PeerFailureReason::HashMismatch, "OP_COMPRESSEDPART returned mismatched file hash"));
                                }

                                let (block_start, chunk) = if is_i64 {
                                    let bs = u64::from_le_bytes(packet.payload[16..24].try_into().unwrap_or([0; 8]));
                                    (bs, &packet.payload[28..])
                                } else {
                                    let bs = u32::from_le_bytes(packet.payload[16..20].try_into().unwrap_or([0; 4])) as u64;
                                    (bs, &packet.payload[24..])
                                };

                                first_data_received = true;

                                if decompressor.is_none() {
                                    decompressor = Some(Decompress::new(true));
                                    decompressed_offset = block_start;
                                }

                                let decomp = decompressor.as_mut().unwrap();
                                let mut out = vec![0u8; 65536];
                                let mut input_slice = chunk;

                                while !input_slice.is_empty() {
                                    let before_in = decomp.total_in();
                                    let before_out = decomp.total_out();
                                    let res = decomp.decompress(input_slice, &mut out, FlushDecompress::Sync);
                                    let produced = (decomp.total_out() - before_out) as usize;
                                    let consumed = (decomp.total_in() - before_in) as usize;
                                    input_slice = &input_slice[consumed..];

                                    if produced > 0 {
                                        total_received += produced as u64;
                                        bytes_since_rate_check += produced as u64;
                                        let is_complete = {
                                            let mut dl_svc = downloads.lock().await;
                                            if let Some(dl) = dl_svc.find_by_ed2k_hash(&file_hash_str) {
                                                if let Ok(snap) = dl_svc.inject_range(
                                                    &dl.id,
                                                    decompressed_offset,
                                                    &out[..produced],
                                                ) {
                                                    snap.status.to_lowercase() == "verifying" || snap.progress >= 1.0
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            }
                                        };
                                        decompressed_offset += produced as u64;

                                        let elapsed = last_rate_check.elapsed();
                                        if elapsed >= Duration::from_millis(500) || is_complete {
                                            let bps = if elapsed >= Duration::from_millis(500) {
                                                let rate = (bytes_since_rate_check as f64 / elapsed.as_secs_f64()) as u64;
                                                bytes_since_rate_check = 0;
                                                last_rate_check = std::time::Instant::now();
                                                rate
                                            } else {
                                                inner.lock().await.sessions.get(&address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0)
                                            };
                                            let mut dl_svc = downloads.lock().await;
                                            if let Some(snap) = dl_svc.set_download_rate_by_hash(&file_hash_str, bps) {
                                                emit_download_update(&event_tx, &sequence, &snap);
                                            }
                                        }
                                    }

                                    match res {
                                        Ok(Status::StreamEnd) => {
                                            info!("Peer {target_addr}: completed decompression of block at offset {}", decompressed_offset);
                                            decompressor = None;
                                            block_finished = true;
                                            break;
                                        }
                                        Ok(Status::Ok) => {
                                            if consumed == 0 && produced == 0 {
                                                break;
                                            }
                                        }
                                        Ok(Status::BufError) => break,
                                        Err(e) => {
                                            warn!("Peer {target_addr}: Zlib decompression error: {e}");
                                            break;
                                        }
                                    }
                                }

                                // Calculate transfer rate
                                let elapsed = last_rate_check.elapsed();
                                let bps = if elapsed >= Duration::from_millis(500) {
                                    let rate = (bytes_since_rate_check as f64 / elapsed.as_secs_f64()) as u64;
                                    bytes_since_rate_check = 0;
                                    last_rate_check = std::time::Instant::now();
                                    rate
                                } else {
                                    inner.lock().await.sessions.get(&address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0)
                                };

                                let progress_snapshot = {
                                    let mut g = inner.lock().await;
                                    if let Some(session) = g.sessions.get_mut(&address_str) {
                                        session.snapshot.transferred_bytes = total_received;
                                        session.snapshot.transfer_rate_bps = bps;
                                        Some(session.snapshot.clone())
                                    } else {
                                        None
                                    }
                                };
                                if let Some(snap) = progress_snapshot {
                                    emit_event(&event_tx, &sequence, &snap);
                                }

                                if elapsed >= Duration::from_millis(500) {
                                    let mut dl_svc = downloads.lock().await;
                                    if let Some(snap) = dl_svc.set_download_rate_by_hash(&file_hash_str, bps) {
                                        emit_download_update(&event_tx, &sequence, &snap);
                                    }
                                }
                            } else if (packet.protocol == PR_ED2K || packet.protocol == PR_EMULE) && packet.opcode == OP_END_OF_DOWNLOAD {
                                info!("Peer {} sent OP_END_OF_DOWNLOAD", target_addr);
                                break;
                            } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
                                && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
                            {
                                handle_sx_answer(&packet, dl_hash_bytes, &file_hash_str, target_addr, &decoder, &downloads).await;
                            } else if packet.protocol == PR_EMULE
                                && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
                            {
                                handle_sx_request(&packet, dl_hash_bytes, &file_hash_str, target_addr, &decoder, &encoder, &downloads, &mut stream).await;
                            } else {
                                info!(
                                    "Peer {target_addr}: received unhandled packet proto=0x{:02X}, opcode=0x{:02X}, payload_len={}",
                                    packet.protocol,
                                    packet.opcode,
                                    packet.payload.len()
                                );
                            }

                            // If active sub-block finished, immediately pipeline request for next sub-block
                            if block_finished {
                                // 1. Release finished block
                                {
                                    let mut g = inner.lock().await;
                                    g.in_flight_blocks.retain(|b| !(b.peer_id == address_str && b.part_index == current_part_idx && b.start == current_req_range.start as u64));
                                }

                                // 2. Query updated completed ranges
                                let completed = {
                                    let dl_svc = downloads.lock().await;
                                    dl_svc.get_completed_ranges_by_hash(&file_hash_str)
                                };

                                // 3. Plan next block
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as i64;

                                let next_block = {
                                    let mut g = inner.lock().await;
                                    match sequencer.plan_next_block_for_peer(&address_str, &completed, &g.in_flight_blocks, now_ms) {
                                        Ok(Some((next_p, next_r))) => {
                                            let ifb = InFlightBlock::new(
                                                address_str.clone(),
                                                next_p,
                                                next_r.start as u64,
                                                next_r.end as u64,
                                                now_ms,
                                                30_000,
                                            );
                                            g.in_flight_blocks.push(ifb);
                                            Some((next_p, next_r))
                                        }
                                        _ => None,
                                    }
                                };

                                if let Some((next_p, next_r)) = next_block {
                                    current_part_idx = next_p;
                                    current_req_range = next_r;
                                    decompressor = None;
                                    decompressed_offset = next_r.start as u64;

                                    let req_parts = Ed2kRequestParts {
                                        file_hash: dl_hash_bytes,
                                        ranges: vec![next_r],
                                    };
                                    if let Ok(req_pkt) = encoder.encode_request_parts(&req_parts) {
                                        let mut out_wire = Vec::with_capacity(6 + req_pkt.payload.len());
                                        out_wire.push(req_pkt.protocol);
                                        out_wire.extend_from_slice(&(req_pkt.frame_len() as u32).to_le_bytes());
                                        out_wire.push(req_pkt.opcode);
                                        out_wire.extend_from_slice(&req_pkt.payload);
                                        if let Err(e) = stream.write_all(&out_wire).await {
                                            tracing::warn!("Failed to send chained OP_REQUESTPARTS to {target_addr}: {e}");
                                            break;
                                        }
                                        tracing::info!("Peer {target_addr}: requested next sub-block [{}, {}) in part {}", next_r.start, next_r.end, next_p);
                                        last_activity = tokio::time::Instant::now();
                                    }
                                } else {
                                    tracing::info!("Peer {target_addr}: all blocks completed or no more blocks needed");
                                    // Graceful termination: send OP_END_OF_DOWNLOAD with 16-byte file hash
                                    let mut end_wire = Vec::with_capacity(6 + 16);
                                    end_wire.push(PR_ED2K);
                                    end_wire.extend_from_slice(&(17u32).to_le_bytes()); // 1 byte opcode + 16 bytes hash
                                    end_wire.push(OP_END_OF_DOWNLOAD);
                                    end_wire.extend_from_slice(&dl_hash_bytes);
                                    let _ = stream.write_all(&end_wire).await;
                                    let _ = stream.flush().await;

                                    break 'transfer_loop;
                                }
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error during transfer: {e}"))),
                }
            }
        }
    }

    // Reset transfer rate for download when leaving transfer_data
    {
        let mut dl_svc = downloads.lock().await;
        if let Some(snap) = dl_svc.set_download_rate_by_hash(&file_hash_str, 0) {
            emit_download_update(&event_tx, &sequence, &snap);
        }
    }

    Ok(total_received)
}

fn emit_event(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    snapshot: &PeerSessionSnapshot,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let notif = JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "event.peerSessionUpdated".to_string(),
        params: serde_json::json!({
            "sequence": seq,
            "session": snapshot,
        }),
    };
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}

fn emit_download_update(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    snapshot: &DownloadSnapshot,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let notif = JsonRpcNotification {
        jsonrpc: "2.0".to_string(),
        method: "event.downloadUpdated".to_string(),
        params: serde_json::json!({
            "sequence": seq,
            "download": snapshot,
        }),
    };
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}

fn parse_hex_16(hex_str: &str) -> Option<[u8; 16]> {
    let trimmed = hex_str.trim();
    if trimmed.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for i in 0..16 {
        bytes[i] = u8::from_str_radix(&trimmed[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}

async fn handle_sx_answer(
    packet: &mule_protocol::framing::RawPacket,
    dl_hash_bytes: [u8; 16],
    file_hash_str: &str,
    target_addr: SocketAddr,
    decoder: &Ed2kDecoder,
    downloads: &Arc<Mutex<DownloadService>>,
) {
    if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
        && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
    {
        match decoder.decode_answer_sources(packet.protocol, packet.opcode, &packet.payload) {
            Ok(ans) => {
                if ans.file_hash == dl_hash_bytes {
                    let mut discovered_sources = Vec::new();
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64;
                    for src in ans.sources {
                        if !src.is_low_id() && src.port > 0 {
                            let ip_u32 = src.ip;
                            let ip_addr = std::net::Ipv4Addr::from(src.ip_bytes);
                            let addr_str = format!("{}:{}", ip_addr, src.port);

                            let hex_user_hash = src.user_hash.map(hex::encode);

                            discovered_sources.push(mule_core::model::PeerSource {
                                file_hash: file_hash_str.to_string(),
                                address: addr_str,
                                ip: ip_u32,
                                port: src.port,
                                origin: mule_core::model::PeerSourceOrigin::SourceExchange,
                                discovered_at_unix_ms: now_ms,
                                last_seen_unix_ms: now_ms,
                                status: PeerSourceStatus::Candidate,
                                fail_count: 0,
                                last_error: None,
                                cooldown_until_unix_ms: None,
                                user_hash: hex_user_hash,
                                crypt_options: src.crypt_options,
                            });
                        }
                    }
                    if !discovered_sources.is_empty() {
                        tracing::info!(
                            "Discovered {} new sources via Source Exchange from peer {target_addr} for file {file_hash_str}",
                            discovered_sources.len()
                        );
                        let mut dl_svc = downloads.lock().await;
                        let _ = dl_svc.upsert_peer_sources(file_hash_str, &discovered_sources);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Peer {target_addr}: failed to decode SX answer: {e}");
            }
        }
    }
}

async fn handle_sx_request(
    packet: &mule_protocol::framing::RawPacket,
    dl_hash_bytes: [u8; 16],
    file_hash_str: &str,
    target_addr: SocketAddr,
    decoder: &Ed2kDecoder,
    encoder: &Ed2kEncoder,
    downloads: &Arc<Mutex<DownloadService>>,
    stream: &mut crate::services::obfuscation_stream::ObfuscatedStream<TcpStream>,
) {
    if packet.protocol == PR_EMULE
        && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
    {
        match decoder.decode_request_sources(packet.protocol, packet.opcode, &packet.payload) {
            Ok(req) => {
                if req.file_hash == dl_hash_bytes {
                    let known_sources = {
                        let dl_svc = downloads.lock().await;
                        dl_svc.list_peer_sources(file_hash_str).unwrap_or_default()
                    };
                    let mut resp_sources = Vec::new();
                    for src in known_sources {
                        if resp_sources.len() >= 50 {
                            break;
                        }
                        if src.port > 0 && src.ip >= 16_777_216 {
                            let ip_bytes = src.ip.to_be_bytes();
                            let user_hash_opt = if let Some(ref h) = src.user_hash {
                                if let Ok(hb) = hex::decode(h) {
                                    if hb.len() == 16 {
                                        let mut h16 = [0u8; 16];
                                        h16.copy_from_slice(&hb);
                                        Some(h16)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            resp_sources.push(SxSource::new(
                                ip_bytes,
                                src.port,
                                user_hash_opt,
                                src.crypt_options,
                            ));
                        }
                    }

                    let ans = Ed2kAnswerSources::new(dl_hash_bytes, resp_sources);
                    if let Ok(ans_pkt) = encoder.encode_answer_sources(&ans, true) {
                        let mut resp_wire = Vec::with_capacity(6 + ans_pkt.payload.len());
                        resp_wire.push(ans_pkt.protocol);
                        resp_wire.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
                        resp_wire.push(ans_pkt.opcode);
                        resp_wire.extend_from_slice(&ans_pkt.payload);
                        if let Err(e) = stream.write_all(&resp_wire).await {
                            tracing::warn!("Failed to send SX answer to {target_addr}: {e}");
                        } else {
                            tracing::info!(
                                "Sent SX answer with {} sources (proto=0x{:02X}, len={}) to peer {target_addr}",
                                ans.sources.len(),
                                ans_pkt.protocol,
                                resp_wire.len()
                            );
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Peer {target_addr}: failed to decode SX request: {e}");
            }
        }
    }
}


