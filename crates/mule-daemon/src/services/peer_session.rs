//! Swarm eD2K Peer Connection & In-flight Block Manager (Dual-source Multi-Peer).
//!
//! Manages up to 2 active TCP peer connections per download, and up to 4 global peer connections.
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

use mule_core::model::{PeerFailureReason, PeerSourceStatus};
use mule_network_policy::SocketPermit;
use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kDownloadSequencer, Ed2kEncoder, Ed2kFileStatusReq, Ed2kHello,
    Ed2kRequestParts, InFlightBlock, OP_END_OF_DOWNLOAD, OP_FILEREQANSNOFILE, OP_FILESTATUS,
    OP_HELLO, OP_HELLO_ANSWER, OP_SENDINGPART, PR_ED2K,
};
use mule_protocol::framing::PacketCodec;

use crate::error::JsonRpcError;
use crate::rpc::types::JsonRpcNotification;
use crate::services::downloads::DownloadService;

/// Maximum active peer connections per file hash.
pub const MAX_PEERS_PER_DOWNLOAD: usize = 2;

/// Maximum active peer connections across the entire daemon.
pub const MAX_GLOBAL_PEERS: usize = 4;

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
            connect_timeout: Duration::from_secs(8),
            handshake_timeout: Duration::from_secs(8),
            status_timeout: Duration::from_secs(8),
            queue_timeout: Duration::from_secs(30),
            first_data_timeout: Duration::from_secs(10),
            inactivity_timeout: Duration::from_secs(10),
        }
    }
}

/// Computes the exponential backoff cooldown in milliseconds based on fail_count.
/// Formule: min(600_000, 2^(fail_count - 1) * 5_000) (5s, 10s, 20s, 40s... max 10 min)
pub fn calculate_backoff_cooldown_ms(fail_count: u32) -> i64 {
    if fail_count == 0 {
        return 0;
    }
    let exponent = (fail_count - 1).min(8);
    let base = 5_000i64;
    let factor = 1i64 << exponent;
    (base * factor).min(600_000)
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
}

/// Thread-safe manager for multi-source eD2K peer connections.
#[derive(Clone)]
pub struct PeerSessionManager {
    inner: Arc<Mutex<SwarmInnerState>>,
    event_tx: broadcast::Sender<String>,
    sequence: Arc<AtomicU64>,
    timeouts: PeerTimeouts,
}

