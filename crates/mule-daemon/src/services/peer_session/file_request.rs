// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! eMule `CUpDownClient::SendFileRequest` (`DownloadClient.cpp:257-430`)
//! — the file-request bundle sent on connect, on A4AF swap and at every
//! `FILEREASKTIME` re-ask while queued.
//!
//! eMule emits **one** `OP_MULTIPACKET*` packet when the peer negotiated
//! it (`SupportMultiPacket() || SupportsFileIdentifiers()`), wrapping
//! `OP_REQFILENAME` + `OP_SETREQFILEID` + `OP_REQUESTSOURCES(2)` +
//! `OP_AICHFILEHASHREQ` as embedded sub-opcodes; otherwise it sends the
//! same requests as separate packets. This module builds both forms and
//! shares the input gathering so the wire content is identical whatever
//! the envelope.

use std::net::SocketAddr;
use std::sync::Arc;

use mule_protocol::ed2k::{
    Ed2kEncoder, Ed2kFileIdentifier, Ed2kMultiPacketFileRef, Ed2kMultiPacketRequest,
    Ed2kMultiPacketSubRequest, Ed2kRequestExtInfo, Ed2kRequestSources, PartAvailabilityMask,
    SOURCE_EXCHANGE2_VERSION,
};
use mule_protocol::framing::RawPacket;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::services::downloads::DownloadService;
use crate::services::obfuscation_stream::ObfuscatedStream;

use super::log_peer_addr;
use super::source_exchange::{
    mark_sx_request_sent, planned_sx_request, MultiPacketLevel, PeerSxCaps, SxEnv,
};
use super::types::PeerError;
use mule_core::model::PeerFailureReason;

/// Inputs of one `SendFileRequest` bundle, gathered once per send so the
/// multipacket and split forms carry identical content.
pub(crate) struct FileRequestInputs {
    /// MD4 hash of the requested file.
    pub file_hash: [u8; 16],
    /// File size in bytes (EXT/EXT2 envelopes).
    pub file_size: u64,
    /// Number of eD2K parts — `OP_SETREQFILEID` is skipped for
    /// single-part files (`DownloadClient.cpp:292`).
    pub part_count: u16,
    /// Negotiated extended-requests version (`m_byExtendedRequestsVer`).
    pub peer_ext_req: u8,
    /// Our own part availability for the file — `WritePartStatus`
    /// ext-info, present iff `peer_ext_req > 0`.
    pub our_part_status: Option<PartAvailabilityMask>,
    /// SX request admitted by `IsSourceRequestAllowed` this cycle.
    pub sx_request: Option<Ed2kRequestSources>,
    /// Whether to request the AICH root — `peer_supports_aich` and no
    /// trusted root known yet (eMule asks unconditionally; skipping once
    /// the root is learned is a documented optimization).
    pub aich_file_hash_req: bool,
    /// Our trusted AICH root — embedded in the EXT2 `FileIdentifier`
    /// (`bHasAICHHash` → `WriteIdentifier` includes it).
    pub aich_root: Option<[u8; 20]>,
    /// Negotiated multipacket envelope level.
    pub level: MultiPacketLevel,
}

