// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use mule_protocol::ed2k::Ed2kEncoder;
use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub use super::events::{emit_download_update, emit_event};
use super::handshake::perform_peer_handshake;
use super::source_exchange::{PeerSxCaps, SxEnv, SxLocalIds};
use super::status_query::query_file_status_and_enter_queue;
use super::transfer::run_transfer_loop;
pub use super::transfer::{OP_COMPRESSEDPART_I64, OP_SENDINGPART_I64};
use super::types::{PeerError, PeerTimeouts, SwarmInnerState};
use crate::services::downloads::DownloadService;

pub(crate) async fn run_peer_connection(
    file_hash_str: String,
    dl_hash_bytes: [u8; 16],
    address_str: String,
    target_addr: SocketAddr,
    client_port: u16,
    nickname: String,
    user_hash: [u8; 16],
    downloads: Arc<Mutex<DownloadService>>,
    cancel_token: mule_network_policy::CancellationToken,
    inner: Arc<Mutex<SwarmInnerState>>,
    event_tx: broadcast::Sender<String>,
    sequence: Arc<AtomicU64>,
    timeouts: PeerTimeouts,
    client_id: u32,
    server_ip: u32,
    server_port: u16,
    kad_port: u16,
    kad_service: Option<Arc<crate::services::KadService>>,
    target_user_hash: Option<[u8; 16]>,
    peer_crypt_options: u8,
    obfuscation_enabled: bool,
    obfuscation_required: bool,
    obfuscation_padding_max: u8,
    identity: Option<Arc<Mutex<crate::services::IdentityService>>>,
    sui_stats: Arc<crate::services::identity::SuiStats>,
    rate_limiters: Option<Arc<mule_network_policy::PolicyRateLimiters>>,
    sx_env: SxEnv,
) -> Result<u64, PeerError> {
    // eMule advertises SecIdent=3 (v1|v2) in MiscOptions1 bits 16-19 when a
    // usable RSA key is loaded, 0 otherwise.
    let sec_ident_cap: u8 = match &identity {
        Some(id) if id.lock().await.has_key() => 3,
        _ => 0,
    };
    // 1-3. Perform TCP connect, obfuscation negotiation, OP_HELLO, and await OP_HELLO_ANSWER
    let mut handshake_res = perform_peer_handshake(
        target_addr,
        &file_hash_str,
        &address_str,
        client_port,
        &nickname,
        user_hash,
        &timeouts,
        &cancel_token,
        &inner,
        &event_tx,
        &sequence,
        client_id,
        server_ip,
        server_port,
        kad_port,
        &kad_service,
        target_user_hash,
        peer_crypt_options,
        obfuscation_enabled,
        obfuscation_required,
        obfuscation_padding_max,
        &downloads,
        sec_ident_cap,
    )
    .await?;

    // 4-7. Query file status, request filename, start upload req, and wait in queue.
    // Outbound SUI (OP_SECIDENTSTATE challenges from the remote) is handled
    // inside the status/queue loops via `sui`.
    // Record the peer's announced eMule UDP endpoint for client-UDP
    // reasks (`OP_REASKFILEPING` keep-alives toward queued sources).
    if let Some(udp_port) = handshake_res.peer_udp_port {
        inner.lock().await.peer_udp.insert(
            address_str.clone(),
            (udp_port, handshake_res.peer_udp_version),
        );
    }

    let mut sui = super::sui::OutboundSui::new(identity).with_stats(sui_stats);
    sui.set_peer_sec_ident(handshake_res.peer_sec_ident);
    // eMule InfoPacketsReceived: once info packets are exchanged, challenge the
    // peer if it announced Secure Ident support (verifier role).
    sui.initiate(
        target_addr,
        &Ed2kEncoder::default(),
        &mut handshake_res.stream,
    )
    .await;
    // Capacités SX négociées au handshake (eMule GetSourceExchange1Version /
    // SupportsSourceExchange2) + identité locale pour les self-checks
    // `CanAddSource` (eMule PartFile.cpp:2380-2403).
    let sx_caps = PeerSxCaps {
        sx1_version: handshake_res.peer_sx1_version,
        sx2: handshake_res.peer_supports_sx2,
        complete_source: false,
    };
    let kad_open = match &kad_service {
        Some(kad) => {
            let udp_snap = kad.udp().get_snapshot().await;
            let fw_status = {
                let tracker = kad.udp().firewall_tracker();
                let fw = tracker.lock().await;
                fw.status
            };
            udp_snap.is_active
                && fw_status == crate::services::kad_udp::types::UdpFirewallStatus::Open
        }
        None => false,
    };
    let sx_local = SxLocalIds {
        client_id,
        server_ip,
        server_port,
        tcp_port: client_port,
        is_high_id: client_id >= 16_777_216,
        kad_open,
    };
    // 8-10. Multi-source block planner, request blocks, and streaming
    // transfers — with socket-level A4AF swap on NNP/FNF
    // (`SwapToAnotherFile`: the live connection re-asks another file).
    run_status_transfer_with_a4af(
        &mut handshake_res.stream,
        target_addr,
        file_hash_str,
        dl_hash_bytes,
        &address_str,
        &sx_caps,
        handshake_res.peer_supports_aich,
        handshake_res.peer_ext_req,
        &timeouts,
        &cancel_token,
        &inner,
        &event_tx,
        &sequence,
        &downloads,
        &sx_env,
        &sx_local,
        &mut sui,
        rate_limiters.as_deref(),
    )
    .await
}

