//! eD2K Server Connection Manager.
//!
//! Manages at most 1 active TCP connection to an eD2K server.
//! Enforces network policy, token bucket rate limits, instant kill-switch abortion,
//! and bounded wire framing decoding (OP_LOGINREQUEST, OP_SERVERMESSAGE, OP_IDCHANGE, OP_SERVERSTATUS, OP_SERVERIDENT).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::BytesMut;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{error, info, warn};

use mule_network_policy::SocketPermit;
use mule_protocol::ed2k::{
    decode_server_response, Ed2kLimits, FoundSources, FoundSourcesObfu, GetSourcesRequest, SearchQuery, SearchResult,
    ServerLoginRequest, ServerResponse,
};
use mule_protocol::framing::{PacketCodec, RawPacket};

use crate::error::JsonRpcError;
use crate::rpc::types::JsonRpcNotification;
use crate::services::downloads::DownloadService;
use mule_core::model::{PeerSource, PeerSourceOrigin, PeerSourceStatus};

/// Current lifecycle status of an eD2K server connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerConnectionStatus {
    /// No connection active.
    Disconnected,
    /// TCP handshake and login request in progress.
    Connecting,
    /// Successfully logged in and ready for operations.
    Connected,
    /// Disconnection in progress.
    Disconnecting,
}

impl ServerConnectionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Disconnecting => "disconnecting",
        }
    }
}

/// Snapshot of the current eD2K server session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerSessionSnapshot {
    /// Current connection status.
    pub status: ServerConnectionStatus,
    /// Server address ("ip:port") if connected or connecting.
    pub server_address: Option<String>,
    /// Server name if received in ident or hello.
    pub server_name: Option<String>,
    /// Server description.
    pub server_desc: Option<String>,
    /// Server assigned Client ID.
    pub client_id: Option<u32>,
    /// True if client ID >= 16_777_216.
    pub is_high_id: bool,
    /// Server MOTD / greeting message.
    pub welcome_message: Option<String>,
    /// Active users reported by server.
    pub current_users: Option<u32>,
    /// Active files reported by server.
    pub current_files: Option<u32>,
    /// Timestamp when connection completed (unix ms).
    pub connected_at_unix_ms: Option<i64>,
    /// Ping / roundtrip time estimate in milliseconds.
    pub ping_ms: Option<u32>,
}

impl Default for ServerSessionSnapshot {
    fn default() -> Self {
        Self {
            status: ServerConnectionStatus::Disconnected,
            server_address: None,
            server_name: None,
            server_desc: None,
            client_id: None,
            is_high_id: false,
            welcome_message: None,
            current_users: None,
            current_files: None,
            connected_at_unix_ms: None,
            ping_ms: None,
        }
    }
}

struct ActiveSession {
    snapshot: ServerSessionSnapshot,
    abort_handle: Option<tokio::task::AbortHandle>,
    _permit: Option<SocketPermit>,
    outgoing_tx: Option<mpsc::Sender<RawPacket>>,
}

/// Thread-safe manager for the single eD2K server connection.
#[derive(Clone)]
pub struct ServerSessionManager {
    inner: Arc<Mutex<ActiveSession>>,
    event_tx: broadcast::Sender<String>,
    sequence: Arc<AtomicU64>,
    downloads: Arc<std::sync::RwLock<Option<Arc<Mutex<DownloadService>>>>>,
}

