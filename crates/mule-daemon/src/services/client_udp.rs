// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Client-to-client UDP service (`ClientUDPSocket`, eMule 0.72a).
//!
//! Shares the daemon UDP socket with Kad (eMule multiplexes eD2K UDP and
//! Kademlia on the same UDP port): `KadUdpService` routes datagrams whose
//! protocol byte is `PR_EMULE`/`PR_ED2K` here.
//!
//! Responsibilities:
//! - Inbound `OP_REASKFILEPING`: answer `OP_REASKACK` with the requester's
//!   upload queue rank, `OP_FILENOTFOUND` when the file is not shared, or
//!   `OP_QUEUEFULL` when the queue is saturated (eMule ignores unknown
//!   senders unless the queue is nearly full).
//! - Inbound `OP_REASKACK`/`OP_QUEUEFULL`/`OP_FILENOTFOUND`: validated
//!   against pending reasks and applied to the matching queued peer
//!   session's rank bookkeeping.
//! - Outbound `send_reask_file_ping`: periodic keep-alive to queued
//!   sources that announced an eMule UDP port (deduplicated, ~21 min
//!   cadence like `CUpDownClient::UDPReaskForDownload`).

use crate::services::peer_session::log_peer_addr;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::sync::Mutex;

use mule_protocol::ed2k::client_udp::{self, ClientUdpMessage};
use mule_protocol::ed2k::protocol::{PR_ED2K, PR_EMULE};

use crate::services::downloads::DownloadService;
use crate::services::upload_queue::UploadManager;

/// Minimum interval between two `OP_REASKFILEPING` toward the same
/// (peer, file) — eMule reasks queued sources roughly every 21 minutes.
pub const UDP_REASK_INTERVAL: Duration = Duration::from_secs(21 * 60);
/// A reask stays "pending" this long awaiting its ack.
const PENDING_TTL: Duration = Duration::from_secs(120);
/// eMule `UDPReaskForDownload` (`DownloadClient.cpp:1340`) — the failure
/// guard engages only once more than this many UDP packets were sent to
/// the peer (`m_nTotalUDPPackets > 3`).
const UDP_GUARD_MIN_ATTEMPTS: u32 = 3;
/// eMule `m_nFailedUDPPackets / m_nTotalUDPPackets > .3f` — above this
/// failure ratio (percent) UDP reasks to the peer are abandoned.
const UDP_GUARD_MAX_FAILED_PERCENT: u32 = 30;
/// eMule `AllowCalbackRequest` (`ClientList.cpp:921`) — at most one
/// accepted `OP_DIRECTCALLBACKREQ` per source IP in this window.
const DIRECT_CALLBACK_MIN_INTERVAL: Duration = Duration::from_secs(180);

/// Result of a remote UDP reask, recorded per peer IP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UdpReaskResult {
    /// Peer answered `OP_REASKACK` with this queue rank.
    Rank(u16),
    /// Peer answered `OP_QUEUEFULL`.
    QueueFull,
    /// Peer answered `OP_FILENOTFOUND` — it no longer shares the file.
    FileNotFound,
}

/// Outbound keep-alive target produced by `PeerSessionManager`.
#[derive(Debug, Clone)]
pub struct UdpReaskTarget {
    /// UDP endpoint of the queued source (its announced eMule UDP port).
    pub addr: SocketAddr,
    /// File hash of the queued session.
    pub file_hash: [u8; 16],
    /// When the underlying TCP session was established — used to apply
    /// eMule's "last connection attempt older than ~20 min" condition
    /// before spending a UDP reask.
    pub connected_at_unix_ms: Option<i64>,
}

/// Compteurs UDP par pair — eMule `m_nTotalUDPPackets` /
/// `m_nFailedUDPPackets` (`updownclient.h:561-562`).
#[derive(Debug, Default, Clone)]
struct UdpPeerStats {
    /// Total `OP_REASKFILEPING` envoyés au pair.
    total: u32,
    /// Pings restés sans réponse jusqu'au reask suivant.
    failed: u32,
}

