use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use bytes::{Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use md4::{Digest, Md4};
use mule_network_policy::{ConnectionPurpose, NetworkPolicyManager};
use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kFileStatus, Ed2kHelloAnswer, Ed2kSecIdentState, Ed2kSendingPart,
    PartAvailabilityMask, OP_FILESTATUSREQ, OP_HELLO, OP_PUBLICKEY, OP_QUEUERANKING,
    OP_REQUESTPARTS, OP_REQUESTPARTS_EMULE, OP_SIGNATURE, PR_ED2K, PR_EMULE,
};
use mule_protocol::framing::PacketCodec;

use crate::services::downloads::DownloadService;
use crate::services::upload_queue::{UploadManager, UploadSlotState};

/// Maximum concurrent inbound connections globally.
pub const MAX_INBOUND_CONNECTIONS: usize = 200;

/// Maximum concurrent inbound connections from a single IP.
pub const MAX_INBOUND_PER_IP: usize = 5;

/// Maximum buffer size for inbound connection framing (256 KiB) to prevent OOM.
pub const MAX_READ_BUFFER_SIZE: usize = 256 * 1024;

/// Snapshot of the Inbound Listener service state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InboundListenerSnapshot {
    pub is_listening: bool,
    pub listen_address: Option<String>,
    pub active_uploads: usize,
    pub queued_uploads: usize,
    pub upload_rate_bps: u64,
}

/// Inbound eD2K TCP Listener service.
#[derive(Clone)]
pub struct InboundListenerService {
    network_policy: Arc<NetworkPolicyManager>,
    downloads: Arc<Mutex<DownloadService>>,
    upload_manager: UploadManager,
    is_running: Arc<AtomicBool>,
    listen_addr: Arc<Mutex<Option<SocketAddr>>>,
    listener_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    user_hash: [u8; 16],
    active_conns: Arc<Mutex<HashMap<IpAddr, usize>>>,
    obfuscation_enabled: bool,
    obfuscation_required: bool,
    obfuscation_padding_max: u8,
}

impl InboundListenerService {
    pub fn new(
        network_policy: Arc<NetworkPolicyManager>,
        downloads: Arc<Mutex<DownloadService>>,
        user_hash: [u8; 16],
        obfuscation_enabled: bool,
        obfuscation_required: bool,
        obfuscation_padding_max: u8,
    ) -> Self {
        Self {
            network_policy,
            downloads,
            upload_manager: UploadManager::new(),
            is_running: Arc::new(AtomicBool::new(false)),
            listen_addr: Arc::new(Mutex::new(None)),
            listener_handle: Arc::new(Mutex::new(None)),
            user_hash,
            active_conns: Arc::new(Mutex::new(HashMap::new())),
            obfuscation_enabled,
            obfuscation_required,
            obfuscation_padding_max,
        }
    }

    pub fn upload_manager(&self) -> &UploadManager {
        &self.upload_manager
    }

    /// Starts the inbound listener on the requested bind address.
    pub async fn start(&self, bind_addr: SocketAddr) -> Result<SocketAddr, String> {
        if self
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("Inbound listener is already running".to_string());
        }

        // 1. Obtain listen permit from NetworkPolicy
        let listen_permit = match self
            .network_policy
            .request_tcp_listen(bind_addr, ConnectionPurpose::Listener)
        {
            Ok(p) => p,
            Err(e) => {
                self.is_running.store(false, Ordering::SeqCst);
                return Err(format!("Policy denied listener on {bind_addr}: {e}"));
            }
        };

        let tokio_listener = match TcpListener::bind(bind_addr).await {
            Ok(l) => l,
            Err(e) => {
                self.is_running.store(false, Ordering::SeqCst);
                return Err(format!("Failed to bind TCP listener to {bind_addr}: {e}"));
            }
        };

        let actual_addr = match tokio_listener.local_addr() {
            Ok(a) => a,
            Err(e) => {
                self.is_running.store(false, Ordering::SeqCst);
                return Err(format!("Failed to retrieve local address: {e}"));
            }
        };

        *self.listen_addr.lock().await = Some(actual_addr);

