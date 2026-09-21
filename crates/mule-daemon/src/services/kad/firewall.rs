//! Kad UDP firewall/reachability test (KADEMLIA2_FIREWALLUDP / OP_FWCHECKUDPREQ).
//!
//! Mirrors eMule `CUDPFirewallTester` (UDPFirewallTester.cpp): the local node
//! asks up to 2 suitable Kad contacts over a TCP eD2K connection to send
//! `KADEMLIA2_FIREWALLUDP` datagrams to our advertised Kad UDP port(s). One
//! positive datagram proves the port is open; if all candidates fail, the
//! port is considered firewalled (eMule ~6 min overall timeout).

use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use mule_core::kad::{KadContact, KadId};
use mule_network_policy::ConnectionPurpose;
use mule_protocol::ed2k::{
    Ed2kEncoder, Ed2kHello, OP_FWCHECKUDPREQ, OP_HELLO, OP_HELLO_ANSWER, PR_ED2K, PR_EMULE,
};
use mule_protocol::framing::PacketCodec;

use super::KadService;
use crate::services::kad_udp::{FirewallUdpResult, UdpFirewallStatus};

/// Number of contacts asked per check round (eMule
/// `UDP_FIREWALLTEST_CLIENTSTOASK = 2`).
const UDP_FW_CLIENTS_TO_ASK: usize = 2;
/// Minimum Kad version a candidate must announce to serve a UDP firewall
/// check (eMule `KADEMLIA_VERSION6_49aBETA`).
const MIN_FW_TEST_KAD_VERSION: u8 = 6;
/// Kad version we advertise (eMule `KADEMLIA_VERSION8_49b`): supports
/// `TAG_KADMISCOPTIONS`, `KADEMLIA2_HELLO_RES_ACK` and the Kad >= 7
/// `FIREWALLED2_REQ` / `OP_KAD_FWTCPCHECK_ACK` TCP check flow.
pub(crate) const OUR_KAD_VERSION: u8 = 8;

const FW_TCP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const FW_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// How long to wait for the FIREWALLUDP datagram after sending the request.
const FW_UDP_WAIT_TIMEOUT: Duration = Duration::from_secs(30);
/// Overall check budget (eMule: ~6 minutes before declaring timed-out
/// firewalled).
const FW_OVERALL_TIMEOUT: Duration = Duration::from_secs(360);

impl KadService {
    /// Sets the local user hash used in test-connection hellos.
    pub fn set_fw_check_identity(&self, user_hash: [u8; 16]) {
        *self.fw_user_hash.write().unwrap() = user_hash;
    }

    /// Current UDP firewall status (Unverified/Testing/Open/Firewalled).
    pub async fn udp_firewall_status(&self) -> UdpFirewallStatus {
        self.udp.firewall_tracker().lock().await.status
    }

    /// Runs one UDP firewall check round against suitable routing contacts.
    /// Returns the resulting status. No-op if a check is already running or
    /// the UDP socket is down.
    pub async fn run_udp_firewall_check(&self) -> UdpFirewallStatus {
        let tracker = self.udp.firewall_tracker();

        // Mark testing (or bail if a round is already in progress).
        {
            let mut fw = tracker.lock().await;
            if fw.status == UdpFirewallStatus::Testing {
                return fw.status;
            }
            fw.status = UdpFirewallStatus::Testing;
        }

        let result =
            match tokio::time::timeout(FW_OVERALL_TIMEOUT, self.fw_check_round(&tracker)).await {
                Ok(st) => st,
                Err(_) => {
                    tracing::warn!(
                        "Kad UDP firewall check timed out after {:?} — declaring firewalled",
                        FW_OVERALL_TIMEOUT
                    );
                    UdpFirewallStatus::Firewalled
                }
            };

        let mut fw = tracker.lock().await;
        fw.status = result;
        if result == UdpFirewallStatus::Open || result == UdpFirewallStatus::Firewalled {
            fw.verified = true;
            fw.last_check = Some(std::time::Instant::now());
        }
        result
    }

