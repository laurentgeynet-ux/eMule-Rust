// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

pub mod handlers;
pub mod routing_client;
pub mod search_client;
pub mod storage;
pub mod types;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;

use mule_core::kad::{KBucketTable, KadId};
use mule_network_policy::{
    classify_ip, ConnectionPurpose, IpCategory, ListenPermit, NetworkPolicyManager, NetworkProfile,
};
use mule_protocol::kad::*;

use crate::services::client_udp::ClientUdpService;

pub use types::{
    ActiveBuddy, BuddyRegistry, FirewallTestTracker, FirewallUdpResult, InFlightTransaction,
    IpRateLimiter, KadExpectedResponse, KadUdpSnapshot, PendingBuddy, UdpFirewallStatus,
    BUDDY_PENDING_TTL, KAD_TRANSACTION_TIMEOUT_SECS, MAX_IN_FLIGHT_TRANSACTIONS,
    MAX_KAD_REQUESTS_PER_SEC_PER_IP,
};

/// Inner state of the Kad UDP service.
pub struct KadUdpInner {
    pub socket: Option<Arc<UdpSocket>>,
    pub listen_permit: Option<ListenPermit>,
    pub bind_addr: Option<SocketAddr>,
    pub listen_task: Option<JoinHandle<()>>,
    pub in_flight: Arc<Mutex<HashMap<u32, InFlightTransaction>>>,
    /// Indexed-entry store (DHT storage node role).
    pub storage: Arc<Mutex<storage::KadStorage>>,
}

impl KadUdpInner {
    /// Registers a transaction and returns a guard that removes it on drop.
    ///
    /// Entries abandoned by cancelled `send_*` futures (outer timeout/select
    /// dropping the future before its own timeout branch runs) are dead once
    /// older than `KAD_TRANSACTION_TIMEOUT_SECS` — the inner `select!` would
    /// have fired otherwise. They are swept here so leaks cannot saturate the
    /// map nor steal responses from live transactions to the same peer.
    pub async fn insert_in_flight(
        &self,
        tx_id: u32,
        entry: InFlightTransaction,
    ) -> Result<types::InFlightTxGuard, String> {
        let mut inf = self.in_flight.lock().await;
        let now = Instant::now();
        let max_age = Duration::from_secs(KAD_TRANSACTION_TIMEOUT_SECS + 2);
        inf.retain(|_, tx| now.duration_since(tx.created_at) < max_age);
        if inf.len() >= MAX_IN_FLIGHT_TRANSACTIONS {
            return Err(format!(
                "Too many in-flight Kad transactions ({MAX_IN_FLIGHT_TRANSACTIONS} max)"
            ));
        }
        inf.insert(tx_id, entry);
        Ok(types::InFlightTxGuard::new(
            Arc::clone(&self.in_flight),
            tx_id,
        ))
    }
}

/// Kad UDP Service managing transport and transaction lifecycles.
pub struct KadUdpService {
    /// Identité Kad locale — partagée avec `KadService` (rotation à chaud).
    pub(crate) local_id: Arc<std::sync::RwLock<KadId>>,
    pub(crate) policy_mgr: NetworkPolicyManager,
    pub(crate) inner: Arc<RwLock<KadUdpInner>>,
    pub(crate) next_tx_id: AtomicU32,
    pub(crate) packets_sent: AtomicU64,
    pub(crate) packets_received: AtomicU64,
    /// Client-to-client UDP service sharing this socket (eMule multiplexes
    /// eD2K UDP and Kademlia on the same UDP port).
    client_udp: Arc<RwLock<Option<ClientUdpService>>>,
    /// Download service handle — persists indexed entries (storage node
    /// role) and rehydrates the in-memory store at startup.
    downloads: Arc<RwLock<Option<Arc<Mutex<crate::services::downloads::DownloadService>>>>>,
    /// UDP firewall/reachability test state (KADEMLIA2_FIREWALLUDP).
    firewall: Arc<Mutex<FirewallTestTracker>>,
    /// Buddy relay state (FINDBUDDY_REQ acceptances + active buddy link).
    buddy: Arc<Mutex<BuddyRegistry>>,
    /// Local eD2K TCP port advertised in `KADEMLIA_FINDBUDDY_RES` — the buddy
    /// client connects to it for its relay link.
    buddy_tcp_port: Arc<std::sync::atomic::AtomicU16>,
    /// Connection option flags sent in `KADEMLIA_FINDBUDDY_RES`
    /// (eMule `GetMyConnectOptions(true, false)` byte).
    buddy_connect_options: Arc<std::sync::atomic::AtomicU8>,
    /// Shared Kad routing table, injected by `KadService` after construction
    /// (`set_routing_table`). The UDP listener needs it to answer
    /// `KADEMLIA2_REQ`/`BOOTSTRAP_REQ` with real contacts and to record
    /// inbound contacts — eMule feeds its routing zone from every packet.
    routing_table: Arc<std::sync::RwLock<Option<Arc<Mutex<KBucketTable>>>>>,
}

