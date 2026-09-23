// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, Mutex};

use mule_protocol::kad::KadMessage;

/// Transaction timeout in seconds.
pub const KAD_TRANSACTION_TIMEOUT_SECS: u64 = 3;

/// Maximum number of concurrent in-flight transactions to prevent memory exhaustion.
pub const MAX_IN_FLIGHT_TRANSACTIONS: usize = 200;

/// Maximum requests per second allowed per source IP (anti-amplification / rate limiting).
pub const MAX_KAD_REQUESTS_PER_SEC_PER_IP: u32 = 120;

/// Simple per-IP sliding rate limiter.
#[derive(Debug, Default)]
pub struct IpRateLimiter {
    counts: HashMap<IpAddr, (u32, Instant)>,
}

impl IpRateLimiter {
    pub fn new() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }

    pub fn check_and_record(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        if self.counts.len() > 1000 {
            self.counts
                .retain(|_, (_, ts)| now.duration_since(*ts) < Duration::from_secs(10));
        }

        let entry = self.counts.entry(ip).or_insert((0, now));
        if now.duration_since(entry.1) >= Duration::from_secs(1) {
            *entry = (1, now);
            true
        } else if entry.0 < MAX_KAD_REQUESTS_PER_SEC_PER_IP {
            entry.0 += 1;
            true
        } else {
            false
        }
    }
}

/// Expected response variant for a pending in-flight transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KadExpectedResponse {
    Pong,
    HelloRes,
    BootstrapRes,
    FindNodeRes,
    SearchSourceRes,
    SearchKeywordRes,
    PublishSourceRes,
    PublishKeywordRes,
    SearchNotesRes,
    PublishNotesRes,
}

/// Pending in-flight transaction state.
pub struct InFlightTransaction {
    pub target: SocketAddr,
    pub expected_response: KadExpectedResponse,
    pub expected_target_hash: Option<[u8; 16]>,
    pub created_at: Instant,
    /// Multi-packet responses (SEARCH_*_RES arrive as several ~50-entry
    /// batches) need a channel that stays open after the first packet —
    /// hence unbounded mpsc rather than oneshot.
    pub sender: mpsc::UnboundedSender<Result<KadMessage, String>>,
}

/// RAII guard for a registered in-flight transaction: removes the entry on
/// drop. Callers may wrap a `send_*` future in an outer timeout or `select!`
/// branch — cancellation then skips the timeout branch's manual `remove`,
/// leaking the entry toward `MAX_IN_FLIGHT_TRANSACTIONS` until the map
/// saturates and every subsequent send fails. `try_lock` keeps `Drop`
/// synchronous; a contended map is healed by the stale-entry sweep in
/// `insert_in_flight`. Removing an already-consumed tx_id is a no-op.
pub struct InFlightTxGuard {
    map: InFlightMap,
    tx_id: u32,
}

impl InFlightTxGuard {
    /// Creates a guard that removes `tx_id` from `map` on drop.
    pub fn new(map: InFlightMap, tx_id: u32) -> Self {
        Self { map, tx_id }
    }
}

impl Drop for InFlightTxGuard {
    fn drop(&mut self) {
        if let Ok(mut map) = self.map.try_lock() {
            map.remove(&self.tx_id);
        }
    }
}

/// Statistics snapshot of the UDP Kad service.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KadUdpSnapshot {
    pub is_active: bool,
    pub bind_address: Option<String>,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub active_transactions: usize,
}

pub type InFlightMap = Arc<Mutex<HashMap<u32, InFlightTransaction>>>;

/// UDP firewall/reachability state for the local Kad port
/// (mirrors eMule `CUDPFirewallTester`: `m_bFirewalledUDP` + verified flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UdpFirewallStatus {
    /// No conclusive test has run yet (eMule `!m_bIsFWVerifiedUDP`).
    #[default]
    Unverified,
    /// A check is in progress.
    Testing,
    /// At least one `KADEMLIA2_FIREWALLUDP` packet was received → port open.
    Open,
    /// All candidates answered/failed without a positive packet → firewalled.
    Firewalled,
}

