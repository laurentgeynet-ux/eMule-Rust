// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use crate::services::peer_session::log_peer_addr;
use bytes::BytesMut;
use flate2::{Decompress, FlushDecompress, Status};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};
use tracing::info;

use mule_core::model::PeerFailureReason;
use mule_protocol::ed2k::{
    Ed2kBlockRange, Ed2kBlockRange64, Ed2kDecoder, Ed2kDownloadSequencer, Ed2kEncoder,
    Ed2kFileStatus, Ed2kRequestParts, Ed2kRequestPartsI64, InFlightBlock,
    MAX_REQUEST_RANGES_PER_PACKET, OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_CANCELTRANSFER,
    OP_COMPRESSEDPART, OP_END_OF_DOWNLOAD, OP_OUTOFPARTS, OP_REQUESTSOURCES, OP_REQUESTSOURCES2,
    OP_SENDINGPART, PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED,
};
use mule_protocol::framing::PacketCodec;

use super::events::{emit_download_update, emit_event};
use super::source_exchange::{
    handle_sx_answer, handle_sx_request, maybe_send_sx_request, PeerSxCaps, SxEnv, SxLocalIds,
};
use super::types::{
    PeerConnectionStatus, PeerError, PeerTimeouts, SwarmInnerState, MAX_PEER_READ_BUFFER_SIZE,
};
use super::PeerSessionManager;
use crate::services::downloads::DownloadService;
use crate::services::obfuscation_stream::ObfuscatedStream;

pub use mule_protocol::ed2k::{OP_COMPRESSEDPART_I64, OP_SENDINGPART_I64};

