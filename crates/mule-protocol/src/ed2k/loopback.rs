//! Controlled loopback transport and test peer for closed eD2K handshake verification.
//! Strictly confined to the local loopback interface (127.0.0.1 / ::1).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::timeout;

use super::decoder::Ed2kDecoder;
use super::encoder::Ed2kEncoder;
use super::error::Ed2kParseError;
use super::opcodes::*;
use super::packet::{
    Ed2kBlockRange, Ed2kFileStatus, Ed2kFileStatusReq, Ed2kHello, Ed2kHelloAnswer, Ed2kMessage,
    Ed2kRequestParts, Ed2kSendingPart, PartAvailabilityMask,
};
use super::peer_info::Ed2kPeerInfo;
use super::profile::Ed2kCompatibilityProfile;
use super::protocol::{PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED};
use super::sequencer::{Ed2kDownloadSequencer, MAX_REQUEST_RANGES_PER_PACKET};
use super::session::{Ed2kSession, Ed2kSessionError, Ed2kSessionState};
use super::storage::Ed2kBlockWriter;
use super::tags::{Ed2kTag, TagName, TagValue, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION};
use crate::framing::{PacketCodec, RawPacket};

pub fn decode_test_file_status_req(
    decoder: &Ed2kDecoder,
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<Ed2kFileStatusReq, Ed2kParseError> {
    let profile = if opcode == OP_SETREQFILEID_EMULE {
        Ed2kCompatibilityProfile::Emule050a
    } else {
        Ed2kCompatibilityProfile::LegacyEdonkey
    };
    decoder.decode_file_status_req_with_profile(profile, protocol, opcode, payload, 0)
}

pub fn decode_test_request_parts(
    decoder: &Ed2kDecoder,
    protocol: u8,
    opcode: u8,
    payload: &[u8],
) -> Result<Ed2kRequestParts, Ed2kParseError> {
    let profile = if opcode == OP_REQUESTPARTS_EMULE {
        Ed2kCompatibilityProfile::Emule050a
    } else {
        Ed2kCompatibilityProfile::LegacyEdonkey
    };
    decoder.decode_request_parts_with_profile(profile, protocol, opcode, payload, 0)
}

/// Configurable test peer emulation modes for adversarial and compliance validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestPeerMode {
    /// Nominal compliance: waits for initiator's Hello, responds with HelloAnswer, serves file statuses.
    #[default]
    Nominal,
    /// Transmits the HelloAnswer packet byte-by-byte with small delays.
    FragmentedByteByByte,
    /// Violates protocol by sending HelloAnswer before receiving Hello.
    HelloAnswerBeforeHello,
    /// Violates protocol by sending two consecutive Hello packets.
    DoubleHello,
    /// Emits a packet with unsupported compressed protocol byte (0xD4).
    UnsupportedProtocol0xD4,
    /// Emits an unexpected opcode during handshake.
    UnexpectedOpcode,
    /// Sends a HelloAnswer with an invalid user hash (all zeros).
    ZeroUserHash,
    /// Does not respond to Hello, causing handshake timeout.
    HangTimeout,
    /// Immediately closes the TCP socket without sending any data.
    ImmediateClose,
    /// Sends an OP_FILESTATUS packet before handshake completion.
    FileStatusBeforeReady,
    /// Responds to OP_FILESTATUSREQ with a mismatched file hash.
    FileStatusMismatchHash,
    /// Responds with a truncated bitfield mask (shorter than expected).
    FileStatusShortMask,
    /// Responds with extraneous trailing bytes after the bitfield mask.
    FileStatusLongMask,
    /// Responds with part_count exceeding safety limits.
    FileStatusExcessiveParts,
    /// Responds with trailing unused bits in the last byte set to 1.
    FileStatusTrailingBitsNonZero,
    /// Responds to OP_FILESTATUSREQ with unsupported compressed 0xD4 frame.
    FileStatusCompressed0xD4,
    /// Completes handshake but never responds to OP_FILESTATUSREQ.
    FileStatusHangTimeout,
    /// Emits an unsolicited OP_FILESTATUS after handshake.
    FileStatusUnsolicited,
    /// Sends two identical OP_FILESTATUS packets consecutively for one request.
    FileStatusDoubleAnswer,
    /// Sends an OP_SENDINGPART packet before handshake completion.
    SendingPartBeforeReady,
    /// Responds to OP_REQUESTPARTS with a mismatched file hash.
    SendingPartUnexpectedHash,
    /// Responds to OP_REQUESTPARTS with an unexpected offset range.
    SendingPartUnexpectedOffset,
    /// Responds to OP_REQUESTPARTS with payload length != (end - start).
    SendingPartLengthMismatch,
    /// Emits an unsolicited OP_SENDINGPART after handshake without prior request.
    SendingPartUnsolicited,
    /// Sends duplicate OP_SENDINGPART packets for a single request.
    SendingPartDuplicate,
    /// Never responds to OP_REQUESTPARTS, triggering timeout.
    SendingPartHangTimeout,
    /// Responds to OP_REQUESTPARTS with unsupported compressed 0xD4 frame.
    SendingPartCompressed0xD4,
    /// Immediately closes connection upon receiving OP_REQUESTPARTS.
    SendingPartImmediateClose,
}

/// Active loopback client establishing and driving eD2K sessions.
pub struct Ed2kLoopbackClient {
    stream: TcpStream,
    session: Ed2kSession,
    codec: PacketCodec,
    encoder: Ed2kEncoder,
    read_buf: BytesMut,
}