impl PeerSessionManager {
    /// Creates a new `PeerSessionManager`.
    pub fn new(event_tx: broadcast::Sender<String>, sequence: Arc<AtomicU64>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(SwarmInnerState {
                sessions: HashMap::new(),
                in_flight_blocks: Vec::new(),
                last_session: None,
            })),
            event_tx,
            sequence,
            timeouts: PeerTimeouts::default(),
        }
    }

    /// Sets custom timeouts.
    pub fn with_timeouts(mut self, timeouts: PeerTimeouts) -> Self {
        self.timeouts = timeouts;
        self
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
        let mut guard = self.inner.lock().await;
        if let Some(addr) = address {
            if let Some(handle) = guard.sessions.remove(addr) {
                handle.abort_handle.abort();
                guard.in_flight_blocks.retain(|b| b.peer_id != addr);
                let disconnected_snap = PeerSessionSnapshot {
                    status: PeerConnectionStatus::Disconnected,
                    file_hash: handle.snapshot.file_hash,
                    peer_address: Some(addr.to_string()),
                    ..Default::default()
                };
                emit_event(&self.event_tx, &self.sequence, &disconnected_snap);
            }
        } else {
            for (addr, handle) in guard.sessions.drain() {
                handle.abort_handle.abort();
                let disconnected_snap = PeerSessionSnapshot {
                    status: PeerConnectionStatus::Disconnected,
                    file_hash: handle.snapshot.file_hash,
                    peer_address: Some(addr),
                    ..Default::default()
                };
                emit_event(&self.event_tx, &self.sequence, &disconnected_snap);
            }
            guard.in_flight_blocks.clear();
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
    ) -> Result<(), JsonRpcError> {
        let address_str = target_addr.to_string();
        let mut guard = self.inner.lock().await;

        if guard.sessions.len() >= MAX_GLOBAL_PEERS {
            return Err(JsonRpcError::business(format!(
                "Global peer connection limit reached ({MAX_GLOBAL_PEERS} max)"
            )));
        }

        let file_active_count = guard
            .sessions
            .values()
            .filter(|h| h.snapshot.file_hash.as_deref() == Some(file_hash_hex))
            .count();

        if file_active_count >= MAX_PEERS_PER_DOWNLOAD {
            return Err(JsonRpcError::business(format!(
                "Maximum concurrent peer connections reached for this download ({MAX_PEERS_PER_DOWNLOAD} max)"
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
        };
        emit_event(&self.event_tx, &self.sequence, &initial_snapshot);

        let inner_clone = Arc::clone(&self.inner);
        let event_tx_clone = self.event_tx.clone();
        let sequence_clone = Arc::clone(&self.sequence);
        let cancel_token = permit.cancellation_token();
        let file_hash_str = file_hash_hex.to_string();
        let timeouts = self.timeouts;
        let addr_for_task = address_str.clone();

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
                Err(e) => {
                    warn!("Peer session with {} ended with failure: [{:?}] {}", target_addr, e.reason, e.message);
                    (PeerSourceStatus::Failed, Some(e.reason), Some(e.message))
                }
            };

            // Update SQLite persistence with fail_count and cooldown
            {
                let mut dl_svc = downloads.lock().await;
                let sources = dl_svc.list_peer_sources(&file_hash_str).unwrap_or_default();
                let current_source = sources.into_iter().find(|s| s.address == addr_for_task);
                let current_fail_count = current_source.map(|s| s.fail_count).unwrap_or(0);
                let new_fail_count = if final_status == PeerSourceStatus::Failed {
                    current_fail_count + 1
                } else {
                    current_fail_count
                };

                let cooldown_ms = calculate_backoff_cooldown_ms(new_fail_count);
                let cooldown_until = if final_status == PeerSourceStatus::Failed && cooldown_ms > 0 {
                    Some(now + cooldown_ms)
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
                snapshot: initial_snapshot,
                abort_handle: task.abort_handle(),
                _permit: permit,
            },
        );

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

    let mut stream = stream_res;
    let connected_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Update status: Handshaking
    {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(&address_str) {
            session.snapshot.status = PeerConnectionStatus::Handshaking;
            session.snapshot.connected_at_unix_ms = Some(connected_ms);
            emit_event(&event_tx, &sequence, &session.snapshot);
        }
    }

    // 2. Send OP_HELLO
    let encoder = Ed2kEncoder::default();
    let name_tag = mule_protocol::ed2k::tags::Ed2kTag {
        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_NAME),
        value: mule_protocol::ed2k::tags::TagValue::String(nickname),
    };
    let hello = Ed2kHello {
        user_hash,
        client_id: 0,
        tcp_port: client_port,
        tags: vec![name_tag],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };

    let hello_pkt = encoder.encode_hello(&hello).map_err(|e| {
        PeerError::new(PeerFailureReason::InvalidHandshake, format!("Failed to encode OP_HELLO: {e}"))
    })?;

    let mut out_wire = Vec::with_capacity(6 + hello_pkt.payload.len());
    out_wire.push(hello_pkt.protocol);
    out_wire.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(hello_pkt.opcode);
    out_wire.extend_from_slice(&hello_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_HELLO: {e}")));
    }

    // 3. Receive OP_HELLO_ANSWER with Handshake Timeout
    let decoder = Ed2kDecoder::default();
    let codec = PacketCodec::new();
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let mut received_hello_answer = false;

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
                    Ok(0) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Peer closed connection during handshake")),
                    Ok(n) => {
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            if packet.protocol == PR_ED2K && (packet.opcode == OP_HELLO || packet.opcode == OP_HELLO_ANSWER) {
                                // Validate handshake packet
                                if let Err(e) = decoder.decode_hello_answer(packet.protocol, packet.opcode, &packet.payload, 0) {
                                    return Err(PeerError::new(PeerFailureReason::InvalidHandshake, format!("Invalid OP_HELLO_ANSWER: {e}")));
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

    // 4. Send OP_FILESTATUSREQ for the requested file hash
    let file_status_req = Ed2kFileStatusReq {
        file_hash: dl_hash_bytes,
    };
    let status_pkt = encoder.encode_file_status_req(&file_status_req).map_err(|e| {
        PeerError::new(PeerFailureReason::InvalidHandshake, format!("Failed to encode OP_FILESTATUSREQ: {e}"))
    })?;

    out_wire.clear();
    out_wire.push(status_pkt.protocol);
    out_wire.extend_from_slice(&(status_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(status_pkt.opcode);
    out_wire.extend_from_slice(&status_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_FILESTATUSREQ: {e}")));
    }

    {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(&address_str) {
            session.snapshot.status = PeerConnectionStatus::Queued;
            emit_event(&event_tx, &sequence, &session.snapshot);
        }
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
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            if packet.protocol == PR_ED2K {
                                if packet.opcode == OP_FILESTATUS {
                                    // Decode and verify file hash
                                    let status_res = decoder.decode_file_status(packet.protocol, packet.opcode, &packet.payload, 0);
                                    match status_res {
                                        Ok(status) => {
                                            if status.file_hash != dl_hash_bytes {
                                                return Err(PeerError::new(
                                                    PeerFailureReason::HashMismatch,
                                                    format!("OP_FILESTATUS returned unexpected file hash"),
                                                ));
                                            }
                                            file_status_opt = Some(status);
                                            break;
                                        }
                                        Err(e) => return Err(PeerError::new(PeerFailureReason::CorruptedData, format!("Malformed OP_FILESTATUS: {e}"))),
                                    }
                                } else if packet.opcode == OP_FILEREQANSNOFILE {
                                    return Err(PeerError::new(PeerFailureReason::FileNotFound, "Remote peer does not share this file (OP_FILEREQANSNOFILE)"));
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

    // 6. Multi-source block planner setup
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

    // 7. Plan disjoint block for this peer
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

    {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(&address_str) {
            session.snapshot.status = PeerConnectionStatus::Transferring;
            session.snapshot.current_part = Some(part_idx);
            emit_event(&event_tx, &sequence, &session.snapshot);
        }
    }

    info!("Requested sub-block [{}, {}) from peer {}", req_range.start, req_range.end, target_addr);

    // 8. Receive OP_SENDINGPART with First Data Timeout & Inactivity Timeout
    let mut total_received = 0u64;
    let mut last_activity = tokio::time::Instant::now();
    let mut first_data_received = false;
    let mut last_rate_check = std::time::Instant::now();
    let mut bytes_since_rate_check = 0u64;

    loop {
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
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            if packet.protocol == PR_ED2K && packet.opcode == OP_SENDINGPART {
                                first_data_received = true;
                                let part = decoder.decode_sending_part(packet.protocol, packet.opcode, &packet.payload, 0)
                                    .map_err(|e| PeerError::new(PeerFailureReason::CorruptedData, format!("Malformed OP_SENDINGPART: {e}")))?;

                                // Validate file hash
                                if part.file_hash != dl_hash_bytes {
                                    return Err(PeerError::new(PeerFailureReason::HashMismatch, "OP_SENDINGPART returned mismatched file hash"));
                                }

                                // Validate range bounds against the planned block request
                                if part.start_offset < req_range.start || part.end_offset > req_range.end || part.start_offset >= part.end_offset {
                                    return Err(PeerError::new(PeerFailureReason::InvalidRange, format!("OP_SENDINGPART range out of bounds for requested range: [{}, {}) vs [{}, {})", part.start_offset, part.end_offset, req_range.start, req_range.end)));
                                }

                                let data_len = part.data.len() as u64;
                                total_received += data_len;
                                bytes_since_rate_check += data_len;

                                // Pass received data chunk into download service
                                {
                                    let mut dl_svc = downloads.lock().await;
                                    if let Some(dl) = dl_svc.find_by_ed2k_hash(&file_hash_str) {
                                        let _ = dl_svc.inject_range(
                                            &dl.id,
                                            part.start_offset as u64,
                                            &part.data,
                                        );
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

                                {
                                    let mut g = inner.lock().await;
                                    if let Some(session) = g.sessions.get_mut(&address_str) {
                                        session.snapshot.transferred_bytes = total_received;
                                        session.snapshot.transfer_rate_bps = bps;
                                        emit_event(&event_tx, &sequence, &session.snapshot);
                                    }
                                }
                            } else if packet.protocol == PR_ED2K && packet.opcode == OP_END_OF_DOWNLOAD {
                                info!("Peer {} sent OP_END_OF_DOWNLOAD", target_addr);
                                break;
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error during transfer: {e}"))),
                }
            }
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

