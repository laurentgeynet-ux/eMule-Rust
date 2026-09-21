use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;

use mule_protocol::ed2k::OP_CALLBACK;
use mule_protocol::kad::*;

use super::storage::{KadStorage, KAD_STORE_TTL};
use super::types::{
    BuddyRegistry, FirewallTestTracker, FirewallUdpResult, InFlightMap, KadExpectedResponse,
    UdpFirewallStatus,
};
use crate::services::downloads::DownloadService;
use crate::services::kad::firewall::OUR_KAD_VERSION;

type DownloadsSlot = Arc<tokio::sync::RwLock<Option<Arc<tokio::sync::Mutex<DownloadService>>>>>;

/// Internal dispatcher for incoming messages.
#[allow(clippy::too_many_arguments)]
pub async fn handle_incoming_message(
    socket: &Arc<UdpSocket>,
    peer_addr: SocketAddr,
    msg: KadMessage,
    in_flight: &InFlightMap,
    local_kad_id: &[u8; 16],
    local_port: u16,
    storage: &Arc<tokio::sync::Mutex<KadStorage>>,
    downloads: &DownloadsSlot,
    firewall: &Arc<tokio::sync::Mutex<FirewallTestTracker>>,
    buddy: &Arc<tokio::sync::Mutex<BuddyRegistry>>,
    buddy_tcp_port: u16,
    buddy_connect_options: u8,
) {
    match msg {
        KadMessage::Ping(_ping) => {
            tracing::info!("Kad UDP: received Ping from {peer_addr}, replying with Pong");
            let pong_bytes = KadCodec::encode_pong(peer_addr.port());
            let _ = socket.send_to(&pong_bytes, peer_addr).await;
        }
        KadMessage::Pong(pong) => {
            let mut inf = in_flight.lock().await;
            let mut matched_tx = None;
            for (id, tx) in inf.iter() {
                if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip())
                    && tx.expected_response == KadExpectedResponse::Pong
                {
                    matched_tx = Some(*id);
                    break;
                }
            }
            tracing::info!("Kad UDP: received Pong from {peer_addr} (matched_tx={matched_tx:?})");
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    let _ = tx.sender.send(Ok(KadMessage::Pong(pong)));
                }
            }
        }
        KadMessage::HelloReq(hello) => {
            tracing::info!(
                "Kad UDP: received HelloReq from {peer_addr} (kad_v{}, tcp={}, misc={:?}), replying with HelloRes",
                hello.version,
                hello.tcp_port,
                hello.misc_options
            );
            // eMule KADMISCOPTIONS: bit0 = UDP firewalled, bit2 = ask the
            // receiver to complete the handshake with HELLO_RES_ACK.
            let mut misc = 0x04u8;
            {
                let fw = firewall.lock().await;
                if fw.verified && fw.status == UdpFirewallStatus::Firewalled {
                    misc |= 0x01;
                }
            }
            let hello_res = KadCodec::encode_hello_res(
                local_kad_id,
                local_port,
                OUR_KAD_VERSION,
                buddy_tcp_port,
                Some(misc),
            );
            let _ = socket.send_to(&hello_res, peer_addr).await;
        }
        KadMessage::HelloRes(hello_res) => {
            // Kad >= 8: the responder may ask us to prove our IP with a
            // HELLO_RES_ACK (KADMISCOPTIONS bit 2).
            let wants_ack = hello_res.requests_ack();
            let peer_udp_port = if hello_res.udp_port != 0 {
                hello_res.udp_port
            } else {
                peer_addr.port()
            };
            let mut inf = in_flight.lock().await;
            let mut matched_tx = None;
            for (id, tx) in inf.iter() {
                if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip())
                    && tx.expected_response == KadExpectedResponse::HelloRes
                {
                    matched_tx = Some(*id);
                    break;
                }
            }
            tracing::info!(
                "Kad UDP: received HelloRes from {peer_addr} (kad_v{}, tcp={}, udp_tag={}, matched_tx={matched_tx:?})",
                hello_res.version,
                hello_res.tcp_port,
                peer_udp_port
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    let _ = tx.sender.send(Ok(KadMessage::HelloRes(hello_res)));
                }
            }
            if wants_ack {
                let ack = KadCodec::encode_hello_res_ack(local_kad_id);
                let _ = socket.send_to(&ack, peer_addr).await;
            }
        }
        KadMessage::HelloResAck(ack) => {
            // Completes the three-way hello we initiated — the peer proved it
            // owns its advertised IP (eMule marks the contact verified).
            tracing::info!(
                "Kad UDP: received HelloResAck from {peer_addr} (kad_id={})",
                hex::encode(ack.kad_id)
            );
        }
        KadMessage::FirewalledReq(req) => {
            handle_firewalled_check(socket, peer_addr, req.tcp_port, false).await;
        }
        KadMessage::FirewalledReq2(req) => {
            handle_firewalled_check(socket, peer_addr, req.tcp_port, true).await;
        }
        KadMessage::FirewalledRes(res) => {
            // Answer to a FIREWALLED_REQ we sent — carries our public IP as
            // seen by the responder. We only log: no TCP-check requester flow
            // is implemented yet, so unsolicited ones are dropped.
            let ip = std::net::Ipv4Addr::from(res.ip.to_le_bytes());
            tracing::info!("Kad UDP: received FirewalledRes from {peer_addr} (observed ip={ip})");
        }
        KadMessage::FirewalledAckRes => {
            tracing::info!(
                "Kad UDP: received FirewalledAckRes from {peer_addr} (TCP port reachable)"
            );
        }
        KadMessage::BootstrapReq(_) | KadMessage::Req(_) => {
            tracing::info!(
                "Kad UDP: received BootstrapReq/Req from {peer_addr}, replying with empty contacts"
            );
            let empty_res = KadCodec::encode_bootstrap_res(local_kad_id, local_port, 2, &[]);
            let _ = socket.send_to(&empty_res, peer_addr).await;
        }
        KadMessage::BootstrapRes(boot_res) => {
            let mut inf = in_flight.lock().await;
            let mut matched_tx = None;
            for (id, tx) in inf.iter() {
                if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip())
                    && (tx.expected_response == KadExpectedResponse::BootstrapRes
                        || tx.expected_response == KadExpectedResponse::FindNodeRes)
                {
                    matched_tx = Some(*id);
                    break;
                }
            }
            tracing::info!(
                "Kad UDP: received BootstrapRes/FindNodeRes from {peer_addr} with {} contacts (matched_tx={matched_tx:?})",
                boot_res.contacts.len()
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    let _ = tx.sender.send(Ok(KadMessage::BootstrapRes(boot_res)));
                }
            }
        }
        KadMessage::SearchSourceReq(req) => {
            let sources = {
                let mut store = storage.lock().await;
                store.get_sources(&req.target_file_hash, req.start_pos)
            };
            tracing::info!(
                "Kad UDP: received SearchSourceReq from {peer_addr} for file {}, replying with {} stored sources",
                hex::encode(req.target_file_hash),
                sources.len()
            );
            let res = KadCodec::encode_search_source_res(&req.target_file_hash, &sources);
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::SearchSourceRes(search_res) => {
            let mut inf = in_flight.lock().await;
            let mut matched_tx = None;
            for (id, tx) in inf.iter() {
                let ip_match = tx.target == peer_addr || tx.target.ip() == peer_addr.ip();
                let hash_match = tx
                    .expected_target_hash
                    .is_none_or(|h| h == search_res.target_file_hash);
                let resp_match = tx.expected_response == KadExpectedResponse::SearchSourceRes;
                if ip_match && hash_match && resp_match {
                    matched_tx = Some(*id);
                    break;
                }
            }
            tracing::info!(
                "Kad UDP: received SearchSourceRes from {peer_addr} with {} sources for file {} (matched_tx={matched_tx:?})",
                search_res.sources.len(),
                hex::encode(search_res.target_file_hash)
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    let _ = tx.sender.send(Ok(KadMessage::SearchSourceRes(search_res)));
                }
            }
        }
        KadMessage::PublishSourceReq(req) => {
            let peer_ip_bytes = match peer_addr.ip() {
                std::net::IpAddr::V4(v4) => v4.octets(),
                std::net::IpAddr::V6(_) => [127, 0, 0, 1],
            };
            // eMule CKademliaUDPListener::ProcessPublishSourceRequest: the
            // advertised contact must match the sender's IP and carry a TCP port.
            let valid_contact = req.contact.ip == peer_ip_bytes && req.contact.tcp_port > 0;
            let status = if valid_contact {
                let stored = {
                    let mut store = storage.lock().await;
                    store.store_source(
                        req.target_file_hash,
                        KadSourceWire {
                            kad_id: req.contact.kad_id,
                            ip: req.contact.ip,
                            udp_port: req.contact.udp_port,
                            tcp_port: req.contact.tcp_port,
                            encryption: 0,
                            buddy_id: None,
                        },
                    )
                };
                if stored {
                    persist_indexed_source(downloads, &req).await;
                    0
                } else {
                    1
                }
            } else {
                1
            };
            tracing::info!(
                "Kad UDP: received PublishSourceReq from {peer_addr} for file {} (status={})",
                hex::encode(req.target_file_hash),
                status
            );
            let res = KadCodec::encode_publish_source_res(&req.target_file_hash, status);
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::PublishSourceRes(pub_res) => {
            // Opcode 0x4B is the unified publish response for both source and keyword publish.
            // Match in-flight transactions expecting either PublishSourceRes or PublishKeywordRes.
            let mut inf = in_flight.lock().await;
            let mut matched_tx = None;
            for (id, tx) in inf.iter() {
                let ip_match = tx.target == peer_addr || tx.target.ip() == peer_addr.ip();
                let hash_match = tx
                    .expected_target_hash
                    .is_none_or(|h| h == pub_res.target_file_hash);
                let resp_match = tx.expected_response == KadExpectedResponse::PublishSourceRes
                    || tx.expected_response == KadExpectedResponse::PublishKeywordRes
                    || tx.expected_response == KadExpectedResponse::PublishNotesRes;
                if ip_match && hash_match && resp_match {
                    matched_tx = Some(*id);
                    break;
                }
            }
            tracing::info!(
                "Kad UDP: received PublishRes (0x4B) from {peer_addr} with status {} (matched_tx={matched_tx:?})",
                pub_res.status
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    if tx.expected_response == KadExpectedResponse::PublishNotesRes {
                        let _ =
                            tx.sender
                                .send(Ok(KadMessage::PublishNotesRes(KadPublishNotesRes {
                                    target_file_hash: pub_res.target_file_hash,
                                    load: pub_res.status,
                                })));
                    } else if tx.expected_response == KadExpectedResponse::PublishKeywordRes {
                        let _ = tx.sender.send(Ok(KadMessage::PublishKeywordRes(
                            KadPublishKeywordRes {
                                keyword_hash: pub_res.target_file_hash,
                                status: pub_res.status,
                            },
                        )));
                    } else {
                        let _ = tx.sender.send(Ok(KadMessage::PublishSourceRes(pub_res)));
                    }
                }
            }
        }
        KadMessage::SearchKeywordReq(req) => {
            let entries = {
                let mut store = storage.lock().await;
                store.get_keyword_entries(&req.keyword_hash, req.start_pos)
            };
            tracing::info!(
                "Kad UDP: received SearchKeywordReq from {peer_addr} for keyword {}, replying with {} stored entries",
                hex::encode(req.keyword_hash),
                entries.len()
            );
            let res = KadCodec::encode_search_keyword_res(&req.keyword_hash, &entries);
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::SearchKeywordRes(search_res) => {
            let mut inf = in_flight.lock().await;
            let mut matched_tx = None;
            for (id, tx) in inf.iter() {
                let ip_match = tx.target == peer_addr || tx.target.ip() == peer_addr.ip();
                let hash_match = tx
                    .expected_target_hash
                    .is_none_or(|h| h == search_res.keyword_hash);
                let resp_match = tx.expected_response == KadExpectedResponse::SearchKeywordRes;
                if ip_match && hash_match && resp_match {
                    matched_tx = Some(*id);
                    break;
                }
            }
            tracing::info!(
                "Kad UDP: received SearchKeywordRes from {peer_addr} with {} entries for keyword {} (matched_tx={matched_tx:?})",
                search_res.entries.len(),
                hex::encode(search_res.keyword_hash)
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    let _ = tx.sender.send(Ok(KadMessage::SearchKeywordRes(search_res)));
                }
            }
        }
        KadMessage::PublishKeywordReq(req) => {
            // Reject malformed entries: zero file hash or empty/oversized name.
            let valid_entries: Vec<_> = req
                .entries
                .into_iter()
                .filter(|e| {
                    e.file_hash != [0u8; 16] && !e.file_name.is_empty() && e.file_name.len() <= 255
                })
                .collect();
            let keyword_hash = req.keyword_hash;
            let to_persist = valid_entries.clone();
            let status = {
                let mut store = storage.lock().await;
                if store.store_keyword_entries(keyword_hash, valid_entries) {
                    0
                } else {
                    1
                }
            };
            if status == 0 {
                persist_keyword_entries(downloads, &keyword_hash, &to_persist).await;
            }
            tracing::info!(
                "Kad UDP: received PublishKeywordReq from {peer_addr} for keyword {} (status={})",
                hex::encode(req.keyword_hash),
                status
            );
            let res = KadCodec::encode_publish_keyword_res(&req.keyword_hash, status);
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::PublishKeywordRes(pub_res) => {
            let mut inf = in_flight.lock().await;
            let mut matched_tx = None;
            for (id, tx) in inf.iter() {
                if (tx.target == peer_addr || tx.target.ip() == peer_addr.ip())
                    && tx.expected_response == KadExpectedResponse::PublishKeywordRes
                {
                    matched_tx = Some(*id);
                    break;
                }
            }
            tracing::info!(
                "Kad UDP: received PublishKeywordRes from {peer_addr} with status {} (matched_tx={matched_tx:?})",
                pub_res.status
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    let _ = tx.sender.send(Ok(KadMessage::PublishKeywordRes(pub_res)));
                }
            }
        }
        KadMessage::FirewallUdp(fw) => {
            // eMule Process_KADEMLIA2_FIREWALLUDP → CUDPFirewallTester::
            // SetUDPFWCheckResult: only answer packets from candidate IPs we
            // actually asked are routed to a pending test.
            let delivered = {
                let mut fw_track = firewall.lock().await;
                fw_track.notify_packet(
                    peer_addr.ip(),
                    FirewallUdpResult {
                        error_code: fw.error_code,
                        port_tested: fw.udp_port,
                    },
                )
            };
            if delivered {
                tracing::info!(
                    "Kad UDP: received FIREWALLUDP from {peer_addr} (error={}, tested_port={})",
                    fw.error_code,
                    fw.udp_port
                );
            } else {
                tracing::warn!(
                    "Kad UDP: ignoring unsolicited FIREWALLUDP from {peer_addr} (tested_port={})",
                    fw.udp_port
                );
            }
        }
        // --- Buddy relay role (eMule CClientList::GetBuddy machinery) ---
        KadMessage::FindBuddyReq(req) => {
            // eMule Process_KADEMLIA_FINDBUDDY_REQ: only a verified,
            // non-firewalled node with no current buddy may accept.
            let eligible = {
                let fw = firewall.lock().await;
                fw.verified && fw.status == UdpFirewallStatus::Open
            };
            let mut reg = buddy.lock().await;
            if !eligible || buddy_tcp_port == 0 || reg.active.is_some() || req.tcp_port == 0 {
                tracing::debug!(
                    "Kad UDP: rejecting FINDBUDDY_REQ from {peer_addr} (eligible={eligible}, has_buddy={})",
                    reg.active.is_some()
                );
                return;
            }
            reg.register_pending(peer_addr.ip(), req.buddy_id, req.user_id, req.tcp_port);
            drop(reg);
            let res = KadCodec::encode_findbuddy_res(
                &req.buddy_id,
                local_kad_id,
                buddy_tcp_port,
                buddy_connect_options,
            );
            tracing::info!(
                "Kad UDP: accepted FINDBUDDY_REQ from {peer_addr}, awaiting buddy TCP link on port {} (buddy_id={})",
                buddy_tcp_port,
                hex::encode(req.buddy_id)
            );
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::FindBuddyRes(res) => {
            // We never send FINDBUDDY_REQ (the LowID-self buddy client is
            // not implemented); log unsolicited responses.
            tracing::debug!(
                "Kad UDP: ignoring unsolicited FINDBUDDY_RES from {peer_addr} (tcp_port={})",
                res.tcp_port
            );
        }
        KadMessage::CallbackReq(req) => {
            // eMule Process_KADEMLIA_CALLBACK_REQ: forward to our buddy's
            // TCP link as OP_CALLBACK
            // [check:16][file:16][requester_ip:4][requester_tcp:2].
            let relayed = {
                let reg = buddy.lock().await;
                if reg.active.is_some() {
                    let mut payload = Vec::with_capacity(38);
                    payload.extend_from_slice(&req.buddy_id);
                    payload.extend_from_slice(&req.file_id);
                    match peer_addr.ip() {
                        IpAddr::V4(v4) => payload.extend_from_slice(&v4.octets()),
                        IpAddr::V6(v6) => payload.extend_from_slice(
                            &v6.to_ipv4_mapped()
                                .map(|v4| v4.octets())
                                .unwrap_or([0, 0, 0, 0]),
                        ),
                    }
                    payload.extend_from_slice(&req.tcp_port.to_le_bytes());
                    let frame = frame_emule_packet(OP_CALLBACK, &payload);
                    reg.send_to_buddy(frame)
                } else {
                    false
                }
            };
            if relayed {
                tracing::info!(
                    "Kad UDP: relayed CALLBACK_REQ from {peer_addr} to buddy link (file={}, cb_port={})",
                    hex::encode(req.file_id),
                    req.tcp_port
                );
            } else {
                tracing::debug!(
                    "Kad UDP: dropping CALLBACK_REQ from {peer_addr} — no active buddy link"
                );
            }
        }
        // --- Kad notes (file comments / ratings) ---
        KadMessage::SearchNotesReq(req) => {
            let notes = {
                let mut store = storage.lock().await;
                store.get_notes(
                    &req.target_file_hash,
                    req.file_size,
                    MAX_NOTE_SEARCH_RESULTS,
                )
            };
            if notes.is_empty() {
                tracing::debug!(
                    "Kad UDP: SearchNotesReq from {peer_addr} for file {} — no stored notes",
                    hex::encode(req.target_file_hash)
                );
                return;
            }
            tracing::info!(
                "Kad UDP: received SearchNotesReq from {peer_addr} for file {}, replying with {} notes",
                hex::encode(req.target_file_hash),
                notes.len()
            );
            if let Some(res) =
                KadCodec::encode_search_notes_res(local_kad_id, &req.target_file_hash, &notes)
            {
                let _ = socket.send_to(&res, peer_addr).await;
            }
        }
        KadMessage::SearchNotesRes(res) => {
            let mut inf = in_flight.lock().await;
            let mut matched_tx = None;
            for (id, tx) in inf.iter() {
                let ip_match = tx.target == peer_addr || tx.target.ip() == peer_addr.ip();
                let hash_match = tx
                    .expected_target_hash
                    .is_none_or(|h| h == res.target_file_hash);
                let resp_match = tx.expected_response == KadExpectedResponse::SearchNotesRes;
                if ip_match && hash_match && resp_match {
                    matched_tx = Some(*id);
                    break;
                }
            }
            tracing::info!(
                "Kad UDP: received SearchNotesRes from {peer_addr} with {} notes for file {} (matched_tx={matched_tx:?})",
                res.notes.len(),
                hex::encode(res.target_file_hash)
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    let _ = tx.sender.send(Ok(KadMessage::SearchNotesRes(res)));
                }
            }
        }
        KadMessage::PublishNotesReq(req) => {
            // eMule CKademliaUDPListener::Process_KADEMLIA2_PUBLISH_NOTES_REQ:
            // a UDP-firewalled node refuses indexing work.
            let firewalled = {
                let fw = firewall.lock().await;
                fw.verified && fw.status == UdpFirewallStatus::Firewalled
            };
            if firewalled {
                tracing::debug!(
                    "Kad UDP: rejecting PublishNotesReq from {peer_addr} — we are UDP-firewalled"
                );
                return;
            }
            let publisher_ip = match peer_addr.ip() {
                IpAddr::V4(v4) => v4.octets(),
                IpAddr::V6(_) => [127, 0, 0, 1],
            };
            let note = KadNoteWire {
                source_id: req.source_id,
                tags: req.tags,
                unknown_tags: req.unknown_tags,
            };
            let accepted = {
                let mut store = storage.lock().await;
                store.store_note(
                    req.target_file_hash,
                    note.clone(),
                    publisher_ip,
                    peer_addr.port(),
                )
            };
            if accepted {
                persist_indexed_note(
                    downloads,
                    &req.target_file_hash,
                    &note,
                    publisher_ip,
                    peer_addr.port(),
                )
                .await;
            }
            let load = {
                let store = storage.lock().await;
                store.note_load(&req.target_file_hash)
            };
            tracing::info!(
                "Kad UDP: received PublishNotesReq from {peer_addr} for file {} (accepted={}, load={})",
                hex::encode(req.target_file_hash),
                accepted,
                load
            );
            let res = KadCodec::encode_publish_notes_res(&req.target_file_hash, load);
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::PublishNotesRes(_) => {
            // Shares opcode 0x4B with PublishSourceRes/PublishKeywordRes; the
            // unified PublishSourceRes arm above already routes it by the
            // in-flight expected_response. Unreachable in practice.
        }
    }
}

