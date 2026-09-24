// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use crate::services::peer_session::log_peer_addr;
use bytes::BytesMut;
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
    Ed2kAichFileHashReq, Ed2kAichRequest, Ed2kDecoder, Ed2kEncoder, Ed2kFileStatus,
    Ed2kFileStatusReq, Ed2kHashSetReq, OP_ACCEPTUPLOADREQ, OP_AICHFILEHASHRES, OP_AICH_ANSWER,
    OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_FILEREQANSNOFILE, OP_FILESTATUS, OP_HASHSETANSWER,
    OP_QUEUERANKING, OP_REQFILENAME, OP_REQFILENAME_ANSWER, OP_REQUESTSOURCES, OP_REQUESTSOURCES2,
    OP_SETREQFILEID_EMULE, OP_STARTUPLOADREQ, PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED,
};
use mule_protocol::framing::{PacketCodec, RawPacket};

use super::events::{emit_download_update, emit_event};
use super::source_exchange::{
    handle_sx_answer, handle_sx_request, maybe_send_sx_request, PeerSxCaps, SxEnv, SxLocalIds,
};
use super::types::{
    PeerConnectionStatus, PeerError, PeerTimeouts, SwarmInnerState, MAX_PEER_READ_BUFFER_SIZE,
};
use crate::services::downloads::DownloadService;
use crate::services::obfuscation_stream::ObfuscatedStream;

/// Handles an `OP_HASHSETANSWER` packet arriving in any peer session loop:
/// validates the file hash and persists the per-part MD4 hashset.
/// Returns true if the packet was a hashset answer and has been consumed.
pub(crate) async fn handle_hashset_answer(
    packet: &RawPacket,
    dl_hash_bytes: [u8; 16],
    file_hash_str: &str,
    target_addr: SocketAddr,
    decoder: &Ed2kDecoder,
    downloads: &Arc<Mutex<DownloadService>>,
) -> bool {
    if packet.protocol != PR_ED2K || packet.opcode != OP_HASHSETANSWER {
        return false;
    }

    match decoder.decode_hashset_answer(packet.protocol, packet.opcode, &packet.payload, 0) {
        Ok(ans) => {
            if ans.file_hash != dl_hash_bytes {
                tracing::warn!(
                    "Peer {peer} sent OP_HASHSETANSWER for a different file hash, ignoring",
                    peer = log_peer_addr(target_addr)
                );
                return true;
            }
            let mut dl_svc = downloads.lock().await;
            if let Err(e) = dl_svc.store_part_hashes_by_hash(file_hash_str, &ans.part_hashes) {
                tracing::warn!(
                    "Failed to persist hashset from {peer} for {file_hash_str}: {e}",
                    peer = log_peer_addr(target_addr)
                );
            } else {
                info!(
                    "Stored hashset ({} part hashes) for {} from peer {}",
                    ans.part_hashes.len(),
                    file_hash_str,
                    log_peer_addr(target_addr)
                );
            }
            true
        }
        Err(e) => {
            tracing::warn!(
                "Malformed OP_HASHSETANSWER from {peer}: {e}",
                peer = log_peer_addr(target_addr)
            );
            true
        }
    }
}

/// Masks a peer IP like eMule `CAICHUntrustedHash` (`dwIP &= 0x00F0FFFF`),
/// so multiple clients behind the same address range count as one vote.
fn mask_peer_ip(addr: SocketAddr) -> u32 {
    match addr.ip() {
        std::net::IpAddr::V4(v4) => u32::from_le_bytes(v4.octets()) & 0x00F0FFFF,
        std::net::IpAddr::V6(v6) => {
            u32::from_le_bytes(v6.octets()[..4].try_into().unwrap_or_default()) & 0x00F0FFFF
        }
    }
}

