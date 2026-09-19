use std::net::SocketAddr;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

pub use super::events::{emit_download_update, emit_event};
use super::handshake::perform_peer_handshake;
use super::status_query::query_file_status_and_enter_queue;
pub use super::transfer::{
    OP_COMPRESSEDPART_I64, OP_SENDINGPART_I64,
};
use super::transfer::run_transfer_loop;
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
) -> Result<u64, PeerError> {
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
    )
    .await?;

    // 4-7. Query file status, request filename, start upload req, and wait in queue
    let file_status = query_file_status_and_enter_queue(
        &mut handshake_res.stream,
        target_addr,
        &file_hash_str,
        dl_hash_bytes,
        &address_str,
        handshake_res.peer_supports_sx,
        handshake_res.peer_supports_sx2,
        &timeouts,
        &cancel_token,
        &inner,
        &event_tx,
        &sequence,
        &downloads,
    )
    .await?;

    // 8-10. Multi-source block planner, request blocks, and streaming transfers
    run_transfer_loop(
        &mut handshake_res.stream,
        target_addr,
        &file_hash_str,
        dl_hash_bytes,
        &address_str,
        file_status,
        &timeouts,
        &cancel_token,
        &inner,
        &event_tx,
        &sequence,
        &downloads,
    )
    .await
}