/// Handles a `KADEMLIA_FIREWALLED_REQ` (0x50) / `KADEMLIA_FIREWALLED2_REQ`
/// (0x53): eMule `Process_KADEMLIA_FIREWALLED{2}_REQ`. We immediately echo the
/// sender's observed IPv4 back in `KADEMLIA_FIREWALLED_RES`, then TCP-connect
/// to their advertised port. On success we acknowledge via
/// `OP_KAD_FWTCPCHECK_ACK` on the TCP stream (Kad >= 7) or a UDP
/// `KADEMLIA_FIREWALLED_ACK_RES` null packet (legacy).
async fn handle_firewalled_check(
    socket: &Arc<UdpSocket>,
    peer_addr: SocketAddr,
    tcp_port: u16,
    kad7_plus: bool,
) {
    let IpAddr::V4(v4) = peer_addr.ip() else {
        tracing::debug!("Kad UDP: ignoring firewalled check from non-IPv4 {peer_addr}");
        return;
    };
    if tcp_port == 0 {
        tracing::debug!("Kad UDP: ignoring firewalled check from {peer_addr} with tcp_port=0");
        return;
    }

    // FIREWALLED_RES echoes the requester's observed IP (eMule sends it once
    // RequestTCP is accepted — before the probe outcome is known).
    let res = KadCodec::encode_firewalled_res(u32::from_le_bytes(v4.octets()));
    let _ = socket.send_to(&res, peer_addr).await;

    let socket2 = Arc::clone(socket);
    tokio::spawn(async move {
        let target = SocketAddr::new(IpAddr::V4(v4), tcp_port);
        let probe = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::net::TcpStream::connect(target),
        )
        .await;
        match probe {
            Ok(Ok(mut stream)) => {
                if kad7_plus {
                    // Kad >= 7: OP_KAD_FWTCPCHECK_ACK (PR_EMULE, empty payload)
                    // on the probe connection itself.
                    let frame = frame_emule_packet(mule_protocol::ed2k::OP_KAD_FWTCPCHECK_ACK, &[]);
                    use tokio::io::AsyncWriteExt;
                    match stream.write_all(&frame).await {
                        Ok(()) => tracing::info!(
                            "Kad UDP: TCP firewall probe to {target} succeeded, sent OP_KAD_FWTCPCHECK_ACK"
                        ),
                        Err(e) => tracing::debug!(
                            "Kad UDP: probe to {target} connected but ack send failed: {e}"
                        ),
                    }
                } else {
                    // Legacy (<7): empty UDP KADEMLIA_FIREWALLED_ACK_RES.
                    let ack = [PR_KADEMLIA, KADEMLIA_FIREWALLED_ACK_RES];
                    let _ = socket2.send_to(&ack, peer_addr).await;
                    tracing::info!(
                        "Kad UDP: TCP firewall probe to {target} succeeded, sent FIREWALLED_ACK_RES"
                    );
                }
            }
            Ok(Err(e)) => {
                tracing::info!("Kad UDP: TCP firewall probe to {target} refused: {e}");
            }
            Err(_) => {
                tracing::info!("Kad UDP: TCP firewall probe to {target} timed out");
            }
        }
    });
}