/// Writes a framed eMule packet onto the peer stream.
async fn write_packet(
    stream: &mut ObfuscatedStream<TcpStream>,
    pkt: &RawPacket,
) -> std::io::Result<()> {
    let mut out_wire = Vec::with_capacity(pkt.payload.len() + 6);
    out_wire.push(pkt.protocol);
    out_wire.extend_from_slice(&(pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(pkt.opcode);
    out_wire.extend_from_slice(&pkt.payload);
    stream.write_all(&out_wire).await
}

/// Sends `OP_AICHREQUEST` for every corrupted part awaiting AICH recovery on
/// this file (eMule `CDownloadClient::SendAICHRequest`). No-op when the peer
/// does not advertise AICH or no root is trusted yet.
pub(crate) async fn send_pending_aich_requests(
    stream: &mut ObfuscatedStream<TcpStream>,
    encoder: &Ed2kEncoder,
    target_addr: SocketAddr,
    file_hash_str: &str,
    dl_hash_bytes: [u8; 16],
    peer_supports_aich: bool,
    downloads: &Arc<Mutex<DownloadService>>,
) {
    if !peer_supports_aich {
        return;
    }
    loop {
        let next = {
            let mut dl_svc = downloads.lock().await;
            dl_svc.next_aich_recovery_by_hash(file_hash_str)
        };
        let Some((part, master)) = next else { break };
        let req = Ed2kAichRequest {
            file_hash: dl_hash_bytes,
            part: part as u16,
            master_hash: master,
        };
        match encoder.encode_aich_request(&req) {
            Ok(pkt) => {
                if let Err(e) = write_packet(stream, &pkt).await {
                    tracing::warn!(
                        "Failed to send OP_AICHREQUEST to {peer}: {e}",
                        peer = log_peer_addr(target_addr)
                    );
                    let mut dl_svc = downloads.lock().await;
                    if let Some(snap) = dl_svc.find_by_ed2k_hash(file_hash_str) {
                        dl_svc.fail_aich_recovery(&snap.id, part);
                    }
                    break;
                }
                tracing::info!(
                    "Sent OP_AICHREQUEST for part {part} of {file_hash_str} to {peer}",
                    peer = log_peer_addr(target_addr)
                );
            }
            Err(e) => {
                tracing::warn!("Failed to encode OP_AICHREQUEST: {e}");
                break;
            }
        }
    }
}

/// Handles `OP_AICHFILEHASHANS` (0x9D): records a master-hash vote. Once the
/// root reaches consensus, pending recovery requests are sent to this peer.
/// Returns true if the packet was consumed.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_aich_file_hash_res(
    stream: &mut ObfuscatedStream<TcpStream>,
    packet: &RawPacket,
    dl_hash_bytes: [u8; 16],
    file_hash_str: &str,
    target_addr: SocketAddr,
    decoder: &Ed2kDecoder,
    encoder: &Ed2kEncoder,
    peer_supports_aich: bool,
    downloads: &Arc<Mutex<DownloadService>>,
) -> bool {
    if packet.protocol != PR_EMULE || packet.opcode != OP_AICHFILEHASHRES {
        return false;
    }
    match decoder.decode_aich_file_hash_res(packet.protocol, packet.opcode, &packet.payload, 0) {
        Ok(res) => {
            if res.file_hash != dl_hash_bytes {
                tracing::warn!(
                    "Peer {peer} sent OP_AICHFILEHASHANS for a different file hash, ignoring",
                    peer = log_peer_addr(target_addr)
                );
                return true;
            }
            let outcome = {
                let mut dl_svc = downloads.lock().await;
                dl_svc.find_by_ed2k_hash(file_hash_str).map(|snap| {
                    dl_svc.record_aich_root_vote(&snap.id, res.aich_hash, mask_peer_ip(target_addr))
                })
            };
            match outcome {
                Some(crate::services::downloads::operations::AichVoteOutcome::Trusted) => {
                    tracing::info!(
                        "AICH master hash for {file_hash_str} confirmed by {peer}",
                        peer = log_peer_addr(target_addr)
                    );
                    send_pending_aich_requests(
                        stream,
                        encoder,
                        target_addr,
                        file_hash_str,
                        dl_hash_bytes,
                        peer_supports_aich,
                        downloads,
                    )
                    .await;
                }
                Some(crate::services::downloads::operations::AichVoteOutcome::Mismatch) => {
                    tracing::warn!(
                        "Peer {peer} reported a different AICH master hash for {file_hash_str}",
                        peer = log_peer_addr(target_addr)
                    );
                }
                Some(crate::services::downloads::operations::AichVoteOutcome::Untrusted) => {
                    tracing::debug!(
                        "AICH root vote from {peer} recorded for {file_hash_str} (no consensus yet)"
                    , peer = log_peer_addr(target_addr));
                }
                None => {}
            }
            true
        }
        Err(e) => {
            tracing::warn!(
                "Malformed OP_AICHFILEHASHANS from {peer}: {e}",
                peer = log_peer_addr(target_addr)
            );
            true
        }
    }
}

/// Handles `OP_AICHANSWER` (0x9C): applies recovery data to the corrupted
/// part (block-level eviction) or requeues the part on a failed answer.
/// Returns true if the packet was consumed.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_aich_answer(
    stream: &mut ObfuscatedStream<TcpStream>,
    packet: &RawPacket,
    dl_hash_bytes: [u8; 16],
    file_hash_str: &str,
    target_addr: SocketAddr,
    decoder: &Ed2kDecoder,
    encoder: &Ed2kEncoder,
    peer_supports_aich: bool,
    downloads: &Arc<Mutex<DownloadService>>,
) -> bool {
    if packet.protocol != PR_EMULE || packet.opcode != OP_AICH_ANSWER {
        return false;
    }
    match decoder.decode_aich_answer(packet.protocol, packet.opcode, &packet.payload, 0) {
        Ok(ans) => {
            if ans.file_hash != dl_hash_bytes {
                tracing::warn!(
                    "Peer {peer} sent OP_AICHANSWER for a different file hash, ignoring",
                    peer = log_peer_addr(target_addr)
                );
                return true;
            }
            let mut dl_svc = downloads.lock().await;
            let dl_id = dl_svc.find_by_ed2k_hash(file_hash_str).map(|s| s.id);
            match (dl_id, ans.recovery) {
                (Some(id), Some(rec)) => {
                    let entries: Vec<mule_format::AichRecoveryEntry> = rec
                        .entries16
                        .iter()
                        .map(|&(ident, hash)| mule_format::AichRecoveryEntry {
                            ident: ident as u32,
                            hash: mule_format::AichHash(hash),
                        })
                        .chain(rec.entries32.iter().map(|&(ident, hash)| {
                            mule_format::AichRecoveryEntry {
                                ident,
                                hash: mule_format::AichHash(hash),
                            }
                        }))
                        .collect();
                    match dl_svc.apply_aich_recovery(&id, rec.part as u32, rec.master_hash, entries)
                    {
                        Ok(report) => {
                            if report.corrupted_blocks == 0 {
                                tracing::debug!(
                                    "AICH recovery from {peer}: part {} of {file_hash_str} fully verified",
                                    report.part
                                , peer = log_peer_addr(target_addr));
                            } else {
                                tracing::debug!(
                                    "AICH recovery from {peer}: part {} of {file_hash_str}, {} corrupted block(s) evicted",
                                    report.part, report.corrupted_blocks
                                , peer = log_peer_addr(target_addr));
                            }
                        }
                        Err(e) => {
                            tracing::warn!(
                                "AICH recovery from {peer} rejected: {e}",
                                peer = log_peer_addr(target_addr)
                            );
                        }
                    }
                }
                (Some(id), None) => {
                    tracing::warn!(
                        "Peer {peer} could not serve AICH recovery for {file_hash_str}",
                        peer = log_peer_addr(target_addr)
                    );
                    // The part index is not echoed in a failed answer; fail the
                    // most recently sent request.
                    dl_svc.fail_last_aich_request(&id);
                }
                (None, _) => {}
            }
            drop(dl_svc);
            send_pending_aich_requests(
                stream,
                encoder,
                target_addr,
                file_hash_str,
                dl_hash_bytes,
                peer_supports_aich,
                downloads,
            )
            .await;
            true
        }
        Err(e) => {
            tracing::warn!(
                "Malformed OP_AICHANSWER from {peer}: {e}",
                peer = log_peer_addr(target_addr)
            );
            true
        }
    }
}

