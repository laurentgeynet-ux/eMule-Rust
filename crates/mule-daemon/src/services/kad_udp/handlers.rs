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
        KadMessage::HelloReq(_hello) => {
            tracing::info!("Kad UDP: received HelloReq from {peer_addr}, replying with HelloRes");
            let hello_res = KadCodec::encode_hello_res(local_kad_id, local_port, 2, 0);
            let _ = socket.send_to(&hello_res, peer_addr).await;
        }
        KadMessage::HelloRes(hello_res) => {
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
                "Kad UDP: received HelloRes from {peer_addr} (matched_tx={matched_tx:?})"
            );
            if let Some(id) = matched_tx {
                if let Some(tx) = inf.remove(&id) {
                    let _ = tx.sender.send(Ok(KadMessage::HelloRes(hello_res)));
                }
            }
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
                    .map_or(true, |h| h == search_res.target_file_hash);
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
                    .map_or(true, |h| h == pub_res.target_file_hash);
                let resp_match = tx.expected_response == KadExpectedResponse::PublishSourceRes
                    || tx.expected_response == KadExpectedResponse::PublishKeywordRes;
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
                    if tx.expected_response == KadExpectedResponse::PublishKeywordRes {
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
                    .map_or(true, |h| h == search_res.keyword_hash);
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
    }
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