#[derive(Default)]
struct ClientUdpInner {
    socket: Option<Arc<UdpSocket>>,
    /// Intervalle de reask : (peer IP, file hash) → sent at (throttle
    /// `UDP_REASK_INTERVAL`, indépendant de l'attente d'ack).
    pending: HashMap<(IpAddr, [u8; 16]), Instant>,
    /// Pings en attente d'une réponse (eMule `m_bUDPPending`) — expirés
    /// après `PENDING_TTL` et alors comptés comme échecs.
    awaiting_ack: HashMap<(IpAddr, [u8; 16]), Instant>,
    /// Compteurs d'échec par IP (garde 30 % de `UDPReaskForDownload`).
    udp_stats: HashMap<IpAddr, UdpPeerStats>,
    /// Last result per peer IP.
    results: HashMap<IpAddr, (UdpReaskResult, Instant)>,
    /// UDP version announced by each queued inbound client (IP → version),
    /// used to shape the `OP_REASKACK` payload (`sender->GetUDPVersion()`).
    client_udp_versions: HashMap<IpAddr, u8>,
    /// Dernière demande de rappel direct acceptée par IP —
    /// `listDirectCallbackRequests` eMule (fenêtre 180 s).
    direct_callbacks: HashMap<IpAddr, Instant>,
}

/// Shared client UDP service (cloneable handle).
#[derive(Clone, Default)]
pub struct ClientUdpService {
    inner: Arc<Mutex<ClientUdpInner>>,
    downloads: Arc<std::sync::RwLock<Option<Arc<Mutex<DownloadService>>>>>,
    upload_manager: Arc<std::sync::RwLock<Option<UploadManager>>>,
    peer_session: Arc<std::sync::RwLock<Option<crate::services::peer_session::PeerSessionManager>>>,
    /// Buddy relay registry — `OP_REASKCALLBACKUDP` datagrams for our buddy
    /// client's ID are forwarded onto its TCP link as `OP_REASKCALLBACKTCP`.
    buddy: Arc<std::sync::RwLock<Option<Arc<Mutex<crate::services::kad_udp::BuddyRegistry>>>>>,
    /// Inbound listener used for `connect_back` when a peer asks a
    /// Kad-firewalled us to dial out (`OP_DIRECTCALLBACKREQ` → eMule
    /// `TryToConnect`).
    inbound: Arc<std::sync::RwLock<Option<crate::services::inbound_listener::InboundListenerService>>>,
    /// Kad UDP firewall tracker — direct callbacks are only honoured
    /// while `UdpFirewallStatus::Firewalled` (eMule `CKademlia::
    /// IsRunning() && IsFirewalled()`).
    firewall_tracker:
        Arc<std::sync::RwLock<Option<Arc<Mutex<crate::services::kad_udp::FirewallTestTracker>>>>>,
    /// Server session — provides our eD2K `client_id` for the connect-back
    /// hello (eMule sends its real ID in `TryToConnect`).
    server_session: Arc<std::sync::RwLock<Option<crate::services::ServerSessionManager>>>,
}

impl ClientUdpService {
    /// Creates a new unbound service.
    pub fn new() -> Self {
        Self::default()
    }

    /// Injects the shared UDP socket once `KadUdpService` is bound.
    pub async fn set_socket(&self, socket: Arc<UdpSocket>) {
        self.inner.lock().await.socket = Some(socket);
    }

    /// Injects the download service (shared file lookup).
    pub fn set_downloads(&self, downloads: Arc<Mutex<DownloadService>>) {
        *self.downloads.write().unwrap() = Some(downloads);
    }

    /// Injects the upload queue manager (rank lookup for inbound pings).
    pub fn set_upload_manager(&self, mgr: UploadManager) {
        *self.upload_manager.write().unwrap() = Some(mgr);
    }

    /// Injects the peer session manager (rank application + reask bookkeeping).
    pub fn set_peer_session(&self, mgr: crate::services::peer_session::PeerSessionManager) {
        *self.peer_session.write().unwrap() = Some(mgr);
    }

    /// Injects the buddy relay registry shared with `KadUdpService`.
    pub fn set_buddy_registry(&self, reg: Arc<Mutex<crate::services::kad_udp::BuddyRegistry>>) {
        *self.buddy.write().unwrap() = Some(reg);
    }

    /// Injects the inbound listener used for direct-callback connect-back.
    pub fn set_inbound_listener(
        &self,
        svc: crate::services::inbound_listener::InboundListenerService,
    ) {
        *self.inbound.write().unwrap() = Some(svc);
    }