impl UdpFirewallStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unverified => "unverified",
            Self::Testing => "testing",
            Self::Open => "open",
            Self::Firewalled => "firewalled",
        }
    }
}

/// Result delivered to a pending firewall test when a
/// `KADEMLIA2_FIREWALLUDP` datagram arrives from a candidate IP.
#[derive(Debug, Clone, Copy)]
pub struct FirewallUdpResult {
    pub error_code: u8,
    /// UDP port the remote peer probed (its view of our intern/extern port).
    pub port_tested: u16,
}

/// Tracks pending outbound UDP firewall checks and the last known state.
/// Pending candidates are keyed by IPv4 so unsolicited or stray
/// `KADEMLIA2_FIREWALLUDP` datagrams are ignored (eMule `bRequested` check).
#[derive(Debug, Default)]
pub struct FirewallTestTracker {
    pub status: UdpFirewallStatus,
    /// True once a check produced a conclusive result (open or firewalled).
    pub verified: bool,
    /// Extern Kad UDP port learned from a probe that reached a port different
    /// from the bound one (NAT-mapped). `use_extern` mirrors eMule's
    /// `GetUseExternKadPort` preference.
    pub extern_port: Option<u16>,
    pub use_extern: bool,
    /// Our public IPv4 as observed by a peer (`KADEMLIA_FIREWALLED_RES`
    /// echoes it back). Used as the advertised IP when publishing ourselves
    /// as a source — the socket bind address may be `0.0.0.0` or NATed.
    pub observed_ip: Option<Ipv4Addr>,
    /// Last completed check time (for cooldowns / re-verification).
    pub last_check: Option<Instant>,
    /// Pending tests per candidate IP: `(verify_key, result_sender)`. The
    /// verify key is the `sender_key` we sent in `OP_FWCHECKUDPREQ` — remote
    /// peers encrypt their `FIREWALLUDP` reply with it.
    pending: HashMap<IpAddr, Vec<(u32, oneshot::Sender<FirewallUdpResult>)>>,
}

impl FirewallTestTracker {
    /// Registers a pending test for `candidate_ip`. Returns a receiver that
    /// resolves on the first matching `KADEMLIA2_FIREWALLUDP` datagram.
    pub fn register_candidate(
        &mut self,
        candidate_ip: IpAddr,
        verify_key: u32,
    ) -> oneshot::Receiver<FirewallUdpResult> {
        let (tx, rx) = oneshot::channel();
        self.pending
            .entry(candidate_ip)
            .or_default()
            .push((verify_key, tx));
        rx
    }

    /// Removes all pending tests for an IP (timeout/cancel path).
    pub fn unregister_candidate(&mut self, candidate_ip: &IpAddr) {
        self.pending.remove(candidate_ip);
    }

    /// UDP verify keys of pending tests for `ip` — used to try decrypting
    /// obfuscated Kad datagrams from that peer.
    pub fn pending_verify_keys(&self, ip: &IpAddr) -> Vec<u32> {
        self.pending
            .get(ip)
            .map(|v| v.iter().map(|(k, _)| *k).collect())
            .unwrap_or_default()
    }

    /// Routes an incoming FIREWALLUDP datagram to the matching pending test.
    /// Returns false when the source IP was never asked (unsolicited packet).
    pub fn notify_packet(&mut self, from_ip: IpAddr, result: FirewallUdpResult) -> bool {
        match self.pending.get_mut(&from_ip) {
            Some(v) if !v.is_empty() => {
                // One successful datagram resolves the test; extra datagrams
                // (intern+extern pair) are delivered to any remaining waiters.
                let (_, tx) = v.remove(0);
                if v.is_empty() {
                    self.pending.remove(&from_ip);
                }
                tx.send(result).is_ok()
            }
            _ => false,
        }
    }