/// Runs `query_file_status_and_enter_queue` + `run_transfer_loop` for the
/// requested file, applying eMule's socket-level A4AF on NNP/FNF
/// (`CUpDownClient::SwapToAnotherFile`): instead of dropping the
/// connection, the same socket re-issues `OP_SETREQFILEID` for the best
/// alternate file this peer is a source of. Bounded to
/// `a4af::max_swaps()` consecutive swaps.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_status_transfer_with_a4af(
    stream: &mut crate::services::obfuscation_stream::ObfuscatedStream<tokio::net::TcpStream>,
    target_addr: SocketAddr,
    file_hash_str: String,
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
    rate_limiters: Option<&mule_network_policy::PolicyRateLimiters>,
) -> Result<u64, PeerError> {
    let mut cur_hash_str = file_hash_str;
    let mut cur_hash = dl_hash_bytes;
    let mut swaps = 0u32;
    loop {
        let file_status = match query_file_status_and_enter_queue(
            stream,
            target_addr,
            &cur_hash_str,
            cur_hash,
            address_str,
            sx_caps,
            peer_supports_aich,
            peer_ext_req,
            timeouts,
            cancel_token,
            inner,
            event_tx,
            sequence,
            downloads,
            sx_env,
            sx_local,
            sui,
        )
        .await
        {
            Ok(s) => s,
            Err(e) => {
                match try_a4af_swap(
                    downloads,
                    inner,
                    event_tx,
                    sequence,
                    address_str,
                    &cur_hash_str,
                    &e,
                    &mut swaps,
                )
                .await
                {
                    Some((h, b)) => {
                        cur_hash_str = h;
                        cur_hash = b;
                        continue;
                    }
                    None => return Err(e),
                }
            }
        };
        match run_transfer_loop(
            stream,
            target_addr,
            &cur_hash_str,
            cur_hash,
            address_str,
            file_status,
            peer_supports_aich,
            peer_ext_req,
            timeouts,
            cancel_token,
            inner,
            event_tx,
            sequence,
            downloads,
            sx_caps,
            sx_env,
            sx_local,
            sui,
            rate_limiters,
        )
        .await
        {
            Ok(n) => return Ok(n),
            Err(e) => {
                match try_a4af_swap(
                    downloads,
                    inner,
                    event_tx,
                    sequence,
                    address_str,
                    &cur_hash_str,
                    &e,
                    &mut swaps,
                )
                .await
                {
                    Some((h, b)) => {
                        cur_hash_str = h;
                        cur_hash = b;
                        continue;
                    }
                    None => return Err(e),
                }
            }
        }
    }
}

