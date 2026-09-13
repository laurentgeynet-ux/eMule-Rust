use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::Duration;

use bytes::BytesMut;
use mule_core::DownloadStatus;
use mule_protocol::lab::*;
use mule_protocol::{BlockRequest, PacketCodec, PeerId};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{error, info};
use uuid::Uuid;

use crate::state::AppState;

pub async fn run_lab_download(
    download_id: Uuid,
    peer_addr: SocketAddr,
    state: AppState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Transition to WaitingForSources
    let (temp_path, _target_path, expected_hash, total_size) = {
        let mut dls = state.downloads.lock().await;
        let snap = dls.update_status(&download_id, DownloadStatus::WaitingForSources)?;
        state.broadcast_download_update(snap);
        match dls.get_download_file_info(&download_id) {
            Some(info) => info,
            None => return Err(format!("Download {download_id} not found").into()),
        }
    };

    // 2. Connect to peer with timeout
    let mut socket =
        match tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(peer_addr)).await {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => {
                let mut dls = state.downloads.lock().await;
                let snap = dls.update_status(&download_id, DownloadStatus::Failed)?;
                state.broadcast_download_update(snap);
                return Err(Box::new(e));
            }
            Err(_) => {
                let mut dls = state.downloads.lock().await;
                let snap = dls.update_status(&download_id, DownloadStatus::Failed)?;
                state.broadcast_download_update(snap);
                return Err("Connection timed out".into());
            }
        };

    // 3. Transition to Downloading
    {
        let mut dls = state.downloads.lock().await;
        let snap = dls.update_status(&download_id, DownloadStatus::Downloading)?;
        state.broadcast_download_update(snap);
    }

    // 4. Handshake
    let codec = PacketCodec::new();
    let mut read_buf = BytesMut::new();

    let client_peer_id = PeerId::new([0xAA; 16]);
    let local_nonce = [0x42; 16];
    let (mut sm, hello) = PeerStateMachine::new_initiator(client_peer_id, 1, local_nonce);
    send_msg(&mut socket, &codec, &hello).await?;

    let ack = read_msg(&mut socket, &codec, &mut read_buf).await?;
    sm.on_message(ack)?;

    let offer = read_msg(&mut socket, &codec, &mut read_buf).await?;
    match offer {
        LabMessage::FileOffer {
            file_hash,
            file_size,
            ..
        } => {
            if file_hash != expected_hash.0 {
                return Err("Peer offered unexpected file hash".into());
            }
            if file_size != total_size {
                return Err("Peer offered file with size mismatch".into());
            }
        }
        other => return Err(format!("Expected FileOffer, got {other:?}").into()),
    }

    // 5. Calculate missing blocks
    let missing_ranges = {
        let dls = state.downloads.lock().await;
        dls.get_missing_ranges(&download_id).unwrap_or_default()
    };

    let mut pending_blocks: VecDeque<(u64, u32)> = VecDeque::new();
    for range in missing_ranges {
        let mut offset = range.start;
        while offset < range.end {
            let chunk_len = std::cmp::min(184_320, range.end - offset) as u32;
            pending_blocks.push_back((offset, chunk_len));
            offset += chunk_len as u64;
        }
    }

    // 6. Pipeline with InFlightTracker
    let mut tracker = InFlightTracker::new();
    let file_hash = expected_hash.0;

    while !pending_blocks.is_empty() || !tracker.is_empty() {
        // Check if paused or canceled
        {
            let dls = state.downloads.lock().await;
            if let Some(snap) = dls.get_download(&download_id) {
                if snap.status == DownloadStatus::Paused.as_str() {
                    break;
                }
            } else {
                break;
            }
        }

        // Fill pipeline up to capacity
        while tracker.can_request() {
            if let Some((offset, length)) = pending_blocks.pop_front() {
                let req = BlockRequest::new(file_hash, offset, length);
                tracker.record_request(&req)?;
                let wire_req = LabMessage::RequestBlock {
                    file_hash,
                    start_offset: offset,
                    length,
                };
                send_msg(&mut socket, &codec, &wire_req).await?;
            } else {
                break;
            }
        }

        if tracker.is_empty() && pending_blocks.is_empty() {
            break;
        }

        // Read incoming packet or timeout
        tokio::select! {
            msg_res = read_msg(&mut socket, &codec, &mut read_buf) => {
                let msg = msg_res?;
                match msg {
                    LabMessage::BlockData {
                        file_hash: resp_hash,
                        start_offset,
                        data,
                    } => {
                        tracker.validate_and_remove(&resp_hash, start_offset, data.len())?;
                        let snap = {
                            let mut dls = state.downloads.lock().await;
                            dls.inject_range(&download_id, start_offset, &data)?
                        };
                        state.broadcast_download_update(snap);
                    }
                    LabMessage::BlockRejected {
                        file_hash: rej_hash,
                        start_offset,
                        ..
                    } => {
                        if let Ok(req) = tracker.handle_rejection(&rej_hash, start_offset) {
                            pending_blocks.push_back((req.start_offset, req.length));
                        }
                    }
                    LabMessage::Goodbye { .. } => {
                        break;
                    }
                    _ => {}
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                let timeouts = tracker.reap_timeouts(Duration::from_secs(10));
                for req in timeouts {
                    pending_blocks.push_back((req.start_offset, req.length));
                }
            }
        }
    }

    // Check if download is fully received
    let is_complete = {
        let dls = state.downloads.lock().await;
        dls.get_missing_ranges(&download_id)
            .map(|m| m.is_empty())
            .unwrap_or(false)
    };

    if !is_complete {
        return Ok(());
    }

    // 7. Send Goodbye
    let goodbye = LabMessage::Goodbye {
        reason: "Download complete".into(),
    };
    let _ = send_msg(&mut socket, &codec, &goodbye).await;

    // 8. Verification & Finalization
    let mut file = std::fs::File::open(&temp_path)?;
    let computed_result = mule_format::ed2k::hash_reader(&mut file)?;

    if computed_result.root_hash == expected_hash {
        let mut dls = state.downloads.lock().await;
        let snap = dls.finalize_download(&download_id)?;
        state.broadcast_download_update(snap);
        info!(download_id = %download_id, "Download successfully verified and finalized");
        Ok(())
    } else {
        let mut dls = state.downloads.lock().await;
        let snap = dls.update_status(&download_id, DownloadStatus::Failed)?;
        state.broadcast_download_update(snap);
        error!(download_id = %download_id, "eD2K hash verification failed");
        Err("eD2K hash mismatch".into())
    }
}

async fn send_msg(
    stream: &mut TcpStream,
    codec: &PacketCodec,
    msg: &LabMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let raw = msg.to_raw_packet()?;
    let mut buf = BytesMut::new();
    codec.encode(&raw, &mut buf)?;
    stream.write_all(&buf).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_msg(
    stream: &mut TcpStream,
    codec: &PacketCodec,
    buf: &mut BytesMut,
) -> Result<LabMessage, Box<dyn std::error::Error + Send + Sync>> {
    loop {
        if let Some(packet) = codec.decode(buf)? {
            return Ok(LabMessage::from_raw_packet(&packet)?);
        }
        let mut chunk = [0u8; 8192];
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err("Unexpected EOF on socket".into());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}