    /// Whether any firewall check is currently waiting on datagrams.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Effective UDP port other peers should use to reach us on Kad.
    pub fn effective_port(&self, intern_port: u16) -> u16 {
        if self.use_extern {
            self.extern_port.unwrap_or(intern_port)
        } else {
            intern_port
        }
    }
}

/// Buddy link relay state (eMule `CClientList::m_pBuddy` — a single buddy
/// at a time). The daemon can act as a *relay* for one LowID client:
/// `KADEMLIA_FINDBUDDY_REQ` registers a pending entry, the requester's TCP
/// connection (adopted by the inbound listener) becomes the active link, and
/// `KADEMLIA_CALLBACK_REQ` / `OP_REASKCALLBACKUDP` datagrams are forwarded
/// onto it as `OP_CALLBACK` / `OP_REASKCALLBACKTCP`.
#[derive(Debug, Default)]
pub struct BuddyRegistry {
    /// Accepted `FINDBUDDY_REQ` senders awaiting their TCP buddy link.
    pending: HashMap<IpAddr, PendingBuddy>,
    /// The live buddy link (at most one, mirroring eMule).
    pub active: Option<ActiveBuddy>,
}

/// How long a `FINDBUDDY_REQ` acceptance stays valid awaiting the TCP link.
pub const BUDDY_PENDING_TTL: Duration = Duration::from_secs(120);

/// A `FINDBUDDY_REQ` we accepted; awaiting the requester's TCP connection.
#[derive(Debug, Clone)]
pub struct PendingBuddy {
    /// Buddy ID the requester asked us to register under.
    pub buddy_id: [u8; 16],
    /// Requester's Kad client ID.
    pub user_id: [u8; 16],
    /// Requester's announced TCP port.
    pub tcp_port: u16,
    /// Expiry instant of this pending registration.
    pub expires: Instant,
}

/// A live buddy TCP link; `tx` accepts fully framed eD2K wire packets
/// (`proto + len:u32 + opcode + payload`) to write on the buddy socket.
#[derive(Debug)]
pub struct ActiveBuddy {
    /// Buddy ID registered at `FINDBUDDY_REQ` time.
    pub buddy_id: [u8; 16],
    /// Buddy client's Kad ID / user hash.
    pub user_id: [u8; 16],
    /// Remote socket address of the buddy link.
    pub remote_addr: SocketAddr,
    /// Outbound packet sink for the buddy socket writer task.
    pub tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
}

impl BuddyRegistry {
    /// Registers an accepted buddy candidate. Caller must have checked
    /// eligibility (verified non-firewalled node, no active buddy).
    pub fn register_pending(
        &mut self,
        ip: IpAddr,
        buddy_id: [u8; 16],
        user_id: [u8; 16],
        tcp_port: u16,
    ) {
        self.pending.retain(|_, p| p.expires > Instant::now());
        self.pending.insert(
            ip,
            PendingBuddy {
                buddy_id,
                user_id,
                tcp_port,
                expires: Instant::now() + BUDDY_PENDING_TTL,
            },
        );
    }

    /// Consumes the pending registration for `ip` when its TCP link arrives.
    pub fn take_pending(&mut self, ip: &IpAddr) -> Option<PendingBuddy> {
        self.pending.retain(|_, p| p.expires > Instant::now());
        self.pending.remove(ip)
    }

    /// Installs the active buddy link. Returns false when a buddy is
    /// already connected (eMule accepts a single buddy).
    pub fn set_active(&mut self, buddy: ActiveBuddy) -> bool {
        if self.active.is_some() {
            return false;
        }
        self.active = Some(buddy);
        true
    }

    /// Clears the active buddy if it belongs to `addr`.
    pub fn clear_active(&mut self, addr: &SocketAddr) {
        if self
            .active
            .as_ref()
            .map(|b| b.remote_addr == *addr)
            .unwrap_or(false)
        {
            self.active = None;
        }
    }

