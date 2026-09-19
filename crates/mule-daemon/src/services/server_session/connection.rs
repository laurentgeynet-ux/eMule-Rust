use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{error, info, warn};

use mule_core::model::{PeerSource, PeerSourceOrigin, PeerSourceStatus};
use mule_network_policy::SocketPermit;
use mule_protocol::ed2k::{decode_server_response, Ed2kLimits, ServerLoginRequest, ServerResponse};
use mule_protocol::framing::{PacketCodec, RawPacket};

use super::events::{emit_event, emit_found_sources, emit_found_sources_obfu, emit_search_results};
use super::types::{ServerConnectionStatus, ServerSessionSnapshot};
use crate::services::downloads::DownloadService;

const MAX_SERVER_READ_BUFFER_SIZE: usize = 256 * 1024;

pub struct ActiveSession {
    pub snapshot: ServerSessionSnapshot,
    pub abort_handle: Option<tokio::task::AbortHandle>,
    pub permit: Option<SocketPermit>,
    pub outgoing_tx: Option<mpsc::Sender<RawPacket>>,
}

impl ActiveSession {
    pub fn new() -> Self {
        Self {
            snapshot: ServerSessionSnapshot::default(),
            abort_handle: None,
            permit: None,
            outgoing_tx: None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_connection_task(
    target_addr: SocketAddr,
    cancel_token: mule_network_policy::CancellationToken,
    connect_timeout_ms: u64,
    handshake_timeout_ms: u64,
    client_port: u16,
    nickname: String,
    user_hash: [u8; 16],
    mut outgoing_rx: mpsc::Receiver<RawPacket>,
    inner_clone: Arc<Mutex<ActiveSession>>,
    event_tx_clone: broadcast::Sender<String>,
    sequence_clone: Arc<AtomicU64>,
    downloads_clone: Option<Arc<Mutex<DownloadService>>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
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
                g.permit = None;
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
                g.permit = None;
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
            g.permit = None;
            g.abort_handle = None;
            g.outgoing_tx = None;
            emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
            return;
        }
        info!("Sent OP_LOGINREQUEST ({} wire bytes) to {}. Awaiting server response...", wire_buf.len(), target_addr);

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
                                                                if src.port == 0 || src.ip == 0 {
                                                                    rejected_invalid += 1;
                                                                    continue;
                                                                }

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
        g.permit = None;
        g.abort_handle = None;
        g.outgoing_tx = None;
        emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
    })
}