/// Gathers everything `SendFileRequest` needs for `file_hash_str` on
/// `target_addr`: part geometry, our part-status ext-info, the SX
/// decision (`IsSourceRequestAllowed`) and the AICH state.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn gather_file_request_inputs(
    downloads: &Arc<Mutex<DownloadService>>,
    sx_env: &SxEnv,
    sx_caps: &PeerSxCaps,
    target_addr: SocketAddr,
    file_hash_str: &str,
    dl_hash_bytes: [u8; 16],
    peer_ext_req: u8,
    peer_supports_aich: bool,
) -> FileRequestInputs {
    let (total_bytes, completed_ranges, aich_root, aich_needed) = {
        let dl_svc = downloads.lock().await;
        let snap = dl_svc.find_by_ed2k_hash(file_hash_str);
        (
            snap.as_ref().map(|d| d.total_bytes).unwrap_or(0),
            dl_svc.get_completed_ranges_by_hash(file_hash_str),
            snap.as_ref()
                .and_then(|s| dl_svc.trusted_aich_root(&s.id)),
            snap.as_ref()
                .and_then(|s| dl_svc.trusted_aich_root(&s.id))
                .is_none(),
        )
    };
    let part_size = mule_protocol::ed2k::ED2K_PART_SIZE as u64;
    let part_count = if total_bytes > 0 {
        total_bytes.div_ceil(part_size) as u16
    } else {
        0
    };
    // Notre propre part-status : parts dont la plage complète est déjà
    // téléchargée — eMule `WritePartStatus` (envoyé iff ext_req > 0).
    let our_part_status = if peer_ext_req > 0 && part_count > 0 {
        let available: Vec<u16> = (0..part_count)
            .filter(|&p| {
                let start = u64::from(p) * part_size;
                let end = (start + part_size).min(total_bytes);
                completed_ranges
                    .iter()
                    .any(|&(rs, re)| rs <= start && re >= end)
            })
            .collect();
        Some(PartAvailabilityMask::from_parts(&available, part_count))
    } else {
        None
    };
    let sx_request = planned_sx_request(
        target_addr,
        file_hash_str,
        dl_hash_bytes,
        downloads,
        sx_env,
        sx_caps,
    )
    .await;
    FileRequestInputs {
        file_hash: dl_hash_bytes,
        file_size: total_bytes,
        part_count,
        peer_ext_req,
        our_part_status,
        sx_request,
        aich_file_hash_req: peer_supports_aich && aich_needed,
        aich_root,
        level: sx_caps.multipacket,
    }
}