pub(crate) async fn query_file_status_and_enter_queue(
    stream: &mut ObfuscatedStream<TcpStream>,
    target_addr: SocketAddr,
    file_hash_str: &str,
    dl_hash_bytes: [u8; 16],
    address_str: &str,
    sx_caps: &PeerSxCaps,
    peer_supports_aich: bool,
    peer_ext_req: u8,
    timeouts: &PeerTimeouts,
    cancel_token: &mule_network_policy::CancellationToken,
    inner: &Arc<Mutex<SwarmInnerState>>,
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    downloads: &Arc<Mutex<DownloadService>>,
    sx_env: &SxEnv,
    sx_local: &SxLocalIds,
    sui: &mut super::sui::OutboundSui,
) -> Result<Ed2kFileStatus, PeerError> {
    let encoder = Ed2kEncoder::default();
    // ExtendedRequestsVersion négociée au handshake (OP_EMULEINFO) — pilote le
    // parsing déterministe de l'ext-info `0x58` dans les MULTIPACKET*.
    let decoder = Ed2kDecoder::default().with_extended_requests_version(peer_ext_req);
    let codec = PacketCodec::new();
    let mut out_wire = Vec::with_capacity(64);
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];

    info!(
        "Peer {} handshake completed. Requesting file status for {file_hash_str}...",
        log_peer_addr(target_addr)
    );

    // 4a. Send OP_REQFILENAME. Quand le pair a négocié ExtendedRequestsVersion
    // > 0, eMule lit l'ext-info (part status + complete sources) de façon
    // inconditionnelle après le hash (UploadClient.cpp ProcessExtendedInfo) :
    // un paquet nu provoque une exception filaire et la fermeture de connexion.
    let file_status_req = Ed2kFileStatusReq {
        file_hash: dl_hash_bytes,
    };
    // Nombre de parts du fichier ciblé — eMule saute `OP_SETREQFILEID`
    // pour les fichiers ≤ 1 part (`DownloadClient.cpp:362-366`) : la
    // réponse `OP_REQFILENAMEANSWER` suffit à établir la complétude.
    let file_part_count = {
        let dl_svc = downloads.lock().await;
        let total_bytes = dl_svc
            .find_by_ed2k_hash(file_hash_str)
            .map(|d| d.total_bytes)
            .unwrap_or(0);
        if total_bytes == 0 {
            0
        } else {
            total_bytes.div_ceil(mule_protocol::ed2k::ED2K_PART_SIZE as u64) as u16
        }
    };

    let req_filename_pkt = if peer_ext_req > 0 {
        // Notre propre part-status : parts dont la plage complète est déjà
        // téléchargée (comme PartFile::WritePartStatus chez eMule).
        let (total_bytes, completed_ranges) = {
            let dl_svc = downloads.lock().await;
            (
                dl_svc
                    .find_by_ed2k_hash(file_hash_str)
                    .map(|d| d.total_bytes)
                    .unwrap_or(0),
                dl_svc.get_completed_ranges_by_hash(file_hash_str),
            )
        };
        let part_size = mule_protocol::ed2k::ED2K_PART_SIZE as u64;
        let part_count = if total_bytes > 0 {
            total_bytes.div_ceil(part_size) as u16
        } else {
            0
        };
        let available: Vec<u16> = (0..part_count)
            .filter(|&p| {
                let start = u64::from(p) * part_size;
                let end = (start + part_size).min(total_bytes);
                completed_ranges
                    .iter()
                    .any(|&(rs, re)| rs <= start && re >= end)
            })
            .collect();
        let part_status =
            mule_protocol::ed2k::PartAvailabilityMask::from_parts(&available, part_count);
        // Complete sources : non suivi par part ; 0 est licite (inconnu).
        mule_protocol::ed2k::encoder::encode_req_filename(
            &encoder.limits,
            &dl_hash_bytes,
            &part_status,
            0,
            peer_ext_req,
        )
        .map_err(|e| {
            PeerError::new(
                PeerFailureReason::InvalidHandshake,
                format!("Failed to encode OP_REQFILENAME: {e}"),
            )
        })?
    } else {
        encoder
            .encode_file_status_req_with_opcode(&file_status_req, OP_REQFILENAME)
            .map_err(|e| {
                PeerError::new(
                    PeerFailureReason::InvalidHandshake,
                    format!("Failed to encode OP_REQFILENAME: {e}"),
                )
            })?
    };

    out_wire.clear();
    out_wire.push(req_filename_pkt.protocol);
    out_wire.extend_from_slice(&(req_filename_pkt.frame_len() as u32).to_le_bytes());
    out_wire.push(req_filename_pkt.opcode);
    out_wire.extend_from_slice(&req_filename_pkt.payload);

    if let Err(e) = stream.write_all(&out_wire).await {
        return Err(PeerError::new(
            PeerFailureReason::ConnectRefused,
            format!("Failed to send OP_REQFILENAME: {e}"),
        ));
    }

    // 4b. Send OP_SETREQFILEID_EMULE — omis pour les fichiers ≤ 1 part
    // (eMule `SendFileRequest`, `DownloadClient.cpp:366`) : un pair qui
    // répond `OP_REQFILENAMEANSWER` possède alors le fichier complet.
    if file_part_count != 1 {
        let status_pkt = encoder
            .encode_file_status_req_with_opcode(&file_status_req, OP_SETREQFILEID_EMULE)
            .map_err(|e| {
                PeerError::new(
                    PeerFailureReason::InvalidHandshake,
                    format!("Failed to encode OP_SETREQFILEID_EMULE: {e}"),
                )
            })?;

        out_wire.clear();
        out_wire.push(status_pkt.protocol);
        out_wire.extend_from_slice(&(status_pkt.frame_len() as u32).to_le_bytes());
        out_wire.push(status_pkt.opcode);
        out_wire.extend_from_slice(&status_pkt.payload);

        if let Err(e) = stream.write_all(&out_wire).await {
            return Err(PeerError::new(
                PeerFailureReason::ConnectRefused,
                format!("Failed to send OP_SETREQFILEID_EMULE: {e}"),
            ));
        }
    }

    // 4b'. Request the per-part MD4 hashset (OP_HASHSETREQUEST) when unknown,
    // so completed parts can be verified incrementally (eMule PartFile hashset).
    let hashset_known = {
        let dl_svc = downloads.lock().await;
        !dl_svc.get_part_hashes_by_hash(file_hash_str).is_empty()
    };
    if !hashset_known {
        let hs_req = Ed2kHashSetReq {
            file_hash: dl_hash_bytes,
        };
        match encoder.encode_hashset_request(&hs_req) {
            Ok(hs_pkt) => {
                out_wire.clear();
                out_wire.push(hs_pkt.protocol);
                out_wire.extend_from_slice(&(hs_pkt.frame_len() as u32).to_le_bytes());
                out_wire.push(hs_pkt.opcode);
                out_wire.extend_from_slice(&hs_pkt.payload);
                if let Err(e) = stream.write_all(&out_wire).await {
                    tracing::warn!(
                        "Failed to send OP_HASHSETREQUEST to {peer}: {e}",
                        peer = log_peer_addr(target_addr)
                    );
                } else {
                    tracing::info!(
                        "Sent OP_HASHSETREQUEST to peer {peer} for {file_hash_str}",
                        peer = log_peer_addr(target_addr)
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to encode OP_HASHSETREQUEST: {e}");
            }
        }
    }

    // 4b''. Request the AICH master hash (OP_AICHFILEHASHREQ, eMule protocol)
    // when the peer advertises AICH support and no trusted root is known yet.
    if peer_supports_aich {
        let aich_needed = {
            let dl_svc = downloads.lock().await;
            dl_svc
                .find_by_ed2k_hash(file_hash_str)
                .and_then(|snap| dl_svc.trusted_aich_root(&snap.id))
                .is_none()
        };
        if aich_needed {
            let req = Ed2kAichFileHashReq {
                file_hash: dl_hash_bytes,
            };
            match encoder.encode_aich_file_hash_req(&req) {
                Ok(pkt) => {
                    if let Err(e) = write_packet(stream, &pkt).await {
                        tracing::warn!(
                            "Failed to send OP_AICHFILEHASHREQ to {peer}: {e}",
                            peer = log_peer_addr(target_addr)
                        );
                    } else {
                        tracing::info!(
                            "Sent OP_AICHFILEHASHREQ to peer {peer} for {file_hash_str}",
                            peer = log_peer_addr(target_addr)
                        );
                    }
                }
                Err(e) => tracing::warn!("Failed to encode OP_AICHFILEHASHREQ: {e}"),
            }
        }
        // A trusted root may already exist while corrupted parts wait for
        // recovery data — send the pending OP_AICHREQUESTs now.
        send_pending_aich_requests(
            stream,
            &encoder,
            target_addr,
            file_hash_str,
            dl_hash_bytes,
            peer_supports_aich,
            downloads,
        )
        .await;
    }

    // 4c. OP_REQUESTSOURCES / OP_REQUESTSOURCES2 — eMule envoie la requête
    // SX quand `IsSourceRequestAllowed` passe (`DownloadClient.cpp:212-255`) :
    // pas de demande aux sources complètes, pas de spam (40 min/client, 5 min
    // fichier rare / 20 min fichier commun), cap souple de sources.
    maybe_send_sx_request(
        stream,
        &encoder,
        target_addr,
        file_hash_str,
        dl_hash_bytes,
        downloads,
        sx_env,
        sx_caps,
    )
    .await;

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
                            if sui.handle(&packet, target_addr, &decoder, &encoder, stream).await {
                                continue;
                            }
                            if handle_hashset_answer(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, downloads).await {
                                continue;
                            }
                            if handle_aich_file_hash_res(stream, &packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, &encoder, peer_supports_aich, downloads).await {
                                continue;
                            }
                            if handle_aich_answer(stream, &packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, &encoder, peer_supports_aich, downloads).await {
                                continue;
                            }
                            if packet.protocol == PR_ED2K || packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED {
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
                                            // Propager le bitmap de parts du pair au store
                                            // swarm (`act.sources`) — il alimente
                                            // `available_source_count` (eMule
                                            // `m_SrcPartFrequency` / `DrawStatusBar`).
                                            // `part_count == 0` = source complète.
                                            let mut dl_svc = downloads.lock().await;
                                            if let Some(dl_id) = dl_svc
                                                .find_by_ed2k_hash(file_hash_str)
                                                .map(|s| s.id)
                                            {
                                                if let Ok(snap) = dl_svc.record_source_availability(
                                                    &dl_id,
                                                    target_addr,
                                                    status.availability_mask.clone(),
                                                ) {
                                                    emit_download_update(event_tx, sequence, &snap);
                                                }
                                            }
                                            let mask = status.availability_mask.raw_bytes();
                                            if !mask.is_empty() {
                                                let _ = dl_svc.update_peer_source_details(
                                                    file_hash_str,
                                                    address_str,
                                                    None,
                                                    Some(mask),
                                                );
                                            }
                                            file_status_opt = Some(status);
                                            break;
                                        }
                                        Err(e) => return Err(PeerError::new(PeerFailureReason::CorruptedData, format!("Malformed OP_FILESTATUS: {e}"))),
                                    }
                                } else if packet.opcode == OP_REQFILENAME_ANSWER {
                                    // Fichier ≤ 1 part : `OP_SETREQFILEID`
                                    // n'a pas été envoyé — la réponse au nom
                                    // vaut statut « source complète » (eMule
                                    // `DownloadClient.cpp:362-365`).
                                    if file_part_count == 1
                                        && packet.payload.len() >= 16
                                        && packet.payload[..16] == dl_hash_bytes
                                    {
                                        file_status_opt = Some(mule_protocol::ed2k::Ed2kFileStatus {
                                            file_hash: dl_hash_bytes,
                                            part_count: 0,
                                            availability_mask: mule_protocol::ed2k::PartAvailabilityMask::from_parts(&[], 0),
                                        });
                                        break;
                                    }
                                    tracing::debug!("Peer {peer} sent OP_REQFILENAME_ANSWER (len={})", packet.payload.len(), peer = log_peer_addr(target_addr));
                                } else if packet.opcode == OP_QUEUERANKING {
                                    let rank = if packet.payload.len() >= 4 {
                                        u32::from_le_bytes(packet.payload[..4].try_into().unwrap_or([0, 0, 0, 0]))
                                    } else if packet.payload.len() >= 2 {
                                        u16::from_le_bytes(packet.payload[..2].try_into().unwrap_or([0, 0])) as u32
                                    } else {
                                        0
                                    };
                                    tracing::debug!("Peer {peer} responded with queue ranking {rank} during status query", peer = log_peer_addr(target_addr));
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
                                    tracing::info!("Peer {peer} directly accepted upload slot (OP_ACCEPTUPLOADREQ) during status query!", peer = log_peer_addr(target_addr));
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
                                    handle_sx_answer(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, downloads, sx_env, sx_local).await;
                                } else if packet.protocol == PR_EMULE
                                    && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
                                {
                                    handle_sx_request(&packet, Some(dl_hash_bytes), target_addr, &decoder, &encoder, downloads, stream, sx_env, sx_caps, None).await;
                                } else if packet.protocol == PR_EMULE
                                {
                                    super::source_exchange::handle_sx_multipacket(
                                        &packet,
                                        dl_hash_bytes,
                                        None,
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
                            }
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error awaiting file status: {e}"))),
                }
            }
        }
    }

    let file_status = file_status_opt.unwrap();

    // eMule "No Needed Parts": if the peer advertises a part mask but none
    // of its parts is still missing locally, the source is useless for this
    // download — drop it now instead of occupying an upload slot. Complete
    // sources (part_count == 0) and queue-accepted sessions bypass the check.
    if !file_status.is_complete() && file_status.part_count > 0 {
        let has_needed_part = {
            let dl_svc = downloads.lock().await;
            let completed = dl_svc.get_completed_ranges_by_hash(file_hash_str);
            let total_size = dl_svc
                .find_by_ed2k_hash(file_hash_str)
                .map(|d| d.total_bytes)
                .unwrap_or(0);
            let part_size = mule_protocol::ed2k::ED2K_PART_SIZE as u64;
            let total_parts = if total_size == 0 {
                0
            } else {
                total_size.div_ceil(part_size).max(1) as u32
            };
            (0..total_parts).any(|p| {
                u16::try_from(p).is_ok_and(|pi| file_status.availability_mask.has_part(pi)) && {
                    let start = p as u64 * part_size;
                    let end = ((p as u64 + 1) * part_size).min(total_size);
                    completed.iter().fold(start, |acc, &(s, e)| {
                        if s <= acc && e > acc {
                            acc.max(e.min(end))
                        } else {
                            acc
                        }
                    }) < end
                }
            })
        };
        if !has_needed_part {
            return Err(PeerError::new(
                PeerFailureReason::NoNeededParts,
                "Peer has the file but no needed parts (eMule NNS)",
            ));
        }
    }

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
    send_start_upload_req(stream, dl_hash_bytes, file_hash_str, target_addr).await?;

    // 7. Wait in upload queue (eMule DS_ONQUEUE + FILEREASKTIME re-asks)
    wait_in_queue(
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
        file_status,
        false,
    )
    .await
}

