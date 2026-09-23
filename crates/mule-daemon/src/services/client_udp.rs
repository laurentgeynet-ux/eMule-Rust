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

#[derive(Default)]
struct ClientUdpInner {
    socket: Option<Arc<UdpSocket>>,
    /// In-flight outbound reasks: (peer IP, file hash) → sent at.
    pending: HashMap<(IpAddr, [u8; 16]), Instant>,
    /// Last result per peer IP.
    results: HashMap<IpAddr, (UdpReaskResult, Instant)>,
    /// UDP version announced by each queued inbound client (IP → version),
    /// used to shape the `OP_REASKACK` payload (`sender->GetUDPVersion()`).
    client_udp_versions: HashMap<IpAddr, u8>,
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
        let key = (target.addr.ip(), target.file_hash);
        {
            let mut inner = self.inner.lock().await;
            inner
                .pending
                .retain(|_, at| at.elapsed() < UDP_REASK_INTERVAL);
            if inner.pending.contains_key(&key) {
                return false;
            }
            let Some(socket) = inner.socket.clone() else {
                return false;
            };
            inner.pending.insert(key, Instant::now());
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
        let rank = match &mgr {
            Some(m) => m.waiting_rank_by_ip(&peer_addr.ip()).await,
            None => None,
        };

        let response = if !shared {
            client_udp::encode_file_not_found()
        } else if let Some(rank) = rank {
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
        } else {
            let nearly_full = match &mgr {
                Some(m) => m.is_nearly_full().await,
                None => false,
            };
            if nearly_full {
                client_udp::encode_queue_full()
            } else {
                // eMule: "Don't answer him. Force him to establish a TCP connection."
                return;
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

    /// Validates an ack against pending reasks and applies the rank to the
    /// queued peer session (eMule `sender->UDPReaskACK(nRank)`).
    async fn handle_reask_ack(
        &self,
        peer_addr: SocketAddr,
        result: UdpReaskResult,
        _part_status: Option<Vec<u8>>,
    ) {
        let had_pending = {
            let mut inner = self.inner.lock().await;
            let key_exists = inner
                .pending
                .iter()
                .any(|((ip, _), at)| *ip == peer_addr.ip() && at.elapsed() < PENDING_TTL);
            if key_exists {
                inner.pending.retain(|(ip, _), _| *ip != peer_addr.ip());
                inner
                    .results
                    .insert(peer_addr.ip(), (result.clone(), Instant::now()));
            }
            key_exists
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

        if let UdpReaskResult::Rank(rank) = result {
            let ps = self.peer_session.read().unwrap().clone();
            if let Some(ps) = ps {
                ps.apply_udp_queue_rank(&peer_addr.ip(), rank as u32).await;
            }
        }
    }
}