    /// Sends a framed packet to the active buddy. Returns false when no
    /// buddy link exists or its writer task is gone.
    pub fn send_to_buddy(&self, packet: Vec<u8>) -> bool {
        match &self.active {
            Some(b) => b.tx.send(packet).is_ok(),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn pending_candidate_receives_result() {
        let mut t = FirewallTestTracker::default();
        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        let mut rx = t.register_candidate(ip, 0xABCD);
        assert!(t.has_pending());
        assert_eq!(t.pending_verify_keys(&ip), vec![0xABCD]);
        assert!(t.notify_packet(
            ip,
            FirewallUdpResult {
                error_code: 0,
                port_tested: 4672
            }
        ));
        let res = rx.try_recv().expect("result delivered");
        assert_eq!(res.error_code, 0);
        assert_eq!(res.port_tested, 4672);
        assert!(!t.has_pending());
    }

    #[test]
    fn unsolicited_packet_is_ignored() {
        let mut t = FirewallTestTracker::default();
        let stranger = IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9));
        assert!(!t.notify_packet(
            stranger,
            FirewallUdpResult {
                error_code: 0,
                port_tested: 4672
            }
        ));
    }

    #[test]
    fn packet_from_wrong_ip_does_not_resolve() {
        let mut t = FirewallTestTracker::default();
        let asked = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        let other = IpAddr::V4(Ipv4Addr::new(5, 6, 7, 8));
        let mut rx = t.register_candidate(asked, 1);
        assert!(!t.notify_packet(
            other,
            FirewallUdpResult {
                error_code: 0,
                port_tested: 4672
            }
        ));
        assert!(rx.try_recv().is_err());
        assert!(t.has_pending());
        t.unregister_candidate(&asked);
        assert!(!t.has_pending());
    }

    #[test]
    fn buddy_pending_lifecycle() {
        let mut r = BuddyRegistry::default();
        let ip = IpAddr::V4(Ipv4Addr::new(9, 8, 7, 6));
        r.register_pending(ip, [1u8; 16], [2u8; 16], 4662);
        let p = r.take_pending(&ip).expect("pending registered");
        assert_eq!(p.buddy_id, [1u8; 16]);
        assert_eq!(p.tcp_port, 4662);
        assert!(r.take_pending(&ip).is_none());
    }

    #[test]
    fn buddy_single_active() {
        let mut r = BuddyRegistry::default();
        let (tx1, _rx1) = tokio::sync::mpsc::unbounded_channel();
        let (tx2, _rx2) = tokio::sync::mpsc::unbounded_channel();
        let a1 = ActiveBuddy {
            buddy_id: [1u8; 16],
            user_id: [2u8; 16],
            remote_addr: "1.2.3.4:4662".parse().unwrap(),
            tx: tx1,
        };
        let a2 = ActiveBuddy {
            buddy_id: [3u8; 16],
            user_id: [4u8; 16],
            remote_addr: "5.6.7.8:4662".parse().unwrap(),
            tx: tx2,
        };
        assert!(r.set_active(a1));
        assert!(!r.set_active(a2));
        assert!(r.send_to_buddy(vec![0xC5]));
        r.clear_active(&"9.9.9.9:1".parse().unwrap());
        assert!(r.send_to_buddy(vec![0xC5]));
        r.clear_active(&"1.2.3.4:4662".parse().unwrap());
        assert!(!r.send_to_buddy(vec![0xC5]));
    }

    #[test]
    fn effective_port_prefers_intern_unless_extern_verified() {
        let mut t = FirewallTestTracker::default();
        assert_eq!(t.effective_port(4672), 4672);
        t.extern_port = Some(13370);
        assert_eq!(t.effective_port(4672), 4672);
        t.use_extern = true;
        assert_eq!(t.effective_port(4672), 13370);
    }
}