/// eMule `OP_STARTUPLOADREQ` (hash payload) — (re)enters the remote upload
/// queue for `file_hash`. Repeated at each `FILEREASKTIME` re-ask so the
/// remote `LastUpRequest` stays fresh (purge horizon is 1 hour).
pub(crate) async fn send_start_upload_req(
    stream: &mut ObfuscatedStream<TcpStream>,
    dl_hash_bytes: [u8; 16],
    file_hash_str: &str,
    target_addr: SocketAddr,
) -> Result<(), PeerError> {
    info!(
        "Requesting upload slot via OP_STARTUPLOADREQ for file {} from peer {}",
        file_hash_str,
        log_peer_addr(target_addr)
    );
    let mut out_wire = Vec::with_capacity(23);
    out_wire.push(PR_ED2K);
    out_wire.extend_from_slice(&(17u32).to_le_bytes());
    out_wire.push(OP_STARTUPLOADREQ);
    out_wire.extend_from_slice(&dl_hash_bytes);
    stream.write_all(&out_wire).await.map_err(|e| {
        PeerError::new(
            PeerFailureReason::ConnectRefused,
            format!("Failed to send OP_STARTUPLOADREQ: {e}"),
        )
    })
}

/// eMule file re-ask cadence (`FILEREASKTIME`, Opcodes.h): while queued, the
/// client resends the file-request bundle + `OP_STARTUPLOADREQ` every
/// ~29 minutes. The remote uploader purges entries whose `LastUpRequest` is
/// older than `MAX_PURGEQUEUETIME` (1 h), so this keep-alive is what actually
/// preserves the queue position — a bare `OP_SETREQFILEID` does NOT refresh it.
pub(crate) const FILE_REASK_INTERVAL: Duration = Duration::from_secs(29 * 60);