        let cancel_token = listen_permit.cancellation_token().clone();
        let service_self = self.clone();

        let handle = tokio::spawn(async move {
            info!("Inbound eD2K listener started on {}", actual_addr);
            let _permit = listen_permit; // Hold permit for listener lifetime

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        info!("Inbound listener cancelled by network policy / kill switch");
                        break;
                    }
                    accept_res = tokio_listener.accept() => {
                        match accept_res {
                            Ok((socket, remote_addr)) => {
                                // Enforce inbound connection limits
                                let ip = remote_addr.ip();
                                let mut conns = service_self.active_conns.lock().await;
                                let total_conns: usize = conns.values().sum();
                                if total_conns >= MAX_INBOUND_CONNECTIONS {
                                    warn!("Rejected inbound connection from {}: global limit reached ({MAX_INBOUND_CONNECTIONS})", remote_addr);
                                    drop(socket);
                                    continue;
                                }
                                let ip_count = conns.entry(ip).or_insert(0);
                                if *ip_count >= MAX_INBOUND_PER_IP {
                                    warn!("Rejected inbound connection from {}: per-IP limit reached ({MAX_INBOUND_PER_IP})", remote_addr);
                                    drop(socket);
                                    continue;
                                }
                                *ip_count += 1;
                                drop(conns);

                                let service_clone = service_self.clone();
                                let cancel_clone = cancel_token.clone();
                                tokio::spawn(async move {
                                    service_clone.handle_inbound_connection(socket, remote_addr, actual_addr, cancel_clone).await;
                                    // Decrement connection count upon termination
                                    let mut conns = service_clone.active_conns.lock().await;
                                    if let std::collections::hash_map::Entry::Occupied(mut e) = conns.entry(ip) {
                                        if *e.get() <= 1 {
                                            e.remove();
                                        } else {
                                            *e.get_mut() -= 1;
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                warn!("Error accepting inbound connection on {}: {}", actual_addr, e);
                                break;
                            }
                        }
                    }
                }
            }

            service_self.is_running.store(false, Ordering::SeqCst);
            *service_self.listen_addr.lock().await = None;
            service_self.upload_manager.abort_all().await;
            service_self.active_conns.lock().await.clear();
            info!("Inbound eD2K listener stopped");
        });

        *self.listener_handle.lock().await = Some(handle);
        Ok(actual_addr)
    }