    /// Injects the Kad UDP firewall tracker (`OP_DIRECTCALLBACKREQ` gate).
    pub fn set_firewall_tracker(
        &self,
        tracker: Arc<Mutex<crate::services::kad_udp::FirewallTestTracker>>,
    ) {
        *self.firewall_tracker.write().unwrap() = Some(tracker);
    }

    /// Injects the server session (eD2K `client_id` for connect-back).
    pub fn set_server_session(&self, svc: crate::services::ServerSessionManager) {
        *self.server_session.write().unwrap() = Some(svc);
    }

    /// Records the UDP version an inbound client announced in its hello,
    /// so `OP_REASKACK` replies match `sender->GetUDPVersion()` semantics.
    pub async fn note_client_udp_version(&self, ip: IpAddr, version: u8) {
        self.inner
            .lock()
            .await
            .client_udp_versions
            .insert(ip, version);
    }

    /// Sends `OP_REASKFILEPING` to `target` unless an equivalent reask was
    /// sent within [`UDP_REASK_INTERVAL`]. Returns true when a packet went
    /// out.
    pub async fn send_reask_file_ping(
        &self,
        target: &UdpReaskTarget,
        complete_sources: u16,
    ) -> bool {
        let ip = target.addr.ip();
        let key = (ip, target.file_hash);
        {
            let mut inner = self.inner.lock().await;

            // Un ping sans réponse au-delà de `PENDING_TTL` est un échec —
            // eMule incrémente `m_nFailedUDPPackets` quand le reask TCP
            // suivant trouve `m_bUDPPending` encore vrai (`AskForDownload`).
            let mut expired: Vec<IpAddr> = Vec::new();
            inner.awaiting_ack.retain(|(eip, _), at| {
                if at.elapsed() >= PENDING_TTL {
                    expired.push(*eip);
                    false
                } else {
                    true
                }
            });
            for eip in expired {
                inner.udp_stats.entry(eip).or_default().failed += 1;
            }

            // Garde eMule `UDPReaskForDownload` (`DownloadClient.cpp:1340`) :
            // après >3 paquets, on cesse les reasks UDP quand plus de 30 %
            // sont restés sans réponse.
            if inner.udp_stats.get(&ip).is_some_and(|st| {
                st.total > UDP_GUARD_MIN_ATTEMPTS
                    && st.failed * 100 > st.total * UDP_GUARD_MAX_FAILED_PERCENT
            }) {
                tracing::debug!(
                    "Client UDP: reask skipped for {} — UDP failure ratio > {}%",
                    log_peer_addr(target.addr),
                    UDP_GUARD_MAX_FAILED_PERCENT
                );
                return false;
            }

            // eMule `if (!m_reqfile || m_bUDPPending) return;` — un seul
            // reask UDP en vol par pair, quel que soit le fichier.
            if inner.awaiting_ack.keys().any(|(eip, _)| *eip == ip) {
                return false;
            }

            inner
                .pending
                .retain(|_, at| at.elapsed() < UDP_REASK_INTERVAL);
            if inner.pending.contains_key(&key) {
                return false;
            }
            let Some(socket) = inner.socket.clone() else {
                return false;
            };
            let now = Instant::now();
            inner.pending.insert(key, now);
            inner.awaiting_ack.insert(key, now);
            inner.udp_stats.entry(ip).or_default().total += 1;
            let pkt = client_udp::encode_reask_file_ping(&target.file_hash, complete_sources);
            tracing::debug!(
                "Client UDP: OP_REASKFILEPING -> {} (hash {})",
                log_peer_addr(target.addr),
                hex::encode(target.file_hash)
            );
            if let Err(e) = socket.send_to(&pkt, target.addr).await {
                tracing::debug!(
                    "Client UDP: send_reask_file_ping to {} failed: {e}",
                    log_peer_addr(target.addr)
                );
            }
        }
        true
    }

    /// Last recorded UDP reask result for a peer IP.
    pub async fn last_result(&self, ip: IpAddr) -> Option<UdpReaskResult> {
        let inner = self.inner.lock().await;
        inner
            .results
            .get(&ip)
            .filter(|(_, at)| at.elapsed() < UDP_REASK_INTERVAL)
            .map(|(r, _)| r.clone())
    }

    /// Returns true if the datagram protocol byte belongs to the client
    /// UDP channel (`PR_EMULE` / `PR_ED2K`, i.e. not Kademlia).
    pub fn is_client_udp_protocol(proto: u8) -> bool {
        proto == PR_EMULE || proto == PR_ED2K
    }