/// Waits for `OP_ACCEPTUPLOADREQ` on the remote upload queue — eMule
/// `DS_ONQUEUE`. Shared between the initial status query and the
/// stall-recovery path (`CheckDownloadTimeout` → `SendCancelTransfer` → back
/// to queue on the same socket).
///
/// `requeue_now` resends `OP_SETREQFILEID` + `OP_STARTUPLOADREQ` immediately
/// (post-stall re-entry); the initial path already sent them before calling.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn wait_in_queue(
    stream: &mut ObfuscatedStream<TcpStream>,
    target_addr: SocketAddr,
    file_hash_str: &str,
    dl_hash_bytes: [u8; 16],
    address_str: &str,
    sx_caps: &PeerSxCaps,
    timeouts: &PeerTimeouts,
    cancel_token: &mule_network_policy::CancellationToken,
    inner: &Arc<Mutex<SwarmInnerState>>,
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    downloads: &Arc<Mutex<DownloadService>>,
    sx_env: &SxEnv,
    sx_local: &SxLocalIds,
    sui: &mut super::sui::OutboundSui,
    peer_ext_req: u8,
    peer_supports_aich: bool,
    mut file_status: Ed2kFileStatus,
    requeue_now: bool,
) -> Result<Ed2kFileStatus, PeerError> {
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default().with_extended_requests_version(peer_ext_req);
    let codec = PacketCodec::new();
    let mut out_wire = Vec::with_capacity(64);
    let mut read_buf = BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let file_status_req = Ed2kFileStatusReq {
        file_hash: dl_hash_bytes,
    };

    // eMule saute `OP_SETREQFILEID` pour les fichiers ≤ 1 part
    // (`DownloadClient.cpp:366`) — à chaque cycle de reask, pas seulement
    // à la requête initiale.
    let file_part_count = {
        let dl_svc = downloads.lock().await;
        let total_bytes = dl_svc
            .find_by_ed2k_hash(file_hash_str)
            .map(|d| d.total_bytes)
            .unwrap_or(0);
        if total_bytes == 0 {
            0
        } else {
            total_bytes.div_ceil(mule_protocol::ed2k::ED2K_PART_SIZE as u64) as u16
        }
    };

    if requeue_now {
        if file_part_count != 1 {
            let status_pkt = encoder
                .encode_file_status_req(&file_status_req)
                .map_err(|e| {
                    PeerError::new(
                        PeerFailureReason::CorruptedData,
                        format!("Failed to encode file status re-ask: {e}"),
                    )
                })?;
            out_wire.clear();
            out_wire.push(status_pkt.protocol);
            out_wire.extend_from_slice(&(status_pkt.frame_len() as u32).to_le_bytes());
            out_wire.push(status_pkt.opcode);
            out_wire.extend_from_slice(&status_pkt.payload);
            if let Err(e) = stream.write_all(&out_wire).await {
                return Err(PeerError::new(
                    PeerFailureReason::ConnectRefused,
                    format!("Failed to send file status re-ask: {e}"),
                ));
            }
        }
        send_start_upload_req(stream, dl_hash_bytes, file_hash_str, target_addr).await?;
    }

    // eMule re-ask cycle: resend the request bundle every FILE_REASK_INTERVAL;
    // the peer is considered dead when nothing is heard for a whole
    // re-ask interval + `queue_timeout` (a remote answering our re-asks keeps
    // this alive indefinitely — eMule has no queue timeout at all).
    let mut next_reask = tokio::time::Instant::now() + FILE_REASK_INTERVAL;
    let mut activity_deadline =
        tokio::time::Instant::now() + FILE_REASK_INTERVAL + timeouts.queue_timeout;

    'queue_loop: loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(PeerError::new(PeerFailureReason::KillSwitchAborted, "Connection aborted by kill switch while in queue"));
            }
            _ = tokio::time::sleep_until(activity_deadline) => {
                return Err(PeerError::new(PeerFailureReason::TimeoutStatus, "Peer silent for a full re-ask cycle in queue"));
            }
            _ = tokio::time::sleep_until(next_reask) => {
                next_reask = tokio::time::Instant::now() + FILE_REASK_INTERVAL;
                // eMule `SendFileRequest` re-ask: file status request keeps
                // the part bitmap fresh, then `OP_STARTUPLOADREQ` refreshes
                // `LastUpRequest` on the remote queue (prevents the 1 h purge).
                // Fichier ≤ 1 part : `OP_SETREQFILEID` omis comme eMule.
                if file_part_count != 1 {
                    let status_pkt = encoder.encode_file_status_req(&file_status_req).map_err(|e| {
                        PeerError::new(PeerFailureReason::CorruptedData, format!("Failed to encode re-ask: {e}"))
                    })?;
                    out_wire.clear();
                    out_wire.push(status_pkt.protocol);
                    out_wire.extend_from_slice(&(status_pkt.frame_len() as u32).to_le_bytes());
                    out_wire.push(status_pkt.opcode);
                    out_wire.extend_from_slice(&status_pkt.payload);
                    if let Err(e) = stream.write_all(&out_wire).await {
                        return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Failed to send re-ask: {e}")));
                    }
                }
                send_start_upload_req(stream, dl_hash_bytes, file_hash_str, target_addr).await?;
                // Re-ask SX périodique — eMule attache la requête source au
                // cycle de reask (`SendFileRequest` → `IsSourceRequestAllowed`).
                // `complete_source` : le pair est source complète quand son
                // part-status couvre tout le fichier — eMule ne lui demande
                // alors pas de sources.
                let reask_caps = PeerSxCaps {
                    complete_source: file_status.is_complete(),
                    ..*sx_caps
                };
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
                        let rank = inner.lock().await.sessions.get(address_str).and_then(|s| s.snapshot.queue_rank);
                        if let Some(r) = rank {
                            info!("Peer {peer} closed connection while we are in queue (rank {r}). Scheduled re-ask in 15 minutes.", peer = log_peer_addr(target_addr));
                            return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Disconnected in queue"));
                        } else {
                            return Err(PeerError::new(PeerFailureReason::ConnectRefused, "Peer closed connection while in queue"));
                        }
                    }
                    Ok(n) => {
                        activity_deadline = tokio::time::Instant::now() + FILE_REASK_INTERVAL + timeouts.queue_timeout;
                        read_buf.extend_from_slice(&raw_buf[..n]);
                        if read_buf.len() > MAX_PEER_READ_BUFFER_SIZE {
                            return Err(PeerError::new(PeerFailureReason::CorruptedData, "Peer read buffer exceeded limit while in queue"));
                        }
                        while let Ok(Some(packet)) = codec.decode(&mut read_buf) {
                            if sui.handle(&packet, target_addr, &decoder, &encoder, stream).await {
                                continue;
                            }
                            if handle_hashset_answer(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, downloads).await {
                                continue;
                            }
                            if handle_aich_file_hash_res(stream, &packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, &encoder, peer_supports_aich, downloads).await {
                                continue;
                            }
                            if handle_aich_answer(stream, &packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, &encoder, peer_supports_aich, downloads).await {
                                continue;
                            }
                            if packet.protocol == PR_ED2K && packet.opcode == OP_ACCEPTUPLOADREQ {
                                info!("Upload slot accepted (OP_ACCEPTUPLOADREQ) by peer {}", log_peer_addr(target_addr));
                                break 'queue_loop;
                            } else if (packet.protocol == PR_ED2K || packet.protocol == PR_EMULE) && packet.opcode == OP_QUEUERANKING {
                                let rank = if packet.payload.len() >= 4 {
                                    u32::from_le_bytes(packet.payload[..4].try_into().unwrap_or([0, 0, 0, 0]))
                                } else if packet.payload.len() >= 2 {
                                    u16::from_le_bytes(packet.payload[..2].try_into().unwrap_or([0, 0])) as u32
                                } else {
                                    0
                                };
                                tracing::debug!("Queue rank from peer {}: {}", log_peer_addr(target_addr), rank);
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
                                // Keep-alive part-status : rafraîchit le bitmap
                                // du pair (et `last_seen`) — la barre de parts
                                // reflète alors la dispo courante de la file.
                                if let Ok(status) = decoder.decode_file_status(
                                    packet.protocol,
                                    packet.opcode,
                                    &packet.payload,
                                    0,
                                ) {
                                    if status.file_hash == dl_hash_bytes {
                                        // eMule ProcessFileStatus : le bitmap
                                        // du pair est remplacé — le séquenceur
                                        // recalcule la dispo à la reprise.
                                        file_status = status.clone();
                                        let mut dl_svc = downloads.lock().await;
                                        if let Some(dl_id) = dl_svc
                                            .find_by_ed2k_hash(file_hash_str)
                                            .map(|s| s.id)
                                        {
                                            if let Ok(snap) = dl_svc.record_source_availability(
                                                &dl_id,
                                                target_addr,
                                                status.availability_mask.clone(),
                                            ) {
                                                emit_download_update(event_tx, sequence, &snap);
                                            }
                                        }
                                    }
                                }
                            } else if packet.protocol == PR_ED2K && packet.opcode == OP_FILEREQANSNOFILE {
                                return Err(PeerError::new(PeerFailureReason::FileNotFound, "Remote peer no longer shares this file"));
                            } else if (packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
                                && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2)
                            {
                                handle_sx_answer(&packet, dl_hash_bytes, file_hash_str, target_addr, &decoder, downloads, sx_env, sx_local).await;
                            } else if packet.protocol == PR_EMULE
                                && (packet.opcode == OP_REQUESTSOURCES || packet.opcode == OP_REQUESTSOURCES2)
                            {
                                // `reqstatus` : part-status du demandeur (son
                                // OP_FILESTATUS) — eMule ne sert que des
                                // sources couvrant des parts qui lui manquent.
                                handle_sx_request(&packet, Some(dl_hash_bytes), target_addr, &decoder, &encoder, downloads, stream, sx_env, sx_caps, Some(file_status.availability_mask.raw_bytes())).await;
                            } else if packet.protocol == PR_EMULE
                            {
                                super::source_exchange::handle_sx_multipacket(
                                    &packet,
                                    dl_hash_bytes,
                                    Some(file_status.availability_mask.raw_bytes()),
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
                        }
                    }
                    Err(e) => return Err(PeerError::new(PeerFailureReason::ConnectRefused, format!("Read error while in queue: {e}"))),
                }
            }
        }
    }

    Ok(file_status)
}