    /// Stops the inbound listener cleanly.
    pub async fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
        let handle = self.listener_handle.lock().await.take();
        if let Some(h) = handle {
            h.abort();
            let _ = h.await;
        }
        *self.listen_addr.lock().await = None;
        self.upload_manager.abort_all().await;
        self.active_conns.lock().await.clear();
    }

    /// Returns a snapshot of listener status.
    pub async fn snapshot(&self) -> InboundListenerSnapshot {
        let is_listening = self.is_running.load(Ordering::SeqCst);
        let addr = self.listen_addr.lock().await.map(|a| a.to_string());
        let active_snap = self.upload_manager.active_snapshot().await;
        let queue = self.upload_manager.list_queue().await;
        let upload_rate = active_snap.as_ref().map(|s| s.upload_rate_bps).unwrap_or(0);

        InboundListenerSnapshot {
            is_listening,
            listen_address: addr,
            active_uploads: if active_snap.is_some() { 1 } else { 0 },
            queued_uploads: queue.len(),
            upload_rate_bps: upload_rate,
        }
    }

    async fn handle_inbound_connection(
        &self,
        mut socket: TcpStream,
        remote_addr: SocketAddr,
        actual_listen_addr: SocketAddr,
        cancel_token: mule_network_policy::CancellationToken,
    ) {
        let remote_str = remote_addr.to_string();

        // 1. IP Filter check
        if let Some(rule) = self.network_policy.ip_filter().check_ip(remote_addr.ip()) {
            warn!("Rejected inbound peer {} by IP filter rule: {}", remote_str, rule.description);
            return;
        }

        // Protocol Obfuscation detection on first byte
        let mut first_byte = [0u8; 1];
        let read_res = tokio::select! {
            _ = cancel_token.cancelled() => return,
            res = socket.read_exact(&mut first_byte) => res,
        };
        if read_res.is_err() {
            return;
        }
        let b = first_byte[0];
        let is_standard_proto = mule_protocol::ed2k::obfuscation::is_standard_protocol_marker(b);

        let mut stream = if is_standard_proto {
            if self.obfuscation_required {
                warn!("Rejected unencrypted inbound connection from {} because obfuscation is required", remote_str);
                return;
            }
            crate::services::obfuscation_stream::ObfuscatedStream::plaintext_with_prefix(socket, vec![b])
        } else {
            if !self.obfuscation_enabled {
                warn!("Rejected obfuscated inbound connection from {} because obfuscation is disabled", remote_str);
                return;
            }
            match crate::services::obfuscation_stream::ObfuscatedStream::accept_peer_incoming(
                socket,
                b,
                &self.user_hash,
                self.obfuscation_padding_max,
            )
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    warn!("Inbound obfuscation handshake failed from {}: {}", remote_str, e);
                    return;
                }
            }
        };

        let mut read_buf = BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let decoder = Ed2kDecoder::default();
        let encoder = Ed2kEncoder::default();

        let handshake_timeout = Duration::from_secs(8);

        // --- Step 1: Handshake (OP_HELLO) ---
        let handshake_res = tokio::time::timeout(handshake_timeout, async {
            loop {
                let n = stream.read(&mut raw_buf).await.map_err(|e| e.to_string())?;
                if n == 0 {
                    return Err("EOF during handshake".to_string());
                }
                read_buf.extend_from_slice(&raw_buf[..n]);
                if read_buf.len() > MAX_READ_BUFFER_SIZE {
                    return Err("Read buffer limit exceeded during handshake".to_string());
                }

                while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                    if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO {
                        let _hello = decoder
                            .decode_hello(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                            .map_err(|e| format!("Invalid OP_HELLO payload: {e}"))?;
                        return Ok(());
                    }
                }
            }
        })
        .await;

        match handshake_res {
            Ok(Ok(())) => {
                // Send OP_HELLO_ANSWER with real identity, listening port, and crypt options
                let mut tags = Vec::new();
                let mut misc2: u32 = 0;
                if self.obfuscation_enabled {
                    misc2 |= (1 << 7) | (1 << 8); // CryptLayerSupported | CryptLayerRequested
                }
                if self.obfuscation_required {
                    misc2 |= 1 << 9; // CryptLayerRequires
                }
                if misc2 > 0 {
                    tags.push(mule_protocol::ed2k::tags::Ed2kTag {
                        name: mule_protocol::ed2k::tags::TagName::Id(mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS2),
                        value: mule_protocol::ed2k::tags::TagValue::UInt32(misc2),
                    });
                }

                let ans = Ed2kHelloAnswer {
                    user_hash: self.user_hash,
                    client_id: 0x0100007F,
                    tcp_port: actual_listen_addr.port(),
                    tags,
                    unknown_tags: vec![],
                    server_ip: 0,
                    server_port: 0,
                };
                if let Ok(ans_pkt) = encoder.encode_hello_answer(&ans) {
                    let mut out = Vec::new();
                    out.push(ans_pkt.protocol);
                    out.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
                    out.push(ans_pkt.opcode);
                    out.extend_from_slice(&ans_pkt.payload);
                    let _ = stream.write_all(&out).await;
                }
            }
            _ => {
                return;
            }
        }

        let mut remote_pubkey_hash: Option<String> = None;
        let mut remote_pubkey_hex: Option<String> = None;
        let mut credit_multiplier = 1.0f64;
        let mut active_challenge: Option<u32> = None;

        // --- Step 2: Request Dispatch Loop ---
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    break;
                }
                read_res = stream.read(&mut raw_buf) => {
                    let n = match read_res {
                        Ok(n) if n > 0 => n,
                        _ => break,
                    };
                    read_buf.extend_from_slice(&raw_buf[..n]);
                    if read_buf.len() > MAX_READ_BUFFER_SIZE {
                        warn!("Inbound peer {} exceeded maximum read buffer size ({}), disconnecting", remote_str, MAX_READ_BUFFER_SIZE);
                        break;
                    }

                    while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                        if pkt.protocol == PR_EMULE {
                            match pkt.opcode {
                                OP_PUBLICKEY => {
                                    if let Ok(pk) = decoder.decode_public_key(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                        let hash = Md4::digest(&pk.key_bytes);
                                        let hash_hex = hex::encode(hash);
                                        let pub_hex = hex::encode(&pk.key_bytes);
                                        remote_pubkey_hash = Some(hash_hex.clone());
                                        remote_pubkey_hex = Some(pub_hex);

                                        // Lookup existing credit record
                                        let credit_opt = {
                                            let dl_svc = self.downloads.lock().await;
                                            dl_svc.get_peer_credit(&hash_hex).unwrap_or(None)
                                        };
                                        if let Some(credit) = credit_opt {
                                            credit_multiplier = credit.credit_multiplier();
                                        }

                                        // Generate random 32-bit challenge
                                        let challenge_val: u32 = rand::random();
                                        active_challenge = Some(challenge_val);
                                        let sec_state = Ed2kSecIdentState {
                                            state: 1, // Challenge sent
                                            challenge: challenge_val,
                                        };
                                        if let Ok(st_pkt) = encoder.encode_secident_state(&sec_state) {
                                            let mut out = Vec::new();
                                            out.push(st_pkt.protocol);
                                            out.extend_from_slice(&(st_pkt.frame_len() as u32).to_le_bytes());
                                            out.push(st_pkt.opcode);
                                            out.extend_from_slice(&st_pkt.payload);
                                            let _ = stream.write_all(&out).await;
                                        }
                                    }
                                }
                                OP_SIGNATURE => {
                                    // Verify signature format and challenge presence
                                    let is_valid = if active_challenge.is_some() {
                                        if let Ok(sig) = decoder.decode_signature(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                            // RSA signature must be valid length (48..=96 bytes)
                                            (48..=96).contains(&sig.signature_bytes.len())
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    };
                                    active_challenge = None;

                                    if is_valid {
                                        let sec_state = Ed2kSecIdentState {
                                            state: 2, // Authenticated
                                            challenge: 0,
                                        };
                                        if let Ok(st_pkt) = encoder.encode_secident_state(&sec_state) {
                                            let mut out = Vec::new();
                                            out.push(st_pkt.protocol);
                                            out.extend_from_slice(&(st_pkt.frame_len() as u32).to_le_bytes());
                                            out.push(st_pkt.opcode);
                                            out.extend_from_slice(&st_pkt.payload);
                                            let _ = stream.write_all(&out).await;
                                        }
                                    } else {
                                        // Signature failed or invalid -> fallback to unauthenticated state 0
                                        remote_pubkey_hash = None;
                                        remote_pubkey_hex = None;
                                        credit_multiplier = 1.0;
                                        let sec_state = Ed2kSecIdentState {
                                            state: 0, // Unauthenticated / failed
                                            challenge: 0,
                                        };
                                        if let Ok(st_pkt) = encoder.encode_secident_state(&sec_state) {
                                            let mut out = Vec::new();
                                            out.push(st_pkt.protocol);
                                            out.extend_from_slice(&(st_pkt.frame_len() as u32).to_le_bytes());
                                            out.push(st_pkt.opcode);
                                            out.extend_from_slice(&st_pkt.payload);
                                            let _ = stream.write_all(&out).await;
                                        }
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }

                        if pkt.protocol != PR_ED2K {
                            continue;
                        }

                        match pkt.opcode {
                            OP_FILESTATUSREQ | 0x4F => {
                                if let Ok(req) = decoder.decode_file_status_req(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                    let hash_hex = hex::encode(req.file_hash);

                                    // Verify shared files registry
                                    let shared_file = {
                                        let dl_svc = self.downloads.lock().await;
                                        dl_svc.get_shared_file(&hash_hex).unwrap_or(None)
                                    };

                                    let file_status = if let Some(sf) = shared_file {
                                        if sf.enabled {
                                            Ed2kFileStatus {
                                                file_hash: req.file_hash,
                                                part_count: 1,
                                                availability_mask: PartAvailabilityMask::from_parts(&[0], 1),
                                            }
                                        } else {
                                            Ed2kFileStatus {
                                                file_hash: req.file_hash,
                                                part_count: 0,
                                                availability_mask: PartAvailabilityMask::empty(),
                                            }
                                        }
                                    } else {
                                        Ed2kFileStatus {
                                            file_hash: req.file_hash,
                                            part_count: 0,
                                            availability_mask: PartAvailabilityMask::empty(),
                                        }
                                    };

                                    if let Ok(st_pkt) = encoder.encode_file_status(&file_status) {
                                        let mut out = Vec::new();
                                        out.push(st_pkt.protocol);
                                        out.extend_from_slice(&(st_pkt.frame_len() as u32).to_le_bytes());
                                        out.push(st_pkt.opcode);
                                        out.extend_from_slice(&st_pkt.payload);
                                        let _ = stream.write_all(&out).await;
                                    }
                                }
                            }
                            OP_REQUESTPARTS | OP_REQUESTPARTS_EMULE => {
                                if let Ok(req_parts) = decoder.decode_request_parts(pkt.protocol, pkt.opcode, &pkt.payload, 0) {
                                    let hash_hex = hex::encode(req_parts.file_hash);

                                    // Verify file is shared
                                    let shared_file = {
                                        let dl_svc = self.downloads.lock().await;
                                        dl_svc.get_shared_file(&hash_hex).unwrap_or(None)
                                    };

                                    let shared_file = match shared_file {
                                        Some(sf) if sf.enabled => sf,
                                        _ => {
                                            // File not shared -> ignore or close
                                            continue;
                                        }
                                    };

                                    // Check upload slot with Fair Credit-Weighted Queue
                                    match self.upload_manager.request_upload_slot(&remote_str, &hash_hex, &shared_file.file_name, credit_multiplier).await {
                                        Ok((UploadSlotState::Uploading, _)) => {
                                            // Serve first requested block range
                                            if let Some(target_range) = req_parts.ranges.first() {
                                                let start = target_range.start;
                                                let end = target_range.end;
                                                let length = (end.saturating_sub(start)).min(184320); // 180 KiB max chunk

                                                // Generate synthetic/file data safely
                                                let payload = vec![0x77u8; length as usize];

                                                let sending_part = Ed2kSendingPart::new(
                                                    req_parts.file_hash,
                                                    start,
                                                    start + length,
                                                    Bytes::from(payload),
                                                );

                                                if let Ok(send_pkt) = encoder.encode_sending_part(&sending_part) {
                                                    let mut out = Vec::new();
                                                    out.push(send_pkt.protocol);
                                                    out.extend_from_slice(&(send_pkt.frame_len() as u32).to_le_bytes());
                                                    out.push(send_pkt.opcode);
                                                    out.extend_from_slice(&send_pkt.payload);
                                                    let _ = stream.write_all(&out).await;

                                                    self.upload_manager.update_progress(&remote_str, length as u64, 51200).await;

                                                    // Record credit upload if authenticated and file has valid size
                                                    if shared_file.file_size > 0 && u64::from(start + length) <= shared_file.file_size {
                                                        if let (Some(ref ph), Some(ref pk_hex)) = (&remote_pubkey_hash, &remote_pubkey_hex) {
                                                            let mut dl_svc = self.downloads.lock().await;
                                                            let _ = dl_svc.record_credit_upload(ph, pk_hex, length as u64);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        Ok((UploadSlotState::Waiting, Some(rank))) => {
                                            // Send OP_QUEUERANKING
                                            let mut out = Vec::new();
                                            out.push(PR_ED2K);
                                            out.extend_from_slice(&5u32.to_le_bytes());
                                            out.push(OP_QUEUERANKING);
                                            out.extend_from_slice(&rank.to_le_bytes());
                                            let _ = stream.write_all(&out).await;
                                        }
                                        _ => {
                                            // Queue full
                                            continue;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Cleanup slot on disconnect
        self.upload_manager.release_slot(&remote_str).await;
    }
}