    /// Handles one inbound client UDP datagram (already policy-filtered by
    /// the shared socket loop).
    pub async fn handle_datagram(&self, peer_addr: SocketAddr, data: &[u8]) {
        let msg = match client_udp::decode_client_udp(data) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!(
                    "Client UDP: decode error from {peer}: {e}",
                    peer = log_peer_addr(peer_addr)
                );
                return;
            }
        };
        match msg {
            ClientUdpMessage::ReaskFilePing { file_hash, .. } => {
                self.handle_reask_file_ping(peer_addr, file_hash).await
            }
            ClientUdpMessage::ReaskAck {
                queue_rank,
                part_status,
            } => {
                self.handle_reask_ack(peer_addr, UdpReaskResult::Rank(queue_rank), part_status)
                    .await
            }
            ClientUdpMessage::QueueFull => {
                self.handle_reask_ack(peer_addr, UdpReaskResult::QueueFull, None)
                    .await
            }
            ClientUdpMessage::FileNotFound => {
                self.handle_reask_ack(peer_addr, UdpReaskResult::FileNotFound, None)
                    .await
            }
            ClientUdpMessage::ReaskCallback { buddy_id, rest } => {
                self.handle_reask_callback(peer_addr, buddy_id, rest).await
            }
            ClientUdpMessage::DirectCallbackReq {
                tcp_port,
                user_hash,
                conn_opts,
            } => {
                self.handle_direct_callback_req(peer_addr, tcp_port, user_hash, conn_opts)
                    .await
            }
            ClientUdpMessage::PortTest { request } => {
                // eMule répond au test `0x12` via sa socket TCP de test
                // dédiée (`SendPortTestReply('1')`) — pas de réponse UDP.
                // Nous n'implémentons pas l'outil de test de ports :
                // accusé journalisé uniquement.
                tracing::debug!(
                    "Client UDP: OP_PORTTEST 0x{request:02X} from {peer} (no reply — port test tool unsupported)",
                    peer = log_peer_addr(peer_addr)
                );
            }
            ClientUdpMessage::Unknown { opcode } => {
                tracing::debug!(
                    "Client UDP: ignoring opcode 0x{opcode:02X} from {peer}",
                    peer = log_peer_addr(peer_addr)
                );
            }
        }
    }

    /// eMule `case OP_REASKFILEPING` — answer with the queue rank when the
    /// sender waits in our upload queue for that file, `OP_FILENOTFOUND`
    /// when the hash is not shared, `OP_QUEUEFULL` when saturated; unknown
    /// senders are otherwise ignored (forcing a TCP connection).
    async fn handle_reask_file_ping(&self, peer_addr: SocketAddr, file_hash: [u8; 16]) {
        let hash_hex = hex::encode(file_hash);

        let shared = {
            let downloads = self.downloads.read().unwrap().clone();
            match downloads.as_ref() {
                Some(d) => {
                    let d = d.lock().await;
                    d.get_shared_file(&hash_hex).unwrap_or(None).is_some()
                }
                None => false,
            }
        };

        let mgr = self.upload_manager.read().unwrap().clone();
        // eMule `GetWaitingClientByIP_UDP` + `md4equ(reqfilehash,
        // sender->GetUploadFileID())` : le rang n'est révélé qu'au client
        // effectivement en file **pour ce fichier** — un pair en file pour
        // un autre fichier (ou plusieurs clients derrière la même IP sans
        // correspondance de port UDP) ne reçoit aucune réponse.
        let lookup = match &mgr {
            Some(m) => {
                m.waiting_rank_for_udp(&peer_addr.ip(), peer_addr.port(), &hash_hex)
                    .await
            }
            None => crate::services::upload_queue::UdpRankLookup::NoSender,
        };

        use crate::services::upload_queue::UdpRankLookup;
        let response = if !shared {
            client_udp::encode_file_not_found()
        } else {
            match lookup {
                UdpRankLookup::Rank(rank) => {
                    // eMule `sender->SetLastUpRequest()` sur ping accepté.
                    if let Some(m) = &mgr {
                        m.note_udp_reask(&peer_addr.ip()).await;
                    }
                    let include_status = {
                        let inner = self.inner.lock().await;
                        inner
                            .client_udp_versions
                            .get(&peer_addr.ip())
                            .copied()
                            .unwrap_or(0)
                            > 3
                    };
                    client_udp::encode_reask_ack(rank as u16, include_status)
                }
                UdpRankLookup::NoSender => {
                    let nearly_full = match &mgr {
                        Some(m) => m.is_nearly_full().await,
                        None => false,
                    };
                    if nearly_full {
                        client_udp::encode_queue_full()
                    } else {
                        // eMule: "Don't answer him. Force him to establish
                        // a TCP connection."
                        return;
                    }
                }
                UdpRankLookup::Ambiguous => {
                    tracing::debug!(
                        "Client UDP: REASKFILEPING from {peer} dropped — multiple queued clients share this IP (bSenderMultipleIpUnknown)",
                        peer = log_peer_addr(peer_addr)
                    );
                    return;
                }
                UdpRankLookup::FileMismatch => {
                    tracing::debug!(
                        "Client UDP: REASKFILEPING from {peer} dropped — reqfile does not match queued file",
                        peer = log_peer_addr(peer_addr)
                    );
                    return;
                }
            }
        };

        let socket = { self.inner.lock().await.socket.clone() };
        if let Some(socket) = socket {
            if let Err(e) = socket.send_to(&response, peer_addr).await {
                tracing::debug!(
                    "Client UDP: reask reply to {peer} failed: {e}",
                    peer = log_peer_addr(peer_addr)
                );
            }
        }
    }

    /// eMule `case OP_REASKCALLBACKUDP` — when the datagram targets our
    /// buddy client's ID, forward it on the buddy TCP link as
    /// `OP_REASKCALLBACKTCP`: `[requester_ip:4][requester_udp:2][file_hash:16]
    /// [part_status..]` (eMule pokes ip/port at payload+10 then copies from
    /// there — bytes 10..16 of the buddy ID are replaced).
    async fn handle_reask_callback(
        &self,
        peer_addr: SocketAddr,
        buddy_id: [u8; 16],
        rest: Vec<u8>,
    ) {
        let reg = self.buddy.read().unwrap().clone();
        let Some(reg) = reg else { return };
        let ip4 = match peer_addr.ip() {
            IpAddr::V4(v4) => v4.octets(),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => v4.octets(),
                None => return,
            },
        };
        let mut payload = Vec::with_capacity(6 + rest.len());
        payload.extend_from_slice(&ip4);
        payload.extend_from_slice(&peer_addr.port().to_le_bytes());
        payload.extend_from_slice(&rest);
        let mut frame = Vec::with_capacity(6 + payload.len());
        frame.push(PR_EMULE);
        frame.extend_from_slice(&(payload.len() as u32 + 1).to_le_bytes());
        frame.push(mule_protocol::ed2k::OP_REASKCALLBACKTCP);
        frame.extend_from_slice(&payload);
        let relayed = {
            let reg = reg.lock().await;
            if reg
                .active
                .as_ref()
                .map(|b| b.buddy_id == buddy_id)
                .unwrap_or(false)
            {
                reg.send_to_buddy(frame)
            } else {
                false
            }
        };
        if relayed {
            tracing::info!(
                "Client UDP: relayed OP_REASKCALLBACKUDP from {peer} to buddy link",
                peer = log_peer_addr(peer_addr)
            );
        } else {
            tracing::debug!(
                "Client UDP: dropping OP_REASKCALLBACKUDP from {peer} — buddy_id mismatch or no buddy"
            , peer = log_peer_addr(peer_addr));
        }
    }

    /// eMule `case OP_DIRECTCALLBACKREQ` — a peer that cannot reach us
    /// over TCP asks us (as a Kad-firewalled node) to dial out to it.
    /// Honoured only while our Kad UDP firewall status is `Firewalled`
    /// (`CKademlia::IsRunning() && IsFirewalled()`), rate-limited to one
    /// accepted request per IP per `DIRECT_CALLBACK_MIN_INTERVAL`
    /// (`AllowCalbackRequest`, 180 s).
    async fn handle_direct_callback_req(
        &self,
        peer_addr: SocketAddr,
        tcp_port: u16,
        user_hash: [u8; 16],
        conn_opts: u8,
    ) {
        // Rate limit — eMule drops repeat requesters inside the window.
        {
            let mut inner = self.inner.lock().await;
            let now = Instant::now();
            inner
                .direct_callbacks
                .retain(|_, at| now.duration_since(*at) < DIRECT_CALLBACK_MIN_INTERVAL);
            if inner.direct_callbacks.contains_key(&peer_addr.ip()) {
                tracing::debug!(
                    "Client UDP: DIRECTCALLBACKREQ from {peer} dropped — too many requests",
                    peer = log_peer_addr(peer_addr)
                );
                return;
            }
        }

        // Gate — eMule only accepts when Kad is running and firewalled.
        let tracker = { self.firewall_tracker.read().unwrap().clone() };
        let firewalled = match tracker {
            Some(t) => {
                t.lock().await.status
                    == crate::services::kad_udp::UdpFirewallStatus::Firewalled
            }
            None => false,
        };
        if !firewalled {
            tracing::debug!(
                "Client UDP: DIRECTCALLBACKREQ from {peer} ignored — not Kad-firewalled",
                peer = log_peer_addr(peer_addr)
            );
            return;
        }

        let Some(inbound) = self.inbound.read().unwrap().clone() else {
            return;
        };
        let server_session = { self.server_session.read().unwrap().clone() };
        let our_id = match server_session {
            Some(s) => s.snapshot().await.client_id.unwrap_or(0),
            None => 0,
        };

        // eMule `AddTrackCallbackRequests` — tracked only once accepted.
        self.inner
            .lock()
            .await
            .direct_callbacks
            .insert(peer_addr.ip(), Instant::now());

        let target = SocketAddr::new(peer_addr.ip(), tcp_port);
        tracing::info!(
            "Client UDP: DIRECTCALLBACKREQ accepted from {peer} — connect-back to {target} (conn_opts=0x{conn_opts:02X})",
            peer = log_peer_addr(peer_addr),
            target = log_peer_addr(target)
        );
        tokio::spawn(async move {
            // server_ip/port: inconnus hors session serveur — `0` est le
            // fallback utilisé par `sx_local_ids` quand on n'est pas
            // connecté à un serveur.
            if let Err(e) = inbound
                .connect_back(target, Some(user_hash), conn_opts, our_id, 0, 0)
                .await
            {
                tracing::debug!(
                    "Client UDP: direct callback connect-back to {target} failed: {e}",
                    target = log_peer_addr(target)
                );
            }
        });
    }

    /// Validates an ack against pending reasks and applies the rank to the
    /// queued peer session (eMule `sender->UDPReaskACK(nRank)`).
    async fn handle_reask_ack(
        &self,
        peer_addr: SocketAddr,
        result: UdpReaskResult,
        part_status: Option<client_udp::UdpPartStatus>,
    ) {
        // Hash du fichier pour lequel le ping était en vol — eMule le
        // retrouve via `sender->m_reqfile` ; un seul ping en vol par pair
        // (`m_bUDPPending`), on retire donc toutes les entrées de l'IP et
        // on garde le hash pour les traitements FNF / part-status.
        let (had_pending, pending_hash) = {
            let mut inner = self.inner.lock().await;
            // Ack attendu = ping encore dans sa fenêtre `PENDING_TTL` —
            // eMule `m_bUDPPending` (remis à faux par `UDPReaskACK`).
            let key_exists = inner
                .awaiting_ack
                .iter()
                .any(|((ip, _), at)| *ip == peer_addr.ip() && at.elapsed() < PENDING_TTL);
            let mut hash = None;
            if key_exists {
                inner.awaiting_ack.retain(|(ip, h), _| {
                    if *ip == peer_addr.ip() {
                        hash = Some(*h);
                        false
                    } else {
                        true
                    }
                });
                inner
                    .results
                    .insert(peer_addr.ip(), (result.clone(), Instant::now()));
            }
            (key_exists, hash)
        };

        if !had_pending {
            tracing::debug!(
                "Client UDP: ignoring unsolicited {:?} from {peer} (pendingflag == false)",
                result,
                peer = log_peer_addr(peer_addr)
            );
            return;
        }

        tracing::debug!(
            "Client UDP: {:?} from {peer}",
            result,
            peer = log_peer_addr(peer_addr)
        );

        let ps = self.peer_session.read().unwrap().clone();
        match result {
            UdpReaskResult::Rank(rank) => {
                // eMule `SetRemoteQueueFull(false)` + `UDPReaskACK(nRank)`.
                if let Some(ps) = ps {
                    ps.apply_udp_queue_rank(&peer_addr.ip(), rank as u32)
                        .await;
                }
                // UDP ver > 3 : `ProcessFileStatus` — le bitfield décrit
                // les parts que le pair possède du fichier requis ; il
                // alimente la rareté (`m_SrcPartFrequency`).
                if let (Some(status), Some(hash)) = (part_status, pending_hash) {
                    self.apply_ack_part_status(peer_addr, hash, status)
                        .await;
                }
            }
            UdpReaskResult::QueueFull => {
                // eMule `SetRemoteQueueFull(true)` + `UDPReaskACK(0)`.
                if let Some(ps) = ps {
                    ps.apply_udp_queue_full(&peer_addr.ip()).await;
                }
            }
            UdpReaskResult::FileNotFound => {
                // eMule `UDPReaskFNF` : la source est « deadlistée » pour
                // ce fichier et retirée des sources.
                if let Some(ps) = ps {
                    ps.apply_udp_file_not_found(&peer_addr.ip()).await;
                }
                if let Some(hash) = pending_hash {
                    self.mark_source_file_not_found(peer_addr, hash).await;
                }
            }
        }
    }

    /// eMule `ProcessFileStatus(true, data_in, reqfile)` : applique le
    /// bitfield de parts annoncé par le pair à sa disponibilité pour le
    /// fichier requis (rareté `m_SrcPartFrequency` + statut persisté).
    async fn apply_ack_part_status(
        &self,
        peer_addr: SocketAddr,
        file_hash: [u8; 16],
        status: client_udp::UdpPartStatus,
    ) {
        let Some(dl_arc) = self.downloads.read().unwrap().clone() else {
            return;
        };
        let mask = match mule_protocol::ed2k::PartAvailabilityMask::from_bytes(
            bytes::Bytes::from(status.bitfield.clone()),
            status.part_count,
        ) {
            Ok(m) => m,
            Err(e) => {
                tracing::debug!(
                    "Client UDP: malformed REASKACK part status from {peer}: {e:?}",
                    peer = log_peer_addr(peer_addr)
                );
                return;
            }
        };
        let hash_hex = hex::encode(file_hash);
        let mut dl = dl_arc.lock().await;
        let Some(dl_id) = dl.find_by_ed2k_hash(&hash_hex).map(|s| s.id) else {
            return;
        };
        // La session TCP du pair (le port UDP du datagramme ≠ le port TCP
        // de la connexion) : on retrouve sa `ActiveSource` par IP.
        if let Some(src_addr) = dl
            .get_sources(&dl_id)
            .and_then(|srcs| {
                srcs.keys()
                    .find(|a| a.ip() == peer_addr.ip())
                    .copied()
            })
        {
            let _ = dl.record_source_availability(&dl_id, src_addr, mask);
        }
        if let Some(src) = dl
            .list_peer_sources(&hash_hex)
            .unwrap_or_default()
            .into_iter()
            .find(|s| {
                s.address
                    .rsplit_once(':')
                    .and_then(|(h, _)| h.parse::<IpAddr>().ok())
                    .is_some_and(|ip| ip == peer_addr.ip())
            })
        {
            let _ = dl.update_peer_source_details(
                &hash_hex,
                &src.address,
                None,
                Some(status.bitfield.as_slice()),
            );
        }
    }

    /// eMule `UDPReaskFNF` — `m_DeadSourceList.AddDeadSource` : le pair a
    /// répondu `OP_FILENOTFOUND` pour le fichier requis ; la source est
    /// marquée morte (statut `Failed`, hors du srclist — deadlist eMule).
    async fn mark_source_file_not_found(&self, peer_addr: SocketAddr, file_hash: [u8; 16]) {
        let Some(dl_arc) = self.downloads.read().unwrap().clone() else {
            return;
        };
        let hash_hex = hex::encode(file_hash);
        let mut dl = dl_arc.lock().await;
        let src = dl
            .list_peer_sources(&hash_hex)
            .unwrap_or_default()
            .into_iter()
            .find(|s| {
                s.address
                    .rsplit_once(':')
                    .and_then(|(h, _)| h.parse::<IpAddr>().ok())
                    .is_some_and(|ip| ip == peer_addr.ip())
            });
        if let Some(src) = src {
            let _ = dl.update_peer_source_status(
                &hash_hex,
                &src.address,
                mule_core::model::PeerSourceStatus::Failed,
                Some("peer answered OP_FILENOTFOUND (UDP deadlist)"),
                None,
            );
            tracing::info!(
                "Client UDP: source {} marked dead for {hash_hex} (OP_FILENOTFOUND)",
                src.address
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn target(ip: IpAddr, file_hash: [u8; 16]) -> UdpReaskTarget {
        UdpReaskTarget {
            addr: SocketAddr::new(ip, 12345),
            file_hash,
            connected_at_unix_ms: Some(0),
        }
    }

    async fn bound_service() -> ClientUdpService {
        let svc = ClientUdpService::new();
        let sock = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0))
            .await
            .expect("bind loopback UDP socket");
        svc.set_socket(Arc::new(sock)).await;
        svc
    }

    /// Vieillit les entrées internes comme si le ping précédent n'avait
    /// jamais reçu de réponse : `pending` au-delà de `UDP_REASK_INTERVAL`
    /// (le throttle laisse repasser l'appel) et `awaiting_ack` au-delà de
    /// `PENDING_TTL` (compté comme échec au prochain `send`).
    async fn age_out(svc: &ClientUdpService) {
        let mut inner = svc.inner.lock().await;
        for at in inner.pending.values_mut() {
            *at = Instant::now() - (UDP_REASK_INTERVAL + Duration::from_secs(60));
        }
        for at in inner.awaiting_ack.values_mut() {
            *at = Instant::now() - (PENDING_TTL + Duration::from_secs(60));
        }
    }

    /// eMule `UDPReaskForDownload` : après >3 paquets et >30 % d'échecs,
    /// le pair n'est plus sollicité en UDP (`DownloadClient.cpp:1340`).
    #[tokio::test]
    async fn udp_failure_ratio_guard_stops_reasks() {
        let svc = bound_service().await;
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let t = target(ip, [0xAA; 16]);

        // 4 pings tous restés sans réponse → 100 % d'échecs.
        for _ in 0..4 {
            assert!(svc.send_reask_file_ping(&t, 0).await);
            age_out(&svc).await;
        }
        let stats = svc.inner.lock().await.udp_stats.get(&ip).cloned().unwrap_or_default();
        assert_eq!(stats.total, 4);
        assert_eq!(stats.failed, 3); // le 4e échec est compté au send suivant

        // 5e appel : le 4e ping expire → failed=4, total=4>3, ratio 100 %
        // > 30 % → la garde bloque l'envoi.
        assert!(!svc.send_reask_file_ping(&t, 0).await);
    }

    /// Ratio ≤ 30 % : les reasks UDP continuent (1 échec sur 4 = 25 %).
    #[tokio::test]
    async fn udp_reasks_continue_below_failure_threshold() {
        let svc = bound_service().await;
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        let t = target(ip, [0xBB; 16]);
        let peer_addr = t.addr;

        // 4 pings : 3 acks + 1 silence.
        for round in 0..4 {
            assert!(svc.send_reask_file_ping(&t, 0).await);
            if round < 3 {
                svc.handle_reask_ack(peer_addr, UdpReaskResult::Rank(7), None)
                    .await;
            }
            age_out(&svc).await;
        }

        // total=4, failed=1 → 25 % ≤ 30 % : le reask suivant est permis.
        assert!(svc.send_reask_file_ping(&t, 0).await);
    }

    /// `m_bUDPPending` : tant qu'un ack peut encore arriver, aucun nouveau
    /// ping ne part vers ce pair (même pour un autre fichier).
    #[tokio::test]
    async fn udp_pending_blocks_second_ping_same_peer() {
        let svc = bound_service().await;
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));
        let t1 = target(ip, [0xCC; 16]);
        let t2 = target(ip, [0xDD; 16]);

        assert!(svc.send_reask_file_ping(&t1, 0).await);
        assert!(!svc.send_reask_file_ping(&t2, 0).await);
    }

    /// Un ack non sollicité (aucun ping en attente) est ignoré.
    #[tokio::test]
    async fn udp_unsolicited_ack_ignored() {
        let svc = bound_service().await;
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 4));
        svc.handle_reask_ack(
            SocketAddr::new(ip, 12345),
            UdpReaskResult::Rank(3),
            None,
        )
        .await;
        assert!(svc.last_result(ip).await.is_none());
    }
}