impl Ed2kLoopbackClient {
    /// Connects to a target socket address (enforcing loopback only) and initiates handshake.
    #[allow(clippy::too_many_arguments)]
    pub async fn connect_and_handshake(
        addr: SocketAddr,
        local_user_hash: [u8; 16],
        local_client_id: u32,
        local_tcp_port: u16,
        tags: Vec<Ed2kTag>,
        server_ip: u32,
        server_port: u16,
        handshake_timeout: Duration,
    ) -> Result<(Self, Ed2kPeerInfo), Ed2kSessionError> {
        // Enforce loopback confinement
        if !addr.ip().is_loopback() {
            return Err(Ed2kSessionError::Io(format!(
                "Non-loopback address {addr} is strictly forbidden"
            )));
        }

        let stream = timeout(Duration::from_millis(500), TcpStream::connect(addr))
            .await
            .map_err(|_| Ed2kSessionError::Timeout(Duration::from_millis(500)))?
            .map_err(|e| Ed2kSessionError::Io(format!("TCP connect to loopback failed: {e}")))?;

        let mut session = Ed2kSession::new_initiator(local_user_hash, local_client_id, local_tcp_port);
        let hello = session.create_hello(tags, server_ip, server_port)?;

        let encoder = Ed2kEncoder::new();
        let wire_bytes = encoder
            .encode_to_bytes(&Ed2kMessage::Hello(hello))
            .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;

        let mut client = Self {
            stream,
            session,
            codec: PacketCodec::new(),
            encoder,
            read_buf: BytesMut::with_capacity(2048),
        };

        // Transmit Hello frame
        client
            .stream
            .write_all(&wire_bytes)
            .await
            .map_err(|e| Ed2kSessionError::Io(format!("Failed to transmit Hello: {e}")))?;

        // Await HelloAnswer within timeout
        let peer_info = client.await_handshake_answer(handshake_timeout).await?;
        Ok((client, peer_info))
    }