/// Builds a block-request packet for `range`, choosing the 64-bit wire format
/// (`OP_REQUESTPARTS_I64`) when offsets exceed the 32-bit boundary. I64
/// requests require the peer's `ExtendedRequestsVer >= 2` (eMule
/// `GetExtendedRequestsVersion`); returns `None` when the peer cannot serve
/// the requested range.
/// Extensions de fichiers prévisualisables — eMule n'applique la priorité
/// première/dernière part (`PR_VERYHIGH`) qu'aux médias ouvrables en cours
/// de téléchargement.
fn is_previewable_media(file_name: &str) -> bool {
    const PREVIEW_EXTS: &[&str] = &[
        "avi", "mpg", "mpeg", "mp4", "mkv", "wmv", "mov", "flv", "ts", "vob", "mp3", "flac", "ogg",
        "wav", "m4a", "aac",
    ];
    std::path::Path::new(file_name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| PREVIEW_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Statuts pour lesquels une session peer peut émettre de nouvelles requêtes
/// de blocs. Un download pausé/échoué/retiré ne doit plus rien demander —
/// les écritures déjà en vol sont tout de même persistées côté `inject_range`
/// (eMule termine les blocs en vol sans dépauser le partfile).
fn is_download_active_status(status: &str) -> bool {
    matches!(
        status.to_lowercase().as_str(),
        "queued" | "downloading" | "waiting_for_sources" | "waitingforsources"
    )
}

/// Block request pending toward one peer — eMule `Pending_Block_Struct`:
/// the range was planned (`fQueued == false`) or already sent in an
/// `OP_REQUESTPARTS` packet (`fQueued == true`); `recv_to` is the frontier
/// up to which data has been delivered.
struct PendingBlock {
    part_idx: u16,
    range: Ed2kBlockRange64,
    queued: bool,
    recv_to: u64,
}

/// eMule `SendBlockRequests` pending-block count, adaptive to the measured
/// download rate of this source (DownloadClient.cpp):
/// `<4 KiB/s → 1`, `<9 KiB/s → 2`, `>150 KiB/s → 9`, `>75 KiB/s → 6`, else 3.
/// The low-rate tiers only apply to eMule clients (`IsEmuleClient()`).
fn emule_pending_block_count(peer_bps: u64, is_emule_client: bool) -> usize {
    if is_emule_client && peer_bps < 9 * 1024 {
        if peer_bps < 4 * 1024 {
            1
        } else {
            2
        }
    } else if peer_bps > 150 * 1024 {
        9
    } else if peer_bps > 75 * 1024 {
        6
    } else {
        3
    }
}

/// How long an in-flight block stays reserved after its last sign of life —
/// eMule has no TTL at all (the block is released when the session ends or
/// stalls into requeue); the refresh-on-data approach bounds stale entries
/// without duplicating requests toward slow trickle sources.
const INFLIGHT_BLOCK_TTL_MS: i64 = 100_000;

/// Consecutive stall→requeue recoveries allowed on one socket before the
/// session is dropped — eMule re-queues forever; the cap keeps a pathological
/// peer from pinning a session slot while the swarm rotates sources.
const MAX_CONSECUTIVE_STALL_REQUEUES: u32 = 3;

/// Builds one `OP_REQUESTPARTS`/`OP_REQUESTPARTS_I64` packet carrying up to
/// [`MAX_REQUEST_RANGES_PER_PACKET`] ranges — the eMule wire layout packs
/// three `start`/`end` slots per packet. The I64 variant is required when any
/// range crosses the 4 GiB boundary and needs `ExtendedRequestsVer >= 2`
/// (eMule `GetExtendedRequestsVersion`); returns `None` when the peer cannot
/// serve the requested ranges.
fn encode_block_request_packet(
    encoder: &Ed2kEncoder,
    file_hash: [u8; 16],
    ranges: &[Ed2kBlockRange64],
    peer_ext_req: u8,
) -> Option<mule_protocol::framing::RawPacket> {
    if ranges.is_empty() || ranges.len() > MAX_REQUEST_RANGES_PER_PACKET {
        return None;
    }
    let needs_i64 = ranges.iter().any(|r| r.end > u32::MAX as u64);
    if !needs_i64 {
        let ranges32: Vec<Ed2kBlockRange> = ranges
            .iter()
            .map(|r| Ed2kBlockRange::new(r.start as u32, r.end as u32))
            .collect();
        encoder
            .encode_request_parts(&Ed2kRequestParts::multi(file_hash, ranges32))
            .ok()
    } else if peer_ext_req >= 2 {
        encoder
            .encode_request_parts_i64(&Ed2kRequestPartsI64 {
                file_hash,
                ranges: ranges.to_vec(),
            })
            .ok()
    } else {
        tracing::warn!(
            "Block ranges cross the 4 GiB boundary but peer does not support extended requests (ext_req={peer_ext_req}) — cannot request"
        );
        None
    }
}

/// eMule `SendBlockRequests`: tops `pending` up to `block_count` disjoint
/// blocks via the sequencer, registers each as in-flight, then emits
/// `OP_REQUESTPARTS` packets of up to three ranges each. Returns `true` when
/// at least one new range was sent on the wire.
#[allow(clippy::too_many_arguments)]
async fn fill_and_send_block_requests(
    stream: &mut ObfuscatedStream<TcpStream>,
    encoder: &Ed2kEncoder,
    sequencer: &mut Ed2kDownloadSequencer,
    address_str: &str,
    inner: &Arc<Mutex<SwarmInnerState>>,
    dl_hash_bytes: [u8; 16],
    peer_ext_req: u8,
    completed_ranges: &[(u64, u64)],
    pending: &mut VecDeque<PendingBlock>,
    block_count: usize,
    target_addr: SocketAddr,
) -> Result<bool, PeerError> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Plan the missing blocks and reserve them in the shared in-flight list.
    let planned = {
        let mut g = inner.lock().await;
        let want = block_count.saturating_sub(pending.len());
        let blocks = if want == 0 {
            Vec::new()
        } else {
            sequencer
                .plan_next_blocks_for_peer64(
                    address_str,
                    completed_ranges,
                    &g.in_flight_blocks,
                    now_ms,
                    want,
                )
                .map_err(|e| {
                    PeerError::new(
                        PeerFailureReason::InvalidRange,
                        format!("Planning error: {e}"),
                    )
                })?
        };
        for (part_idx, block) in &blocks {
            g.in_flight_blocks.push(InFlightBlock::new(
                address_str.to_string(),
                *part_idx,
                block.start,
                block.end,
                now_ms,
                INFLIGHT_BLOCK_TTL_MS,
            ));
            pending.push_back(PendingBlock {
                part_idx: *part_idx,
                range: *block,
                queued: false,
                recv_to: block.start,
            });
        }
        blocks.len()
    };

    // Emit the un-queued pending blocks in packets of ≤3 ranges.
    let mut sent_any = false;
    let unqueued: Vec<Ed2kBlockRange64> = pending
        .iter()
        .filter(|p| !p.queued)
        .map(|p| p.range)
        .collect();
    for chunk in unqueued.chunks(MAX_REQUEST_RANGES_PER_PACKET) {
        let Some(pkt) = encode_block_request_packet(encoder, dl_hash_bytes, chunk, peer_ext_req)
        else {
            // Unencodable chunk (e.g. >4 GiB without I64 support): mark the
            // ranges as sent so they stop blocking the pipeline; they will be
            // re-planned on the next pass for a different peer.
            tracing::warn!(
                "Peer {peer}: dropping unencodable request chunk ({} range(s))",
                chunk.len(),
                peer = log_peer_addr(target_addr)
            );
            for r in chunk {
                if let Some(p) = pending
                    .iter_mut()
                    .find(|p| p.range.start == r.start && p.range.end == r.end)
                {
                    p.queued = true;
                }
            }
            continue;
        };
        let mut out_wire = Vec::with_capacity(6 + pkt.payload.len());
        out_wire.push(pkt.protocol);
        out_wire.extend_from_slice(&(pkt.frame_len() as u32).to_le_bytes());
        out_wire.push(pkt.opcode);
        out_wire.extend_from_slice(&pkt.payload);
        if let Err(e) = stream.write_all(&out_wire).await {
            return Err(PeerError::new(
                PeerFailureReason::ConnectRefused,
                format!("Failed to send OP_REQUESTPARTS: {e}"),
            ));
        }
        for r in chunk {
            if let Some(p) = pending
                .iter_mut()
                .find(|p| p.range.start == r.start && p.range.end == r.end)
            {
                p.queued = true;
            }
        }
        sent_any = true;
    }
    if planned > 0 {
        tracing::debug!(
            "Peer {peer}: {planned} new pending block(s) planned ({sent} sent, cap {block_count})",
            peer = log_peer_addr(target_addr),
            sent = pending.iter().filter(|p| p.queued).count()
        );
    }
    Ok(sent_any)
}

async fn sync_transfer_rates(
    inner: &Arc<Mutex<SwarmInnerState>>,
    downloads: &Arc<Mutex<DownloadService>>,
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    address_str: &str,
    file_hash_str: &str,
    peer_bps: u64,
    total_received: u64,
) {
    let (progress_snapshot, total_file_rate_bps, pending_ranges) = {
        let mut g = inner.lock().await;
        let snap = if let Some(session) = g.sessions.get_mut(address_str) {
            session.snapshot.transferred_bytes = total_received;
            session.snapshot.transfer_rate_bps = peer_bps;
            Some(session.snapshot.clone())
        } else {
            None
        };
        let total_rate: u64 = g
            .sessions
            .values()
            .filter(|s| s.snapshot.file_hash.as_deref() == Some(file_hash_str))
            .map(|s| s.snapshot.transfer_rate_bps)
            .sum();
        let pending = PeerSessionManager::pending_ranges_locked(&g, file_hash_str);
        (snap, total_rate, pending)
    };

    if let Some(snap) = progress_snapshot {
        emit_event(event_tx, sequence, &snap);
    }

    let mut dl_svc = downloads.lock().await;
    if let Some(mut snap) = dl_svc.set_download_rate_by_hash(file_hash_str, total_file_rate_bps) {
        snap.pending_ranges = pending_ranges;
        emit_download_update(event_tx, sequence, &snap);
    }
}

pub(crate) async fn run_transfer_loop(
    stream: &mut ObfuscatedStream<TcpStream>,
    target_addr: SocketAddr,
    file_hash_str: &str,
    dl_hash_bytes: [u8; 16],
    address_str: &str,
    file_status: Ed2kFileStatus,
    peer_supports_aich: bool,
    peer_ext_req: u8,
    timeouts: &PeerTimeouts,
    cancel_token: &mule_network_policy::CancellationToken,
    inner: &Arc<Mutex<SwarmInnerState>>,
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    downloads: &Arc<Mutex<DownloadService>>,
    sx_caps: &PeerSxCaps,
    sx_env: &SxEnv,
    sx_local: &SxLocalIds,
    sui: &mut super::sui::OutboundSui,
    rate_limiters: Option<&mule_network_policy::PolicyRateLimiters>,
) -> Result<u64, PeerError> {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default().with_extended_requests_version(peer_ext_req);
    let codec = PacketCodec::new();
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    // État du pair capturé avant que `file_status` ne soit consommé par le
    // sequencer : part-status du demandeur (`reqstatus` eMule pour les
    // réponses SX) et complétude (`m_bCompleteSource` — eMule ne demande
    // pas de sources à une source complète).
    let peer_part_status: Vec<u8> = file_status.availability_mask.raw_bytes().to_vec();
    let peer_is_complete = file_status.is_complete();
    // Copie conservée pour le chemin stall→requeue (`file_status` est consommé
    // par le séquenceur) ; rafraîchie par `wait_in_queue` à chaque re-entrée.
    let file_status_fallback = file_status.clone();
    let mut file_status_refreshed: Option<Ed2kFileStatus> = None;

    // Flush any pending AICH recovery requests accumulated before this session
    // (eMule SendAICHRequest for corrupted parts).
    super::status_query::send_pending_aich_requests(
        stream,
        &encoder,
        target_addr,
        file_hash_str,
        dl_hash_bytes,
        peer_supports_aich,
        downloads,
    )
    .await;

    // Multi-source block planner setup
    let (file_size, completed_ranges, is_previewable, part_scarcity) = {
        let dl_svc = downloads.lock().await;
        let dl = dl_svc.find_by_ed2k_hash(file_hash_str).ok_or_else(|| {
            PeerError::new(
                PeerFailureReason::FileNotFound,
                "Download not found in local service",
            )
        })?;
        // The download may have been paused/stopped between scheduling and
        // session start — never request blocks for an inactive file.
        if !is_download_active_status(&dl.status) {
            info!(
                "Peer {peer}: download '{}' is '{}' — no block requests will be sent",
                dl.file_name,
                dl.status,
                peer = log_peer_addr(target_addr)
            );
            return Ok(0);
        }
        let ranges = dl_svc.get_completed_ranges_by_hash(file_hash_str);
        // Swarm healing (eMule-Rust `swarm_healing` setting) : biaise le
        // choix de part vers les plus rares du swarm (sources connues).
        let healing = dl_svc
            .get_setting("swarm_healing")
            .ok()
            .flatten()
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "on"))
            .unwrap_or(false);
        let scarcity = if healing {
            dl_svc.part_source_counts(file_hash_str)
        } else {
            None
        };
        (
            dl.total_bytes,
            ranges,
            is_previewable_media(&dl.file_name),
            scarcity,
        )
    };

    let mut sequencer = Ed2kDownloadSequencer::new(dl_hash_bytes, file_size, file_status)
        .map_err(|e| {
            PeerError::new(
                PeerFailureReason::CorruptedData,
                format!("Sequencer init error: {e}"),
            )
        })?
        .with_preview_priority(is_previewable);
    if let Some(counts) = &part_scarcity {
        sequencer = sequencer.with_part_scarcity(counts.clone());
    }

    // eMule `SendBlockRequests` — pending-block pipeline. `blockCount` is
    // adaptive to the measured rate of this source (1/2/3/6/9) and each
    // OP_REQUESTPARTS packet packs up to three ranges.
    let is_emule_client = peer_ext_req > 0;
    let peer_bps = inner
        .lock()
        .await
        .sessions
        .get(address_str)
        .map(|s| s.snapshot.transfer_rate_bps)
        .unwrap_or(0);
    let mut pending: VecDeque<PendingBlock> = VecDeque::new();
    fill_and_send_block_requests(
        stream,
        &encoder,
        &mut sequencer,
        address_str,
        inner,
        dl_hash_bytes,
        peer_ext_req,
        &completed_ranges,
        &mut pending,
        emule_pending_block_count(peer_bps, is_emule_client),
        target_addr,
    )
    .await?;

    if pending.is_empty() {
        info!(
            "No unassigned/missing blocks available for peer {}",
            log_peer_addr(target_addr)
        );
        // eMule CUpDownClient::SendOutOfPartReqsAndPacket: notify the
        // uploader that we currently have no pending block requests
        // (OP_OUTOFPARTS, empty payload) before leaving the session.
        let mut out_wire = Vec::with_capacity(6);
        out_wire.push(PR_ED2K);
        out_wire.extend_from_slice(&1u32.to_le_bytes());
        out_wire.push(OP_OUTOFPARTS);
        let _ = stream.write_all(&out_wire).await;
        return Ok(0);
    }

    let transferring_snapshot = {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(address_str) {
            session.snapshot.status = PeerConnectionStatus::Transferring;
            session.snapshot.current_part = pending.front().map(|p| p.part_idx);
            Some(session.snapshot.clone())
        } else {
            None
        }
    };
    if let Some(snap) = transferring_snapshot {
        emit_event(event_tx, sequence, &snap);
    }

    // Receive chunks loop
    let mut total_received = 0u64;
    // eMule DOWNLOADTIMEOUT (100 s) is measured from `m_dwLastBlockReceived`,
    // which SendBlockRequests refreshes — a request send counts as activity.
    let mut last_block_data = tokio::time::Instant::now();
    let mut last_rate_check = std::time::Instant::now();
    let mut bytes_since_rate_check = 0u64;
    // Zlib streams are per pending block (eMule attaches `zStream` to the
    // Pending_Block_Struct, keyed here by the block's start offset).
    let mut zstreams: HashMap<u64, Decompress> = HashMap::new();
    let mut last_dl_progress_log = std::time::Instant::now();
    let mut consecutive_stall_requeues = 0u32;
    // eMule credits: received bytes accumulate here and are persisted under
    // the peer's verified pubkey identity in batches (clients.met semantics).
    let mut credit_pending_bytes = 0u64;

    struct RateResetGuard<'a> {
        downloads: &'a Arc<Mutex<DownloadService>>,
        inner: &'a Arc<Mutex<SwarmInnerState>>,
        file_hash_str: &'a str,
        address_str: &'a str,
        event_tx: &'a broadcast::Sender<String>,
        sequence: &'a Arc<AtomicU64>,
    }

    impl<'a> Drop for RateResetGuard<'a> {
        fn drop(&mut self) {
            let downloads = Arc::clone(self.downloads);
            let inner = Arc::clone(self.inner);
            let file_hash_str = self.file_hash_str.to_string();
            let address_str = self.address_str.to_string();
            let event_tx = self.event_tx.clone();
            let sequence = Arc::clone(self.sequence);
            tokio::spawn(async move {
                let (total_remaining_bps, pending_ranges): (u64, Vec<(u64, u64)>) = {
                    let mut g = inner.lock().await;
                    if let Some(session) = g.sessions.get_mut(&address_str) {
                        session.snapshot.transfer_rate_bps = 0;
                    }
                    let total = g
                        .sessions
                        .values()
                        .filter(|s| s.snapshot.file_hash.as_deref() == Some(&file_hash_str))
                        .map(|s| s.snapshot.transfer_rate_bps)
                        .sum();
                    let pending = PeerSessionManager::pending_ranges_locked(&g, &file_hash_str);
                    (total, pending)
                };
                let mut dl_svc = downloads.lock().await;
                if let Some(mut snap) =
                    dl_svc.set_download_rate_by_hash(&file_hash_str, total_remaining_bps)
                {
                    snap.pending_ranges = pending_ranges;
                    emit_download_update(&event_tx, &sequence, &snap);
                }
            });
        }
    }

    let _rate_guard = RateResetGuard {
        downloads,
        inner,
        file_hash_str,
        address_str,
        event_tx,
        sequence,
    };

    // Re-ask SX périodique — eMule attache `OP_REQUESTSOURCES` au cycle de
    // reask du client (`SendFileRequest` → `IsSourceRequestAllowed`, cadence
    // dominée par SOURCECLIENTREASKS = 40 min). Vérifié chaque minute.
    let mut next_sx_check = tokio::time::Instant::now() + Duration::from_secs(60);
    let reask_caps = PeerSxCaps {
        complete_source: peer_is_complete,
        ..*sx_caps
    };

    'transfer_loop: loop {
        // eMule DOWNLOADTIMEOUT (100 s): no block data since the last request
        // send / received packet → SendCancelTransfer + back to queue on the
        // same socket. `first_data_timeout` covers the pre-first-block window.
        let stall_timeout = if total_received == 0 {
            timeouts.first_data_timeout
        } else {
            timeouts.inactivity_timeout
        };

        tokio::select! {
            _ = cancel_token.cancelled() => {
                // eMule sends OP_CANCELTRANSFER (empty payload) when an
                // ongoing transfer is cancelled, so the uploader frees the
                // slot immediately instead of relying on TCP teardown.
                let _ = stream
                    .write_all(&[PR_ED2K, 1, 0, 0, 0, OP_CANCELTRANSFER])
                    .await;
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Transfer aborted by kill switch"));
            }
            _ = tokio::time::sleep_until(last_block_data + stall_timeout) => {
                // eMule CUpDownClient::CheckDownloadTimeout: the remote
                // uploader stalled → OP_CANCELTRANSFER then re-enter its
                // upload queue on the same socket (queue position handled by
                // the remote, which moves us back like a normal re-ask).
                consecutive_stall_requeues += 1;
                if consecutive_stall_requeues > MAX_CONSECUTIVE_STALL_REQUEUES {
                    return Err(PeerError::new(PeerFailureReason::TimeoutInactivity, "Peer stalled repeatedly — giving up on this session"));
                }
                info!(
                    "Peer {peer}: no block data for {}s — OP_CANCELTRANSFER + re-queue on same socket (stall #{consecutive_stall_requeues})",
                    stall_timeout.as_secs(),
                    peer = log_peer_addr(target_addr)
                );
                let _ = stream
                    .write_all(&[PR_ED2K, 1, 0, 0, 0, OP_CANCELTRANSFER])
                    .await;
                // Drop local pending state and free the in-flight ranges so
                // other peers can take them over while we wait.
                pending.clear();
                zstreams.clear();
                {
                    let mut g = inner.lock().await;
                    g.in_flight_blocks.retain(|b| b.peer_id != address_str);
                    if let Some(session) = g.sessions.get_mut(address_str) {
                        session.snapshot.status = PeerConnectionStatus::Queued;
                        session.snapshot.queue_rank = None;
                        let snap = session.snapshot.clone();
                        emit_event(event_tx, sequence, &snap);
                    }
                }
                match super::status_query::wait_in_queue(
                    stream,
                    target_addr,
                    file_hash_str,
                    dl_hash_bytes,
                    address_str,
                    sx_caps,
                    timeouts,
                    cancel_token,
                    inner,
                    event_tx,
                    sequence,
                    downloads,
                    sx_env,
                    sx_local,
                    sui,
                    peer_ext_req,
                    peer_supports_aich,
                    file_status_refreshed.take().unwrap_or_else(|| file_status_fallback.clone()),
                    true,
                )
                .await
                {
                    Ok(fresh_status) => {
                        file_status_refreshed = Some(fresh_status.clone());
                        // Rebuild the sequencer with the refreshed part
                        // bitmap — the peer's availability may have changed
                        // while we were queued.
                        let mut rebuilt = Ed2kDownloadSequencer::new(dl_hash_bytes, file_size, fresh_status)
                            .map_err(|e| PeerError::new(PeerFailureReason::CorruptedData, format!("Sequencer init error: {e}")))?
                            .with_preview_priority(is_previewable);
                        if let Some(counts) = &part_scarcity {
                            rebuilt = rebuilt.with_part_scarcity(counts.clone());
                        }
                        sequencer = rebuilt;
                        let completed = downloads.lock().await.get_completed_ranges_by_hash(file_hash_str);
                        let peer_bps = inner.lock().await.sessions.get(address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0);
                        fill_and_send_block_requests(
                            stream, &encoder, &mut sequencer, address_str, inner,
                            dl_hash_bytes, peer_ext_req, &completed, &mut pending,
                            emule_pending_block_count(peer_bps, is_emule_client), target_addr,
                        ).await?;
                        if pending.is_empty() {
                            info!("Peer {peer}: no more needed blocks after re-queue", peer = log_peer_addr(target_addr));
                            let mut end_wire = Vec::with_capacity(6 + 16);
                            end_wire.push(PR_ED2K);
                            end_wire.extend_from_slice(&(17u32).to_le_bytes());
                            end_wire.push(OP_END_OF_DOWNLOAD);
                            end_wire.extend_from_slice(&dl_hash_bytes);
                            let _ = stream.write_all(&end_wire).await;
                            break 'transfer_loop;
                        }
                        {
                            let mut g = inner.lock().await;
                            if let Some(session) = g.sessions.get_mut(address_str) {
                                session.snapshot.status = PeerConnectionStatus::Transferring;
                                session.snapshot.current_part = pending.front().map(|p| p.part_idx);
                                let snap = session.snapshot.clone();
                                emit_event(event_tx, sequence, &snap);
                            }
                        }
                        last_block_data = tokio::time::Instant::now();
                    }
                    Err(e) => return Err(e),
                }
            }
            _ = tokio::time::sleep_until(next_sx_check) => {
                next_sx_check = tokio::time::Instant::now() + Duration::from_secs(60);
                maybe_send_sx_request(
                    stream,
                    &encoder,
                    target_addr,
                    file_hash_str,
                    dl_hash_bytes,
                    downloads,
                    sx_env,
                    &reask_caps,
                )
                .await;
            }
            read_res = stream.read(&mut raw_buf) => {
                match read_res {
                    Ok(0) => {
                        info!("Peer {} closed stream", log_peer_addr(target_addr));
                        break;
                    }
                    Ok(n) => {
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::CorruptedData, "Peer read buffer exceeded limit during transfer"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            if sui.handle(&packet, target_addr, &decoder, &encoder, stream).await {
                                continue;
                            }
                            if super::status_query::handle_hashset_answer(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, downloads).await {
                                continue;
                            }
                            if super::status_query::handle_aich_file_hash_res(stream, &packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, &encoder, peer_supports_aich, downloads).await {
                                continue;
                            }
                            if super::status_query::handle_aich_answer(stream, &packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, &encoder, peer_supports_aich, downloads).await {
                                continue;
                            }
                            // Pending block completed by this packet — drained
                            // after the opcode dispatch (removes the pending
                            // entry, frees the in-flight range, refills the
                            // pipeline up to `blockCount`).
                            let mut finished_pending: Option<(u16, Ed2kBlockRange64)> = None;
                            if packet.protocol == PR_ED2K
                                && (packet.opcode == OP_SENDINGPART || packet.opcode == OP_SENDINGPART_I64)
                            {
                                last_block_data = tokio::time::Instant::now();
                                consecutive_stall_requeues = 0;
                                let (part_hash, part_start, part_end, part_data) = if packet.opcode == OP_SENDINGPART_I64 {
                                    let part = mule_protocol::ed2k::decoder::decode_sending_part_i64(packet.protocol, packet.opcode, &packet.payload, 0)
                                        .map_err(|e| PeerError::new(PeerFailureReason::CorruptedData, format!("Malformed OP_SENDINGPART_I64: {e}")))?;
                                    (part.file_hash, part.start_offset, part.end_offset, part.data)
                                } else {
                                    let part = decoder.decode_sending_part(packet.protocol, packet.opcode, &packet.payload, 0)
                                        .map_err(|e| PeerError::new(PeerFailureReason::CorruptedData, format!("Malformed OP_SENDINGPART: {e}")))?;
                                    (part.file_hash, part.start_offset as u64, part.end_offset as u64, part.data)
                                };

                                if part_hash != dl_hash_bytes {
                                    return Err(PeerError::new(PeerFailureReason::HashMismatch, "OP_SENDINGPART returned mismatched file hash"));
                                }

                                // eMule ProcessBlockPacket: the declared range
                                // must match the payload size exactly
                                // (IDS_ERR_BADDATABLOCK → protocol violation).
                                if part_start >= part_end
                                    || part_data.len() as u64 != part_end - part_start
                                {
                                    return Err(PeerError::new(PeerFailureReason::InvalidRange, format!("OP_SENDINGPART declared range [{}, {}) does not match {} bytes payload", part_start, part_end, part_data.len())));
                                }

                                // eMule ProcessBlockPacket: locate the pending
                                // block this packet belongs to — any of the up
                                // to `blockCount` queued ranges may be served.
                                let pending_idx = pending.iter().position(|p| {
                                    p.range.start <= part_start && part_start < p.range.end
                                });
                                let Some(pi) = pending_idx else {
                                    // Not-requested block — eMule drops it
                                    // silently and keeps the session alive.
                                    tracing::debug!(
                                        "Peer {peer}: OP_SENDINGPART [{}, {}) matches no pending block — ignored",
                                        part_start,
                                        part_end,
                                        peer = log_peer_addr(target_addr)
                                    );
                                    continue;
                                };
                                if part_end > pending[pi].range.end {
                                    // eMule: "exceeds requested boundaries" →
                                    // RemoveBlockFromList (frees the range)
                                    // and ignores the packet — no disconnect.
                                    tracing::warn!(
                                        "Peer {peer}: OP_SENDINGPART [{}, {}) exceeds pending block [{}, {}) — block freed",
                                        part_start,
                                        part_end,
                                        pending[pi].range.start,
                                        pending[pi].range.end,
                                        peer = log_peer_addr(target_addr)
                                    );
                                    let freed = pending.remove(pi).expect("pending idx in bounds");
                                    {
                                        let mut g = inner.lock().await;
                                        g.in_flight_blocks.retain(|b| {
                                            !(b.peer_id == address_str
                                                && b.start == freed.range.start
                                                && b.end == freed.range.end)
                                        });
                                    }
                                    continue;
                                }
                                pending[pi].recv_to = pending[pi].recv_to.max(part_end);

                                if part_end >= pending[pi].range.end {
                                    finished_pending =
                                        Some((pending[pi].part_idx, pending[pi].range));
                                }

                                // Refresh the in-flight reservation while the
                                // peer keeps delivering — the block must not be
                                // re-planned to another source mid-transfer.
                                {
                                    let mut g = inner.lock().await;
                                    let now_ms = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis() as i64;
                                    for b in g.in_flight_blocks.iter_mut() {
                                        if b.peer_id == address_str {
                                            b.deadline_unix_ms = now_ms + INFLIGHT_BLOCK_TTL_MS;
                                        }
                                    }
                                }

                                let data_len = part_data.len() as u64;
                                // Global download bandwidth scheduler: pace
                                // received data through the shared token bucket.
                                if let Some(rl) = rate_limiters {
                                    if rl
                                        .download_bucket
                                        .acquire_tokens(data_len, || {
                                            cancel_token.is_cancelled()
                                        })
                                        .await
                                        .is_err()
                                    {
                                        break 'transfer_loop;
                                    }
                                }
                                total_received += data_len;
                                bytes_since_rate_check += data_len;
                                credit_pending_bytes += data_len;
                                if credit_pending_bytes >= CREDIT_FLUSH_BYTES {
                                    flush_download_credit(downloads, sui, &mut credit_pending_bytes).await;
                                }

                                let is_complete = {
                                    let mut dl_svc = downloads.lock().await;
                                    if let Some(dl) = dl_svc.find_by_ed2k_hash(file_hash_str) {
                                        if let Ok(snap) = dl_svc.inject_range(
                                            &dl.id,
                                            part_start,
                                            &part_data,
                                        ) {
                                            let s = snap.status.to_lowercase();
                                            s == "verifying" || s == "completed" || snap.progress >= 1.0
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                };

                                let elapsed = last_rate_check.elapsed();
                                if elapsed >= Duration::from_millis(500) || is_complete {
                                    let bps = if elapsed >= Duration::from_millis(500) {
                                        let rate = (bytes_since_rate_check as f64 / elapsed.as_secs_f64()) as u64;
                                        bytes_since_rate_check = 0;
                                        last_rate_check = std::time::Instant::now();
                                        rate
                                    } else {
                                        inner.lock().await.sessions.get(address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0)
                                    };

                                    sync_transfer_rates(
                                        inner,
                                        downloads,
                                        event_tx,
                                        sequence,
                                        address_str,
                                        file_hash_str,
                                        bps,
                                        total_received,
                                    ).await;
                                }

                                if last_dl_progress_log.elapsed() >= Duration::from_secs(10) {
                                    last_dl_progress_log = std::time::Instant::now();
                                    let dl_svc = downloads.lock().await;
                                    if let Some(dl) = dl_svc.find_by_ed2k_hash(file_hash_str) {
                                        let peer_bps = inner.lock().await.sessions.get(address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0);
                                        tracing::debug!(
                                            "[DL_PROGRESS] File '{}' ({:.1}% - {:.1}/{:.1} MB) | Total Rate: {:.1} KB/s (Peer: {:.1} KB/s) | Active peer: {}",
                                            dl.file_name,
                                            dl.progress * 100.0,
                                            dl.received_bytes as f64 / (1024.0 * 1024.0),
                                            dl.total_bytes as f64 / (1024.0 * 1024.0),
                                            dl.download_rate_bps as f64 / 1024.0,
                                            peer_bps as f64 / 1024.0,
                                            log_peer_addr(target_addr)
                                        );
                                    }
                                }
                            } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED || packet.protocol == PR_ED2K)
                                && (packet.opcode == OP_COMPRESSEDPART || packet.opcode == OP_COMPRESSEDPART_I64)
                            {
                                let is_i64 = packet.opcode == OP_COMPRESSEDPART_I64;
                                let min_len = if is_i64 { 28 } else { 24 };
                                if packet.payload.len() < min_len {
                                    return Err(PeerError::new(PeerFailureReason::CorruptedData, "Compressed part payload too short"));
                                }

                                let mut file_hash = [0u8; 16];
                                file_hash.copy_from_slice(&packet.payload[0..16]);
                                if file_hash != dl_hash_bytes {
                                    return Err(PeerError::new(PeerFailureReason::HashMismatch, "OP_COMPRESSEDPART returned mismatched file hash"));
                                }

                                let (block_start, chunk) = if is_i64 {
                                    let bs = u64::from_le_bytes(packet.payload[16..24].try_into().unwrap_or([0; 8]));
                                    (bs, &packet.payload[28..])
                                } else {
                                    let bs = u32::from_le_bytes(packet.payload[16..20].try_into().unwrap_or([0; 4])) as u64;
                                    (bs, &packet.payload[24..])
                                };

                                last_block_data = tokio::time::Instant::now();
                                consecutive_stall_requeues = 0;

                                // eMule attaches the zlib stream to the
                                // Pending_Block_Struct — locate which pending
                                // block this compressed chunk belongs to.
                                let pending_idx = pending.iter().position(|p| {
                                    p.range.start <= block_start && block_start < p.range.end
                                });
                                let Some(pi) = pending_idx else {
                                    tracing::debug!(
                                        "Peer {peer}: OP_COMPRESSEDPART at {} matches no pending block — ignored",
                                        block_start,
                                        peer = log_peer_addr(target_addr)
                                    );
                                    continue;
                                };
                                let range_key = pending[pi].range.start;
                                let range_end = pending[pi].range.end;
                                // Take ownership of the block's zlib stream so
                                // `pending` stays borrowable inside the loop;
                                // reinserted below unless the block completed.
                                let mut decomp = zstreams
                                    .remove(&range_key)
                                    .unwrap_or_else(|| Decompress::new(true));
                                let mut zstream_alive = true;
                                let mut out = vec![0u8; 65536];
                                let mut input_slice = chunk;

                                while !input_slice.is_empty() {
                                    let before_in = decomp.total_in();
                                    let before_out = decomp.total_out();
                                    let res = decomp
                                        .decompress(input_slice, &mut out, FlushDecompress::Sync);
                                    let produced = (decomp.total_out() - before_out) as usize;
                                    let consumed = (decomp.total_in() - before_in) as usize;
                                    input_slice = &input_slice[consumed..];

                                    // Pace received wire bytes through the
                                    // global download token bucket.
                                    if consumed > 0 {
                                        if let Some(rl) = rate_limiters {
                                            if rl
                                                .download_bucket
                                                .acquire_tokens(consumed as u64, || {
                                                    cancel_token.is_cancelled()
                                                })
                                                .await
                                                .is_err()
                                            {
                                                break 'transfer_loop;
                                            }
                                        }
                                    }

                                    if produced > 0 {
                                        total_received += produced as u64;
                                        bytes_since_rate_check += produced as u64;
                                        credit_pending_bytes += produced as u64;
                                        if credit_pending_bytes >= CREDIT_FLUSH_BYTES {
                                            flush_download_credit(downloads, sui, &mut credit_pending_bytes).await;
                                        }
                                        let write_at = pending[pi].recv_to;
                                        let is_complete = {
                                            let mut dl_svc = downloads.lock().await;
                                            if let Some(dl) = dl_svc.find_by_ed2k_hash(file_hash_str) {
                                                if let Ok(snap) = dl_svc.inject_range(
                                                    &dl.id,
                                                    write_at,
                                                    &out[..produced],
                                                ) {
                                                    let s = snap.status.to_lowercase();
                                                    s == "verifying" || s == "completed" || snap.progress >= 1.0
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            }
                                        };
                                        pending[pi].recv_to += produced as u64;

                                        let elapsed = last_rate_check.elapsed();
                                        if elapsed >= Duration::from_millis(500) || is_complete {
                                            let bps = if elapsed >= Duration::from_millis(500) {
                                                let rate = (bytes_since_rate_check as f64 / elapsed.as_secs_f64()) as u64;
                                                bytes_since_rate_check = 0;
                                                last_rate_check = std::time::Instant::now();
                                                rate
                                            } else {
                                                inner.lock().await.sessions.get(address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0)
                                            };
                                            sync_transfer_rates(
                                                inner,
                                                downloads,
                                                event_tx,
                                                sequence,
                                                address_str,
                                                file_hash_str,
                                                bps,
                                                total_received,
                                            ).await;
                                        }
                                    }

                                    match res {
                                        Ok(Status::StreamEnd) => {
                                            if pending[pi].recv_to < range_end {
                                                tracing::warn!(
                                                    "Peer {peer}: Zlib stream ended at offset {} before reaching sub-block boundary {}",
                                                    pending[pi].recv_to,
                                                    range_end
                                                , peer = log_peer_addr(target_addr));
                                            } else {
                                                tracing::debug!(
                                                    "Peer {peer}: completed decompression of block at offset {}",
                                                    pending[pi].recv_to
                                                , peer = log_peer_addr(target_addr));
                                            }
                                            zstream_alive = false;
                                            if pending[pi].recv_to >= range_end {
                                                finished_pending =
                                                    Some((pending[pi].part_idx, pending[pi].range));
                                            }
                                            break;
                                        }
                                        Ok(Status::Ok) => {
                                            if pending[pi].recv_to >= range_end {
                                                tracing::debug!(
                                                    "Peer {peer}: sub-block reached requested boundary {}",
                                                    pending[pi].recv_to
                                                , peer = log_peer_addr(target_addr));
                                                zstream_alive = false;
                                                finished_pending =
                                                    Some((pending[pi].part_idx, pending[pi].range));
                                                break;
                                            }
                                            if consumed == 0 && produced == 0 {
                                                break;
                                            }
                                        }
                                        Ok(Status::BufError) => break,
                                        Err(e) => {
                                            tracing::warn!("Peer {peer}: Zlib decompression error: {e}", peer = log_peer_addr(target_addr));
                                            break;
                                        }
                                    }
                                }
                                if zstream_alive {
                                    zstreams.insert(range_key, decomp);
                                }

                                let elapsed = last_rate_check.elapsed();
                                if elapsed >= Duration::from_millis(500) {
                                    let bps = (bytes_since_rate_check as f64 / elapsed.as_secs_f64()) as u64;
                                    bytes_since_rate_check = 0;
                                    last_rate_check = std::time::Instant::now();
                                    sync_transfer_rates(
                                        inner,
                                        downloads,
                                        event_tx,
                                        sequence,
                                        address_str,
                                        file_hash_str,
                                        bps,
                                        total_received,
                                    ).await;
                                }

                                if last_dl_progress_log.elapsed() >= Duration::from_secs(10) {
                                    last_dl_progress_log = std::time::Instant::now();
                                    let dl_svc = downloads.lock().await;
                                    if let Some(dl) = dl_svc.find_by_ed2k_hash(file_hash_str) {
                                        let peer_bps = inner.lock().await.sessions.get(address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0);
                                        tracing::debug!(
                                            "[DL_PROGRESS] File '{}' ({:.1}% - {:.1}/{:.1} MB) | Total Rate: {:.1} KB/s (Peer: {:.1} KB/s) | Active peer: {}",
                                            dl.file_name,
                                            dl.progress * 100.0,
                                            dl.received_bytes as f64 / (1024.0 * 1024.0),
                                            dl.total_bytes as f64 / (1024.0 * 1024.0),
                                            dl.download_rate_bps as f64 / 1024.0,
                                            peer_bps as f64 / 1024.0,
                                            log_peer_addr(target_addr)
                                        );
                                    }
                                }
                            } else if (packet.protocol == PR_ED2K || packet.protocol == PR_EMULE) && packet.opcode == OP_END_OF_DOWNLOAD {
                                info!("Peer {} sent OP_END_OF_DOWNLOAD", log_peer_addr(target_addr));
                                break;
                            } else if packet.protocol == PR_ED2K && packet.opcode == OP_OUTOFPARTS {
                                // The uploader tells us it currently has no
                                // requested parts left for us — end the session
                                // gracefully so the swarm can rotate sources.
                                info!("Peer {peer} sent OP_OUTOFPARTS, ending transfer session", peer = log_peer_addr(target_addr));
                                break 'transfer_loop;
                            } else if packet.protocol == PR_ED2K && packet.opcode == OP_CANCELTRANSFER {
                                info!("Peer {peer} cancelled the transfer (OP_CANCELTRANSFER)", peer = log_peer_addr(target_addr));
                                break 'transfer_loop;
                            } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
                                && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
                            {
                                handle_sx_answer(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, downloads, sx_env, sx_local).await;
                            } else if packet.protocol == PR_EMULE
                                && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
                            {
                                handle_sx_request(&packet, Some(dl_hash_bytes), target_addr, &decoder, &encoder, downloads, stream, sx_env, sx_caps, Some(peer_part_status.as_slice())).await;
                            } else if packet.protocol == PR_EMULE
                            {
                                // Sous-opcodes SX dans OP_MULTIPACKET* — le
                                // pair peut aussi nous demander des sources
                                // sur la même socket (eMule ListenSocket).
                                super::source_exchange::handle_sx_multipacket(
                                    &packet,
                                    dl_hash_bytes,
                                    Some(peer_part_status.as_slice()),
                                    target_addr,
                                    &encoder,
                                    downloads,
                                    stream,
                                    sx_env,
                                    sx_caps,
                                    peer_ext_req,
                                )
                                .await;
                            }

                            // A pending block completed — drain it and top the
                            // pipeline back up to `blockCount` (eMule
                            // SendBlockRequests is re-run per received packet).
                            if let Some((fin_part, fin_range)) = finished_pending {
                                pending.retain(|p| {
                                    !(p.part_idx == fin_part
                                        && p.range.start == fin_range.start
                                        && p.range.end == fin_range.end)
                                });
                                zstreams.remove(&fin_range.start);
                                {
                                    let mut g = inner.lock().await;
                                    g.in_flight_blocks.retain(|b| {
                                        !(b.peer_id == address_str
                                            && b.start == fin_range.start
                                            && b.end == fin_range.end)
                                    });
                                }

                                let (completed, dl_still_active) = {
                                    let dl_svc = downloads.lock().await;
                                    let still_active = dl_svc
                                        .find_by_ed2k_hash(file_hash_str)
                                        .map(|s| is_download_active_status(&s.status))
                                        .unwrap_or(false);
                                    (dl_svc.get_completed_ranges_by_hash(file_hash_str), still_active)
                                };

                                // The download was paused/removed/failed while
                                // this block was in flight — stop requesting
                                // new blocks (eMule pauses the partfile, not
                                // just the scheduler).
                                if !dl_still_active {
                                    info!("Peer {peer}: download no longer active, ending transfer session", peer = log_peer_addr(target_addr));
                                    break 'transfer_loop;
                                }

                                let peer_bps = inner
                                    .lock()
                                    .await
                                    .sessions
                                    .get(address_str)
                                    .map(|s| s.snapshot.transfer_rate_bps)
                                    .unwrap_or(0);
                                fill_and_send_block_requests(
                                    stream,
                                    &encoder,
                                    &mut sequencer,
                                    address_str,
                                    inner,
                                    dl_hash_bytes,
                                    peer_ext_req,
                                    &completed,
                                    &mut pending,
                                    emule_pending_block_count(peer_bps, is_emule_client),
                                    target_addr,
                                )
                                .await?;
                                // A request send counts as transfer activity
                                // (eMule refreshes m_dwLastBlockReceived there).
                                last_block_data = tokio::time::Instant::now();

                                let cur_part = pending.front().map(|p| p.part_idx);
                                {
                                    let mut g = inner.lock().await;
                                    if let Some(session) = g.sessions.get_mut(address_str) {
                                        session.snapshot.current_part = cur_part;
                                    }
                                }

                                if pending.is_empty() {
                                    tracing::info!("Peer {peer}: all blocks completed or no more blocks needed", peer = log_peer_addr(target_addr));
                                    let mut end_wire = Vec::with_capacity(6 + 16);
                                    end_wire.push(PR_ED2K);
                                    end_wire.extend_from_slice(&(17u32).to_le_bytes());
                                    end_wire.push(OP_END_OF_DOWNLOAD);
                                    end_wire.extend_from_slice(&dl_hash_bytes);
                                    let _ = stream.write_all(&end_wire).await;
                                    let _ = stream.flush().await;

                                    break 'transfer_loop;
                                }
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error during transfer: {e}"))),
                }
            }
        }
    }

    // Reset rate for this peer session and update aggregate file rate
    {
        let (total_remaining_bps, pending_ranges): (u64, Vec<(u64, u64)>) = {
            let mut g = inner.lock().await;
            if let Some(session) = g.sessions.get_mut(address_str) {
                session.snapshot.transfer_rate_bps = 0;
            }
            let total = g
                .sessions
                .values()
                .filter(|s| s.snapshot.file_hash.as_deref() == Some(file_hash_str))
                .map(|s| s.snapshot.transfer_rate_bps)
                .sum();
            let pending = PeerSessionManager::pending_ranges_locked(&g, file_hash_str);
            (total, pending)
        };
        let mut dl_svc = downloads.lock().await;
        if let Some(mut snap) = dl_svc.set_download_rate_by_hash(file_hash_str, total_remaining_bps)
        {
            snap.pending_ranges = pending_ranges;
            emit_download_update(event_tx, sequence, &snap);
        }
    }

    // Persist any remaining credited bytes for this session.
    flush_download_credit(downloads, sui, &mut credit_pending_bytes).await;

    Ok(total_received)
}

/// eMule credit accounting batch threshold (8 MiB) — bounds how many SQLite
/// `peer_credits` updates a fast session performs.
const CREDIT_FLUSH_BYTES: u64 = 8 * 1024 * 1024;

/// Persists accumulated downloaded bytes under the peer's *verified* SUI
/// pubkey identity (eMule credits the client key, never the transient IP).
/// Unverified peers accrue nothing — `peer_credits` is keyed by pubkey hash.
async fn flush_download_credit(
    downloads: &Arc<Mutex<DownloadService>>,
    sui: &super::sui::OutboundSui,
    pending: &mut u64,
) {
    if *pending == 0 {
        return;
    }
    if let Some((pubkey_hash, pubkey_hex)) = sui.verified_credit_identity() {
        let mut dl_svc = downloads.lock().await;
        if dl_svc
            .record_credit_download(&pubkey_hash, &pubkey_hex, *pending)
            .is_ok()
        {
            tracing::debug!(
                "[CREDITS] {} bytes downloaded credited to peer {}…",
                pending,
                &pubkey_hash[..16.min(pubkey_hash.len())]
            );
        }
        *pending = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::emule_pending_block_count;

    /// eMule `SendBlockRequests` blockCount tiers (DownloadClient.cpp):
    /// eMule clients below 9 KiB/s get the 1/2 low tiers; everyone else gets
    /// the 3/6/9 tiers regardless of client family.
    #[test]
    fn test_emule_pending_block_count_tiers() {
        // eMule client, low-rate tiers
        assert_eq!(emule_pending_block_count(0, true), 1);
        assert_eq!(emule_pending_block_count(4 * 1024 - 1, true), 1);
        assert_eq!(emule_pending_block_count(4 * 1024, true), 2);
        assert_eq!(emule_pending_block_count(9 * 1024 - 1, true), 2);
        // eMule client, upper tiers
        assert_eq!(emule_pending_block_count(9 * 1024, true), 3);
        assert_eq!(emule_pending_block_count(75 * 1024 + 1, true), 6);
        assert_eq!(emule_pending_block_count(150 * 1024 + 1, true), 9);
        // Non-eMule client: skips the low tiers entirely
        assert_eq!(emule_pending_block_count(0, false), 3);
        assert_eq!(emule_pending_block_count(2 * 1024, false), 3);
        assert_eq!(emule_pending_block_count(200 * 1024, false), 9);
    }
}