/// eMule `SendFileRequest` — returns the wire packets in send order:
/// a single `OP_MULTIPACKET*` when negotiated, else the split bundle
/// `REQFILENAME(+extinfo)` → `SETREQFILEID` → `REQUESTSOURCES` →
/// `AICHFILEHASHREQ` (same sub-request order as eMule's multipacket).
pub(crate) fn build_file_request_packets(
    encoder: &Ed2kEncoder,
    inputs: &FileRequestInputs,
) -> Result<Vec<RawPacket>, PeerError> {
    let ext_info = inputs
        .our_part_status
        .as_ref()
        .map(|mask| Ed2kRequestExtInfo {
            part_count: inputs.part_count,
            part_status: bytes::Bytes::copy_from_slice(mask.raw_bytes()),
            complete_sources: (inputs.peer_ext_req > 1).then_some(0),
        });

    let enc_err = |ctx: &str, e: mule_protocol::ed2k::Ed2kEncodeError| {
        PeerError::new(
            PeerFailureReason::InvalidHandshake,
            format!("Failed to encode {ctx}: {e}"),
        )
    };

    if inputs.level != MultiPacketLevel::None {
        // Enveloppe — EXT2 porte le FileIdentifier (hash+size+AICH si
        // connu), EXT hash+size, BASIC hash seul. Un fichier de taille
        // inconnue dégrade proprement vers l'enveloppe hash seule.
        let file_ref = match inputs.level {
            MultiPacketLevel::Ext2 if inputs.file_size > 0 => {
                let mut id = Ed2kFileIdentifier::new(inputs.file_hash, inputs.file_size);
                id.aich_hash = inputs.aich_root;
                Ed2kMultiPacketFileRef::Identifier(id)
            }
            MultiPacketLevel::Ext if inputs.file_size > 0 => {
                Ed2kMultiPacketFileRef::HashAndSize {
                    file_hash: inputs.file_hash,
                    file_size: inputs.file_size,
                }
            }
            _ => Ed2kMultiPacketFileRef::Hash(inputs.file_hash),
        };
        // Sous-opcodes dans l'ordre eMule : REQFILENAME → SETREQFILEID →
        // REQUESTSOURCES(2) → AICHFILEHASHREQ (jamais sous EXT2 : la
        // racine AICH voyage déjà dans le FileIdentifier).
        let mut subs = Vec::with_capacity(4);
        subs.push(Ed2kMultiPacketSubRequest::FileName { ext_info });
        if inputs.part_count > 1 {
            subs.push(Ed2kMultiPacketSubRequest::SetReqFileId);
        }
        if let Some(sx) = &inputs.sx_request {
            subs.push(if sx.version >= SOURCE_EXCHANGE2_VERSION {
                Ed2kMultiPacketSubRequest::RequestSources2 {
                    version: sx.version,
                    options: 0,
                }
            } else {
                Ed2kMultiPacketSubRequest::RequestSources
            });
        }
        if inputs.aich_file_hash_req && !matches!(inputs.level, MultiPacketLevel::Ext2) {
            subs.push(Ed2kMultiPacketSubRequest::AichFileHash);
        }
        let pkt = mule_protocol::ed2k::encoder::encode_multipacket_request(
            &encoder.limits,
            &Ed2kMultiPacketRequest {
                file: file_ref,
                sub_requests: subs,
            },
        )
        .map_err(|e| enc_err("OP_MULTIPACKET*", e))?;
        return Ok(vec![pkt]);
    }

    // Forme éclatée (héritage) — même ordre que la branche `else` de
    // `SendFileRequest` : REQFILENAME, SETREQFILEID, SOURCES, AICH.
    let mut packets = Vec::with_capacity(4);
    let req_filename_pkt = if let (Some(mask), true) =
        (inputs.our_part_status.as_ref(), inputs.peer_ext_req > 0)
    {
        mule_protocol::ed2k::encoder::encode_req_filename(
            &encoder.limits,
            &inputs.file_hash,
            mask,
            ext_info
                .as_ref()
                .and_then(|e| e.complete_sources)
                .unwrap_or(0),
            inputs.peer_ext_req,
        )
        .map_err(|e| enc_err("OP_REQFILENAME", e))?
    } else {
        encoder
            .encode_file_status_req_with_opcode(
                &mule_protocol::ed2k::Ed2kFileStatusReq {
                    file_hash: inputs.file_hash,
                },
                mule_protocol::ed2k::OP_REQFILENAME,
            )
            .map_err(|e| enc_err("OP_REQFILENAME", e))?
    };
    packets.push(req_filename_pkt);

    if inputs.part_count > 1 {
        packets.push(
            encoder
                .encode_file_status_req_with_opcode(
                    &mule_protocol::ed2k::Ed2kFileStatusReq {
                        file_hash: inputs.file_hash,
                    },
                    mule_protocol::ed2k::OP_SETREQFILEID_EMULE,
                )
                .map_err(|e| enc_err("OP_SETREQFILEID_EMULE", e))?,
        );
    }
    if let Some(sx) = &inputs.sx_request {
        packets.push(
            encoder
                .encode_request_sources(sx)
                .map_err(|e| enc_err("OP_REQUESTSOURCES", e))?,
        );
    }
    if inputs.aich_file_hash_req {
        packets.push(
            encoder
                .encode_aich_file_hash_req(&mule_protocol::ed2k::Ed2kAichFileHashReq {
                    file_hash: inputs.file_hash,
                })
                .map_err(|e| enc_err("OP_AICHFILEHASHREQ", e))?,
        );
    }
    Ok(packets)
}

