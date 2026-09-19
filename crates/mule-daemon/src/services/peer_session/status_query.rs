use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, Mutex};
use tracing::info;

use mule_core::model::PeerFailureReason;
use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kFileStatus, Ed2kFileStatusReq,
    OP_ACCEPTUPLOADREQ, OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_FILEREQANSNOFILE,
    OP_FILESTATUS, OP_QUEUERANKING, OP_REQFILENAME, OP_REQFILENAME_ANSWER, OP_REQUESTSOURCES,
    OP_REQUESTSOURCES2, OP_SETREQFILEID_EMULE, OP_STARTUPLOADREQ, PR_ED2K, PR_EMULE,
    PR_EMULE_COMPRESSED,
};
use mule_protocol::framing::PacketCodec;

use crate::services::downloads::DownloadService;
use crate::services::obfuscation_stream::ObfuscatedStream;
use super::events::emit_event;
use super::source_exchange::{handle_sx_answer, handle_sx_request};
use super::types::{
    PeerConnectionStatus, PeerError, PeerTimeouts, SwarmInnerState, MAX_PEER_READ_BUFFER_SIZE,
};

pub(crate) async fn query_file_status_and_enter_queue(
    stream: &mut ObfuscatedStream<TcpStream>,
    target_addr: SocketAddr,
    file_hash_str: &str,
    dl_hash_bytes: [u8; 16],
    address_str: &str,
    peer_supports_sx: bool,
    peer_supports_sx2: bool,
    timeouts: &PeerTimeouts,
    cancel_token: &mule_network_policy::CancellationToken,
    inner: &Arc<Mutex<SwarmInnerState>>,
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    downloads: &Arc<Mutex<DownloadService>>,
) -> Result<Ed2kFileStatus, PeerError> {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let codec = PacketCodec::new();
    let mut out_wire = Vec::with_capacity(64);
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];

    info!("Peer {} handshake completed. Requesting file status...", target_addr);

    // 4a. Send OP_REQFILENAME
    let file_status_req = Ed2kFileStatusReq {
        file_hash: dl_hash_bytes,
    };
    let req_filename_pkt = encoder.encode_file_status_req_with_opcode(&file_status_req, OP_REQFILENAME).map_err(|e| {
        PeerError::new(PeerFailureReason::InvalidHandshake, format!("Failed to encode OP_REQFILENAME: {e}"))
    })?;

    out_wire.clear();
    out_wire.push(req_filename_pkt.protocol);
    out_wire.extend_from_slice(&(req_filename_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(req_filename_pkt.opcode);
    out_wire.extend_from_slice(&req_filename_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_REQFILENAME: {e}")));
    }

    // 4b. Send OP_SETREQFILEID_EMULE
    let status_pkt = encoder.encode_file_status_req_with_opcode(&file_status_req, OP_SETREQFILEID_EMULE).map_err(|e| {
        PeerError::new(PeerFailureReason::InvalidHandshake, format!("Failed to encode OP_SETREQFILEID_EMULE: {e}"))
    })?;

    out_wire.clear();
    out_wire.push(status_pkt.protocol);
    out_wire.extend_from_slice(&(status_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(status_pkt.opcode);
    out_wire.extend_from_slice(&status_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_SETREQFILEID_EMULE: {e}")));
    }

    // 4c. Send OP_REQUESTSOURCES / OP_REQUESTSOURCES2 if supported
    if peer_supports_sx2 || peer_supports_sx {
        let sx_req = if peer_supports_sx2 {
            mule_protocol::ed2k::Ed2kRequestSources::new_v2(dl_hash_bytes)
        } else {
            mule_protocol::ed2k::Ed2kRequestSources::new_v1(dl_hash_bytes)
        };
        if let Ok(sx_pkt) = encoder.encode_request_sources(&sx_req) {
            out_wire.clear();
            out_wire.push(sx_pkt.protocol);
            out_wire.extend_from_slice(&(sx_pkt.frame_len() as u32).to_le_bytes());
            out_wire.push(sx_pkt.opcode);
            out_wire.extend_from_slice(&sx_pkt.payload);
            if let Err(e) = stream.write_all(&out_wire).await {
                tracing::warn!("Failed to send SX request to {target_addr}: {e}");
            } else {
                tracing::info!("Sent OP_REQUESTSOURCES (version {}) to peer {target_addr}", sx_req.version);
            }
        }
    }

    let queued_snapshot = {
        let mut g = inner.lock().await;
        if let Some(session) = g.sessions.get_mut(address_str) {
            session.snapshot.status = PeerConnectionStatus::Queued;
            Some(session.snapshot.clone())
        } else {
            None
        }
    };
    if let Some(snap) = queued_snapshot {
        emit_event(event_tx, sequence, &snap);
    }

    // 5. Await OP_FILESTATUS or OP_FILEREQANSNOFILE
    let status_deadline = tokio::time::Instant::now() + timeouts.status_timeout;
    let mut file_status_opt = None;

    while file_status_opt.is_none() {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Connection aborted by kill switch"));
            }
            _ = tokio::time::sleep_until(status_deadline) => {
                return Err(PeerError::new(PeerFailureReason::TimeoutStatus, "Timed out responding to OP_FILESTATUSREQ"));
            }
            read_res = stream.read(&mut raw_buf) => {
                match read_res {
                    Ok(0) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Peer closed connection while awaiting file status")),
                    Ok(n) => {
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::CorruptedData, "Peer read buffer exceeded limit awaiting file status"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            if packet.protocol == PR_ED2K || packet.protocol == PR_EMULE {
                                if packet.opcode == OP_FILESTATUS {
                                    let status_res = decoder.decode_file_status(packet.protocol, packet.opcode, &packet.payload, 0);
                                    match status_res {
                                        Ok(status) => {
                                            if status.file_hash != dl_hash_bytes {
                                                return Err(PeerError::new(
                                                    PeerFailureReason::HashMismatch,
                                                    "OP_FILESTATUS returned unexpected file hash".to_string(),
                                                ));
                                            }
                                            file_status_opt = Some(status);
                                            break;
                                        }
                                        Err(e) => return Err(PeerError::new(PeerFailureReason::CorruptedData, format!("Malformed OP_FILESTATUS: {e}"))),
                                    }
                                } else if packet.opcode == OP_REQFILENAME_ANSWER {
                                    tracing::debug!("Peer {target_addr} sent OP_REQFILENAME_ANSWER (len={})", packet.payload.len());
                                } else if packet.opcode == OP_QUEUERANKING {
                                    let rank = if packet.payload.len() >= 4 {
                                        u32::from_le_bytes(packet.payload[..4].try_into().unwrap_or([0, 0, 0, 0]))
                                    } else if packet.payload.len() >= 2 {
                                        u16::from_le_bytes(packet.payload[..2].try_into().unwrap_or([0, 0])) as u32
                                    } else {
                                        0
                                    };
                                    tracing::info!("Peer {target_addr} responded with queue ranking {rank} during status query");
                                    let queue_update = {
                                        let mut g = inner.lock().await;
                                        if let Some(session) = g.sessions.get_mut(address_str) {
                                            session.snapshot.queue_rank = Some(rank);
                                            Some(session.snapshot.clone())
                                        } else {
                                            None
                                        }
                                    };
                                    if let Some(snap) = queue_update {
                                        emit_event(event_tx, sequence, &snap);
                                    }
                                    file_status_opt = Some(mule_protocol::ed2k::Ed2kFileStatus {
                                        file_hash: dl_hash_bytes,
                                        part_count: 0,
                                        availability_mask: mule_protocol::ed2k::PartAvailabilityMask::from_parts(&[], 0),
                                    });
                                    break;
                                } else if packet.opcode == OP_ACCEPTUPLOADREQ {
                                    tracing::info!("Peer {target_addr} directly accepted upload slot (OP_ACCEPTUPLOADREQ) during status query!");
                                    file_status_opt = Some(mule_protocol::ed2k::Ed2kFileStatus {
                                        file_hash: dl_hash_bytes,
                                        part_count: 0,
                                        availability_mask: mule_protocol::ed2k::PartAvailabilityMask::from_parts(&[], 0),
                                    });
                                    break;
                                } else if packet.opcode == OP_FILEREQANSNOFILE {
                                    return Err(PeerError::new(PeerFailureReason::FileNotFound, "Remote peer does not share this file (OP_FILEREQANSNOFILE)"));
                                } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
                                    && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
                                {
                                    handle_sx_answer(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, downloads).await;
                                } else if packet.protocol == PR_EMULE
                                    && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
                                {
                                    handle_sx_request(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, &encoder, downloads, stream).await;
                                }
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error awaiting file status: {e}"))),
                }
            }
        }
    }

    let file_status = file_status_opt.unwrap();

    // Mark source as Connected
    {
        let mut dl_svc = downloads.lock().await;
        let _ = dl_svc.update_peer_source_status(
            file_hash_str,
            address_str,
            mule_core::model::PeerSourceStatus::Connected,
            None,
            None,
        );
    }

    // 6. Send OP_STARTUPLOADREQ
    info!("Requesting upload slot via OP_STARTUPLOADREQ for file {} from peer {}", file_hash_str, target_addr);
    out_wire.clear();
    out_wire.push(PR_ED2K);
    out_wire.extend_from_slice(&(17u32).to_le_bytes());
    out_wire.push(OP_STARTUPLOADREQ);
    out_wire.extend_from_slice(&dl_hash_bytes);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send OP_STARTUPLOADREQ: {e}")));
    }

    // 7. Wait in upload queue
    let mut last_queue_activity = tokio::time::Instant::now();
    let mut last_keepalive_sent = tokio::time::Instant::now();
    let keepalive_interval = Duration::from_secs(60);

    'queue_loop: loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Connection aborted by kill switch while in queue"));
            }
            _ = tokio::time::sleep_until(last_queue_activity + timeouts.queue_timeout) => {
                return Err(PeerError::new(PeerFailureReason::TimeoutStatus, "Timed out waiting in queue without updates"));
            }
            _ = tokio::time::sleep_until(last_keepalive_sent + keepalive_interval) => {
                let status_pkt = encoder.encode_file_status_req(&file_status_req).map_err(|e| {
                    PeerError::new(PeerFailureReason::CorruptedData, format!("Failed to encode keep-alive: {e}"))
                })?;
                out_wire.clear();
                out_wire.push(status_pkt.protocol);
                out_wire.extend_from_slice(&(status_pkt.frame_len() as u32).to_le_bytes());
                out_wire.push(status_pkt.opcode);
                out_wire.extend_from_slice(&status_pkt.payload);

                if let Err(e) = stream.write_all(&out_wire).await {
                    return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send keep-alive: {e}")));
                }
                last_keepalive_sent = tokio::time::Instant::now();
            }
            read_res = stream.read(&mut raw_buf) => {
                match read_res {
                    Ok(0) => {
                        let rank = inner.lock().await.sessions.get(address_str).and_then(|s| s.snapshot.queue_rank);
                        if let Some(r) = rank {
                            info!("Peer {target_addr} closed connection while we are in queue (rank {r}). Scheduled re-ask in 15 minutes.");
                            return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Disconnected in queue"));
                        } else {
                            return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Peer closed connection while in queue"));
                        }
                    }
                    Ok(n) => {
                        last_queue_activity = tokio::time::Instant::now();
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::CorruptedData, "Peer read buffer exceeded limit while in queue"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            if packet.protocol == PR_ED2K && packet.opcode == OP_ACCEPTUPLOADREQ {
                                info!("Upload slot accepted (OP_ACCEPTUPLOADREQ) by peer {}", target_addr);
                                break 'queue_loop;
                            } else if (packet.protocol == PR_ED2K || packet.protocol == PR_EMULE) && packet.opcode == OP_QUEUERANKING {
                                let rank = if packet.payload.len() >= 4 {
                                    u32::from_le_bytes(packet.payload[..4].try_into().unwrap_or([0, 0, 0, 0]))
                                } else if packet.payload.len() >= 2 {
                                    u16::from_le_bytes(packet.payload[..2].try_into().unwrap_or([0, 0])) as u32
                                } else {
                                    0
                                };
                                info!("Queue rank from peer {}: {}", target_addr, rank);
                                let queue_update = {
                                    let mut g = inner.lock().await;
                                    if let Some(session) = g.sessions.get_mut(address_str) {
                                        session.snapshot.queue_rank = Some(rank);
                                        Some(session.snapshot.clone())
                                    } else {
                                        None
                                    }
                                };
                                if let Some(snap) = queue_update {
                                    emit_event(event_tx, sequence, &snap);
                                }
                            } else if packet.protocol == PR_ED2K && packet.opcode == OP_FILESTATUS {
                                tracing::debug!("Peer {target_addr}: received keep-alive OP_FILESTATUS response");
                            } else if packet.protocol == PR_ED2K && packet.opcode == OP_FILEREQANSNOFILE {
                                return Err(PeerError::new(PeerFailureReason::FileNotFound, "Remote peer no longer shares this file"));
                            } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
                                && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
                            {
                                handle_sx_answer(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, downloads).await;
                            } else if packet.protocol == PR_EMULE
                                && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
                            {
                                handle_sx_request(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, &encoder, downloads, stream).await;
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error while in queue: {e}"))),
                }
            }
        }
    }

    Ok(file_status)
}
