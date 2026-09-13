//! Controlled loopback transport and test peer for closed eD2K handshake verification.
//! Strictly confined to the local loopback interface (127.0.0.1 / ::1).

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::timeout;

use super::decoder::Ed2kDecoder;
use super::encoder::Ed2kEncoder;
use super::opcodes::OP_HELLO;
use super::packet::{Ed2kHello, Ed2kHelloAnswer, Ed2kMessage};

use super::peer_info::Ed2kPeerInfo;
use super::protocol::{PR_ED2K, PR_EMULE_COMPRESSED};
use super::session::{Ed2kSession, Ed2kSessionError, Ed2kSessionState};
use super::tags::{Ed2kTag, TagName, TagValue, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION};
use crate::framing::{PacketCodec, RawPacket};

/// Configurable test peer emulation modes for adversarial and compliance validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TestPeerMode {
    /// Nominal compliance: waits for initiator's Hello, responds with HelloAnswer.
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
        })
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
                    // Send HelloAnswer before client sends Hello
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
                    // Keep connection open briefly so client can receive and reject
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::DoubleHello => {
                    // Send two Hello messages
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
                    // Send frame with protocol byte 0xD4
                    let fake_frame = [PR_EMULE_COMPRESSED, 0x01, 0x00, 0x00, 0x00, OP_HELLO];
                    stream.write_all(&fake_frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write 0xD4 frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::UnexpectedOpcode => {
                    // Send frame with unknown opcode 0x99
                    let fake_frame = [PR_ED2K, 0x01, 0x00, 0x00, 0x00, 0x99];
                    stream.write_all(&fake_frame).await.map_err(|e| {
                        Ed2kSessionError::Io(format!("Failed to write unknown opcode frame: {e}"))
                    })?;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok(())
                }
                TestPeerMode::ZeroUserHash => {
                    // Read client Hello
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

                    // Respond with zero user hash
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
                    // Read client Hello
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

                    // Write one byte at a time with 2ms interval
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
                    // Read client Hello
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
                    self.handshake_success.store(true, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok(())
                }
            }
        })
    }
}

/// Helper reading a single framed packet from an async stream.
async fn read_packet(
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
                    let peer = Ed2kPeerInfo::from_hello_answer(&answer);
                    return Ok(peer);
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

    /// Gracefully shuts down the client connection.
    pub async fn close(mut self) {
        let _ = self.stream.shutdown().await;
        self.session.state = Ed2kSessionState::Closed;
    }
}