    async fn fw_check_round(
        &self,
        tracker: &Arc<tokio::sync::Mutex<crate::services::kad_udp::FirewallTestTracker>>,
    ) -> UdpFirewallStatus {
        let intern_port = match self.udp.local_udp_port().await {
            Some(p) if p != 0 => p,
            _ => {
                tracing::debug!("Kad UDP firewall check skipped: UDP socket not bound");
                return UdpFirewallStatus::Unverified;
            }
        };
        let extern_port = tracker.lock().await.extern_port;

        // Candidate selection (eMule FindNodeFWCheckUDP + suitability rules):
        // verified contacts, Kad version >= 6, known TCP port, distinct IPs.
        let candidates: Vec<KadContact> = {
            let table = self.routing_table.lock().await;
            let mut seen = HashSet::new();
            table
                .find_closest(&KadId::random(), 20)
                .into_iter()
                .filter(|c| {
                    c.verified
                        && c.version >= MIN_FW_TEST_KAD_VERSION
                        && c.tcp_port.is_some()
                        && c.kad_id != self.local_id
                        && seen.insert(c.ip)
                })
                .take(UDP_FW_CLIENTS_TO_ASK)
                .collect()
        };

        if candidates.is_empty() {
            tracing::debug!(
                "Kad UDP firewall check: no suitable candidate contacts (need verified Kad>=6 with TCP port)"
            );
            return UdpFirewallStatus::Unverified;
        }

        for contact in &candidates {
            match self
                .fw_test_candidate(contact, tracker, intern_port, extern_port)
                .await
            {
                Ok(res) => {
                    // eMule SetUDPFWCheckResult: a datagram is only conclusive
                    // when error_code == 0 and the probed port is a port we
                    // actually advertised (intern or known extern).
                    let valid_port = res.port_tested == intern_port
                        || (extern_port == Some(res.port_tested));
                    if res.error_code == 0 && valid_port {
                        let mut fw = tracker.lock().await;
                        if res.port_tested != intern_port {
                            // Reached us through the NAT-mapped extern port.
                            fw.extern_port = Some(res.port_tested);
                            fw.use_extern = true;
                            tracing::info!(
                                "Kad UDP firewall check: OPEN via extern port {} (from {})",
                                res.port_tested,
                                contact.ip
                            );
                        } else {
                            fw.use_extern = false;
                            tracing::info!(
                                "Kad UDP firewall check: OPEN on intern port {} (from {})",
                                intern_port,
                                contact.ip
                            );
                        }
                        return UdpFirewallStatus::Open;
                    }
                    tracing::warn!(
                        "Kad UDP firewall check from {} returned unusable result (error={}, port={}) — trying next candidate",
                        contact.ip,
                        res.error_code,
                        res.port_tested
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        "Kad UDP firewall check via {} failed: {} — trying next candidate",
                        contact.ip,
                        e
                    );
                }
            }
        }

        UdpFirewallStatus::Firewalled
    }

    /// Performs a single check against one contact: TCP connect → OP_HELLO →
    /// OP_FWCHECKUDPREQ (PR_EMULE) → wait for the FIREWALLUDP datagram.
    async fn fw_test_candidate(
        &self,
        contact: &KadContact,
        tracker: &Arc<tokio::sync::Mutex<crate::services::kad_udp::FirewallTestTracker>>,
        intern_port: u16,
        extern_port: Option<u16>,
    ) -> Result<FirewallUdpResult, String> {
        let ip = IpAddr::V4(contact.ip);
        let tcp_port = contact.tcp_port.ok_or("candidate has no TCP port")?;
        let addr = SocketAddr::new(ip, tcp_port);

        // Per-candidate UDP verify key (eMule GetUDPVerifyKey): remote peers
        // encrypt their FIREWALLUDP reply with it — stored in the tracker so
        // the UDP receive loop can try decryption.
        let udp_verify_key: u32 = rand::random::<u32>().wrapping_add(1);

        // Register before connecting so an early datagram is never missed.
        let rx = tracker.lock().await.register_candidate(ip, udp_verify_key);

        let result = self
            .fw_test_candidate_inner(addr, rx, udp_verify_key, intern_port, extern_port)
            .await;

        tracker.lock().await.unregister_candidate(&ip);
        result
    }