    /// Waits for `OP_HELLO_ANSWER` from the remote loopback peer.
    async fn await_handshake_answer(
        &mut self,
        handshake_timeout: Duration,
    ) -> Result<Ed2kPeerInfo, Ed2kSessionError> {
        let deadline = tokio::time::Instant::now() + handshake_timeout;

        loop {
            // Check if buffer already contains a packet
            if let Some(raw_packet) = self
                .codec
                .decode(&mut self.read_buf)
                .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?
            {
                let msg_opt = self.session.on_packet_received(&raw_packet)?;
                if let Some(Ed2kMessage::HelloAnswer(answer)) = msg_opt {
                    let mut peer = Ed2kPeerInfo::from_hello_answer(&answer);
                    if let Some(session_peer) = self.session.peer_info() {
                        if peer.emule_info.is_none() {
                            peer.emule_info = session_peer.emule_info.clone();
                        }
                    }
                    return Ok(peer);
                } else if raw_packet.protocol == PR_EMULE {
                    // Extended eMule packet (e.g. OP_EMULEINFO under PR_EMULE 0xC5) received alongside handshake.
                    // Continue awaiting OP_HELLOANSWER in the stream.
                    continue;
                } else {
                    return Err(Ed2kSessionError::UnexpectedMessageInState {
                        state: "AwaitingHelloAnswer",
                        opcode: raw_packet.opcode,
                    });
                }
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                self.session.on_timeout(handshake_timeout);
                return Err(Ed2kSessionError::Timeout(handshake_timeout));
            }

            let mut tmp = [0u8; 1024];
            let read_future = self.stream.read(&mut tmp);
            let n = match timeout(remaining, read_future).await {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    self.session.fail(Ed2kSessionError::Io(e.to_string()));
                    return Err(Ed2kSessionError::Io(format!("Socket read error: {e}")));
                }
                Err(_) => {
                    self.session.on_timeout(handshake_timeout);
                    return Err(Ed2kSessionError::Timeout(handshake_timeout));
                }
            };

            if n == 0 {
                self.session.on_remote_closed();
                return Err(Ed2kSessionError::Io("Connection closed by peer before HelloAnswer".to_string()));
            }

            self.read_buf.extend_from_slice(&tmp[..n]);
        }
    }

    /// Sends any eD2K message to the connected loopback peer.
    pub async fn send_message(&mut self, msg: &Ed2kMessage) -> Result<(), Ed2kSessionError> {
        let bytes = self
            .encoder
            .encode_to_bytes(msg)
            .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
        self.stream
            .write_all(&bytes)
            .await
            .map_err(|e| Ed2kSessionError::Io(format!("Write failed: {e}")))?;
        Ok(())
    }

    /// Reads the next incoming packet from the loopback peer.
    pub async fn recv_packet(&mut self) -> Result<RawPacket, Ed2kSessionError> {
        let mut tmp = [0u8; 1024];
        loop {
            if let Some(packet) = self
                .codec
                .decode(&mut self.read_buf)
                .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?
            {
                return Ok(packet);
            }
            let n = self.stream.read(&mut tmp).await.map_err(|e| {
                Ed2kSessionError::Io(format!("Read failed: {e}"))
            })?;
            if n == 0 {
                return Err(Ed2kSessionError::Io("EOF reached".to_string()));
            }
            self.read_buf.extend_from_slice(&tmp[..n]);
        }
    }

    /// Returns a reference to the underlying session state machine.
    pub fn session(&self) -> &Ed2kSession {
        &self.session
    }

    /// Returns a mutable reference to the underlying session state machine.
    pub fn session_mut(&mut self) -> &mut Ed2kSession {
        &mut self.session
    }

    /// Returns a reference to the negotiated remote peer info if available.
    pub fn peer_info(&self) -> Option<&Ed2kPeerInfo> {
        self.session.peer_info()
    }

    /// Sends a raw frame directly over the TCP stream.
    pub async fn send_raw_packet(&mut self, packet: &RawPacket) -> Result<(), Ed2kSessionError> {
        let wire_bytes = self
            .codec
            .encode_to_bytes(packet)
            .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
        self.stream
            .write_all(&wire_bytes)
            .await
            .map_err(|e| Ed2kSessionError::Io(format!("Failed to write raw packet: {e}")))?;
        Ok(())
    }

    /// Sends an `OP_FILESTATUSREQ` and waits for the corresponding `OP_FILESTATUS` response.
    pub async fn request_file_status(
        &mut self,
        file_hash: [u8; 16],
        timeout_duration: Duration,
    ) -> Result<Ed2kFileStatus, Ed2kSessionError> {
        let now = Instant::now();
        let packet = self.session.send_file_status_request(file_hash, now)?;
        let wire_bytes = self
            .codec
            .encode_to_bytes(&packet)
            .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
        self.stream
            .write_all(&wire_bytes)
            .await
            .map_err(|e| Ed2kSessionError::Io(format!("Failed to send file status req: {e}")))?;

        let deadline = tokio::time::Instant::now() + timeout_duration;
        loop {
            if let Some(raw_packet) = self
                .codec
                .decode(&mut self.read_buf)
                .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?
            {
                let msg_opt = self.session.on_packet_received(&raw_packet)?;
                if let Some(Ed2kMessage::FileStatus(status)) = msg_opt {
                    return Ok(status);
                } else if let Some(Ed2kMessage::FileReqAnsNoFile(ans)) = msg_opt {
                    return Err(Ed2kSessionError::FileNotFound(hex::encode(ans.file_hash)));
                } else {
                    return Err(Ed2kSessionError::UnexpectedMessageInState {
                        state: self.session.state.name(),
                        opcode: raw_packet.opcode,
                    });
                }
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                self.session
                    .expire_file_status_requests(timeout_duration, Instant::now());
                return Err(Ed2kSessionError::Timeout(timeout_duration));
            }

            let mut tmp = [0u8; 1024];
            let read_future = self.stream.read(&mut tmp);
            let n = match timeout(remaining, read_future).await {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    self.session.fail(Ed2kSessionError::Io(e.to_string()));
                    return Err(Ed2kSessionError::Io(format!("Socket read error: {e}")));
                }
                Err(_) => {
                    self.session
                        .expire_file_status_requests(timeout_duration, Instant::now());
                    return Err(Ed2kSessionError::Timeout(timeout_duration));
                }
            };

            if n == 0 {
                self.session.on_remote_closed();
                return Err(Ed2kSessionError::Io(
                    "Connection closed by peer before file status".to_string(),
                ));
            }

            self.read_buf.extend_from_slice(&tmp[..n]);
        }
    }

    /// Sends an `OP_REQUESTPARTS` block request.
    pub async fn send_block_request(
        &mut self,
        file_hash: [u8; 16],
        start: u64,
        end: u64,
        file_size: u64,
    ) -> Result<(), Ed2kSessionError> {
        let now = Instant::now();
        let packet = self
            .session
            .send_block_request(file_hash, start, end, file_size, now)?;
        let wire_bytes = self
            .codec
            .encode_to_bytes(&packet)
            .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
        self.stream
            .write_all(&wire_bytes)
            .await
            .map_err(|e| Ed2kSessionError::Io(format!("Failed to send request parts: {e}")))?;
        Ok(())
    }

    /// Sends an `OP_STARTUPLOADREQ` (`0x54`) to enter the upload queue for a file.
    pub async fn send_start_upload_request(
        &mut self,
        file_hash: [u8; 16],
    ) -> Result<(), Ed2kSessionError> {
        let packet = RawPacket::with_vec(PR_ED2K, OP_STARTUPLOADREQ, file_hash.to_vec());
        let wire_bytes = self
            .codec
            .encode_to_bytes(&packet)
            .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
        self.stream
            .write_all(&wire_bytes)
            .await
            .map_err(|e| Ed2kSessionError::Io(format!("Failed to send start upload request: {e}")))?;
        Ok(())
    }

    /// Reads the next incoming packet from the stream.
    pub async fn read_next_packet(
        &mut self,
        timeout_duration: Duration,
    ) -> Result<RawPacket, Ed2kSessionError> {
        let deadline = tokio::time::Instant::now() + timeout_duration;
        loop {
            if let Some(packet) = self
                .codec
                .decode(&mut self.read_buf)
                .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?
            {
                return Ok(packet);
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(Ed2kSessionError::Timeout(timeout_duration));
            }

            let mut tmp = [0u8; 8192];
            let read_future = self.stream.read(&mut tmp);
            let n = match timeout(remaining, read_future).await {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => return Err(Ed2kSessionError::Io(format!("Socket read error: {e}"))),
                Err(_) => return Err(Ed2kSessionError::Timeout(timeout_duration)),
            };

            if n == 0 {
                return Err(Ed2kSessionError::Io("Connection closed by peer".to_string()));
            }

            self.read_buf.extend_from_slice(&tmp[..n]);
        }
    }

    /// Requests entry into the remote peer's upload queue and awaits upload slot acceptance (`OP_ACCEPTUPLOADREQ` 0x55).
    pub async fn request_upload_slot(
        &mut self,
        file_hash: [u8; 16],
        timeout_duration: Duration,
    ) -> Result<(), Ed2kSessionError> {
        self.send_start_upload_request(file_hash).await?;
        let deadline = tokio::time::Instant::now() + timeout_duration;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(Ed2kSessionError::Timeout(timeout_duration));
            }
            let pkt = self.read_next_packet(remaining).await?;
            if pkt.opcode == OP_ACCEPTUPLOADREQ {
                return Ok(());
            }
        }
    }

    /// Waits for an incoming `OP_SENDINGPART` response within timeout.
    pub async fn receive_sending_part(
        &mut self,
        timeout_duration: Duration,
    ) -> Result<Ed2kSendingPart, Ed2kSessionError> {
        let deadline = tokio::time::Instant::now() + timeout_duration;
        loop {
            if let Some(raw_packet) = self
                .codec
                .decode(&mut self.read_buf)
                .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?
            {
                let msg_opt = self.session.on_packet_received(&raw_packet)?;
                if let Some(Ed2kMessage::SendingPart(part)) = msg_opt {
                    return Ok(part);
                } else if raw_packet.protocol == PR_EMULE || raw_packet.opcode == OP_QUEUERANKING {
                    continue;
                } else {
                    return Err(Ed2kSessionError::UnexpectedMessageInState {
                        state: self.session.state.name(),
                        opcode: raw_packet.opcode,
                    });
                }
            }

            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                self.session
                    .expire_block_requests(timeout_duration, Instant::now());
                return Err(Ed2kSessionError::Timeout(timeout_duration));
            }

            let mut tmp = [0u8; 8192];
            let read_future = self.stream.read(&mut tmp);
            let n = match timeout(remaining, read_future).await {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    self.session.fail(Ed2kSessionError::Io(e.to_string()));
                    return Err(Ed2kSessionError::Io(format!("Socket read error: {e}")));
                }
                Err(_) => {
                    self.session
                        .expire_block_requests(timeout_duration, Instant::now());
                    return Err(Ed2kSessionError::Timeout(timeout_duration));
                }
            };

            if n == 0 {
                self.session.on_remote_closed();
                return Err(Ed2kSessionError::Io(
                    "Connection closed by peer before sending part".to_string(),
                ));
            }

            self.read_buf.extend_from_slice(&tmp[..n]);
        }
    }

    /// Downloads a sequence of blocks using a bounded sliding window of in-flight requests.
    pub async fn download_blocks_pipelined<W: Ed2kBlockWriter>(
        &mut self,
        sequencer: &Ed2kDownloadSequencer,
        blocks_to_download: &[Ed2kBlockRange],
        sink: &mut W,
        max_in_flight: usize,
        stop_after_blocks: Option<usize>,
        chunk_timeout: Duration,
    ) -> Result<usize, Ed2kSessionError> {
        let window_size = max_in_flight.clamp(1, MAX_REQUEST_RANGES_PER_PACKET);
        let target_total = match stop_after_blocks {
            Some(limit) => limit.min(blocks_to_download.len()),
            None => blocks_to_download.len(),
        };

        let mut next_dispatch_idx = 0;
        let mut dispatched_count = 0;
        let mut completed_blocks_count = 0;
        let mut active_indices = std::collections::HashSet::new();

        while completed_blocks_count < target_total {
            while self.session.snapshot().pending_block_count < window_size
                && dispatched_count < target_total
                && next_dispatch_idx < blocks_to_download.len()
            {
                let block = &blocks_to_download[next_dispatch_idx];
                let b_start = block.start as u64;
                let b_end = block.end as u64;

                if sink.has_range(b_start, b_end) {
                    next_dispatch_idx += 1;
                    continue;
                }

                self.send_block_request(
                    sequencer.file_hash(),
                    b_start,
                    b_end,
                    sequencer.file_size(),
                )
                .await?;

                active_indices.insert(next_dispatch_idx);
                dispatched_count += 1;
                next_dispatch_idx += 1;
            }

            if self.session.snapshot().pending_block_count == 0 {
                break;
            }

            let part = self.receive_sending_part(chunk_timeout).await?;
            let _written_range = sink
                .write_block(sequencer, &part)
                .map_err(|e| Ed2kSessionError::Io(e.to_string()))?;

            let mut newly_completed = Vec::new();
            for &idx in &active_indices {
                let block = &blocks_to_download[idx];
                if sink.has_range(block.start as u64, block.end as u64) {
                    newly_completed.push(idx);
                }
            }

            for idx in newly_completed {
                active_indices.remove(&idx);
                completed_blocks_count += 1;
            }
        }

        Ok(completed_blocks_count)
    }

    /// High-level helper: requests a block range and awaits the response packet.
    pub async fn request_block(
        &mut self,
        file_hash: [u8; 16],
        start: u64,
        end: u64,
        file_size: u64,
        timeout_duration: Duration,
    ) -> Result<Ed2kSendingPart, Ed2kSessionError> {
        self.send_block_request(file_hash, start, end, file_size)
            .await?;
        self.receive_sending_part(timeout_duration).await
    }

    /// Gracefully shuts down the client connection.
    pub async fn close(mut self) {
        let _ = self.stream.shutdown().await;
        self.session.state = Ed2kSessionState::Closed;
    }
}

