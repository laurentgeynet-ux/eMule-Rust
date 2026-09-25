// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use crate::services::peer_session::log_peer_addr;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::UdpSocket;

use mule_core::kad::{KBucketTable, KadContact, KadId};
use mule_protocol::ed2k::OP_CALLBACK;
use mule_protocol::kad::*;

use super::storage::{KadStorage, KAD_KEYWORD_TTL, KAD_NOTE_TTL, KAD_SOURCE_TTL};
use super::types::{
    BuddyRegistry, FirewallTestTracker, FirewallUdpResult, InFlightMap, KadExpectedResponse,
    UdpFirewallStatus,
};
use crate::services::downloads::DownloadService;
use crate::services::kad::firewall::OUR_KAD_VERSION;

/// eMule `KADEMLIA_VERSION7_49a` (`Opcodes.h:29`) — contacts strictly above
/// `KADEMLIA_VERSION6_49aBETA` (6) get the extended `FIREWALLED2_REQ` and
/// answer with `OP_KAD_FWTCPCHECK_ACK` over TCP.
const KADEMLIA_VERSION7_49A: u8 = 7;

type DownloadsSlot = Arc<tokio::sync::RwLock<Option<Arc<tokio::sync::Mutex<DownloadService>>>>>;

/// Internal dispatcher for incoming messages.
#[allow(clippy::too_many_arguments)]
pub async fn handle_incoming_message(
    socket: &Arc<UdpSocket>,
    peer_addr: SocketAddr,
    msg: KadMessage,
    in_flight: &InFlightMap,
    local_kad_id: &[u8; 16],
    // Our eD2K user hash — `clientHash` field of `KADEMLIA_FIREWALLED2_REQ`
    // (eMule `CPrefs::GetClientHash()` = the user hash, not the Kad ID).
    our_user_hash: &[u8; 16],
    local_port: u16,
    storage: &Arc<tokio::sync::Mutex<KadStorage>>,
    downloads: &DownloadsSlot,
    firewall: &Arc<tokio::sync::Mutex<FirewallTestTracker>>,
    buddy: &Arc<tokio::sync::Mutex<BuddyRegistry>>,
    // Local eD2K TCP port — also advertised in `BOOTSTRAP_RES`/`HELLO_RES`
    // headers (eMule `thePrefs.GetPort()`).
    buddy_tcp_port: u16,
    buddy_connect_options: u8,
    // Shared routing table (`None` until `KadService` injects it).
    routing_table: Option<Arc<tokio::sync::Mutex<KBucketTable>>>,
) {
    match msg {
        KadMessage::Ping(_ping) => {
            tracing::debug!(
                "Kad UDP: received Ping from {peer}, replying with Pong",
                peer = log_peer_addr(peer_addr)
            );
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
            tracing::debug!(
                "Kad UDP: received Pong from {peer} (matched_tx={matched_tx:?})",
                peer = log_peer_addr(peer_addr)
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    let _ = tx.sender.send(Ok(KadMessage::Pong(pong)));
                }
            }
        }
        KadMessage::HelloReq(hello) => {
            tracing::debug!(
                "Kad UDP: received HelloReq from {peer} (kad_v{}, tcp={}, misc={:?}), replying with HelloRes",
                hello.version,
                hello.tcp_port,
                hello.misc_options
            , peer = log_peer_addr(peer_addr));
            // eMule AddContact_KADEMLIA2: every HELLO_REQ feeds the routing
            // table. The contact stays unverified until it answers one of
            // our requests or completes the HELLO_RES_ACK handshake.
            if let IpAddr::V4(ip) = peer_addr.ip() {
                learn_contact(
                    &routing_table,
                    hello.kad_id,
                    ip,
                    if hello.udp_port != 0 {
                        hello.udp_port
                    } else {
                        peer_addr.port()
                    },
                    if hello.tcp_port != 0 {
                        Some(hello.tcp_port)
                    } else {
                        None
                    },
                    hello.version,
                    false,
                )
                .await;
            }
            // eMule KADMISCOPTIONS (KademliaUDPListener.cpp:131-137):
            // bit0 = UDP firewalled, bit1 = TCP firewalled (CPrefs
            // GetFirewalled), bit2 = ask the receiver to complete the
            // handshake with HELLO_RES_ACK.
            let mut misc = 0x04u8;
            {
                let fw = firewall.lock().await;
                if fw.verified && fw.status == UdpFirewallStatus::Firewalled {
                    misc |= 0x01;
                }
                if fw.tcp_is_firewalled() {
                    misc |= 0x02;
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
            // eMule tail of Process_KADEMLIA2_HELLO_REQ — a recheck round in
            // progress turns each inbound HELLO into a TCP probe request.
            maybe_tcp_firewall_check(
                socket,
                peer_addr,
                hello.version,
                firewall,
                buddy_tcp_port,
                buddy_connect_options,
                our_user_hash,
            )
            .await;
        }
        KadMessage::HelloRes(hello_res) => {
            // Kad >= 8: the responder may ask us to prove our IP with a
            // HELLO_RES_ACK (KADMISCOPTIONS bit 2).
            let wants_ack = hello_res.requests_ack();
            // Captured before `hello_res` is moved into the transaction
            // sender below — needed by the TCP recheck hook.
            let peer_kad_version = hello_res.version;
            let peer_udp_port = if hello_res.udp_port != 0 {
                hello_res.udp_port
            } else {
                peer_addr.port()
            };
            // in_flight must be released before learn_contact awaits
            // routing_table — holding it creates an in_flight → routing_table
            // ordering that deadlocks against get_status (routing_table →
            // get_snapshot → in_flight).
            let (matched_id, matched_sender) = {
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
                (matched_tx, matched_tx.and_then(|id| inf.remove(&id)))
            };
            tracing::debug!(
                "Kad UDP: received HelloRes from {peer} (kad_v{}, tcp={}, udp_tag={}, matched_tx={matched_id:?})",
                hello_res.version,
                hello_res.tcp_port,
                peer_udp_port
            , peer = log_peer_addr(peer_addr));
            if let Some(tx) = matched_sender {
                // A solicited HELLO_RES proves the remote owns its advertised
                // IP:port — the contact is verified (eMule IsIpVerified).
                if let IpAddr::V4(ip) = peer_addr.ip() {
                    learn_contact(
                        &routing_table,
                        hello_res.kad_id,
                        ip,
                        peer_udp_port,
                        if hello_res.tcp_port != 0 {
                            Some(hello_res.tcp_port)
                        } else {
                            None
                        },
                        hello_res.version,
                        true,
                    )
                    .await;
                }
                let _ = tx.sender.send(Ok(KadMessage::HelloRes(hello_res)));
            }
            if wants_ack {
                let ack = KadCodec::encode_hello_res_ack(local_kad_id);
                let _ = socket.send_to(&ack, peer_addr).await;
            }
            // eMule tail of Process_KADEMLIA2_HELLO_RES — same recheck hook.
            maybe_tcp_firewall_check(
                socket,
                peer_addr,
                peer_kad_version,
                firewall,
                buddy_tcp_port,
                buddy_connect_options,
                our_user_hash,
            )
            .await;
        }
        KadMessage::HelloResAck(ack) => {
            // Completes the three-way hello (eMule
            // Process_KADEMLIA2_HELLO_RES_ACK): the peer acked our HELLO_RES,
            // proving it owns its advertised IP — promote the stored contact
            // to verified, but only if the packet source matches the contact's
            // IP (an ACK for someone else's KadID must not verify them).
            if let (Some(table), IpAddr::V4(ip)) = (&routing_table, peer_addr.ip()) {
                let mut t = table.lock().await;
                let id = KadId(ack.kad_id);
                if t.get(&id).map(|c| c.ip == ip).unwrap_or(false) {
                    t.mark_seen(&id, now_unix_ms());
                }
            }
            tracing::debug!(
                "Kad UDP: received HelloResAck from {peer} (kad_id={})",
                hex::encode(ack.kad_id),
                peer = log_peer_addr(peer_addr)
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
            // seen by the responder. eMule `Process_KADEMLIA_FIREWALLED_RES`
            // drops unrequested answers (`IsKadFirewallCheckIP`), records the
            // observed IP (`SetIPAddress`) and ticks the round (`IncRecheckIP`).
            let ip = std::net::Ipv4Addr::from(res.ip.to_le_bytes());
            let mut fw = firewall.lock().await;
            if !fw.is_tcp_check_ip(peer_addr.ip()) {
                tracing::debug!(
                    "Kad UDP: unrequested FirewalledRes from {peer} — ignored",
                    peer = log_peer_addr(peer_addr)
                );
            } else {
                fw.note_firewalled_res(ip);
                tracing::debug!(
                    "Kad UDP: received FirewalledRes from {peer} (observed ip={ip}, round {} done)",
                    fw.tcp_answers,
                    peer = log_peer_addr(peer_addr)
                );
            }
        }
        KadMessage::FirewalledAckRes => {
            // Legacy (Kad < 7) connect-back verdict over UDP — eMule
            // `Process_KADEMLIA_FIREWALLED_ACK_RES` → `IncFirewalled`.
            firewall.lock().await.inc_tcp_acks();
            tracing::debug!(
                "Kad UDP: received FirewalledAckRes from {peer} (TCP port reachable)",
                peer = log_peer_addr(peer_addr)
            );
        }
        KadMessage::BootstrapReq(req) => {
            // eMule Process_KADEMLIA2_BOOTSTRAP_REQ: answer with our KadID +
            // eD2K TCP port + Kad version and up to 20 verified contacts
            // spread across the keyspace (GetBootstrapContacts).
            let contacts: Vec<KadContactWire> = match &routing_table {
                Some(table) => table
                    .lock()
                    .await
                    .spread_verified(20)
                    .iter()
                    .map(contact_to_wire)
                    .collect(),
                None => Vec::new(),
            };
            tracing::debug!(
                "Kad UDP: received BootstrapReq from {peer} (requester_id={}), replying with {} contacts",
                hex::encode(req.kad_id),
                contacts.len()
            , peer = log_peer_addr(peer_addr));
            let res = KadCodec::encode_bootstrap_res(
                local_kad_id,
                buddy_tcp_port,
                OUR_KAD_VERSION,
                &contacts,
            );
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::Req(req) => {
            // eMule Process_KADEMLIA2_REQ: [type][target][uCheck] — uCheck is
            // the KadID the requester believes we own; a mismatch means a
            // stale contact entry and eMule drops the request silently.
            let count = (req.req_type & 0x1F) as usize;
            if count == 0 || req.sender_id != *local_kad_id {
                tracing::debug!(
                    "Kad UDP: dropping Req from {peer} (type=0x{:02X}, id check: {})",
                    req.req_type,
                    if req.sender_id == *local_kad_id {
                        "ok"
                    } else {
                        "stale kad_id"
                    },
                    peer = log_peer_addr(peer_addr)
                );
                return;
            }
            // Only IP-verified contacts may be returned (eMule GetClosestTo
            // type 2 → bOnlyVerified) — propagating unverified contacts would
            // relay potentially spoofed entries.
            let contacts: Vec<KadContactWire> = match &routing_table {
                Some(table) => table
                    .lock()
                    .await
                    .find_verified_closest(&KadId(req.target_id), count.min(32))
                    .iter()
                    .map(contact_to_wire)
                    .collect(),
                None => Vec::new(),
            };
            tracing::debug!(
                "Kad UDP: received Req (0x21) from {peer} for target {}, replying with {} verified contacts",
                hex::encode(req.target_id),
                contacts.len()
            , peer = log_peer_addr(peer_addr));
            let res = KadCodec::encode_find_node_res(&req.target_id, &contacts);
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::BootstrapRes(boot_res) => {
            // Release in_flight before learn_contact awaits routing_table —
            // same in_flight → routing_table ordering hazard as HelloRes.
            let (matched_id, matched_sender) = {
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
                (matched_tx, matched_tx.and_then(|id| inf.remove(&id)))
            };
            tracing::debug!(
                "Kad UDP: received BootstrapRes/FindNodeRes from {peer} with {} contacts (matched_tx={matched_id:?})",
                boot_res.contacts.len()
            , peer = log_peer_addr(peer_addr));
            if matched_id.is_some() {
                // Solicited response → the responder is verified. Its header
                // port field is its TCP port (eMule BOOTSTRAP_RES layout).
                if let IpAddr::V4(ip) = peer_addr.ip() {
                    learn_contact(
                        &routing_table,
                        boot_res.kad_id,
                        ip,
                        peer_addr.port(),
                        if boot_res.udp_port != 0 {
                            Some(boot_res.udp_port)
                        } else {
                            None
                        },
                        boot_res.version,
                        true,
                    )
                    .await;
                }
            }
            if let Some(tx) = matched_sender {
                let _ = tx.sender.send(Ok(KadMessage::BootstrapRes(boot_res)));
            }
        }
        KadMessage::SearchSourceReq(req) => {
            let sources = {
                let mut store = storage.lock().await;
                store.get_sources(&req.target_file_hash, req.start_pos)
            };
            // eMule `SendValidSourceResult`: results are split into as many
            // datagrams as needed to stay under `UDP_KAD_MAXFRAGMENT`.
            let packets =
                KadCodec::encode_search_source_res_fragmented(&req.target_file_hash, &sources);
            tracing::debug!(
                "Kad UDP: received SearchSourceReq from {peer} for file {}, replying with {} stored sources in {} packet(s)",
                hex::encode(req.target_file_hash),
                sources.len(),
                packets.len()
            , peer = log_peer_addr(peer_addr));
            for res in packets {
                let _ = socket.send_to(&res, peer_addr).await;
            }
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
            tracing::debug!(
                "Kad UDP: received SearchSourceRes from {peer} with {} sources for file {} (matched_tx={matched_tx:?})",
                search_res.sources.len(),
                hex::encode(search_res.target_file_hash)
            , peer = log_peer_addr(peer_addr));
            if let Some(id) = matched_tx {
                // Multi-packet response: keep the transaction registered so
                // the next batch from this peer still routes to the sender;
                // drop it only when the collector is gone.
                if let Some(tx) = inf.get(&id) {
                    if tx
                        .sender
                        .send(Ok(KadMessage::SearchSourceRes(search_res)))
                        .is_err()
                    {
                        inf.remove(&id);
                    }
                }
            }
        }
        KadMessage::PublishSourceReq(req) => {
            // eMule ProcessPublishSourceRequest: out-of-tolerance targets are
            // dropped silently — we are not an indexer for this key.
            if !within_publish_tolerance(local_kad_id, &req.target_file_hash) {
                tracing::debug!(
                    "Kad UDP: dropping out-of-tolerance PublishSourceReq from {peer} for file {}",
                    hex::encode(req.target_file_hash),
                    peer = log_peer_addr(peer_addr)
                );
                return;
            }
            let peer_ip_bytes = match peer_addr.ip() {
                std::net::IpAddr::V4(v4) => v4.octets(),
                std::net::IpAddr::V6(_) => [127, 0, 0, 1],
            };
            // eMule CKademliaUDPListener::ProcessPublishSourceRequest: the
            // advertised contact must match the sender's IP and carry a TCP port.
            let valid_contact = req.contact.ip == peer_ip_bytes && req.contact.tcp_port > 0;
            if valid_contact {
                // The publisher's self-announced contact is IP-bound and
                // validated — learn it (unverified) like eMule's AddContact.
                learn_contact(
                    &routing_table,
                    req.contact.kad_id,
                    Ipv4Addr::from(req.contact.ip),
                    req.contact.udp_port,
                    Some(req.contact.tcp_port),
                    req.contact.version,
                    false,
                )
                .await;
            }
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
            tracing::debug!(
                "Kad UDP: received PublishSourceReq from {peer} for file {} (status={})",
                hex::encode(req.target_file_hash),
                status,
                peer = log_peer_addr(peer_addr)
            );
            let res = KadCodec::encode_publish_source_res(&req.target_file_hash, status);
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::PublishSourceRes(pub_res) => {
            // eMule `Process_KADEMLIA2_PUBLISH_RES` ("for future use"): a
            // trailing options byte with bit 0 requests a bare
            // `KADEMLIA2_PUBLISH_RES_ACK` (0x4C) datagram back.
            if pub_res.request_ack {
                let ack = [PR_KADEMLIA, KADEMLIA2_PUBLISH_RES_ACK];
                let _ = socket.send_to(&ack, peer_addr).await;
            }
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
            tracing::debug!(
                "Kad UDP: received PublishRes (0x4B) from {peer} with status {} (matched_tx={matched_tx:?})",
                pub_res.status
            , peer = log_peer_addr(peer_addr));
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
            // eMule `Process_KADEMLIA2_SEARCH_KEY_REQ`: when `restrictive`
            // is set the trailing bytes form a search-expression tree
            // (`CreateSearchExpressionTree`). A malformed tree throws and the
            // whole datagram is dropped.
            let expr = if req.restrictive {
                match req.search_expression.as_deref() {
                    Some(bytes) => {
                        match KadSearchExpression::parse(
                            PR_KADEMLIA,
                            KADEMLIA2_SEARCH_KEY_REQ,
                            bytes,
                        ) {
                            Ok(e) => Some(e),
                            Err(err) => {
                                tracing::debug!(
                                    "Kad UDP: dropping SearchKeywordReq from {peer}: \
                                     malformed restrictive expression ({err})",
                                    peer = log_peer_addr(peer_addr)
                                );
                                return;
                            }
                        }
                    }
                    // Restrictive flag without an expression: eMule serves
                    // unfiltered results (pSearchTerms == NULL).
                    None => None,
                }
            } else {
                None
            };
            let entries = {
                let mut store = storage.lock().await;
                store.get_keyword_entries_filtered(&req.keyword_hash, req.start_pos, |entry| {
                    expr.as_ref().is_none_or(|e| {
                        e.matches(&KadKeywordSearchView::new(entry))
                    })
                })
            };
            let packets =
                KadCodec::encode_search_keyword_res_fragmented(&req.keyword_hash, &entries);
            tracing::debug!(
                "Kad UDP: received SearchKeywordReq from {peer} for keyword {} (restrictive={}), replying with {} stored entries in {} packet(s)",
                hex::encode(req.keyword_hash),
                req.restrictive,
                entries.len(),
                packets.len()
            , peer = log_peer_addr(peer_addr));
            for res in packets {
                let _ = socket.send_to(&res, peer_addr).await;
            }
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
            tracing::debug!(
                "Kad UDP: received SearchKeywordRes from {peer} with {} entries for keyword {} (matched_tx={matched_tx:?})",
                search_res.entries.len(),
                hex::encode(search_res.keyword_hash)
            , peer = log_peer_addr(peer_addr));
            if let Some(id) = matched_tx {
                // Multi-packet response: keep the transaction registered so
                // the next batch from this peer still routes to the sender.
                if let Some(tx) = inf.get(&id) {
                    if tx
                        .sender
                        .send(Ok(KadMessage::SearchKeywordRes(search_res)))
                        .is_err()
                    {
                        inf.remove(&id);
                    }
                }
            }
        }
        KadMessage::PublishKeywordReq(req) => {
            // eMule ProcessPublishKeywordRequest: out-of-tolerance keyword
            // hashes are dropped silently.
            if !within_publish_tolerance(local_kad_id, &req.keyword_hash) {
                tracing::debug!(
                    "Kad UDP: dropping out-of-tolerance PublishKeywordReq from {peer} for keyword {}",
                    hex::encode(req.keyword_hash)
                , peer = log_peer_addr(peer_addr));
                return;
            }
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
            let publisher_ip = match peer_addr.ip() {
                IpAddr::V4(v4) => v4.octets(),
                IpAddr::V6(_) => [0u8; 4],
            };
            let status = {
                let mut store = storage.lock().await;
                if store.store_keyword_entries(keyword_hash, valid_entries, publisher_ip) {
                    0
                } else {
                    1
                }
            };
            if status == 0 {
                persist_keyword_entries(downloads, &keyword_hash, &to_persist).await;
            }
            tracing::debug!(
                "Kad UDP: received PublishKeywordReq from {peer} for keyword {} (status={})",
                hex::encode(req.keyword_hash),
                status,
                peer = log_peer_addr(peer_addr)
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
            tracing::debug!(
                "Kad UDP: received PublishKeywordRes from {peer} with status {} (matched_tx={matched_tx:?})",
                pub_res.status
            , peer = log_peer_addr(peer_addr));
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
                tracing::debug!(
                    "Kad UDP: received FIREWALLUDP from {peer} (error={}, tested_port={})",
                    fw.error_code,
                    fw.udp_port,
                    peer = log_peer_addr(peer_addr)
                );
            } else {
                tracing::warn!(
                    "Kad UDP: ignoring unsolicited FIREWALLUDP from {peer} (tested_port={})",
                    fw.udp_port,
                    peer = log_peer_addr(peer_addr)
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
                    "Kad UDP: rejecting FINDBUDDY_REQ from {peer} (eligible={eligible}, has_buddy={})",
                    reg.active.is_some()
                , peer = log_peer_addr(peer_addr));
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
                "Kad UDP: accepted FINDBUDDY_REQ from {peer}, awaiting buddy TCP link on port {} (buddy_id={})",
                buddy_tcp_port,
                hex::encode(req.buddy_id)
            , peer = log_peer_addr(peer_addr));
            let _ = socket.send_to(&res, peer_addr).await;
        }
        KadMessage::FindBuddyRes(res) => {
            // We never send FINDBUDDY_REQ (the LowID-self buddy client is
            // not implemented); log unsolicited responses.
            tracing::debug!(
                "Kad UDP: ignoring unsolicited FINDBUDDY_RES from {peer} (tcp_port={})",
                res.tcp_port,
                peer = log_peer_addr(peer_addr)
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
                    "Kad UDP: relayed CALLBACK_REQ from {peer} to buddy link (file={}, cb_port={})",
                    hex::encode(req.file_id),
                    req.tcp_port,
                    peer = log_peer_addr(peer_addr)
                );
            } else {
                tracing::debug!(
                    "Kad UDP: dropping CALLBACK_REQ from {peer} — no active buddy link",
                    peer = log_peer_addr(peer_addr)
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
                    "Kad UDP: SearchNotesReq from {peer} for file {} — no stored notes",
                    hex::encode(req.target_file_hash),
                    peer = log_peer_addr(peer_addr)
                );
                return;
            }
            tracing::debug!(
                "Kad UDP: received SearchNotesReq from {peer} for file {}, replying with {} notes",
                hex::encode(req.target_file_hash),
                notes.len(),
                peer = log_peer_addr(peer_addr)
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
            tracing::debug!(
                "Kad UDP: received SearchNotesRes from {peer} with {} notes for file {} (matched_tx={matched_tx:?})",
                res.notes.len(),
                hex::encode(res.target_file_hash)
            , peer = log_peer_addr(peer_addr));
            if let Some(id) = matched_tx {
                // Multi-packet response: keep the transaction registered so
                // the next batch from this peer still routes to the sender.
                if let Some(tx) = inf.get(&id) {
                    if tx.sender.send(Ok(KadMessage::SearchNotesRes(res))).is_err() {
                        inf.remove(&id);
                    }
                }
            }
        }
        KadMessage::PublishNotesReq(req) => {
            // eMule Process_KADEMLIA2_PUBLISH_NOTES_REQ: out-of-tolerance
            // targets are dropped silently before any index work.
            if !within_publish_tolerance(local_kad_id, &req.target_file_hash) {
                tracing::debug!(
                    "Kad UDP: dropping out-of-tolerance PublishNotesReq from {peer} for file {}",
                    hex::encode(req.target_file_hash),
                    peer = log_peer_addr(peer_addr)
                );
                return;
            }
            // eMule CKademliaUDPListener::Process_KADEMLIA2_PUBLISH_NOTES_REQ:
            // a UDP-firewalled node refuses indexing work.
            let firewalled = {
                let fw = firewall.lock().await;
                fw.verified && fw.status == UdpFirewallStatus::Firewalled
            };
            if firewalled {
                tracing::debug!(
                    "Kad UDP: rejecting PublishNotesReq from {peer} — we are UDP-firewalled",
                    peer = log_peer_addr(peer_addr)
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
            tracing::debug!(
                "Kad UDP: received PublishNotesReq from {peer} for file {} (accepted={}, load={})",
                hex::encode(req.target_file_hash),
                accepted,
                load,
                peer = log_peer_addr(peer_addr)
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
        tracing::debug!(
            "Kad UDP: ignoring firewalled check from non-IPv4 {peer}",
            peer = log_peer_addr(peer_addr)
        );
        return;
    };
    if tcp_port == 0 {
        tracing::debug!(
            "Kad UDP: ignoring firewalled check from {peer} with tcp_port=0",
            peer = log_peer_addr(peer_addr)
        );
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

/// Converts a routing-table contact into its wire form for `KADEMLIA2_RES` /
/// `BOOTSTRAP_RES` answers.
fn contact_to_wire(c: &KadContact) -> KadContactWire {
    KadContactWire {
        kad_id: c.kad_id.0,
        ip: c.ip.octets(),
        udp_port: c.udp_port,
        tcp_port: c.tcp_port.unwrap_or(0),
        version: c.version,
    }
}

/// Records a contact learned from an inbound packet into the routing table
/// (eMule `AddContact_KADEMLIA2` / `CRoutingZone::Add` — every packet feeds
/// the routing zone).
///
/// `verified` semantics mirror eMule: contacts are learned *unverified*;
/// `verified` is earned only by a solicited response (`mark_seen = true`,
/// the peer answered one of our requests — proving IP ownership) or by the
/// `HELLO_RES_ACK` three-way handshake.
async fn learn_contact(
    routing_table: &Option<Arc<tokio::sync::Mutex<KBucketTable>>>,
    kad_id_bytes: [u8; 16],
    ip: Ipv4Addr,
    udp_port: u16,
    tcp_port: Option<u16>,
    version: u8,
    mark_seen: bool,
) {
    let Some(table) = routing_table else {
        return;
    };
    if kad_id_bytes == [0u8; 16] {
        return;
    }
    let now_ms = now_unix_ms();
    let mut contact = KadContact::new(KadId(kad_id_bytes), ip, udp_port, tcp_port, version, now_ms);
    if mark_seen {
        contact.mark_seen(now_ms);
    }
    table.lock().await.insert_or_update(contact);
}

/// `SEARCHTOLERANCE` gate shared by the publish handlers: eMule drops a
/// publish request whose target key lies outside tolerance of our KadID
/// (`uDistance.Get32BitChunk(0) > SEARCHTOLERANCE → return` in
/// `KademliaUDPListener.cpp`). Without it this node becomes a universal
/// index sink for arbitrary keys.
fn within_publish_tolerance(local_kad_id: &[u8; 16], target: &[u8; 16]) -> bool {
    KadId(*local_kad_id).is_within_search_tolerance(&KadId(*target))
}

/// Mirrors an accepted `PublishSourceReq` into `kad_sources_index` so the
/// storage-node role survives daemon restarts (TTL = `KAD_SOURCE_TTL`).
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
        expires_at_unix_ms: now_ms + KAD_SOURCE_TTL.as_millis() as u64,
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
        expires_at_unix_ms: (now_ms + KAD_NOTE_TTL.as_millis() as u64) as i64,
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
            expires_at_unix_ms: (now_ms + KAD_KEYWORD_TTL.as_millis() as u64) as i64,
        })
        .collect();
    let mut dl_guard = dl.lock().await;
    if let Err(e) = dl_guard.db_mut().insert_kad_keyword_entries(&rows) {
        tracing::warn!("Kad storage: failed to persist keyword entries: {e}");
    }
}

/// eMule `CKademliaUDPListener::FirewalledCheck` (KademliaUDPListener.cpp:159):
/// while a TCP recheck round is active, each inbound HELLO_REQ/HELLO_RES
/// triggers a `KADEMLIA_FIREWALLED{2}_REQ` back to the sender, asking it to
/// TCP-connect to our advertised eD2K port. The contact version picks the
/// packet shape — `FIREWALLED2_REQ` (≥ `KADEMLIA_VERSION7_49A`) carries our
/// user hash + connect-options so the responder can complete a full Kad
/// handshake on the connect-back.
#[allow(clippy::too_many_arguments)]
async fn maybe_tcp_firewall_check(
    socket: &Arc<UdpSocket>,
    peer_addr: SocketAddr,
    peer_kad_version: u8,
    firewall: &Arc<tokio::sync::Mutex<FirewallTestTracker>>,
    our_tcp_port: u16,
    connect_options: u8,
    our_user_hash: &[u8; 16],
) {
    {
        let mut fw = firewall.lock().await;
        if !fw.tcp_recheck_active() || fw.is_tcp_check_ip(peer_addr.ip()) {
            return;
        }
        // Registered before send (`AddKadFirewallRequest`) so a fast
        // FIREWALLED_RES or TCP connect-back is never dropped as unrequested.
        fw.add_tcp_check_ip(peer_addr.ip());
    }
    if our_tcp_port == 0 {
        tracing::debug!("Kad UDP: TCP firewall check skipped — eD2K TCP port unknown");
        return;
    }
    let pkt = if peer_kad_version >= KADEMLIA_VERSION7_49A {
        KadCodec::encode_firewalled2_req(our_tcp_port, our_user_hash, connect_options)
    } else {
        KadCodec::encode_firewalled_req(our_tcp_port)
    };
    if let Err(e) = socket.send_to(&pkt, peer_addr).await {
        tracing::debug!(
            "Kad UDP: failed to send FIREWALLED{suffix}_REQ to {peer}: {e}",
            suffix = if peer_kad_version >= KADEMLIA_VERSION7_49A { "2" } else { "" },
            peer = log_peer_addr(peer_addr)
        );
        return;
    }
    tracing::debug!(
        "Kad UDP: sent FIREWALLED{suffix}_REQ to {peer} (tcp_port={our_tcp_port}, kad_v{peer_kad_version})",
        suffix = if peer_kad_version >= KADEMLIA_VERSION7_49A { "2" } else { "" },
        peer = log_peer_addr(peer_addr)
    );
}
