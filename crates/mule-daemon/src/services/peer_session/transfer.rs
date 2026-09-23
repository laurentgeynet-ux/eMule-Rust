// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use crate::services::peer_session::log_peer_addr;
use bytes::BytesMut;
use flate2::{Decompress, FlushDecompress, Status};
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
    Ed2kBlockRange64, Ed2kDecoder, Ed2kDownloadSequencer, Ed2kEncoder, Ed2kFileStatus,
    Ed2kRequestParts, Ed2kRequestPartsI64, InFlightBlock, OP_ANSWERSOURCES, OP_ANSWERSOURCES2,
    OP_CANCELTRANSFER, OP_COMPRESSEDPART, OP_END_OF_DOWNLOAD, OP_OUTOFPARTS, OP_REQUESTSOURCES,
    OP_REQUESTSOURCES2, OP_SENDINGPART, PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED,
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

fn encode_block_request(
    encoder: &Ed2kEncoder,
    file_hash: [u8; 16],
    range: Ed2kBlockRange64,
    peer_ext_req: u8,
) -> Option<mule_protocol::framing::RawPacket> {
    if range.end <= u32::MAX as u64 {
        encoder
            .encode_request_parts(&Ed2kRequestParts::single(
                file_hash,
                range.start as u32,
                range.end as u32,
            ))
            .ok()
    } else if peer_ext_req >= 2 {
        encoder
            .encode_request_parts_i64(&Ed2kRequestPartsI64 {
                file_hash,
                ranges: vec![range],
            })
            .ok()
    } else {
        tracing::warn!(
            "Block range [{}, {}) exceeds 4 GiB but peer does not support extended requests (ext_req={peer_ext_req}) — cannot request",
            range.start,
            range.end
        );
        None
    }
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
    let mut out_wire = Vec::with_capacity(64);
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    // État du pair capturé avant que `file_status` ne soit consommé par le
    // sequencer : part-status du demandeur (`reqstatus` eMule pour les
    // réponses SX) et complétude (`m_bCompleteSource` — eMule ne demande
    // pas de sources à une source complète).
    let peer_part_status: Vec<u8> = file_status.availability_mask.raw_bytes().to_vec();
    let peer_is_complete = file_status.is_complete();

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
    if let Some(counts) = part_scarcity {
        sequencer = sequencer.with_part_scarcity(counts);
    }

    // Plan disjoint block
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let planned_block_opt = {
        let mut g = inner.lock().await;
        let block_opt = sequencer
            .plan_next_block_for_peer64(address_str, &completed_ranges, &g.in_flight_blocks, now_ms)
            .map_err(|e| {
                PeerError::new(
                    PeerFailureReason::InvalidRange,
                    format!("Planning error: {e}"),
                )
            })?;

        if let Some((part_idx, block)) = block_opt {
            let ifb = InFlightBlock::new(
                address_str.to_string(),
                part_idx,
                block.start,
                block.end,
                now_ms,
                30_000,
            );
            g.in_flight_blocks.push(ifb);
            Some((part_idx, block))
        } else {
            None
        }
    };

    let (part_idx, req_range) = match planned_block_opt {
        Some(b) => b,
        None => {
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
    };

    // Send OP_REQUESTPARTS (or OP_REQUESTPARTS_I64 for >4 GiB files when the
    // peer announced extended requests v2, eMule GetExtendedRequestsVersion).
    let req_pkt = encode_block_request(&encoder, dl_hash_bytes, req_range, peer_ext_req)
        .ok_or_else(|| {
            PeerError::new(
                PeerFailureReason::CorruptedData,
                "Failed to encode OP_REQUESTPARTS (unsupported range or peer capability)"
                    .to_string(),
            )
        })?;

    out_wire.clear();
    out_wire.push(req_pkt.protocol);
    out_wire.extend_from_slice(&(req_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(req_pkt.opcode);
    out_wire.extend_from_slice(&req_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(
            PeerFailureReason::ConnectRefused,
            format!("Failed to send OP_REQUESTPARTS: {e}"),
        ));
    }

    let transferring_snapshot = {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(address_str) {
            session.snapshot.status = PeerConnectionStatus::Transferring;
            session.snapshot.current_part = Some(part_idx);
            Some(session.snapshot.clone())
        } else {
            None
        }
    };
    if let Some(snap) = transferring_snapshot {
        emit_event(event_tx, sequence, &snap);
    }

    info!(
        "Requested sub-block [{}, {}) from peer {}",
        req_range.start,
        req_range.end,
        log_peer_addr(target_addr)
    );

    // Receive chunks loop
    let mut total_received = 0u64;
    let mut last_activity = tokio::time::Instant::now();
    let mut first_data_received = false;
    let mut last_rate_check = std::time::Instant::now();
    let mut bytes_since_rate_check = 0u64;
    let mut decompressor: Option<Decompress> = None;
    let mut decompressed_offset = req_range.start;
    let mut current_part_idx = part_idx;
    let mut current_req_range = req_range;
    let mut last_dl_progress_log = std::time::Instant::now();
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
        let timeout_duration = if !first_data_received {
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
            _ = tokio::time::sleep_until(last_activity + timeout_duration) => {
                if !first_data_received {
                    return Err(PeerError::new(PeerFailureReason::TimeoutFirstData, "Timed out waiting for first data packet"));
                } else {
                    return Err(PeerError::new(PeerFailureReason::TimeoutInactivity, "Transfer stall / inactivity timeout exceeded"));
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
                        last_activity = tokio::time::Instant::now();
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
                            let mut block_finished = false;
                            if packet.protocol == PR_ED2K
                                && (packet.opcode == OP_SENDINGPART || packet.opcode == OP_SENDINGPART_I64)
                            {
                                first_data_received = true;
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

                                if part_start < current_req_range.start || part_end > current_req_range.end || part_start >= part_end {
                                    return Err(PeerError::new(PeerFailureReason::InvalidRange, format!("OP_SENDINGPART range out of bounds: [{}, {}) vs [{}, {})", part_start, part_end, current_req_range.start, current_req_range.end)));
                                }

                                if part_end >= current_req_range.end {
                                    block_finished = true;
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
                                        tracing::info!(
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

                                first_data_received = true;

                                if decompressor.is_none() {
                                    decompressor = Some(Decompress::new(true));
                                    decompressed_offset = block_start;
                                }

                                let decomp = decompressor.as_mut().unwrap();
                                let mut out = vec![0u8; 65536];
                                let mut input_slice = chunk;

                                while !input_slice.is_empty() {
                                    let before_in = decomp.total_in();
                                    let before_out = decomp.total_out();
                                    let res = decomp.decompress(input_slice, &mut out, FlushDecompress::Sync);
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
                                        let is_complete = {
                                            let mut dl_svc = downloads.lock().await;
                                            if let Some(dl) = dl_svc.find_by_ed2k_hash(file_hash_str) {
                                                if let Ok(snap) = dl_svc.inject_range(
                                                    &dl.id,
                                                    decompressed_offset,
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
                                        decompressed_offset += produced as u64;

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
                                            if decompressed_offset < current_req_range.end {
                                                tracing::warn!(
                                                    "Peer {peer}: Zlib stream ended at offset {} before reaching sub-block boundary {}",
                                                    decompressed_offset,
                                                    current_req_range.end
                                                , peer = log_peer_addr(target_addr));
                                            } else {
                                                tracing::debug!(
                                                    "Peer {peer}: completed decompression of block at offset {}",
                                                    decompressed_offset
                                                , peer = log_peer_addr(target_addr));
                                            }
                                            decompressor = None;
                                            block_finished = decompressed_offset >= current_req_range.end;
                                            break;
                                        }
                                        Ok(Status::Ok) => {
                                            if decompressed_offset >= current_req_range.end {
                                                tracing::debug!(
                                                    "Peer {peer}: sub-block reached requested boundary {}",
                                                    decompressed_offset
                                                , peer = log_peer_addr(target_addr));
                                                decompressor = None;
                                                block_finished = true;
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
                                        tracing::info!(
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

                            // Pipeline next block
                            if block_finished {
                                {
                                    let mut g = inner.lock().await;
                                    g.in_flight_blocks.retain(|b| !(b.peer_id == address_str && b.part_index == current_part_idx && b.start == current_req_range.start));
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

                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as i64;

                                let next_block = {
                                    let mut g = inner.lock().await;
                                    match sequencer.plan_next_block_for_peer64(address_str, &completed, &g.in_flight_blocks, now_ms) {
                                        Ok(Some((next_p, next_r))) => {
                                            let ifb = InFlightBlock::new(
                                                address_str.to_string(),
                                                next_p,
                                                next_r.start,
                                                next_r.end,
                                                now_ms,
                                                30_000,
                                            );
                                            g.in_flight_blocks.push(ifb);
                                            Some((next_p, next_r))
                                        }
                                        _ => None,
                                    }
                                };

                                if let Some((next_p, next_r)) = next_block {
                                    current_part_idx = next_p;
                                    current_req_range = next_r;
                                    decompressor = None;
                                    decompressed_offset = next_r.start;

                                    if let Some(req_pkt) = encode_block_request(&encoder, dl_hash_bytes, next_r, peer_ext_req) {
                                        let mut out_wire = Vec::with_capacity(6 + req_pkt.payload.len());
                                        out_wire.push(req_pkt.protocol);
                                        out_wire.extend_from_slice(&(req_pkt.frame_len() as u32).to_le_bytes());
                                        out_wire.push(req_pkt.opcode);
                                        out_wire.extend_from_slice(&req_pkt.payload);
                                        if let Err(e) = stream.write_all(&out_wire).await {
                                            tracing::warn!("Failed to send chained OP_REQUESTPARTS to {peer}: {e}", peer = log_peer_addr(target_addr));
                                            break;
                                        }
                                        tracing::info!("Peer {peer}: requested next sub-block [{}, {}) in part {}", next_r.start, next_r.end, next_p, peer = log_peer_addr(target_addr));
                                        last_activity = tokio::time::Instant::now();
                                    }
                                } else {
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