/// Frames an eD2K/eMule TCP packet: `[proto][len:u32][opcode][payload]`
/// (opcode included in the length, like `Packet`/`PacketCodec`).
fn frame_emule_packet(opcode: u8, payload: &[u8]) -> Vec<u8> {
    use mule_protocol::ed2k::PR_EMULE;
    let mut out = Vec::with_capacity(6 + payload.len());
    out.push(PR_EMULE);
    out.extend_from_slice(&(payload.len() as u32 + 1).to_le_bytes());
    out.push(opcode);
    out.extend_from_slice(payload);
    out
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Mirrors an accepted `PublishSourceReq` into `kad_sources_index` so the
/// storage-node role survives daemon restarts (TTL = `KAD_STORE_TTL`).
async fn persist_indexed_source(downloads: &DownloadsSlot, req: &KadPublishSourceReq) {
    let Some(dl) = downloads.read().await.as_ref().cloned() else {
        return;
    };
    let now_ms = now_unix_ms();
    let row = mule_core::kad::KadIndexedSource {
        file_hash: req.target_file_hash,
        source_kad_id: mule_core::kad::KadId(req.contact.kad_id),
        ip: std::net::Ipv4Addr::from(req.contact.ip),
        udp_port: req.contact.udp_port,
        tcp_port: req.contact.tcp_port,
        file_size: req.file_size,
        encryption: 0,
        expires_at_unix_ms: now_ms + KAD_STORE_TTL.as_millis() as u64,
        updated_at_unix_ms: now_ms,
    };
    let mut dl_guard = dl.lock().await;
    if let Err(e) = dl_guard.db_mut().insert_kad_indexed_sources(&[row]) {
        tracing::warn!("Kad storage: failed to persist indexed source: {e}");
    }
}

/// Mirrors an accepted `PublishNotesReq` into `kad_notes_index` so the
/// storage-node role survives daemon restarts. The tag list is persisted
/// as its wire-encoded blob (`encode_note_tags_blob`) — arbitrary/unknown
/// tags round-trip exactly.
async fn persist_indexed_note(
    downloads: &DownloadsSlot,
    target_hash: &KadIdBytes,
    note: &KadNoteWire,
    publisher_ip: [u8; 4],
    publisher_udp_port: u16,
) {
    let Some(dl) = downloads.read().await.as_ref().cloned() else {
        return;
    };
    let Some(blob) =
        mule_protocol::kad::encoder::encode_note_tags_blob(&note.tags, &note.unknown_tags)
    else {
        return;
    };
    let now_ms = now_unix_ms();
    let row = mule_core::kad::KadIndexedNote {
        file_hash: hex::encode(target_hash),
        source_id: hex::encode(note.source_id),
        publisher_ip: std::net::Ipv4Addr::from(publisher_ip).to_string(),
        publisher_udp_port,
        tags_blob: blob,
        discovered_at_unix_ms: now_ms as i64,
        expires_at_unix_ms: (now_ms + KAD_STORE_TTL.as_millis() as u64) as i64,
    };
    let mut dl_guard = dl.lock().await;
    if let Err(e) = dl_guard.db_mut().insert_kad_notes(&[row]) {
        tracing::warn!("Kad storage: failed to persist indexed note: {e}");
    }
}

/// Mirrors accepted keyword entries into `kad_keywords_index`.
async fn persist_keyword_entries(
    downloads: &DownloadsSlot,
    keyword_hash: &[u8; 16],
    entries: &[KadKeywordEntryWire],
) {
    if entries.is_empty() {
        return;
    }
    let Some(dl) = downloads.read().await.as_ref().cloned() else {
        return;
    };
    let now_ms = now_unix_ms();
    let kw_hex = hex::encode(keyword_hash);
    let rows: Vec<mule_core::kad::KadKeywordEntry> = entries
        .iter()
        .map(|e| mule_core::kad::KadKeywordEntry {
            keyword_hash: kw_hex.clone(),
            file_hash: hex::encode(e.file_hash),
            file_name: e.file_name.clone(),
            file_size: e.file_size,
            source_count: e.source_count.max(1),
            discovered_at_unix_ms: now_ms as i64,
            expires_at_unix_ms: (now_ms + KAD_STORE_TTL.as_millis() as u64) as i64,
        })
        .collect();
    let mut dl_guard = dl.lock().await;
    if let Err(e) = dl_guard.db_mut().insert_kad_keyword_entries(&rows) {
        tracing::warn!("Kad storage: failed to persist keyword entries: {e}");
    }
}