/// Attempts one socket-level A4AF swap: writes the NNP/FNF bookkeeping on
/// the old file, marks the source live on the new file and updates the
/// session snapshot (`DoSwap`). Returns the new `(hash_hex, hash_bytes)`
/// or `None` when no viable alternate file exists.
#[allow(clippy::too_many_arguments)]
async fn try_a4af_swap(
    downloads: &Arc<Mutex<DownloadService>>,
    inner: &Arc<Mutex<SwarmInnerState>>,
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    address_str: &str,
    from_hash: &str,
    err: &PeerError,
    swaps: &mut u32,
) -> Option<(String, [u8; 16])> {
    if *swaps >= super::a4af::max_swaps() || !super::a4af::is_swap_eligible(&err.reason) {
        return None;
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let max_per_file = inner.lock().await.max_peers_per_download;
    let (to_hash, to_hash_bytes) = super::a4af::find_swap_target(
        downloads,
        address_str,
        from_hash,
        max_per_file,
        now_ms,
    )
    .await?;
    super::a4af::apply_swap(downloads, address_str, from_hash, &to_hash, &err.reason, now_ms).await;
    *swaps += 1;
    tracing::info!(
        "[A4AF] Socket swap: {} re-requested for {to_hash} (was {from_hash}, reason: {})",
        address_str,
        err.reason.as_str()
    );
    // Reflect the swap in the live session snapshot (eMule `SetRequestFile`)
    // and release blocks in flight for the old file — eMule resets the
    // download state on `DoSwap` (`m_PendingBlocks_list` is abandoned).
    let snap = {
        let mut g = inner.lock().await;
        g.in_flight_blocks.retain(|b| b.peer_id != address_str);
        if let Some(session) = g.sessions.get_mut(address_str) {
            session.snapshot.file_hash = Some(to_hash.clone());
            // The next status query sets Queued/Transferring itself.
            session.snapshot.queue_rank = None;
            session.snapshot.current_part = None;
            Some(session.snapshot.clone())
        } else {
            None
        }
    };
    if let Some(s) = snap {
        super::events::emit_event(event_tx, sequence, &s);
    }
    Some((to_hash, to_hash_bytes))
}

/// Runs a client-role download session over an *accepted* inbound connection
/// established through a server LowID callback (`OP_CALLBACKREQUEST` relay).
/// The peer already sent `OP_HELLO` and received our `OP_HELLOANSWER`, so
/// the transport handshake is skipped — we drive `OP_SETREQFILEID`, file
/// status, queueing, and transfers exactly like an outbound session.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_callback_session(
    mut stream: crate::services::obfuscation_stream::ObfuscatedStream<tokio::net::TcpStream>,
    target_addr: SocketAddr,
    file_hash_str: String,
    dl_hash_bytes: [u8; 16],
    address_str: String,
    sx_caps: PeerSxCaps,
    peer_supports_aich: bool,
    peer_sec_ident: u8,
    peer_ext_req: u8,
    downloads: Arc<Mutex<DownloadService>>,
    cancel_token: mule_network_policy::CancellationToken,
    inner: Arc<Mutex<SwarmInnerState>>,
    event_tx: broadcast::Sender<String>,
    sequence: Arc<AtomicU64>,
    timeouts: PeerTimeouts,
    identity: Option<Arc<Mutex<crate::services::IdentityService>>>,
    sui_stats: Arc<crate::services::identity::SuiStats>,
    rate_limiters: Option<Arc<mule_network_policy::PolicyRateLimiters>>,
    sx_env: SxEnv,
    sx_local: SxLocalIds,
) -> Result<u64, PeerError> {
    let mut sui = super::sui::OutboundSui::new(identity).with_stats(sui_stats);
    sui.set_peer_sec_ident(peer_sec_ident);
    sui.initiate(target_addr, &Ed2kEncoder::default(), &mut stream)
        .await;
    run_status_transfer_with_a4af(
        &mut stream,
        target_addr,
        file_hash_str,
        dl_hash_bytes,
        &address_str,
        &sx_caps,
        peer_supports_aich,
        peer_ext_req,
        &timeouts,
        &cancel_token,
        &inner,
        &event_tx,
        &sequence,
        &downloads,
        &sx_env,
        &sx_local,
        &mut sui,
        rate_limiters.as_deref(),
    )
    .await
}
