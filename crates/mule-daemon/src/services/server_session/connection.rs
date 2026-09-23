// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

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
use mule_protocol::ed2k::{
    decode_server_response, Ed2kLimits, ServerLoginRequest, ServerResponse, PR_ED2K,
    PR_EMULE_COMPRESSED,
};
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
    /// Server address → (obf_tcp_port, obf_udp_port) learned from
    /// `OP_SERVERIDENT` tags `ST_TCPPORTOBFUSCATION`/`ST_UDPPORTOBFUSCATION`.
    /// Persists across reconnects (mirrors eMule's server.met storage).
    pub obf_ports: std::collections::HashMap<String, (u16, u16)>,
    /// Addresses where an obfuscated handshake already failed this process
    /// (eMule `SetTriedCrypt`) — subsequent connects go plaintext.
    pub obf_failed: std::collections::HashSet<String>,
}

impl Default for ActiveSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveSession {
    pub fn new() -> Self {
        Self {
            snapshot: ServerSessionSnapshot::default(),
            abort_handle: None,
            permit: None,
            outgoing_tx: None,
            obf_ports: std::collections::HashMap::new(),
            obf_failed: std::collections::HashSet::new(),
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
    inbound_clone: Option<crate::services::inbound_listener::InboundListenerService>,
    kad_clone: Option<Arc<crate::services::kad::KadService>>,
    prefer_obfuscation: bool,
    obfuscation_padding_max: u8,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let start_connect = std::time::Instant::now();

        // eMule CServerSocket::ConnectTo: if the server advertised an
        // obfuscation port (ST_TCPPORTOBFUSCATION in a previous SERVERIDENT)
        // and we prefer the crypt layer, connect to that port with the
        // DH768+RC4 handshake; on failure mark tried and retry plaintext.
        let address_str = target_addr.to_string();
        // eMule: bEncrypt = cryptPreferred && (SupportsObfuscationTCP() || !TriedCrypt())
        // — obfuscation is attempted at least once per server even without an
        // advertised port; on the first failure the server is marked tried and
        // subsequent connects go plaintext.
        let try_obf = {
            let g = inner_clone.lock().await;
            prefer_obfuscation && !g.obf_failed.contains(&address_str)
        };
        let obf_dial_port = {
            if try_obf {
                let g = inner_clone.lock().await;
                g.obf_ports
                    .get(&address_str)
                    .map(|(tcp, _)| *tcp)
                    .filter(|p| *p != 0)
            } else {
                None
            }
        };

        let dial_addr = match obf_dial_port {
            Some(p) => SocketAddr::new(target_addr.ip(), p),
            None => target_addr,
        };
        if try_obf {
            info!(
                "Connecting to eD2K server {} via protocol obfuscation{}",
                dial_addr,
                if obf_dial_port.is_some() {
                    " (dedicated port)"
                } else {
                    " (standard port)"
                }
            );
        }
        info!(
            "Attempting TCP connection to eD2K server {} (timeout: {} ms)...",
            dial_addr, connect_timeout_ms
        );

        // 1. TCP Connect with connect_timeout
        let stream_res = tokio::select! {
            _ = cancel_token.cancelled() => {
                warn!("Server connection to {} cancelled by policy kill switch during connect", dial_addr);
                Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Cancelled by network policy"))
            }
            res = tokio::time::timeout(Duration::from_millis(connect_timeout_ms), TcpStream::connect(dial_addr)) => {
                match res {
                    Ok(Ok(stream)) => {
                        info!("TCP socket connected successfully to eD2K server {} in {} ms", dial_addr, start_connect.elapsed().as_millis());
                        Ok(stream)
                    }
                    Ok(Err(e)) => {
                        warn!("TCP connect failed to eD2K server {}: {}", dial_addr, e);
                        Err(e)
                    }
                    Err(_) => {
                        warn!("TCP connect timed out after {} ms to eD2K server {}", connect_timeout_ms, dial_addr);
                        Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "TCP connect timed out"))
                    }
                }
            }
        };