/// Controlled loopback test peer operating solely on 127.0.0.1.
pub struct Ed2kTestPeer {
    listener: TcpListener,
    local_addr: SocketAddr,
    mode: TestPeerMode,
    local_user_hash: [u8; 16],
    client_id: u32,
    tcp_port: u16,
    received_hello: Arc<Mutex<Option<Ed2kHello>>>,
    handshake_success: Arc<AtomicBool>,
    shared_files: Arc<Mutex<HashMap<[u8; 16], Ed2kFileStatus>>>,
    shared_file_data: Arc<Mutex<HashMap<[u8; 16], bytes::Bytes>>>,
}

impl Ed2kTestPeer {
    /// Binds a new test peer on a free ephemeral port on 127.0.0.1.
    pub async fn bind(mode: TestPeerMode) -> Result<Self, Ed2kSessionError> {
        let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
            Ed2kSessionError::Io(format!("Failed to bind loopback test peer: {e}"))
        })?;
        let local_addr = listener.local_addr().map_err(|e| {
            Ed2kSessionError::Io(format!("Failed to retrieve test peer local address: {e}"))
        })?;

        // Standard test peer identity
        let mut local_user_hash = [0xEE; 16];
        local_user_hash[0] = 0xAA;

        Ok(Self {
            listener,
            local_addr,
            mode,
            local_user_hash,
            client_id: 0x7F000001,
            tcp_port: local_addr.port(),
            received_hello: Arc::new(Mutex::new(None)),
            handshake_success: Arc::new(AtomicBool::new(false)),
            shared_files: Arc::new(Mutex::new(HashMap::new())),
            shared_file_data: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Registers a file's content data in memory to serve block requests.
    pub async fn add_shared_file_data(&self, file_hash: [u8; 16], data: bytes::Bytes) {
        self.shared_file_data.lock().await.insert(file_hash, data);
    }

    /// Registers a shared file on the test peer with specified part count and availability mask.
    pub async fn add_shared_file(
        &self,
        file_hash: [u8; 16],
        part_count: u16,
        mask: PartAvailabilityMask,
    ) {
        let status = Ed2kFileStatus {
            file_hash,
            part_count,
            availability_mask: mask,
        };
        self.shared_files.lock().await.insert(file_hash, status);
    }

    /// Returns the local loopback listening socket address.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Returns a handle to check the received initiator Hello.
    pub fn received_hello_handle(&self) -> Arc<Mutex<Option<Ed2kHello>>> {
        Arc::clone(&self.received_hello)
    }

    /// Returns true if the handshake successfully completed on the test peer.
    pub fn is_handshake_successful(&self) -> bool {
        self.handshake_success.load(Ordering::SeqCst)
    }

    /// Runs the test peer continuously accepting connections in Nominal mode.
    pub fn spawn_service(self) -> (tokio::task::JoinHandle<()>, Arc<AtomicBool>) {
        let stopped = Arc::new(AtomicBool::new(false));
        let stopped_clone = Arc::clone(&stopped);
        let handle = tokio::spawn(async move {
            while !stopped_clone.load(Ordering::SeqCst) {
                let (mut stream, _) = match self.listener.accept().await {
                    Ok(res) => res,
                    Err(_) => break,
                };
                let local_user_hash = self.local_user_hash;
                let client_id = self.client_id;
                let tcp_port = self.tcp_port;
                let received_hello = Arc::clone(&self.received_hello);
                let handshake_success = Arc::clone(&self.handshake_success);
                let shared_files = Arc::clone(&self.shared_files);
                let shared_file_data = Arc::clone(&self.shared_file_data);

                tokio::spawn(async move {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(2048);
                    if do_peer_handshake(
                        &mut stream,
                        local_user_hash,
                        client_id,
                        tcp_port,
                        &received_hello,
                        &handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await
                    .is_err()
                    {
                        return;
                    }

                    let decoder = Ed2kDecoder::new();
                    let encoder = Ed2kEncoder::new();

                    loop {
                        let packet = match read_packet(&mut stream, &mut codec, &mut buf).await {
                            Ok(pkt) => pkt,
                            Err(_) => break,
                        };

                        if packet.opcode == OP_FILESTATUSREQ || packet.opcode == OP_SETREQFILEID_EMULE {
                            let req = match decode_test_file_status_req(
                                &decoder,
                                packet.protocol,
                                packet.opcode,
                                &packet.payload,
                            ) {
                                Ok(r) => r,
                                Err(_) => break,
                            };
                            let shared = shared_files.lock().await;
                            let status = if let Some(st) = shared.get(&req.file_hash) {
                                st.clone()
                            } else {
                                Ed2kFileStatus {
                                    file_hash: req.file_hash,
                                    part_count: 1,
                                    availability_mask: PartAvailabilityMask::from_parts(&[0], 1),
                                }
                            };
                            drop(shared);

                            if let Ok(resp_bytes) =
                                encoder.encode_to_bytes(&Ed2kMessage::FileStatus(status))
                            {
                                if stream.write_all(&resp_bytes).await.is_err() {
                                    break;
                                }
                            }
                        } else if packet.opcode == OP_REQUESTPARTS || packet.opcode == OP_REQUESTPARTS_EMULE {
                            let req = match decode_test_request_parts(
                                &decoder,
                                packet.protocol,
                                packet.opcode,
                                &packet.payload,
                            ) {
                                Ok(r) => r,
                                Err(_) => break,
                            };
                            let file_data_map = shared_file_data.lock().await;
                            let file_data = file_data_map.get(&req.file_hash).cloned();
                            drop(file_data_map);

                            for range in req.ranges {
                                let start = range.start as usize;
                                let end = range.end as usize;
                                let chunk_data = if let Some(ref data) = file_data {
                                    if end <= data.len() {
                                        data.slice(start..end)
                                    } else {
                                        bytes::Bytes::from(vec![0xAA; end.saturating_sub(start)])
                                    }
                                } else {
                                    let len = end.saturating_sub(start);
                                    let mut v = Vec::with_capacity(len);
                                    for i in 0..len {
                                        v.push(((start + i) % 256) as u8);
                                    }
                                    bytes::Bytes::from(v)
                                };

                                let sending_part = Ed2kSendingPart {
                                    file_hash: req.file_hash,
                                    start_offset: range.start,
                                    end_offset: range.end,
                                    data: chunk_data,
                                };
                                if let Ok(resp_bytes) =
                                    encoder.encode_to_bytes(&Ed2kMessage::SendingPart(sending_part))
                                {
                                    if stream.write_all(&resp_bytes).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        } else if packet.opcode == OP_STARTUPLOADREQ {
                            let resp = RawPacket::with_vec(PR_ED2K, OP_ACCEPTUPLOADREQ, Vec::new());
                            if let Ok(resp_bytes) = codec.encode_to_bytes(&resp) {
                                if stream.write_all(&resp_bytes).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                });
            }
        });
        (handle, stopped)
    }

    /// Runs the test peer handling a single incoming connection according to configured mode.
    pub fn spawn(self) -> tokio::task::JoinHandle<Result<(), Ed2kSessionError>> {
        tokio::spawn(async move {
            let (mut stream, _) = self.listener.accept().await.map_err(|e| {
                Ed2kSessionError::Io(format!("Test peer failed to accept connection: {e}"))
            })?;

            match self.mode {
                TestPeerMode::ImmediateClose => {
                    let _ = stream.shutdown().await;
                    Ok(())
                }
                TestPeerMode::HangTimeout => {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    let _ = stream.shutdown().await;
                    Ok(())
                }
                TestPeerMode::HelloAnswerBeforeHello => {
                    let answer = Ed2kHelloAnswer {
                        user_hash: self.local_user_hash,
                        client_id: self.client_id,
                        tcp_port: self.tcp_port,
                        tags: vec![Ed2kTag {
                            name: TagName::Id(TAG_SPECIAL_NAME),
                            value: TagValue::String("EarlyAnswerPeer".to_string()),
                        }],
                        unknown_tags: vec![],
                        server_ip: 0x7F000001,
                        server_port: 4661,
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::HelloAnswer(answer))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write early answer: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::DoubleHello => {
                    let hello = Ed2kHello {
                        user_hash: self.local_user_hash,
                        client_id: self.client_id,
                        tcp_port: self.tcp_port,
                        tags: vec![Ed2kTag {
                            name: TagName::Id(TAG_SPECIAL_NAME),
                            value: TagValue::String("DoubleHelloPeer".to_string()),
                        }],
                        unknown_tags: vec![],
                        server_ip: 0,
                        server_port: 0,
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::Hello(hello))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write first hello: {e}"))
                    })?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write second hello: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::UnsupportedProtocol0xD4 => {
                    let fake_frame = [PR_EMULE_COMPRESSED, 0x01, 0x00, 0x00, 0x00, OP_HELLO];
                    stream.write_all(&fake_frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write 0xD4 frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::UnexpectedOpcode => {
                    let fake_frame = [PR_ED2K, 0x01, 0x00, 0x00, 0x00, 0x99];
                    stream.write_all(&fake_frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write unknown opcode frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::ZeroUserHash => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    let hello_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let hello = decoder.decode_hello(
                        hello_packet.protocol,
                        hello_packet.opcode,
                        &hello_packet.payload,
                        0,
                    )?;
                    *self.received_hello.lock().await = Some(hello);

                    let answer = Ed2kHelloAnswer {
                        user_hash: [0u8; 16],
                        client_id: self.client_id,
                        tcp_port: self.tcp_port,
                        tags: vec![],
                        unknown_tags: vec![],
                        server_ip: 0,
                        server_port: 0,
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::HelloAnswer(answer))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write zero hash answer: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::FragmentedByteByByte => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    let hello_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let hello = decoder.decode_hello(
                        hello_packet.protocol,
                        hello_packet.opcode,
                        &hello_packet.payload,
                        0,
                    )?;
                    *self.received_hello.lock().await = Some(hello);

                    let answer = Ed2kHelloAnswer {
                        user_hash: self.local_user_hash,
                        client_id: self.client_id,
                        tcp_port: self.tcp_port,
                        tags: vec![
                            Ed2kTag {
                                name: TagName::Id(TAG_SPECIAL_NAME),
                                value: TagValue::String("FragmentedPeer".to_string()),
                            },
                            Ed2kTag {
                                name: TagName::Id(TAG_SPECIAL_VERSION),
                                value: TagValue::UInt32(60),
                            },
                        ],
                        unknown_tags: vec![],
                        server_ip: 0x7F000001,
                        server_port: 4661,
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::HelloAnswer(answer))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;

                    for b in bytes.as_ref() {
                        stream.write_all(&[*b]).await.map_err(|e| {
                            Ed2kSessionError::Io(format!("Failed writing fragmented byte: {e}"))
                        })?;
                        tokio::time::sleep(Duration::from_millis(2)).await;
                    }
                    self.handshake_success.store(true, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok(())
                }
                TestPeerMode::Nominal => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let decoder = Ed2kDecoder::new();
                    let encoder = Ed2kEncoder::new();

                    loop {
                        let packet = match read_packet(&mut stream, &mut codec, &mut buf).await {
                            Ok(pkt) => pkt,
                            Err(Ed2kSessionError::Io(ref s)) if s.contains("EOF") => break,
                            Err(e) => return Err(e),
                        };

                        if packet.opcode == OP_FILESTATUSREQ || packet.opcode == OP_SETREQFILEID_EMULE {
                            let req = decode_test_file_status_req(
                                &decoder,
                                packet.protocol,
                                packet.opcode,
                                &packet.payload,
                            )?;
                            let shared = self.shared_files.lock().await;
                            let status = if let Some(st) = shared.get(&req.file_hash) {
                                st.clone()
                            } else {
                                Ed2kFileStatus {
                                    file_hash: req.file_hash,
                                    part_count: 1,
                                    availability_mask: PartAvailabilityMask::from_parts(&[0], 1),
                                }
                            };
                            drop(shared);

                            let resp_bytes = encoder
                                .encode_to_bytes(&Ed2kMessage::FileStatus(status))
                                .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                            stream.write_all(&resp_bytes).await.map_err(|e| {
                                Ed2kSessionError::Io(format!("Failed to write file status: {e}"))
                            })?;
                        } else if packet.opcode == OP_REQUESTPARTS || packet.opcode == OP_REQUESTPARTS_EMULE {
                            let req = decode_test_request_parts(
                                &decoder,
                                packet.protocol,
                                packet.opcode,
                                &packet.payload,
                            )?;
                            let file_data_map = self.shared_file_data.lock().await;
                            let file_data = file_data_map.get(&req.file_hash).cloned();
                            drop(file_data_map);

                            for range in req.ranges {
                                let start = range.start as usize;
                                let end = range.end as usize;
                                let chunk_data = if let Some(ref data) = file_data {
                                    if end <= data.len() {
                                        data.slice(start..end)
                                    } else {
                                        bytes::Bytes::from(vec![0xAA; end.saturating_sub(start)])
                                    }
                                } else {
                                    let len = end.saturating_sub(start);
                                    let mut v = Vec::with_capacity(len);
                                    for i in 0..len {
                                        v.push(((start + i) % 256) as u8);
                                    }
                                    bytes::Bytes::from(v)
                                };

                                let sending_part = Ed2kSendingPart {
                                    file_hash: req.file_hash,
                                    start_offset: range.start,
                                    end_offset: range.end,
                                    data: chunk_data,
                                };
                                let resp_bytes = encoder
                                    .encode_to_bytes(&Ed2kMessage::SendingPart(sending_part))
                                    .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                                stream.write_all(&resp_bytes).await.map_err(|e| {
                                    Ed2kSessionError::Io(format!("Failed to write sending part: {e}"))
                                })?;
                            }
                        }
                    }
                    Ok(())
                }
                TestPeerMode::FileStatusBeforeReady => {
                    let status = Ed2kFileStatus {
                        file_hash: [0x11; 16],
                        part_count: 1,
                        availability_mask: PartAvailabilityMask::from_parts(&[0], 1),
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::FileStatus(status))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write early file status: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::FileStatusMismatchHash => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let _req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let status = Ed2kFileStatus {
                        file_hash: [0x99; 16],
                        part_count: 1,
                        availability_mask: PartAvailabilityMask::from_parts(&[0], 1),
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::FileStatus(status))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write mismatch status: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::FileStatusShortMask => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_file_status_req(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;

                    let mut frame = vec![PR_ED2K, 20, 0, 0, 0, OP_FILESTATUS];
                    frame.extend_from_slice(&req.file_hash);
                    frame.extend_from_slice(&10u16.to_le_bytes());
                    frame.push(0x01);
                    stream.write_all(&frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write short mask frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::FileStatusLongMask => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_file_status_req(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;

                    let mut frame = vec![PR_ED2K, 22, 0, 0, 0, OP_FILESTATUS];
                    frame.extend_from_slice(&req.file_hash);
                    frame.extend_from_slice(&10u16.to_le_bytes());
                    frame.extend_from_slice(&[0x01, 0x02, 0x03]);
                    stream.write_all(&frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write long mask frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::FileStatusExcessiveParts => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_file_status_req(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;

                    let frame_len = 1u32 + 16 + 2 + 8192;
                    let mut frame = vec![PR_ED2K];
                    frame.extend_from_slice(&frame_len.to_le_bytes());
                    frame.push(OP_FILESTATUS);
                    frame.extend_from_slice(&req.file_hash);
                    frame.extend_from_slice(&65535u16.to_le_bytes());
                    frame.resize(frame.len() + 8192, 0);
                    stream.write_all(&frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write excessive parts frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::FileStatusTrailingBitsNonZero => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_file_status_req(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;

                    let mut frame = vec![PR_ED2K, 21, 0, 0, 0, OP_FILESTATUS];
                    frame.extend_from_slice(&req.file_hash);
                    frame.extend_from_slice(&10u16.to_le_bytes());
                    frame.extend_from_slice(&[0x03, 0x80]);
                    stream.write_all(&frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write trailing bits frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::FileStatusCompressed0xD4 => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_file_status_req(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;

                    let mut frame = vec![PR_EMULE_COMPRESSED, 20, 0, 0, 0, OP_FILESTATUS];
                    frame.extend_from_slice(&req.file_hash);
                    frame.extend_from_slice(&1u16.to_le_bytes());
                    frame.push(0x01);
                    stream.write_all(&frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write 0xD4 frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::FileStatusHangTimeout => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let _req = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    Ok(())
                }
                TestPeerMode::FileStatusUnsolicited => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let status = Ed2kFileStatus {
                        file_hash: [0x42; 16],
                        part_count: 1,
                        availability_mask: PartAvailabilityMask::from_parts(&[0], 1),
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::FileStatus(status))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write unsolicited file status: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::FileStatusDoubleAnswer => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_file_status_req(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;

                    let status = Ed2kFileStatus {
                        file_hash: req.file_hash,
                        part_count: 1,
                        availability_mask: PartAvailabilityMask::from_parts(&[0], 1),
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::FileStatus(status))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write first status: {e}"))
                    })?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write second status: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::SendingPartBeforeReady => {
                    let part = Ed2kSendingPart {
                        file_hash: [0x11; 16],
                        start_offset: 0,
                        end_offset: 100,
                        data: bytes::Bytes::from(vec![0xAA; 100]),
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::SendingPart(part))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write early sending part: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::SendingPartUnexpectedHash => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_request_parts(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;
                    let range = req.ranges[0];
                    let len = (range.end - range.start) as usize;
                    let part = Ed2kSendingPart {
                        file_hash: [0x99; 16],
                        start_offset: range.start,
                        end_offset: range.end,
                        data: bytes::Bytes::from(vec![0xBB; len]),
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::SendingPart(part))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write unexpected hash part: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::SendingPartUnexpectedOffset => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_request_parts(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;
                    let range = req.ranges[0];
                    let len = (range.end - range.start) as usize;
                    let part = Ed2kSendingPart {
                        file_hash: req.file_hash,
                        start_offset: range.start + 10,
                        end_offset: range.end + 10,
                        data: bytes::Bytes::from(vec![0xCC; len]),
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::SendingPart(part))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write unexpected offset part: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::SendingPartLengthMismatch => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_request_parts(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;
                    let range = req.ranges[0];
                    let data_len = 10;
                    let frame_len = 1u32 + 16 + 4 + 4 + data_len as u32;
                    let mut frame = vec![PR_ED2K];
                    frame.extend_from_slice(&frame_len.to_le_bytes());
                    frame.push(OP_SENDINGPART);
                    frame.extend_from_slice(&req.file_hash);
                    frame.extend_from_slice(&range.start.to_le_bytes());
                    frame.extend_from_slice(&range.end.to_le_bytes());
                    frame.extend_from_slice(&vec![0xDD; data_len]);

                    stream.write_all(&frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write length mismatch frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::SendingPartUnsolicited => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let part = Ed2kSendingPart {
                        file_hash: [0x55; 16],
                        start_offset: 0,
                        end_offset: 100,
                        data: bytes::Bytes::from(vec![0x55; 100]),
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::SendingPart(part))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write unsolicited part: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::SendingPartDuplicate => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_request_parts(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;
                    let range = req.ranges[0];
                    let len = (range.end - range.start) as usize;
                    let part = Ed2kSendingPart {
                        file_hash: req.file_hash,
                        start_offset: range.start,
                        end_offset: range.end,
                        data: bytes::Bytes::from(vec![0x77; len]),
                    };
                    let encoder = Ed2kEncoder::new();
                    let bytes = encoder
                        .encode_to_bytes(&Ed2kMessage::SendingPart(part))
                        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write first part: {e}"))
                    })?;
                    stream.write_all(&bytes).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write duplicate part: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::SendingPartHangTimeout => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let _req = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    Ok(())
                }
                TestPeerMode::SendingPartCompressed0xD4 => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let req_packet = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let decoder = Ed2kDecoder::new();
                    let req = decode_test_request_parts(
                        &decoder,
                        req_packet.protocol,
                        req_packet.opcode,
                        &req_packet.payload,
                    )?;
                    let range = req.ranges[0];
                    let len = (range.end - range.start) as usize;
                    let frame_len = 1u32 + 16 + 4 + 4 + len as u32;
                    let mut frame = vec![PR_EMULE_COMPRESSED];
                    frame.extend_from_slice(&frame_len.to_le_bytes());
                    frame.push(OP_SENDINGPART);
                    frame.extend_from_slice(&req.file_hash);
                    frame.extend_from_slice(&range.start.to_le_bytes());
                    frame.extend_from_slice(&range.end.to_le_bytes());
                    frame.extend_from_slice(&vec![0xEE; len]);

                    stream.write_all(&frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write 0xD4 part frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::SendingPartImmediateClose => {
                    let mut codec = PacketCodec::new();
                    let mut buf = BytesMut::with_capacity(1024);
                    do_peer_handshake(
                        &mut stream,
                        self.local_user_hash,
                        self.client_id,
                        self.tcp_port,
                        &self.received_hello,
                        &self.handshake_success,
                        &mut codec,
                        &mut buf,
                    )
                    .await?;

                    let _req = read_packet(&mut stream, &mut codec, &mut buf).await?;
                    let _ = stream.shutdown().await;
                    Ok(())
                }
            }
        })
    }
}

/// Helper performing standard responder handshake.
#[allow(clippy::too_many_arguments)]
pub async fn do_peer_handshake(
    stream: &mut TcpStream,
    local_user_hash: [u8; 16],
    client_id: u32,
    tcp_port: u16,
    received_hello: &Arc<Mutex<Option<Ed2kHello>>>,
    handshake_success: &Arc<AtomicBool>,
    codec: &mut PacketCodec,
    buf: &mut BytesMut,
) -> Result<(), Ed2kSessionError> {
    let hello_packet = read_packet(stream, codec, buf).await?;
    let decoder = Ed2kDecoder::new();
    let hello = decoder.decode_hello(
        hello_packet.protocol,
        hello_packet.opcode,
        &hello_packet.payload,
        0,
    )?;
    *received_hello.lock().await = Some(hello);

    let answer = Ed2kHelloAnswer {
        user_hash: local_user_hash,
        client_id,
        tcp_port,
        tags: vec![
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_NAME),
                value: TagValue::String("NominalPeer".to_string()),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_VERSION),
                value: TagValue::UInt32(60),
            },
        ],
        unknown_tags: vec![],
        server_ip: 0x7F000001,
        server_port: 4661,
    };
    let encoder = Ed2kEncoder::new();
    let bytes = encoder
        .encode_to_bytes(&Ed2kMessage::HelloAnswer(answer))
        .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?;

    stream.write_all(&bytes).await.map_err(|e| {
        Ed2kSessionError::Io(format!("Failed to write hello answer: {e}"))
    })?;
    handshake_success.store(true, Ordering::SeqCst);
    Ok(())
}

/// Helper reading a single framed packet from an async stream.
pub async fn read_packet(
    stream: &mut TcpStream,
    codec: &mut PacketCodec,
    buf: &mut BytesMut,
) -> Result<RawPacket, Ed2kSessionError> {
    let mut tmp = [0u8; 1024];
    loop {
        if let Some(packet) = codec
            .decode(buf)
            .map_err(|e| Ed2kSessionError::Malformed(e.to_string()))?
        {
            return Ok(packet);
        }
        let n = stream.read(&mut tmp).await.map_err(|e| {
            Ed2kSessionError::Io(format!("Read failed waiting for packet: {e}"))
        })?;
        if n == 0 {
            return Err(Ed2kSessionError::Io("EOF reached before packet completion".to_string()));
        }
        buf.extend_from_slice(&tmp[..n]);
    }
}
