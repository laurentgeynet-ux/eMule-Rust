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
use tokio::sync::{broadcast, Mutex};
use tracing::{error, info, warn};

use mule_network_policy::SocketPermit;
use mule_protocol::ed2k::{
    decode_server_response, Ed2kLimits, ServerLoginRequest, ServerResponse,
};
use mule_protocol::framing::PacketCodec;

use crate::error::JsonRpcError;
use crate::rpc::types::JsonRpcNotification;

/// Current lifecycle status of an eD2K server connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerConnectionStatus {
    /// No connection active.
    Disconnected,
    /// TCP handshake and login request in progress.
    Connecting,
    /// Connected and logged into the server.
    Connected,
    /// Teardown in progress.
    Disconnecting,
}

impl ServerConnectionStatus {
    /// String representation.
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
}

/// Thread-safe manager for the single eD2K server connection.
#[derive(Clone)]
pub struct ServerSessionManager {
    inner: Arc<Mutex<ActiveSession>>,
    event_tx: broadcast::Sender<String>,
    sequence: Arc<AtomicU64>,
}

impl ServerSessionManager {
    /// Creates a new `ServerSessionManager`.
    pub fn new(event_tx: broadcast::Sender<String>, sequence: Arc<AtomicU64>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ActiveSession {
                snapshot: ServerSessionSnapshot::default(),
                abort_handle: None,
                _permit: None,
            })),
            event_tx,
            sequence,
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

    /// Disconnects any active session and resets state.
    pub async fn disconnect(&self) -> Result<(), JsonRpcError> {
        let mut guard = self.inner.lock().await;
        if let Some(handle) = guard.abort_handle.take() {
            handle.abort();
        }
        guard._permit = None;
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

        let inner_clone = Arc::clone(&self.inner);
        let event_tx_clone = self.event_tx.clone();
        let sequence_clone = Arc::clone(&self.sequence);
        let cancel_token = permit.cancellation_token();

        let task = tokio::spawn(async move {
            let start_connect = std::time::Instant::now();

            // 1. TCP Connect with connect_timeout
            let stream_res = tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("Server connection cancelled by policy kill switch during connect");
                    Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Cancelled by network policy"))
                }
                res = tokio::time::timeout(Duration::from_millis(connect_timeout_ms), TcpStream::connect(target_addr)) => {
                    match res {
                        Ok(Ok(stream)) => Ok(stream),
                        Ok(Err(e)) => Err(e),
                        Err(_) => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "TCP connect timed out")),
                    }
                }
            };

            let mut stream = match stream_res {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to connect to eD2K server {}: {}", target_addr, e);
                    let mut g = inner_clone.lock().await;
                    g.snapshot = ServerSessionSnapshot::default();
                    g._permit = None;
                    g.abort_handle = None;
                    emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
                    return;
                }
            };

            let ping_ms = start_connect.elapsed().as_millis() as u32;

            // 2. Send OP_LOGINREQUEST
            let limits = Ed2kLimits::default();
            let login_req = ServerLoginRequest::new(user_hash, client_port, &nickname, 0x3c);
            let packet = match login_req.encode(&limits) {
                Ok(p) => p,
                Err(e) => {
                    error!("Failed to encode ServerLoginRequest: {}", e);
                    let mut g = inner_clone.lock().await;
                    g.snapshot = ServerSessionSnapshot::default();
                    g._permit = None;
                    g.abort_handle = None;
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
                warn!("Failed to write OP_LOGINREQUEST to {}: {}", target_addr, e);
                let mut g = inner_clone.lock().await;
                g.snapshot = ServerSessionSnapshot::default();
                g._permit = None;
                g.abort_handle = None;
                emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
                return;
            }

            // 3. Read loop with handshake timeout & PacketCodec
            let mut read_buf = BytesMut::with_capacity(8192);
            let codec = PacketCodec::new();
            let mut temp_buf = [0u8; 4096];
            let handshake_deadline = tokio::time::Instant::now() + Duration::from_millis(handshake_timeout_ms);

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("Server session aborted by policy kill switch");
                        break;
                    }
                    read_res = stream.read(&mut temp_buf) => {
                        match read_res {
                            Ok(0) => {
                                info!("Server closed TCP connection cleanly: {}", target_addr);
                                break;
                            }
                            Ok(n) => {
                                read_buf.extend_from_slice(&temp_buf[..n]);

                                loop {
                                    match codec.decode(&mut read_buf) {
                                        Ok(Some(raw_packet)) => {
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
                    _ = tokio::time::sleep_until(handshake_deadline), if guard_is_connecting(&inner_clone).await => {
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

async fn guard_is_connecting(inner: &Arc<Mutex<ActiveSession>>) -> bool {
    let g = inner.lock().await;
    g.snapshot.status == ServerConnectionStatus::Connecting
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