        let mut stream: crate::services::obfuscation_stream::ObfuscatedStream<TcpStream> =
            match stream_res {
                Ok(s) => {
                    if try_obf {
                        match tokio::time::timeout(
                            Duration::from_millis(handshake_timeout_ms),
                            crate::services::obfuscation_stream::ObfuscatedStream::connect_server_outgoing(
                                s,
                                obfuscation_padding_max,
                            ),
                        )
                        .await
                        {
                            Ok(Ok(obf)) => {
                                info!("Obfuscated (DH768+RC4) server handshake completed with {dial_addr}");
                                obf
                            }
                            res => {
                                let why = match res {
                                    Ok(Err(e)) => e.to_string(),
                                    Err(_) => "handshake timeout".to_string(),
                                    _ => unreachable!(),
                                };
                                warn!(
                                    "Obfuscated server handshake with {dial_addr} failed ({why}) — falling back to plaintext {target_addr}"
                                );
                                let mut g = inner_clone.lock().await;
                                g.obf_failed.insert(address_str.clone());
                                drop(g);
                                match tokio::time::timeout(
                                    Duration::from_millis(connect_timeout_ms),
                                    TcpStream::connect(target_addr),
                                )
                                .await
                                {
                                    Ok(Ok(plain)) => crate::services::obfuscation_stream::ObfuscatedStream::plaintext(plain),
                                    _ => {
                                        warn!("Plaintext fallback connect to eD2K server {target_addr} failed");
                                        let mut g = inner_clone.lock().await;
                                        g.snapshot = ServerSessionSnapshot::default();
                                        g.permit = None;
                                        g.abort_handle = None;
                                        g.outgoing_tx = None;
                                        emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
                                        return;
                                    }
                                }
                            }
                        }
                    } else {
                        crate::services::obfuscation_stream::ObfuscatedStream::plaintext(s)
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to establish TCP connection to eD2K server {}: {}",
                        dial_addr, e
                    );
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
        info!(
            "Sending OP_LOGINREQUEST to eD2K server {} (nick='{}', port={}, hash={:?})",
            target_addr,
            nickname,
            client_port,
            hex::encode(user_hash)
        );
        let packet = match login_req.encode(&limits) {
            Ok(p) => p,
            Err(e) => {
                error!(
                    "Failed to encode ServerLoginRequest for {}: {}",
                    target_addr, e
                );
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
            warn!(
                "Failed to write OP_LOGINREQUEST bytes to eD2K server {}: {}",
                target_addr, e
            );
            let mut g = inner_clone.lock().await;
            g.snapshot = ServerSessionSnapshot::default();
            g.permit = None;
            g.abort_handle = None;
            g.outgoing_tx = None;
            emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
            return;
        }
        info!(
            "Sent OP_LOGINREQUEST ({} wire bytes) to {}. Awaiting server response...",
            wire_buf.len(),
            target_addr
        );

        // 3. Read loop with handshake timeout & PacketCodec
        let mut read_buf = BytesMut::with_capacity(8192);
        let codec = PacketCodec::new();
        let mut temp_buf = [0u8; 4096];
        let handshake_deadline =
            tokio::time::Instant::now() + Duration::from_millis(handshake_timeout_ms);
        let mut is_connecting = true;
        let mut publish_login: Option<mule_protocol::ed2k::ServerIdChange> = None;
        // Credentials from the last OP_IDCHANGE, kept for periodic
        // OP_OFFERFILES re-publication (eMule keeps the shared file list
        // fresh on the server and uses an empty offer packet as keep-alive).
        let mut last_idc: Option<mule_protocol::ed2k::ServerIdChange> = None;
        let republish_period = Duration::from_secs(30 * 60);
        let mut republish_timer = tokio::time::interval_at(
            tokio::time::Instant::now() + republish_period,
            republish_period,
        );

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
                                        // eMule OP_PACKEDPROT (0xD4): payload is zlib-compressed;
                                        // unpack with eMule's 250 KB server cap, then dispatch on
                                        // opcode as a normal PR_ED2K packet.
                                        let (resp_protocol, resp_payload);
                                        if raw_packet.protocol == PR_EMULE_COMPRESSED {
                                            match unpack_packed_packet(&raw_packet.payload) {
                                                Some(p) => {
                                                    resp_protocol = PR_ED2K;
                                                    resp_payload = p;
                                                }
                                                None => {
                                                    warn!("Server {} sent an undecodable packed packet (opcode=0x{:02X})", target_addr, raw_packet.opcode);
                                                    continue;
                                                }
                                            }
                                        } else {
                                            resp_protocol = raw_packet.protocol;
                                            resp_payload = raw_packet.payload.to_vec();
                                        }
                                        match decode_server_response(resp_protocol, raw_packet.opcode, &resp_payload, &limits) {
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
                                                        // A HighID encodes our public IPv4 (eD2K convention:
                                                        // id = a<<24|b<<16|c<<8|d) — Kad uses it as the
                                                        // advertised address in PUBLISH_SOURCE contacts.
                                                        if let Some(kad) = &kad_clone {
                                                            kad.set_public_ipv4(if idc.is_high_id() {
                                                                Some(std::net::Ipv4Addr::from(idc.client_id))
                                                            } else {
                                                                None
                                                            });
                                                        }
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
                                                        publish_login = Some(idc);
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
                                                        let obf_tcp = ident.obfuscation_tcp_port();
                                                        let obf_udp = ident.obfuscation_udp_port();
                                                        if obf_tcp.is_some() || obf_udp.is_some() {
                                                            info!(
                                                                "Server {} advertises obfuscation ports: TCP={:?} UDP={:?}",
                                                                target_addr, obf_tcp, obf_udp
                                                            );
                                                            g.obf_ports.insert(
                                                                target_addr.to_string(),
                                                                (
                                                                    obf_tcp.unwrap_or(0),
                                                                    obf_udp.unwrap_or(0),
                                                                ),
                                                            );
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
                                                                if src.ip == 0 {
                                                                    rejected_invalid += 1;
                                                                    continue;
                                                                }

                                                                // LowID source: keep it — reachable only via
                                                                // server callback (OP_CALLBACKREQUEST).
                                                                let addr_str = if src.ip < 16_777_216 {
                                                                    low_id_ignored += 1;
                                                                    format!("lowid:{}", src.ip)
                                                                } else {
                                                                    if src.port == 0 {
                                                                        rejected_invalid += 1;
                                                                        continue;
                                                                    }
                                                                    let ip_addr = std::net::Ipv4Addr::from(src.ip.to_le_bytes());
                                                                    if ip_addr.is_broadcast() || ip_addr.is_multicast() {
                                                                        rejected_invalid += 1;
                                                                        continue;
                                                                    }
                                                                    src.to_address_string()
                                                                };
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
                                                                    buddy_id: None,
software: None,
                                                                    availability: None,
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
                                                                if src.ip == 0 {
                                                                    rejected_invalid += 1;
                                                                    continue;
                                                                }

                                                                // LowID source: keep it — reachable only via
                                                                // server callback (OP_CALLBACKREQUEST).
                                                                let addr_str = if src.ip < 16_777_216 {
                                                                    low_id_ignored += 1;
                                                                    format!("lowid:{}", src.ip)
                                                                } else {
                                                                    if src.port == 0 {
                                                                        rejected_invalid += 1;
                                                                        continue;
                                                                    }
                                                                    let ip_addr = std::net::Ipv4Addr::from(src.ip.to_le_bytes());
                                                                    if ip_addr.is_broadcast() || ip_addr.is_multicast() {
                                                                        rejected_invalid += 1;
                                                                        continue;
                                                                    }
                                                                    src.to_address_string()
                                                                };
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
                                                                    user_hash: src.user_hash.map(hex::encode),
software: None,
                                                                    availability: None,
                                                                    crypt_options: src.obf_settings,
                                                                    buddy_id: None,
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
                                                    ServerResponse::CallbackRequested(cb) => {
                                                        // A client asked the server to relay a callback
                                                        // to us (we are the LowID target). eMule
                                                        // `TryToConnect`s back to the requester; the
                                                        // connect-back runs as an upload-serving session.
                                                        let ip_addr = std::net::Ipv4Addr::from(cb.ip.to_le_bytes());
                                                        info!(
                                                            "OP_CALLBACKREQUESTED: peer {}:{} wants us to connect back (crypt_opts={:?}, user_hash={})",
                                                            ip_addr,
                                                            cb.port,
                                                            cb.crypt_options,
                                                            cb.user_hash.map(hex::encode).as_deref().unwrap_or("<none>")
                                                        );
                                                        if let Some(inbound) = inbound_clone.clone() {
                                                            let target = SocketAddr::new(
                                                                std::net::IpAddr::V4(ip_addr),
                                                                cb.port,
                                                            );
                                                            let uh = cb.user_hash;
                                                            let crypt = cb.crypt_options.unwrap_or(0);
                                                            let our_id = g.snapshot.client_id.unwrap_or(0);
                                                            let srv_addr = g
                                                                .snapshot
                                                                .server_address
                                                                .as_deref()
                                                                .and_then(|a| a.parse::<SocketAddr>().ok());
                                                            let (srv_ip, srv_port) = srv_addr
                                                                .map(|a| match a.ip() {
                                                                    std::net::IpAddr::V4(v4) => {
                                                                        (u32::from_be_bytes(v4.octets()), a.port())
                                                                    }
                                                                    _ => (0, a.port()),
                                                                })
                                                                .unwrap_or((0, 0));
                                                            tokio::spawn(async move {
                                                                if let Err(e) = inbound
                                                                    .connect_back(
                                                                        target, uh, crypt, our_id,
                                                                        srv_ip, srv_port,
                                                                    )
                                                                    .await
                                                                {
                                                                    warn!(
                                                                        "Callback connect-back to {target} failed: {e}"
                                                                    );
                                                                }
                                                            });
                                                        }
                                                    }
                                                    ServerResponse::ServerList { servers } => {
                // OP_SERVERLIST: merge advertised servers into the local DB
                // with low preference (eMule adds them as regular server.met
                // entries; IP-filtered addresses are skipped).
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let entries: Vec<mule_core::model::Ed2kServer> = servers
                    .iter()
                    .filter(|(ip, port)| *port != 0 && *ip != 0)
                    .map(|&(ip, port)| {
                        let ip_be = std::net::Ipv4Addr::from(ip.to_be_bytes());
                        mule_core::model::Ed2kServer {
                            address: format!("{ip_be}:{port}"),
                            ip: ip_be.to_string(),
                            port,
                            name: String::new(),
                            description: "via OP_SERVERLIST".to_string(),
                            ping: 0,
                            preference: 0,
                            failed_count: 0,
                            dyn_ip: None,
                            version: None,
                            filter_status: mule_core::model::ServerFilterStatus::Allowed,
                            last_seen_unix_ms: Some(now_ms),
                            created_at_unix_ms: now_ms,
                            updated_at_unix_ms: now_ms,
                        }
                    })
                    .collect();
                if !entries.is_empty() {
                    if let Some(ref dl_service) = downloads_clone {
                        let mut dl_guard = dl_service.lock().await;
                        match dl_guard.upsert_servers(&entries) {
                            Ok(n) => tracing::info!(
                                "Merged {n} server(s) advertised by OP_SERVERLIST into server list"
                            ),
                            Err(e) => {
                                tracing::warn!("Failed to persist OP_SERVERLIST servers: {e}")
                            }
                        }
                    }
                }
            }
            ServerResponse::CallbackFail { client_id } => {
                                                        // eMule servers send OP_CALLBACK_FAIL
                                                        // with an empty payload — the target
                                                        // client_id is not echoed back.
                                                        match client_id {
                                                            Some(id) => warn!(
                                                                "OP_CALLBACK_FAIL: server could not relay callback to LowID client {id}"
                                                            ),
                                                            None => warn!(
                                                                "OP_CALLBACK_FAIL: server could not relay a pending callback (target offline or unreachable)"
                                                            ),
                                                        }
                                                    }
                                                    ServerResponse::Other { opcode, .. } => {
                                                        info!("Received other server opcode: 0x{:02x}", opcode);
                                                    }
                                                }

                                                if updated {
                                                    emit_event(&event_tx_clone, &sequence_clone, &g.snapshot);
                                                }
                                                drop(g);

                                                // eMule `CSharedFileList::SendListToServer`:
                                                // once the server assigned our ID, publish
                                                // the shared file list via OP_OFFERFILES.
                                                if let Some(idc) = publish_login.take() {
                                                    last_idc = Some(idc.clone());
                                                    publish_shared_files(
                                                        &mut stream,
                                                        target_addr,
                                                        idc.client_id,
                                                        idc.tcp_flags,
                                                        client_port,
                                                        &limits,
                                                        &downloads_clone,
                                                    )
                                                    .await;
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
                _ = republish_timer.tick(), if !is_connecting && last_idc.is_some() => {
                    if let Some(ref idc) = last_idc {
                        publish_shared_files(
                            &mut stream,
                            target_addr,
                            idc.client_id,
                            idc.tcp_flags,
                            client_port,
                            &limits,
                            &downloads_clone,
                        )
                        .await;
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

/// Publishes the shared file list to the connected server via `OP_OFFERFILES`
/// (eMule `CSharedFileList::SendListToServer`, capped at 200 files).
async fn publish_shared_files(
    stream: &mut crate::services::obfuscation_stream::ObfuscatedStream<TcpStream>,
    target_addr: SocketAddr,
    client_id: u32,
    tcp_flags: u32,
    client_port: u16,
    limits: &Ed2kLimits,
    downloads: &Option<Arc<Mutex<DownloadService>>>,
) {
    const MAX_OFFER_FILES: usize = 200;

    let Some(dl_service) = downloads else { return };
    let mut shared = {
        let dl_svc = dl_service.lock().await;
        dl_svc.list_all_shared_files().unwrap_or_default()
    };
    // eMule `SendListToServer`: files are offered sorted by upload priority
    // (highest first), capped at the server soft limit (≤200).
    shared.sort_by_key(|sf| {
        std::cmp::Reverse(crate::services::upload_queue::file_prio_num(&sf.up_priority))
    });

    let mut files = Vec::with_capacity(shared.len().min(MAX_OFFER_FILES));
    for sf in shared.iter().filter(|sf| sf.enabled) {
        if files.len() >= MAX_OFFER_FILES {
            break;
        }
        let Ok(hash_bytes) = <[u8; 16]>::try_from(
            hex::decode(sf.file_hash.trim())
                .unwrap_or_default()
                .as_slice(),
        ) else {
            continue;
        };
        if let Some(of) = mule_protocol::ed2k::OfferedFile::for_server(
            hash_bytes,
            &sf.file_name,
            sf.file_size,
            client_id,
            client_port,
            tcp_flags,
            sf.is_partial,
        ) {
            files.push(of);
        }
    }

    // Empty OP_OFFERFILES doubles as the server keep-alive packet
    // (eMule `KeepConnectionAlive` semantics).
    let count = files.len();
    let offer = mule_protocol::ed2k::OfferFiles { files };
    match offer.encode(limits) {
        Ok(pkt) => {
            let mut out = Vec::with_capacity(6 + pkt.payload.len());
            out.push(pkt.protocol);
            out.extend_from_slice(&(pkt.frame_len() as u32).to_le_bytes());
            out.push(pkt.opcode);
            out.extend_from_slice(&pkt.payload);
            match stream.write_all(&out).await {
                Ok(()) => info!(
                    "Published {} shared file(s) to server {} via OP_OFFERFILES",
                    count, target_addr
                ),
                Err(e) => warn!("Failed to send OP_OFFERFILES to {}: {}", target_addr, e),
            }
        }
        Err(e) => warn!("Failed to encode OP_OFFERFILES for {}: {}", target_addr, e),
    }
}

/// eMule `Packet::UnPackPacket` for server packets: zlib-inflate the packed
/// payload with a hard 250 KB decompressed-size cap (eMule passes 250000).
/// Returns `None` on truncated/oversized/corrupt data.
fn unpack_packed_packet(payload: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read;
    const MAX_DECOMPRESSED: u64 = 250_000;
    let decoder = flate2::read::ZlibDecoder::new(payload).take(MAX_DECOMPRESSED + 1);
    let mut out = Vec::new();
    let mut decoder = decoder;
    decoder.read_to_end(&mut out).ok()?;
    if out.len() as u64 > MAX_DECOMPRESSED {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::unpack_packed_packet;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    #[test]
    fn unpack_packed_packet_roundtrip() {
        let payload = b"search-result-payload".repeat(64);
        assert_eq!(unpack_packed_packet(&zlib(&payload)).unwrap(), payload);
    }

    #[test]
    fn unpack_packed_packet_rejects_corrupt() {
        assert!(unpack_packed_packet(b"not zlib at all").is_none());
        let mut truncated = zlib(b"hello world");
        truncated.truncate(truncated.len() / 2);
        assert!(unpack_packed_packet(&truncated).is_none());
    }

    #[test]
    fn unpack_packed_packet_enforces_250kb_cap() {
        // ~300 KB of zeros compress to a few hundred bytes — a bomb shape.
        let big = vec![0u8; 300_000];
        assert!(unpack_packed_packet(&zlib(&big)).is_none());
        // Just under the cap still passes.
        let ok = vec![0u8; 250_000];
        assert_eq!(unpack_packed_packet(&zlib(&ok)).unwrap().len(), 250_000);
    }
}