/// eMule `ListenSocket.cpp:1071-1099` — expands an inbound
/// `OP_MULTIPACKETANSWER`/`OP_MULTIPACKETANSWER_EXT2` into the equivalent
/// standalone packets (the envelope's file hash is prepended back to each
/// sub-answer), so the normal per-opcode dispatch processes them
/// unchanged. Returns `None` for packets that are not multipacket
/// answers, and `Some(vec![])` for malformed answers (consumed).
pub(crate) fn expand_multipacket_answer(packet: &RawPacket) -> Option<Vec<RawPacket>> {
    use mule_protocol::ed2k::{
        Ed2kMultiPacketSubAnswer, OP_AICHFILEHASHRES, OP_MULTIPACKETANSWER,
        OP_MULTIPACKETANSWER_EXT2, OP_REQFILENAME_ANSWER, OP_FILESTATUS,
    };
    if packet.protocol != mule_protocol::ed2k::PR_EMULE
        || !matches!(
            packet.opcode,
            OP_MULTIPACKETANSWER | OP_MULTIPACKETANSWER_EXT2
        )
    {
        return None;
    }
    let ans = match mule_protocol::ed2k::decoder::decode_multipacket_answer(
        packet.protocol,
        packet.opcode,
        &packet.payload,
        0,
        &mule_protocol::ed2k::Ed2kLimits::default(),
    ) {
        Ok(a) => a,
        Err(e) => {
            tracing::debug!("Malformed OP_MULTIPACKETANSWER ignored: {e}");
            return Some(Vec::new());
        }
    };
    let file_hash = ans.file.file_hash();
    let mut out = Vec::with_capacity(ans.sub_answers.len());
    for sub in ans.sub_answers {
        match sub {
            Ed2kMultiPacketSubAnswer::FileName(name) => {
                // Standalone `OP_REQFILENAMEANSWER` = hash16 + len u16 + name.
                let mut payload = Vec::with_capacity(18 + name.len());
                payload.extend_from_slice(&file_hash);
                payload.extend_from_slice(&(name.len() as u16).to_le_bytes());
                payload.extend_from_slice(name.as_bytes());
                out.push(RawPacket::new(
                    mule_protocol::ed2k::PR_ED2K,
                    OP_REQFILENAME_ANSWER,
                    bytes::Bytes::from(payload),
                ));
            }
            Ed2kMultiPacketSubAnswer::FileStatus {
                part_count,
                part_status,
            } => {
                // Standalone `OP_FILESTATUS` = hash16 + u16 + bitfield.
                let mut payload = Vec::with_capacity(18 + part_status.len());
                payload.extend_from_slice(&file_hash);
                payload.extend_from_slice(&part_count.to_le_bytes());
                payload.extend_from_slice(&part_status);
                out.push(RawPacket::new(
                    mule_protocol::ed2k::PR_ED2K,
                    OP_FILESTATUS,
                    bytes::Bytes::from(payload),
                ));
            }
            Ed2kMultiPacketSubAnswer::AichFileHash(root) => {
                // Standalone `OP_AICHFILEHASHANS` = hash16 + root 20B.
                let mut payload = Vec::with_capacity(36);
                payload.extend_from_slice(&file_hash);
                payload.extend_from_slice(&root);
                out.push(RawPacket::new(
                    mule_protocol::ed2k::PR_EMULE,
                    OP_AICHFILEHASHRES,
                    bytes::Bytes::from(payload),
                ));
            }
        }
    }
    Some(out)
}

/// Builds and writes the `SendFileRequest` bundle, then applies
/// `SetLastAskedForSources` bookkeeping when an SX request went out
/// (eMule marks it after `SendPacket`).
pub(crate) async fn send_file_request_packets(
    stream: &mut ObfuscatedStream<TcpStream>,
    encoder: &Ed2kEncoder,
    sx_env: &SxEnv,
    target_addr: SocketAddr,
    inputs: &FileRequestInputs,
) -> Result<(), PeerError> {
    let packets = build_file_request_packets(encoder, inputs)?;
    for pkt in &packets {
        let mut out_wire = Vec::with_capacity(6 + pkt.payload.len());
        out_wire.push(pkt.protocol);
        out_wire.extend_from_slice(&(pkt.frame_len() as u32).to_le_bytes());
        out_wire.push(pkt.opcode);
        out_wire.extend_from_slice(&pkt.payload);
        if let Err(e) = stream.write_all(&out_wire).await {
            return Err(PeerError::new(
                PeerFailureReason::ConnectRefused,
                format!("Failed to send file request (opcode 0x{:02X}): {e}", pkt.opcode),
            ));
        }
    }
    if inputs.sx_request.is_some() {
        mark_sx_request_sent(sx_env, target_addr.ip(), inputs.file_hash);
    }
    tracing::debug!(
        "SendFileRequest to peer {peer}: {} packet(s), level={:?}",
        packets.len(),
        inputs.level,
        peer = log_peer_addr(target_addr)
    );
    Ok(())
}