impl ServerSessionManager {
    /// Creates a new `ServerSessionManager`.
    pub fn new(event_tx: broadcast::Sender<String>, sequence: Arc<AtomicU64>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ActiveSession {
                snapshot: ServerSessionSnapshot::default(),
                abort_handle: None,
                _permit: None,
                outgoing_tx: None,
            })),
            event_tx,
            sequence,
            downloads: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Sets the download service reference for auto-ingesting found sources.
    pub fn set_downloads(&self, downloads: Arc<Mutex<DownloadService>>) {
        if let Ok(mut d) = self.downloads.write() {
            *d = Some(downloads);
        }
    }

    /// Returns a snapshot of the current server session.
    pub async fn snapshot(&self) -> ServerSessionSnapshot {
        let guard = self.inner.lock().await;
        guard.snapshot.clone()
    }

    /// Returns true if a server connection is active or in progress.
    pub async fn is_active(&self) -> bool {
        let guard = self.inner.lock().await;
        matches!(
            guard.snapshot.status,
            ServerConnectionStatus::Connecting | ServerConnectionStatus::Connected
        )
    }

    /// Sends a search query to the connected server.
    pub async fn search(&self, query: SearchQuery) -> Result<(), JsonRpcError> {
        let guard = self.inner.lock().await;
        if guard.snapshot.status != ServerConnectionStatus::Connected {
            return Err(JsonRpcError::business("Cannot search: server is not connected"));
        }
        let tx = guard.outgoing_tx.as_ref().ok_or_else(|| {
            JsonRpcError::business("Cannot search: outgoing channel closed")
        })?;
        let limits = Ed2kLimits::default();
        let packet = query
            .encode(&limits)
            .map_err(|e| JsonRpcError::business(format!("Failed to encode search query: {e}")))?;
        
        info!(
            "Sending eD2K OP_SEARCHREQUEST to server (term='{}', min_size={:?}, max_size={:?}, ext={:?}, payload_len={})",
            query.term,
            query.min_size,
            query.max_size,
            query.extension,
            packet.payload.len()
        );

        tx.send(packet).await.map_err(|_| {
            JsonRpcError::business("Failed to send search request to server")
        })?;
        Ok(())
    }

    /// Sends a get sources request to the connected server for a specific file hash.
    pub async fn get_sources(&self, request: GetSourcesRequest) -> Result<(), JsonRpcError> {
        let guard = self.inner.lock().await;
        if guard.snapshot.status != ServerConnectionStatus::Connected {
            return Err(JsonRpcError::business("Cannot query sources: server is not connected"));
        }
        let tx = guard.outgoing_tx.as_ref().ok_or_else(|| {
            JsonRpcError::business("Cannot query sources: outgoing channel closed")
        })?;
        let limits = Ed2kLimits::for_server();
        let packet = request
            .encode(&limits)
            .map_err(|e| JsonRpcError::business(format!("Failed to encode get sources request: {e}")))?;
        info!(
            "Sending eD2K OP_GETSOURCES (obfuscation={}) to server for file hash: {}",
            request.obfuscation,
            hex::encode(request.file_hash)
        );
        tx.send(packet).await.map_err(|_| {
            JsonRpcError::business("Failed to send get sources request to server")
        })?;
        Ok(())
    }

    /// Disconnects any active session and resets state.
    pub async fn disconnect(&self) -> Result<(), JsonRpcError> {
        let mut guard = self.inner.lock().await;
        if let Some(handle) = guard.abort_handle.take() {
            handle.abort();
        }
        guard._permit = None;
        guard.outgoing_tx = None;
        guard.snapshot = ServerSessionSnapshot::default();
        self.emit_notification(&guard.snapshot);
        Ok(())
    }

    /// Initiates a connection to `target_addr` under `permit`.
    /// Rejects if already connecting or connected.
    pub async fn connect(
        &self,
        target_addr: SocketAddr,
        server_name: Option<String>,
        permit: SocketPermit,
        connect_timeout_ms: u64,
        handshake_timeout_ms: u64,
        client_port: u16,
        nickname: String,
        user_hash: [u8; 16],
    ) -> Result<(), JsonRpcError> {
        let mut guard = self.inner.lock().await;
        if matches!(
            guard.snapshot.status,
            ServerConnectionStatus::Connecting | ServerConnectionStatus::Connected
        ) {
            return Err(JsonRpcError::business(
                "An eD2K server connection is already in progress or connected",
            ));
        }

        let address_str = target_addr.to_string();
        guard.snapshot = ServerSessionSnapshot {
            status: ServerConnectionStatus::Connecting,
            server_address: Some(address_str.clone()),
            server_name,
            server_desc: None,
            client_id: None,
            is_high_id: false,
            welcome_message: None,
            current_users: None,
            current_files: None,
            connected_at_unix_ms: None,
            ping_ms: None,
        };
        self.emit_notification(&guard.snapshot);

        let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<RawPacket>(32);
        guard.outgoing_tx = Some(outgoing_tx);

        let inner_clone = Arc::clone(&self.inner);
        let event_tx_clone = self.event_tx.clone();
        let sequence_clone = Arc::clone(&self.sequence);
        let cancel_token = permit.cancellation_token();
        let downloads_clone = self.downloads.read().ok().and_then(|g| g.clone());

        let task = tokio::spawn(async move {
            let start_connect = std::time::Instant::now();
            info!("Attempting TCP connection to eD2K server {} (timeout: {} ms)...", target_addr, connect_timeout_ms);

            // 1. TCP Connect with connect_timeout
            let stream_res = tokio::select! {
                _ = cancel_token.cancelled() => {
                    warn!("Server connection to {} cancelled by policy kill switch during connect", target_addr);
                    Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Cancelled by network policy"))
                }
                res = tokio::time::timeout(Duration::from_millis(connect_timeout_ms), TcpStream::connect(target_addr)) => {
                    match res {
                        Ok(Ok(stream)) => {
                            info!("TCP socket connected successfully to eD2K server {} in {} ms", target_addr, start_connect.elapsed().as_millis());
                            Ok(stream)
                        }
                        Ok(Err(e)) => {
                            warn!("TCP connect failed to eD2K server {}: {}", target_addr, e);
                            Err(e)
                        }
                        Err(_) => {
                            warn!("TCP connect timed out after {} ms to eD2K server {}", connect_timeout_ms, target_addr);
                            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "TCP connect timed out"))
                        }
                    }
                }
            };

            let mut stream = match stream_res {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to establish TCP connection to eD2K server {}: {}", target_addr, e);
                    let mut g = inner_clone.lock().await;
                    g.snapshot = ServerSessionSnapshot::default();
                    g._permit = None;
                    g.abort_handle = None;
                    g.outgoing_tx = None;
                    emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
                    return;
                }
            };

            let ping_ms = start_connect.elapsed().as_millis() as u32;

            // 2. Send OP_LOGINREQUEST
            let limits = Ed2kLimits::for_server();
            let login_req = ServerLoginRequest::new(user_hash, client_port, &nickname, 0x3c);
            info!("Sending OP_LOGINREQUEST to eD2K server {} (nick='{}', port={}, hash={:?})", target_addr, nickname, client_port, hex::encode(user_hash));
            let packet = match login_req.encode(&limits) {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to encode ServerLoginRequest for {}: {}", target_addr, e);
                    let mut g = inner_clone.lock().await;
                    g.snapshot = ServerSessionSnapshot::default();
                    g._permit = None;
                    g.abort_handle = None;
                    g.outgoing_tx = None;
                    emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
                    return;
                }
            };

            let mut wire_buf = Vec::with_capacity(6 + packet.payload.len());
            wire_buf.push(packet.protocol);
            wire_buf.extend_from_slice(&(packet.frame_len() as u32).to_le_bytes());
            wire_buf.push(packet.opcode);
            wire_buf.extend_from_slice(&packet.payload);

            if let Err(e) = stream.write_all(&wire_buf).await {
                warn!("Failed to write OP_LOGINREQUEST bytes to eD2K server {}: {}", target_addr, e);
                let mut g = inner_clone.lock().await;
                g.snapshot = ServerSessionSnapshot::default();
                g._permit = None;
                g.abort_handle = None;
                g.outgoing_tx = None;
                emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
                return;
            }
            info!("Sent OP_LOGINREQUEST ({} wire bytes) to {}. Awaiting server response...", wire_buf.len(), target_addr);