impl KadUdpService {
    /// Creates a new Kad UDP service instance.
    pub fn new(
        local_id: Arc<std::sync::RwLock<KadId>>,
        policy_mgr: NetworkPolicyManager,
    ) -> Self {
        Self {
            local_id,
            policy_mgr,
            inner: Arc::new(RwLock::new(KadUdpInner {
                socket: None,
                listen_permit: None,
                bind_addr: None,
                listen_task: None,
                in_flight: Arc::new(Mutex::new(HashMap::new())),
                storage: Arc::new(Mutex::new(storage::KadStorage::default())),
            })),
            next_tx_id: AtomicU32::new(1),
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            client_udp: Arc::new(RwLock::new(None)),
            downloads: Arc::new(RwLock::new(None)),
            firewall: Arc::new(Mutex::new(FirewallTestTracker::default())),
            buddy: Arc::new(Mutex::new(BuddyRegistry::default())),
            buddy_tcp_port: Arc::new(std::sync::atomic::AtomicU16::new(0)),
            buddy_connect_options: Arc::new(std::sync::atomic::AtomicU8::new(0)),
            routing_table: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Injects the shared Kad routing table (`KadService` owns it). Safe to
    /// call before or after `start()` — the listener resolves it per packet.
    pub fn set_routing_table(&self, table: Arc<Mutex<KBucketTable>>) {
        if let Ok(mut slot) = self.routing_table.write() {
            *slot = Some(table);
        }
    }

    /// Shared buddy relay registry (pending acceptances + active link).
    pub fn buddy_registry(&self) -> Arc<Mutex<BuddyRegistry>> {
        Arc::clone(&self.buddy)
    }

    /// Sets the eD2K TCP port + connect-option flags advertised in
    /// `KADEMLIA_FINDBUDDY_RES` answers.
    pub fn set_buddy_tcp_info(&self, tcp_port: u16, connect_options: u8) {
        self.buddy_tcp_port.store(tcp_port, Ordering::Relaxed);
        self.buddy_connect_options
            .store(connect_options, Ordering::Relaxed);
    }

    /// Sends a `KADEMLIA_CALLBACK_REQ` (0x52) to a buddy node, asking it to
    /// relay a callback to its LowID buddy client so that client connects
    /// back to our `tcp_port` (eMule `TryToConnect` → `CCS_KADCALLBACK`).
    /// Sent unencrypted, matching eMule's FIXME comment.
    pub async fn send_callback_req(
        &self,
        buddy_addr: SocketAddr,
        buddy_id: &[u8; 16],
        file_id: &[u8; 16],
        tcp_port: u16,
    ) -> Result<(), String> {
        let socket = { self.inner.read().await.socket.clone() };
        let socket = socket.ok_or_else(|| "Kad UDP socket not bound".to_string())?;
        let pkt = KadCodec::encode_callback_req(buddy_id, file_id, tcp_port);
        socket
            .send_to(&pkt, buddy_addr)
            .await
            .map_err(|e| format!("Failed to send CALLBACK_REQ to {buddy_addr}: {e}"))?;
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        tracing::info!(
            "Kad UDP: CALLBACK_REQ sent to buddy {} (buddy_id={}, file={})",
            buddy_addr,
            hex::encode(buddy_id),
            hex::encode(file_id)
        );
        Ok(())
    }

    /// Shared firewall test tracker (pending candidates + last known state).
    pub fn firewall_tracker(&self) -> Arc<Mutex<FirewallTestTracker>> {
        Arc::clone(&self.firewall)
    }

    /// Sends a KADEMLIA2_FIREWALLUDP probe packet to `target` (server side of
    /// the test, answering a TCP `OP_FWCHECKUDPREQ`).
    pub async fn send_firewall_udp(
        &self,
        target: SocketAddr,
        error_code: u8,
        tested_port: u16,
    ) -> Result<(), String> {
        let socket = { self.inner.read().await.socket.clone() };
        let socket = socket.ok_or_else(|| "Kad UDP socket not bound".to_string())?;
        let pkt = KadCodec::encode_firewall_udp(error_code, tested_port);
        socket
            .send_to(&pkt, target)
            .await
            .map_err(|e| format!("Failed to send FIREWALLUDP to {target}: {e}"))?;
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    /// Local bound UDP port, if the socket is up.
    pub async fn local_udp_port(&self) -> Option<u16> {
        self.inner.read().await.bind_addr.map(|a| a.port())
    }

    /// Local bound socket address, if the socket is up.
    pub async fn bind_addr(&self) -> Option<SocketAddr> {
        self.inner.read().await.bind_addr
    }

    /// Injects the download service for index persistence.
    pub async fn set_downloads(
        &self,
        downloads: Arc<Mutex<crate::services::downloads::DownloadService>>,
    ) {
        *self.downloads.write().await = Some(downloads);
    }

    /// Injects the shared client UDP service; also hands it the bound
    /// socket so it can emit `OP_REASKFILEPING` keep-alives.
    pub async fn set_client_udp(&self, svc: ClientUdpService) {
        let socket = { self.inner.read().await.socket.clone() };
        if let Some(socket) = socket {
            svc.set_socket(socket).await;
        }
        *self.client_udp.write().await = Some(svc);
    }

    /// Starts the UDP listener on specified bind address under network policy compliance.
    pub async fn start(&self, bind_addr: SocketAddr) -> Result<SocketAddr, String> {
        let mut inner = self.inner.write().await;
        if inner.socket.is_some() {
            if let Some(addr) = inner.bind_addr {
                return Ok(addr);
            }
        }

        let permit = self
            .policy_mgr
            .request_tcp_listen(bind_addr, ConnectionPurpose::KadNode)
            .map_err(|e| format!("Policy denied Kad UDP bind on {bind_addr}: {e}"))?;

        let socket = UdpSocket::bind(&bind_addr)
            .await
            .map_err(|e| format!("Failed to bind Kad UDP socket to {bind_addr}: {e}"))?;

        let local_addr = socket
            .local_addr()
            .map_err(|e| format!("Failed to get local addr: {e}"))?;

        let socket_arc = Arc::new(socket);
        inner.socket = Some(socket_arc.clone());
        inner.bind_addr = Some(local_addr);
        inner.listen_permit = Some(permit);

        let socket_for_task = socket_arc.clone();
        let in_flight_arc = inner.in_flight.clone();
        let routing_arc = Arc::clone(&self.routing_table);
        // Arc partagé — relu à chaque datagramme pour suivre la rotation
        // d'identité (`identityRotationHours`).
        let local_id_arc = Arc::clone(&self.local_id);
        let policy_mgr_clone = self.policy_mgr.clone();
        let rate_limiter_arc = Arc::new(Mutex::new(IpRateLimiter::new()));
        let storage_arc = inner.storage.clone();
        let client_udp_arc = Arc::clone(&self.client_udp);
        let downloads_arc = Arc::clone(&self.downloads);
        let firewall_arc = Arc::clone(&self.firewall);
        let buddy_arc = Arc::clone(&self.buddy);
        let buddy_tcp_port = self.buddy_tcp_port.load(Ordering::Relaxed);
        let buddy_connect_options = self.buddy_connect_options.load(Ordering::Relaxed);
        if let Some(svc) = self.client_udp.read().await.as_ref() {
            svc.set_socket(socket_arc.clone()).await;
        }

        // Rehydrate the indexed-entry store from SQLite (storage node role
        // survives restarts); expired rows are purged first.
        if let Some(dl) = downloads_arc.read().await.as_ref() {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let (sources, keywords, notes) = {
                let mut dl_guard = dl.lock().await;
                let _ = dl_guard.db_mut().purge_expired_kad_sources(now_ms);
                let _ = dl_guard.db_mut().purge_expired_kad_keywords(now_ms);
                let _ = dl_guard.db_mut().purge_expired_kad_notes(now_ms);
                (
                    dl_guard
                        .db_mut()
                        .list_all_kad_indexed_sources(now_ms, storage::MAX_TOTAL_ENTRIES)
                        .unwrap_or_default(),
                    dl_guard
                        .db_mut()
                        .list_all_kad_keyword_entries(now_ms, storage::MAX_TOTAL_ENTRIES)
                        .unwrap_or_default(),
                    dl_guard
                        .db_mut()
                        .list_all_kad_notes(now_ms, storage::MAX_TOTAL_ENTRIES)
                        .unwrap_or_default(),
                )
            };
            if !sources.is_empty() || !keywords.is_empty() || !notes.is_empty() {
                let n_s = sources.len();
                let n_k = keywords.len();
                let n_n = notes.len();
                let hydrated_notes: Vec<storage::HydratedNote> = notes
                    .into_iter()
                    .filter_map(|n| {
                        let target =
                            <[u8; 16]>::try_from(hex::decode(&n.file_hash).ok()?.as_slice())
                                .ok()?;
                        let source_id =
                            <[u8; 16]>::try_from(hex::decode(&n.source_id).ok()?.as_slice())
                                .ok()?;
                        let (tags, unknown_tags) =
                            mule_protocol::kad::decoder::decode_note_tags_blob(&n.tags_blob)?;
                        let ip: std::net::Ipv4Addr = n.publisher_ip.parse().ok()?;
                        Some(storage::HydratedNote {
                            target_hash: target,
                            note: KadNoteWire {
                                source_id,
                                tags,
                                unknown_tags,
                            },
                            publisher_ip: ip.octets(),
                            publisher_udp_port: n.publisher_udp_port,
                            remaining_ms: (n.expires_at_unix_ms.max(0) as u64)
                                .saturating_sub(now_ms),
                        })
                    })
                    .collect();
                let mut store = storage_arc.lock().await;
                store.hydrate(sources, keywords, now_ms);
                store.hydrate_notes(hydrated_notes);
                tracing::info!(
                    "Kad storage rehydrated: {n_s} sources, {n_k} keyword entries, {n_n} notes"
                );
            }
        }

        let task = tokio::spawn(async move {
            // Official eMule allocates a 64KB receive buffer to guarantee that incoming UDP datagrams
            // up to the maximum IP payload (65507 bytes) never fail with WSAEMSGSIZE (OS error 10040).
            let mut buf = vec![0u8; 65536];
            loop {
                tokio::select! {
                    recv_res = socket_for_task.recv_from(&mut buf) => {
                        match recv_res {
                            Ok((len, peer_addr)) => {
                                if len < 2 {
                                    continue;
                                }
                                let proto_byte = buf[0];
                                let opcode_byte = buf[1];

                                // 1. Check protocol identifier — non-Kad
                                //    datagrams (PR_EMULE/PR_ED2K) belong to
                                //    the client UDP channel (eMule shares
                                //    one UDP socket for both).
                                if !is_valid_kad_protocol(proto_byte) {
                                    if ClientUdpService::is_client_udp_protocol(proto_byte) {
                                        let cat = classify_ip(peer_addr.ip());
                                        let policy_allowed = match policy_mgr_clone.profile() {
                                            NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                                            NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                                                cat == IpCategory::PublicRoutable
                                            }
                                        };
                                        if policy_allowed {
                                            if let Some(svc) = client_udp_arc.read().await.as_ref() {
                                                svc.handle_datagram(peer_addr, &buf[..len]).await;
                                            }
                                        }
                                    } else {
                                        // Possibly an obfuscated Kad datagram —
                                        // eMule encrypts KADEMLIA2_FIREWALLUDP
                                        // answers with the UDP verify key we
                                        // sent in OP_FWCHECKUDPREQ.
                                        let keys = {
                                            let fw = firewall_arc.lock().await;
                                            fw.pending_verify_keys(&peer_addr.ip())
                                        };
                                        let mut decrypted = false;
                                        for key in keys {
                                            if let Some(dec) = decrypt_kad_udp_packet(&buf[..len], key) {
                                                match KadCodec::decode(&dec.payload) {
                                                    Ok(msg) => {
                                                        tracing::info!(
                                                            "Kad UDP: decrypted obfuscated packet from {} (recvKey=0x{:08X})",
                                                            peer_addr, dec.receiver_key
                                                        );
                                                        let local_kad_id_bytes =
                                                            local_id_arc.read().unwrap().0;
                                                        let routing_table = routing_arc
                                                            .read()
                                                            .ok()
                                                            .and_then(|g| g.clone());
                                                        handlers::handle_incoming_message(
                                                            &socket_for_task,
                                                            peer_addr,
                                                            msg,
                                                            &in_flight_arc,
                                                            &local_kad_id_bytes,
                                                            local_addr.port(),
                                                            &storage_arc,
                                                            &downloads_arc,
                                                            &firewall_arc,
                                                            &buddy_arc,
                                                            buddy_tcp_port,
                                                            buddy_connect_options,
                                                            routing_table,
                                                        ).await;
                                                        decrypted = true;
                                                    }
                                                    Err(e) => {
                                                        tracing::debug!(
                                                            "Kad UDP: decrypted datagram from {} failed decode: {}",
                                                            peer_addr, e
                                                        );
                                                    }
                                                }
                                                break;
                                            }
                                        }
                                        if !decrypted {
                                            tracing::warn!(
                                                "Kad UDP: dropped invalid protocol byte 0x{:02X} from {}",
                                                proto_byte,
                                                peer_addr
                                            );
                                        }
                                    }
                                    continue;
                                }

                                // 2. Policy-based IP classification and conformance
                                let cat = classify_ip(peer_addr.ip());
                                let policy_allowed = match policy_mgr_clone.profile() {
                                    NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                                    NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                                        cat == IpCategory::PublicRoutable
                                    }
                                };
                                if !policy_allowed {
                                    tracing::warn!("Kad UDP: dropped packet from non-conforming IP {} ({:?})", peer_addr.ip(), cat);
                                    continue;
                                }

                                // 3. Anti-amplification / rate limiting per source IP
                                {
                                    let mut rl = rate_limiter_arc.lock().await;
                                    if !rl.check_and_record(peer_addr.ip()) {
                                        tracing::debug!("Kad UDP rate limit exceeded for {}", peer_addr.ip());
                                        continue;
                                    }
                                }

                                let packet_slice = &buf[..len];
                                tracing::info!(
                                    "Kad UDP recv {} bytes from {} (proto=0x{:02X}, opcode=0x{:02X})",
                                    len,
                                    peer_addr,
                                    proto_byte,
                                    opcode_byte
                                );

                                // A 0x3B unified search response is ambiguous:
                                // notes replies (SendValidNoteResult) carry
                                // arbitrary tag lists which the generic decoder
                                // flattens to keyword entries. When a pending
                                // transaction from this peer expects
                                // SearchNotesRes, re-decode the raw payload in
                                // notes form to preserve the full tag lists.
                                let decoded = match KadCodec::decode(packet_slice) {
                                    Ok(msg) => {
                                        if opcode_byte == KADEMLIA2_SEARCH_RES {
                                            let expects_notes = {
                                                let inf = in_flight_arc.lock().await;
                                                inf.values().any(|tx| {
                                                    (tx.target == peer_addr
                                                        || tx.target.ip() == peer_addr.ip())
                                                        && tx.expected_response
                                                            == KadExpectedResponse::SearchNotesRes
                                                })
                                            };
                                            if expects_notes {
                                                match KadCodec::decode_search_notes_res(
                                                    proto_byte,
                                                    opcode_byte,
                                                    &packet_slice[2..],
                                                ) {
                                                    Ok(m) => Ok(m),
                                                    Err(e) => {
                                                        tracing::warn!(
                                                            "Kad UDP: notes-form redecode of 0x3B from {peer_addr} failed: {e} — using generic form"
                                                        );
                                                        Ok(msg)
                                                    }
                                                }
                                            } else {
                                                Ok(msg)
                                            }
                                        } else {
                                            Ok(msg)
                                        }
                                    }
                                    Err(e) => Err(e),
                                };

                                match decoded {
                                    Ok(msg) => {
                                        let local_kad_id_bytes =
                                            local_id_arc.read().unwrap().0;
                                        let routing_table = routing_arc
                                            .read()
                                            .ok()
                                            .and_then(|g| g.clone());
                                        handlers::handle_incoming_message(
                                            &socket_for_task,
                                            peer_addr,
                                            msg,
                                            &in_flight_arc,
                                            &local_kad_id_bytes,
                                            local_addr.port(),
                                            &storage_arc,
                                            &downloads_arc,
                                            &firewall_arc,
                                            &buddy_arc,
                                            buddy_tcp_port,
                                            buddy_connect_options,
                                            routing_table,
                                        ).await;
                                    }
                                    Err(err) => {
                                        tracing::warn!(
                                            "Kad UDP: decode error from {} (proto=0x{:02X}, opcode=0x{:02X}, len={}): {} (prefix: {:02X?})",
                                            peer_addr,
                                            proto_byte,
                                            opcode_byte,
                                            len,
                                            err,
                                            &packet_slice[..packet_slice.len().min(16)]
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Kad UDP: recv_from error: {e}");
                                tokio::time::sleep(Duration::from_millis(10)).await;
                            }
                        }
                    }
                }
            }
        });

        inner.listen_task = Some(task);
        Ok(local_addr)
    }

    /// Stops the UDP socket, drops permits, and cancels pending transactions.
    pub async fn stop(&self) -> Result<(), String> {
        let mut inner = self.inner.write().await;
        if let Some(task) = inner.listen_task.take() {
            task.abort();
        }
        inner.socket = None;
        inner.bind_addr = None;
        inner.listen_permit = None;

        // Abort all in-flight transactions
        let mut in_flight = inner.in_flight.lock().await;
        for (_, tx) in in_flight.drain() {
            let _ = tx.sender.send(Err("Kad UDP socket stopped".to_string()));
        }
        Ok(())
    }

    /// Returns a status snapshot of the Kad UDP service.
    pub async fn get_snapshot(&self) -> KadUdpSnapshot {
        let inner = self.inner.read().await;
        let in_flight_count = {
            let inf = inner.in_flight.lock().await;
            inf.len()
        };

        KadUdpSnapshot {
            is_active: inner.socket.is_some(),
            bind_address: inner.bind_addr.map(|a| a.to_string()),
            packets_sent: self.packets_sent.load(Ordering::Relaxed),
            packets_received: self.packets_received.load(Ordering::Relaxed),
            active_transactions: in_flight_count,
        }
    }
}
