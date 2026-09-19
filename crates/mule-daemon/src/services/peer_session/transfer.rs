use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use bytes::BytesMut;
use flate2::{Decompress, FlushDecompress, Status};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};
use tracing::info;

use mule_core::model::PeerFailureReason;
use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kDownloadSequencer, Ed2kEncoder, Ed2kFileStatus,
    Ed2kRequestParts, InFlightBlock,
    OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_COMPRESSEDPART, OP_END_OF_DOWNLOAD,
    OP_REQUESTSOURCES, OP_REQUESTSOURCES2, OP_SENDINGPART, PR_ED2K, PR_EMULE,
    PR_EMULE_COMPRESSED,
};
use mule_protocol::framing::PacketCodec;

use crate::services::downloads::DownloadService;
use crate::services::obfuscation_stream::ObfuscatedStream;
use super::events::{emit_download_update, emit_event};
use super::source_exchange::{handle_sx_answer, handle_sx_request};
use super::types::{
    PeerConnectionStatus, PeerError, PeerTimeouts, SwarmInnerState, MAX_PEER_READ_BUFFER_SIZE,
};

pub const OP_COMPRESSEDPART_I64: u8 = 0xA1;
pub const OP_SENDINGPART_I64: u8 = 0xA2;

pub(crate) async fn run_transfer_loop(
    stream: &mut ObfuscatedStream<TcpStream>,
    target_addr: SocketAddr,
    file_hash_str: &str,
    dl_hash_bytes: [u8; 16],
    address_str: &str,
    file_status: Ed2kFileStatus,
    timeouts: &PeerTimeouts,
    cancel_token: &mule_network_policy::CancellationToken,
    inner: &Arc<Mutex<SwarmInnerState>>,
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    downloads: &Arc<Mutex<DownloadService>>,
) -> Result<u64, PeerError> {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let codec = PacketCodec::new();
    let mut out_wire = Vec::with_capacity(64);
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];

    // Multi-source block planner setup
    let (file_size, completed_ranges) = {
        let dl_svc = downloads.lock().await;
        let dl = dl_svc.find_by_ed2k_hash(file_hash_str).ok_or_else(|| {
            PeerError::new(PeerFailureReason::FileNotFound, "Download not found in local service")
        })?;
        let ranges = dl_svc.get_completed_ranges_by_hash(file_hash_str);
        (dl.total_bytes, ranges)
    };

    let sequencer = Ed2kDownloadSequencer::new(dl_hash_bytes, file_size, file_status).map_err(|e| {
        PeerError::new(PeerFailureReason::CorruptedData, format!("Sequencer init error: {e}"))
    })?;

    // Plan disjoint block
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let planned_block_opt = {
        let mut g = inner.lock().await;
        let block_opt = sequencer.plan_next_block_for_peer(
            address_str,
            &completed_ranges,
            &g.in_flight_blocks,
            now_ms,
        ).map_err(|e| PeerError::new(PeerFailureReason::InvalidRange, format!("Planning error: {e}")))?;

        if let Some((part_idx, block)) = block_opt {
            let ifb = InFlightBlock::new(
                address_str.to_string(),
                part_idx,
                block.start as u64,
                block.end as u64,
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
            info!("No unassigned/missing blocks available for peer {}", target_addr);
            return Ok(0);
        }
    };

    // Send OP_REQUESTPARTS
    let req_parts = Ed2kRequestParts {
        file_hash: dl_hash_bytes,
        ranges: vec![req_range],
    };

    let req_pkt = encoder.encode_request_parts(&req_parts).map_err(|e| {
        PeerError::new(PeerFailureReason::CorruptedData, format!("Failed to encode OP_REQUESTPARTS: {e}"))
    })?;

    out_wire.clear();
    out_wire.push(req_pkt.protocol);
    out_wire.extend_from_slice(&(req_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(req_pkt.opcode);
    out_wire.extend_from_slice(&req_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_REQUESTPARTS: {e}")));
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

    info!("Requested sub-block [{}, {}) from peer {}", req_range.start, req_range.end, target_addr);

    // Receive chunks loop
    let mut total_received = 0u64;
    let mut last_activity = tokio::time::Instant::now();
    let mut first_data_received = false;
    let mut last_rate_check = std::time::Instant::now();
    let mut bytes_since_rate_check = 0u64;
    let mut decompressor: Option<Decompress> = None;
    let mut decompressed_offset = req_range.start as u64;
    let mut current_part_idx = part_idx;
    let mut current_req_range = req_range;

    struct RateResetGuard<'a> {
        downloads: &'a Arc<Mutex<DownloadService>>,
        file_hash_str: &'a str,
        event_tx: &'a broadcast::Sender<String>,
        sequence: &'a Arc<AtomicU64>,
    }

    impl<'a> Drop for RateResetGuard<'a> {
        fn drop(&mut self) {
            let downloads = Arc::clone(self.downloads);
            let file_hash_str = self.file_hash_str.to_string();
            let event_tx = self.event_tx.clone();
            let sequence = Arc::clone(self.sequence);
            tokio::spawn(async move {
                let mut dl_svc = downloads.lock().await;
                if let Some(snap) = dl_svc.set_download_rate_by_hash(&file_hash_str, 0) {
                    emit_download_update(&event_tx, &sequence, &snap);
                }
            });
        }
    }

    let _rate_guard = RateResetGuard {
        downloads,
        file_hash_str,
        event_tx,
        sequence,
    };

    'transfer_loop: loop {
        let timeout_duration = if !first_data_received {
            timeouts.first_data_timeout
        } else {
            timeouts.inactivity_timeout
        };

        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Transfer aborted by kill switch"));
            }
            _ = tokio::time::sleep_until(last_activity + timeout_duration) => {
                if !first_data_received {
                    return Err(PeerError::new(PeerFailureReason::TimeoutFirstData, "Timed out waiting for first data packet"));
                } else {
                    return Err(PeerError::new(PeerFailureReason::TimeoutInactivity, "Transfer stall / inactivity timeout exceeded"));
                }
            }
            read_res = stream.read(&mut raw_buf) => {
                match read_res {
                    Ok(0) => {
                        info!("Peer {} closed stream", target_addr);
                        break;
                    }
                    Ok(n) => {
                        last_activity = tokio::time::Instant::now();
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::CorruptedData, "Peer read buffer exceeded limit during transfer"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            let mut block_finished = false;
                            if packet.protocol == PR_ED2K && packet.opcode == OP_SENDINGPART {
                                first_data_received = true;
                                let part = decoder.decode_sending_part(packet.protocol, packet.opcode, &packet.payload, 0)
                                    .map_err(|e| PeerError::new(PeerFailureReason::CorruptedData, format!("Malformed OP_SENDINGPART: {e}")))?;

                                if part.file_hash != dl_hash_bytes {
                                    return Err(PeerError::new(PeerFailureReason::HashMismatch, "OP_SENDINGPART returned mismatched file hash"));
                                }

                                if part.start_offset < current_req_range.start || part.end_offset > current_req_range.end || part.start_offset >= part.end_offset {
                                    return Err(PeerError::new(PeerFailureReason::InvalidRange, format!("OP_SENDINGPART range out of bounds: [{}, {}) vs [{}, {})", part.start_offset, part.end_offset, current_req_range.start, current_req_range.end)));
                                }

                                if part.end_offset >= current_req_range.end {
                                    block_finished = true;
                                }

                                let data_len = part.data.len() as u64;
                                total_received += data_len;
                                bytes_since_rate_check += data_len;

                                let is_complete = {
                                    let mut dl_svc = downloads.lock().await;
                                    if let Some(dl) = dl_svc.find_by_ed2k_hash(file_hash_str) {
                                        if let Ok(snap) = dl_svc.inject_range(
                                            &dl.id,
                                            part.start_offset as u64,
                                            &part.data,
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
                                let bps = if elapsed >= Duration::from_millis(500) {
                                    let rate = (bytes_since_rate_check as f64 / elapsed.as_secs_f64()) as u64;
                                    bytes_since_rate_check = 0;
                                    last_rate_check = std::time::Instant::now();
                                    rate
                                } else {
                                    inner.lock().await.sessions.get(address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0)
                                };

                                let progress_snapshot = {
                                    let mut g = inner.lock().await;
                                    if let Some(session) = g.sessions.get_mut(address_str) {
                                        session.snapshot.transferred_bytes = total_received;
                                        session.snapshot.transfer_rate_bps = bps;
                                        Some(session.snapshot.clone())
                                    } else {
                                        None
                                    }
                                };
                                if let Some(snap) = progress_snapshot {
                                    emit_event(event_tx, sequence, &snap);
                                }

                                if elapsed >= Duration::from_millis(500) || is_complete {
                                    let mut dl_svc = downloads.lock().await;
                                    if let Some(snap) = dl_svc.set_download_rate_by_hash(file_hash_str, bps) {
                                        emit_download_update(event_tx, sequence, &snap);
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

                                    if produced > 0 {
                                        total_received += produced as u64;
                                        bytes_since_rate_check += produced as u64;
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
                                            let mut dl_svc = downloads.lock().await;
                                            if let Some(snap) = dl_svc.set_download_rate_by_hash(file_hash_str, bps) {
                                                emit_download_update(event_tx, sequence, &snap);
                                            }
                                        }
                                    }

                                    match res {
                                        Ok(Status::StreamEnd) => {
                                            if decompressed_offset < current_req_range.end as u64 {
                                                tracing::warn!(
                                                    "Peer {target_addr}: Zlib stream ended at offset {} before reaching sub-block boundary {}",
                                                    decompressed_offset,
                                                    current_req_range.end
                                                );
                                            } else {
                                                tracing::info!(
                                                    "Peer {target_addr}: completed decompression of block at offset {}",
                                                    decompressed_offset
                                                );
                                            }
                                            decompressor = None;
                                            block_finished = decompressed_offset >= current_req_range.end as u64;
                                            break;
                                        }
                                        Ok(Status::Ok) => {
                                            if decompressed_offset >= current_req_range.end as u64 {
                                                tracing::info!(
                                                    "Peer {target_addr}: sub-block reached requested boundary {}",
                                                    decompressed_offset
                                                );
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
                                            tracing::warn!("Peer {target_addr}: Zlib decompression error: {e}");
                                            break;
                                        }
                                    }
                                }

                                let elapsed = last_rate_check.elapsed();
                                let bps = if elapsed >= Duration::from_millis(500) {
                                    let rate = (bytes_since_rate_check as f64 / elapsed.as_secs_f64()) as u64;
                                    bytes_since_rate_check = 0;
                                    last_rate_check = std::time::Instant::now();
                                    rate
                                } else {
                                    inner.lock().await.sessions.get(address_str).map(|s| s.snapshot.transfer_rate_bps).unwrap_or(0)
                                };

                                let progress_snapshot = {
                                    let mut g = inner.lock().await;
                                    if let Some(session) = g.sessions.get_mut(address_str) {
                                        session.snapshot.transferred_bytes = total_received;
                                        session.snapshot.transfer_rate_bps = bps;
                                        Some(session.snapshot.clone())
                                    } else {
                                        None
                                    }
                                };
                                if let Some(snap) = progress_snapshot {
                                    emit_event(event_tx, sequence, &snap);
                                }

                                if elapsed >= Duration::from_millis(500) {
                                    let mut dl_svc = downloads.lock().await;
                                    if let Some(snap) = dl_svc.set_download_rate_by_hash(file_hash_str, bps) {
                                        emit_download_update(event_tx, sequence, &snap);
                                    }
                                }
                            } else if (packet.protocol == PR_ED2K || packet.protocol == PR_EMULE) && packet.opcode == OP_END_OF_DOWNLOAD {
                                info!("Peer {} sent OP_END_OF_DOWNLOAD", target_addr);
                                break;
                            } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
                                && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
                            {
                                handle_sx_answer(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, downloads).await;
                            } else if packet.protocol == PR_EMULE
                                && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
                            {
                                handle_sx_request(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, &encoder, downloads, stream).await;
                            }

                            // Pipeline next block
                            if block_finished {
                                {
                                    let mut g = inner.lock().await;
                                    g.in_flight_blocks.retain(|b| !(b.peer_id == address_str && b.part_index == current_part_idx && b.start == current_req_range.start as u64));
                                }

                                let completed = {
                                    let dl_svc = downloads.lock().await;
                                    dl_svc.get_completed_ranges_by_hash(file_hash_str)
                                };

                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as i64;

                                let next_block = {
                                    let mut g = inner.lock().await;
                                    match sequencer.plan_next_block_for_peer(address_str, &completed, &g.in_flight_blocks, now_ms) {
                                        Ok(Some((next_p, next_r))) => {
                                            let ifb = InFlightBlock::new(
                                                address_str.to_string(),
                                                next_p,
                                                next_r.start as u64,
                                                next_r.end as u64,
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
                                    decompressed_offset = next_r.start as u64;

                                    let req_parts = Ed2kRequestParts {
                                        file_hash: dl_hash_bytes,
                                        ranges: vec![next_r],
                                    };
                                    if let Ok(req_pkt) = encoder.encode_request_parts(&req_parts) {
                                        let mut out_wire = Vec::with_capacity(6 + req_pkt.payload.len());
                                        out_wire.push(req_pkt.protocol);
                                        out_wire.extend_from_slice(&(req_pkt.frame_len() as u32).to_le_bytes());
                                        out_wire.push(req_pkt.opcode);
                                        out_wire.extend_from_slice(&req_pkt.payload);
                                        if let Err(e) = stream.write_all(&out_wire).await {
                                            tracing::warn!("Failed to send chained OP_REQUESTPARTS to {target_addr}: {e}");
                                            break;
                                        }
                                        tracing::info!("Peer {target_addr}: requested next sub-block [{}, {}) in part {}", next_r.start, next_r.end, next_p);
                                        last_activity = tokio::time::Instant::now();
                                    }
                                } else {
                                    tracing::info!("Peer {target_addr}: all blocks completed or no more blocks needed");
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

    // Reset rate
    {
        let mut dl_svc = downloads.lock().await;
        if let Some(snap) = dl_svc.set_download_rate_by_hash(file_hash_str, 0) {
            emit_download_update(event_tx, sequence, &snap);
        }
    }

    Ok(total_received)
}