/// Maximum read buffer capacity for incoming server data (256 KiB) to prevent OOM.
const MAX_SERVER_READ_BUFFER_SIZE: usize = 256 * 1024;

            // 3. Read loop with handshake timeout & PacketCodec
            let mut read_buf = BytesMut::with_capacity(8192);
            let codec = PacketCodec::new();
            let mut temp_buf = [0u8; 4096];
            let handshake_deadline = tokio::time::Instant::now() + Duration::from_millis(handshake_timeout_ms);
            let mut is_connecting = true;

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("Server session aborted by policy kill switch");
                        break;
                    }
                    Some(out_packet) = outgoing_rx.recv() => {
                        let mut out_wire = Vec::with_capacity(6 + out_packet.payload.len());
                        out_wire.push(out_packet.protocol);
                        out_wire.extend_from_slice(&(out_packet.frame_len() as u32).to_le_bytes());
                        out_wire.push(out_packet.opcode);
                        out_wire.extend_from_slice(&out_packet.payload);
                        if let Err(e) = stream.write_all(&out_wire).await {
                            warn!("Failed to write outgoing packet to server {}: {}", target_addr, e);
                            break;
                        }
                    }
                    read_res = stream.read(&mut temp_buf) => {
                        match read_res {
                            Ok(0) => {
                                info!("Server closed TCP connection cleanly: {}", target_addr);
                                break;
                            }
                            Ok(n) => {
                                read_buf.extend_from_slice(&temp_buf[..n]);
                                if read_buf.len() > MAX_SERVER_READ_BUFFER_SIZE {
                                    warn!(
                                        "Server {} exceeded maximum read buffer size ({}), disconnecting",
                                        target_addr, MAX_SERVER_READ_BUFFER_SIZE
                                    );
                                    break;
                                }

                                loop {
                                    match codec.decode(&mut read_buf) {
                                        Ok(Some(raw_packet)) => {
                                            tracing::debug!(
                                                "Server TCP packet received: proto=0x{:02X}, opcode=0x{:02X}, payload_len={}",
                                                raw_packet.protocol,
                                                raw_packet.opcode,
                                                raw_packet.payload.len()
                                            );
                                            match decode_server_response(raw_packet.protocol, raw_packet.opcode, &raw_packet.payload, &limits) {
                                                Ok(resp) => {
                                                    let mut g = inner_clone.lock().await;
                                                    let mut updated = false;

                                                    match resp {
                                                        ServerResponse::Message(msg) => {
                                                            info!("Received eD2K server message: {}", msg.message);
                                                            let prev = g.snapshot.welcome_message.clone().unwrap_or_default();
                                                            let combined = if prev.is_empty() {
                                                                msg.message
                                                            } else {
                                                                format!("{}\n{}", prev, msg.message)
                                                            };
                                                            g.snapshot.welcome_message = Some(combined);
                                                            updated = true;
                                                        }
                                                        ServerResponse::IdChange(idc) => {
                                                            info!("Assigned ClientID: {} (HighID: {})", idc.client_id, idc.is_high_id());
                                                            is_connecting = false;
                                                            g.snapshot.client_id = Some(idc.client_id);
                                                            g.snapshot.is_high_id = idc.is_high_id();
                                                            g.snapshot.status = ServerConnectionStatus::Connected;
                                                            if g.snapshot.connected_at_unix_ms.is_none() {
                                                                let now = std::time::SystemTime::now()
                                                                    .duration_since(std::time::UNIX_EPOCH)
                                                                    .unwrap_or_default()
                                                                    .as_millis() as i64;
                                                                g.snapshot.connected_at_unix_ms = Some(now);
                                                                g.snapshot.ping_ms = Some(ping_ms);
                                                            }
                                                            updated = true;
                                                        }
                                                        ServerResponse::Status(status) => {
                                                            info!("Server status: {} users, {} files", status.current_users, status.current_files);
                                                            g.snapshot.current_users = Some(status.current_users);
                                                            g.snapshot.current_files = Some(status.current_files);
                                                            updated = true;
                                                        }
                                                        ServerResponse::Ident(ident) => {
                                                            info!("Server ident: ip={:08x}, port={}", ident.ip, ident.port);
                                                            if let Some(name) = ident.name() {
                                                                g.snapshot.server_name = Some(name);
                                                            }
                                                            updated = true;
                                                        }
                                                        ServerResponse::SearchResult(sr) => {
                                                            info!("Received {} search results from server", sr.files.len());
                                                            emit_search_results(&event_tx_clone, &sequence_clone, &sr);
                                                        }
                                                        ServerResponse::FoundSources(fs) => {
                                                            let total_rcv = fs.sources.len();
                                                            emit_found_sources(&event_tx_clone, &sequence_clone, &fs);

                                                            if let Some(ref dl_service) = downloads_clone {
                                                                let file_hash_hex = hex::encode(fs.file_hash);
                                                                let now_ms = std::time::SystemTime::now()
                                                                    .duration_since(std::time::UNIX_EPOCH)
                                                                    .unwrap_or_default()
                                                                    .as_millis() as i64;
                                                                let mut discovered = Vec::new();
                                                                let mut seen_addrs = std::collections::HashSet::new();
                                                                let mut low_id_ignored = 0;
                                                                let mut rejected_invalid = 0;

                                                                for src in &fs.sources {
                                                                    // Filter out invalid port 0 or 0 IP
                                                                    if src.port == 0 || src.ip == 0 {
                                                                        rejected_invalid += 1;
                                                                        continue;
                                                                    }

                                                                    // LowID check: IDs < 16_777_216 (0x01000000) are LowID client IDs, not public IP addresses.
                                                                    // Outbound direct TCP connections to them are impossible (they are behind NAT/firewall).
                                                                    if src.ip < 16_777_216 {
                                                                        low_id_ignored += 1;
                                                                        continue;
                                                                    }

                                                                    let octets = src.ip.to_le_bytes();
                                                                    let ip_addr = std::net::Ipv4Addr::from(octets);

                                                                    // Filter out broadcast / loopback (unless lab profile)
                                                                    if ip_addr.is_broadcast() || ip_addr.is_multicast() {
                                                                        rejected_invalid += 1;
                                                                        continue;
                                                                    }

                                                                    let addr_str = src.to_address_string();
                                                                    if !seen_addrs.insert(addr_str.clone()) {
                                                                        // Deduplicated within packet
                                                                        continue;
                                                                    }

                                                                    discovered.push(PeerSource {
                                                                        file_hash: file_hash_hex.clone(),
                                                                        address: addr_str,
                                                                        ip: src.ip,
                                                                        port: src.port,
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

                                                                info!(
                                                                    "FoundSources for {}: received={}, accepted={}, low_id_ignored={}, deduplicated={}, rejected_invalid={}",
                                                                    file_hash_hex,
                                                                    total_rcv,
                                                                    discovered.len(),
                                                                    low_id_ignored,
                                                                    total_rcv.saturating_sub(discovered.len() + low_id_ignored + rejected_invalid),
                                                                    rejected_invalid
                                                                );

                                                                if !discovered.is_empty() {
                                                                    let mut dl_guard = dl_service.lock().await;
                                                                    let _ = dl_guard.upsert_peer_sources(&file_hash_hex, &discovered);
                                                                }
                                                            }
                                                        }
                                                        ServerResponse::FoundSourcesObfu(fs) => {
                                                            let total_rcv = fs.sources.len();
                                                            emit_found_sources_obfu(&event_tx_clone, &sequence_clone, &fs);
                                                            if let Some(ref dl_service) = downloads_clone {
                                                                let file_hash_hex = hex::encode(fs.file_hash);
                                                                let now_ms = std::time::SystemTime::now()
                                                                    .duration_since(std::time::UNIX_EPOCH)
                                                                    .unwrap_or_default()
                                                                    .as_millis() as i64;
                                                                let mut discovered = Vec::new();
                                                                let mut seen_addrs = std::collections::HashSet::new();
                                                                let mut low_id_ignored = 0;
                                                                let mut rejected_invalid = 0;

                                                                for src in &fs.sources {
                                                                    if src.port == 0 || src.ip == 0 {
                                                                        rejected_invalid += 1;
                                                                        continue;
                                                                    }

                                                                    // LowID check: IDs < 16_777_216 (0x01000000) are LowID client IDs, not public IP addresses.
                                                                    if src.ip < 16_777_216 {
                                                                        low_id_ignored += 1;
                                                                        continue;
                                                                    }

                                                                    let octets = src.ip.to_le_bytes();
                                                                    let ip_addr = std::net::Ipv4Addr::from(octets);

                                                                    if ip_addr.is_broadcast() || ip_addr.is_multicast() {
                                                                        rejected_invalid += 1;
                                                                        continue;
                                                                    }

                                                                    let addr_str = src.to_address_string();
                                                                    if !seen_addrs.insert(addr_str.clone()) {
                                                                        continue;
                                                                    }

                                                                    discovered.push(PeerSource {
                                                                        file_hash: file_hash_hex.clone(),
                                                                        address: addr_str,
                                                                        ip: src.ip,
                                                                        port: src.port,
                                                                        origin: PeerSourceOrigin::Server,
                                                                        discovered_at_unix_ms: now_ms,
                                                                        last_seen_unix_ms: now_ms,
                                                                        status: PeerSourceStatus::Candidate,
                                                                        fail_count: 0,
                                                                        last_error: None,
                                                                        cooldown_until_unix_ms: None,
                                                                        user_hash: src.user_hash.map(|h| hex::encode(h)),
                                                                        crypt_options: src.obf_settings,
                                                                    });
                                                                }

                                                                info!(
                                                                    "FoundSourcesObfu for {}: received={}, accepted={}, low_id_ignored={}, deduplicated={}, rejected_invalid={}",
                                                                    file_hash_hex,
                                                                    total_rcv,
                                                                    discovered.len(),
                                                                    low_id_ignored,
                                                                    total_rcv.saturating_sub(discovered.len() + low_id_ignored + rejected_invalid),
                                                                    rejected_invalid
                                                                );

                                                                if !discovered.is_empty() {
                                                                    let mut dl_guard = dl_service.lock().await;
                                                                    let _ = dl_guard.upsert_peer_sources(&file_hash_hex, &discovered);
                                                                }
                                                            }
                                                        }
                                                        ServerResponse::Other { opcode, .. } => {
                                                            info!("Received other server opcode: 0x{:02x}", opcode);
                                                        }
                                                    }

                                                    if updated {
                                                        emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!("Error decoding server packet: {}", e);
                                                }
                                            }
                                        }
                                        Ok(None) => break, // Need more bytes
                                        Err(e) => {
                                            error!("Framing error from server {}: {}", target_addr, e);
                                            let mut g = inner_clone.lock().await;
                                            g.snapshot = ServerSessionSnapshot::default();
                                            g.outgoing_tx = None;
                                            emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
                                            return;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Error reading from server socket {}: {}", target_addr, e);
                                break;
                            }
                        }
                    }
                    _ = tokio::time::sleep_until(handshake_deadline), if is_connecting => {
                        warn!("Server handshake timed out after {} ms", handshake_timeout_ms);
                        break;
                    }
                }
            }

            // Teardown
            let mut g = inner_clone.lock().await;
            g.snapshot = ServerSessionSnapshot::default();
            g._permit = None;
            g.abort_handle = None;
            g.outgoing_tx = None;
            emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
        });

        guard.abort_handle = Some(task.abort_handle());
        guard._permit = Some(permit);

        Ok(())
    }

    fn emit_notification(&self, snapshot: &ServerSessionSnapshot) {
        emit_event(&self.event_tx, &self.sequence, snapshot);
    }
}

fn emit_event(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    snapshot: &ServerSessionSnapshot,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let notif = JsonRpcNotification::new(
        "server.sessionUpdated",
        serde_json::json!({
            "sequence": seq,
            "session": snapshot,
        }),
    );
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}

fn emit_search_results(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    sr: &SearchResult,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let files: Vec<serde_json::Value> = sr
        .files
        .iter()
        .map(|f| {
            serde_json::json!({
                "fileHash": hex::encode(f.file_hash),
                "fileName": f.name().unwrap_or_else(|| "Unknown".to_string()),
                "fileSize": f.size().unwrap_or(0),
                "sourceCount": f.source_count().unwrap_or(1),
            })
        })
        .collect();

    let notif = JsonRpcNotification::new(
        "server.searchResult",
        serde_json::json!({
            "sequence": seq,
            "files": files,
        }),
    );
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}

fn emit_found_sources(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    fs: &FoundSources,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let sources: Vec<serde_json::Value> = fs
        .sources
        .iter()
        .map(|s| {
            serde_json::json!({
                "address": s.to_address_string(),
                "ip": s.ip,
                "port": s.port,
            })
        })
        .collect();

    let notif = JsonRpcNotification::new(
        "server.foundSources",
        serde_json::json!({
            "sequence": seq,
            "fileHash": hex::encode(fs.file_hash),
            "sources": sources,
        }),
    );
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}

fn emit_found_sources_obfu(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    fs: &FoundSourcesObfu,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let sources: Vec<serde_json::Value> = fs
        .sources
        .iter()
        .map(|s| {
            serde_json::json!({
                "address": s.to_address_string(),
                "ip": s.ip,
                "port": s.port,
            })
        })
        .collect();

    let notif = JsonRpcNotification::new(
        "server.foundSources",
        serde_json::json!({
            "sequence": seq,
            "fileHash": hex::encode(fs.file_hash),
            "sources": sources,
        }),
    );
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}