    async fn fw_test_candidate_inner(
        &self,
        addr: SocketAddr,
        rx: tokio::sync::oneshot::Receiver<FirewallUdpResult>,
        udp_verify_key: u32,
        intern_port: u16,
        extern_port: Option<u16>,
    ) -> Result<FirewallUdpResult, String> {
        let _permit = self
            .policy_mgr
            .request_tcp_connect(addr, ConnectionPurpose::Ed2kPeer)
            .map_err(|e| format!("policy denied connect to {addr}: {e}"))?;

        let mut stream = tokio::time::timeout(FW_TCP_CONNECT_TIMEOUT, TcpStream::connect(addr))
            .await
            .map_err(|_| format!("TCP connect to {addr} timed out"))?
            .map_err(|e| format!("TCP connect to {addr} refused: {e}"))?;

        // OP_HELLO — the remote peer checks our advertised Kad version
        // (MiscOptions2 bits 0-3) before accepting OP_FWCHECKUDPREQ.
        let hello = self.build_fw_hello(intern_port);
        let encoder = Ed2kEncoder::default();
        let hello_pkt = encoder
            .encode_hello(&hello)
            .map_err(|e| format!("failed to encode OP_HELLO: {e}"))?;
        let mut out = Vec::with_capacity(6 + hello_pkt.payload.len());
        out.push(hello_pkt.protocol);
        out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
        out.push(hello_pkt.opcode);
        out.extend_from_slice(&hello_pkt.payload);
        stream
            .write_all(&out)
            .await
            .map_err(|e| format!("failed to send OP_HELLO: {e}"))?;

        // Wait for OP_HELLO_ANSWER (or a bare OP_HELLO — some clients
        // initiate their own hello first).
        let codec = PacketCodec::new();
        let mut read_buf = BytesMut::with_capacity(8192);
        let mut raw_buf = [0u8; 4096];
        let deadline = tokio::time::Instant::now() + FW_HANDSHAKE_TIMEOUT;
        let mut hello_done = false;
        while !hello_done {
            let read_res = tokio::time::timeout_at(deadline, stream.read(&mut raw_buf))
                .await
                .map_err(|_| "handshake timed out waiting for OP_HELLO_ANSWER")?
                .map_err(|e| format!("read error during handshake: {e}"))?;
            if read_res == 0 {
                return Err("peer closed connection during handshake".to_string());
            }
            read_buf.extend_from_slice(&raw_buf[..read_res]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K
                    && (pkt.opcode == OP_HELLO_ANSWER || pkt.opcode == OP_HELLO)
                {
                    hello_done = true;
                    break;
                }
            }
        }

        // OP_FWCHECKUDPREQ (PR_EMULE): u16 intern + u16 extern + u32 key.
        let mut payload = Vec::with_capacity(8);
        payload.extend_from_slice(&intern_port.to_le_bytes());
        payload.extend_from_slice(&extern_port.unwrap_or(0).to_le_bytes());
        payload.extend_from_slice(&udp_verify_key.to_le_bytes());
        let mut req = Vec::with_capacity(6 + payload.len());
        req.push(PR_EMULE);
        req.extend_from_slice(&((payload.len() as u32) + 1).to_le_bytes());
        req.push(OP_FWCHECKUDPREQ);
        req.extend_from_slice(&payload);
        stream
            .write_all(&req)
            .await
            .map_err(|e| format!("failed to send OP_FWCHECKUDPREQ: {e}"))?;

        tracing::info!(
            "Kad UDP firewall check: OP_FWCHECKUDPREQ sent to {addr} (intern={intern_port}, extern={})",
            extern_port.unwrap_or(0)
        );

        // Keep the TCP connection open while waiting for the datagram (eMule
        // drops the test if the requesting client disconnects).
        tokio::time::timeout(FW_UDP_WAIT_TIMEOUT, rx)
            .await
            .map_err(|_| "no FIREWALLUDP datagram within timeout".to_string())?
            .map_err(|_| "firewall test channel dropped".to_string())
    }

    /// Minimal OP_HELLO for firewall-test connections — same capability flags
    /// as the regular peer handshake, with Kad version >= 6 in MiscOptions2
    /// (remote peers reject OP_FWCHECKUDPREQ from older clients).
    fn build_fw_hello(&self, intern_port: u16) -> Ed2kHello {
        use mule_protocol::ed2k::tags::*;
        let user_hash = *self.fw_user_hash.read().unwrap();
        let ed2k_tcp_port = self
            .ed2k_tcp_port
            .load(std::sync::atomic::Ordering::Relaxed);

        let mut tags = Vec::with_capacity(6);
        tags.push(Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_NAME),
            value: TagValue::String("Emule-Sion".to_string()),
        });
        tags.push(Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_VERSION),
            value: TagValue::UInt32(0x3C),
        });
        let udp_ports = ((intern_port as u32) << 16) | (intern_port as u32);
        tags.push(Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_EMULE_UDPPORTS),
            value: TagValue::UInt32(udp_ports),
        });
        tags.push(Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_CT_EMULE_VERSION),
            value: TagValue::UInt32(50 << 10),
        });
        let misc1: u32 =
            (1 << 29) | (1 << 28) | (4 << 24) | (1 << 20) | (3 << 16) | (4 << 12) | (1 << 4) | 1;
        tags.push(Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_EMULE_MISCOPTIONS1),
            value: TagValue::UInt32(misc1),
        });
        // MiscOptions2: SX2<<10 | ExtMultiPkt<<5 | LargeFiles<<4 | KadVersion<<0
        let misc2: u32 = (1 << 10) | (1 << 5) | (1 << 4) | (OUR_KAD_VERSION as u32);
        tags.push(Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_EMULE_MISCOPTIONS2),
            value: TagValue::UInt32(misc2),
        });

        Ed2kHello {
            user_hash,
            client_id: 0,
            tcp_port: ed2k_tcp_port,
            tags,
            unknown_tags: vec![],
            server_ip: 0,
            server_port: 0,
        }
    }
}
