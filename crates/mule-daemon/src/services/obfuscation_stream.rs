use mule_protocol::ed2k::obfuscation::{
    derive_initiator_peer_ciphers, derive_responder_peer_ciphers, derive_server_ciphers,
    generate_semi_random_marker, Dh768KeyPair, Rc4Cipher, ENM_OBFUSCATION, MAGICVALUE_SYNC,
};
use rand::Rng;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

/// A bi-directional stream wrapper that transparently handles eMule protocol obfuscation
/// via RC4 stream cipher, or passes data through unmodified when in plaintext mode.
pub struct ObfuscatedStream<S> {
    inner: S,
    send_cipher: Option<Rc4Cipher>,
    recv_cipher: Option<Rc4Cipher>,
    read_prefix: Vec<u8>,
    read_prefix_offset: usize,
    write_buf: Vec<u8>,
    write_offset: usize,
}

impl<S> ObfuscatedStream<S> {
    /// Creates a new unencrypted (plaintext) stream.
    pub fn plaintext(inner: S) -> Self {
        Self {
            inner,
            send_cipher: None,
            recv_cipher: None,
            read_prefix: Vec::new(),
            read_prefix_offset: 0,
            write_buf: Vec::new(),
            write_offset: 0,
        }
    }

    /// Creates an unencrypted stream with a pre-read byte buffer (useful for inbound connections
    /// where the first byte was inspected to check if obfuscation was requested).
    pub fn plaintext_with_prefix(inner: S, prefix: Vec<u8>) -> Self {
        Self {
            inner,
            send_cipher: None,
            recv_cipher: None,
            read_prefix: prefix,
            read_prefix_offset: 0,
            write_buf: Vec::new(),
            write_offset: 0,
        }
    }

    /// Returns whether this stream is currently encrypted with RC4 protocol obfuscation.
    pub fn is_obfuscated(&self) -> bool {
        self.send_cipher.is_some()
    }

    /// Returns a reference to the underlying transport stream.
    pub fn inner(&self) -> &S {
        &self.inner
    }

    /// Returns a mutable reference to the underlying transport stream.
    pub fn inner_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    /// Consumes the wrapper and returns the underlying stream.
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> ObfuscatedStream<S> {
    /// Performs an outbound eD2K client-to-client obfuscation handshake according to eMule protocol specifications.
    ///
    /// The initiator generates a 32-bit random key part and derives:
    /// - SendKey = MD5(TargetUserHash || 34 || RandomKeyPart)
    /// - RecvKey = MD5(TargetUserHash || 203 || RandomKeyPart)
    ///
    /// It sends:
    /// - `[SemiRandomMarker (1B), RandomKeyPart (4B)]` (unencrypted)
    /// - `[MAGICVALUE_SYNC (4B), ENM_OBFUSCATION (1B), ENM_OBFUSCATION (1B), PadLen (1B), RandomPadding]` (encrypted)
    ///
    /// And awaits the responder's encrypted answer:
    /// - `[MAGICVALUE_SYNC (4B), ENM_OBFUSCATION (1B), RespPadLen (1B), RespRandomPadding]`
    pub async fn connect_peer_outgoing(
        mut stream: S,
        target_user_hash: &[u8; 16],
        max_padding: u8,
    ) -> Result<Self, io::Error> {
        let (random_key_part, semi_random_marker, padding) = {
            let mut rng = rand::rng();
            let random_key_part: u32 = rng.random();
            let semi_random_marker = generate_semi_random_marker(&mut rng);
            let pad_len = if max_padding > 0 {
                rng.random_range(0..=max_padding)
            } else {
                0
            };
            let mut padding = Vec::with_capacity(pad_len as usize);
            for _ in 0..pad_len {
                padding.push(rng.random());
            }
            (random_key_part, semi_random_marker, padding)
        };

        // Derive ciphers (RC4 with 1024 bytes discarded)
        let (mut send_cipher, mut recv_cipher) =
            derive_initiator_peer_ciphers(target_user_hash, random_key_part);

        // Prepare 5-byte unencrypted header: [semi_random_marker, random_key_part (little-endian)]
        let mut header = [0u8; 5];
        header[0] = semi_random_marker;
        header[1..5].copy_from_slice(&random_key_part.to_le_bytes());

        // Prepare encrypted initiator payload
        let mut payload = Vec::with_capacity(7 + padding.len());
        payload.extend_from_slice(&MAGICVALUE_SYNC.to_le_bytes());
        payload.push(ENM_OBFUSCATION); // Supported encryption method
        payload.push(ENM_OBFUSCATION); // Preferred encryption method
        payload.push(padding.len() as u8);
        payload.extend_from_slice(&padding);

        // Encrypt payload with send_cipher
        send_cipher.process(&mut payload);

        // Write header and encrypted payload to stream
        stream.write_all(&header).await?;
        stream.write_all(&payload).await?;

        // Read and verify responder's response
        let mut resp_sync = [0u8; 4];
        stream.read_exact(&mut resp_sync).await?;
        recv_cipher.process(&mut resp_sync);
        let sync_val = u32::from_le_bytes(resp_sync);
        if sync_val != MAGICVALUE_SYNC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid obfuscation magic sync from peer: 0x{sync_val:08X}"),
            ));
        }

        let mut resp_meta = [0u8; 2]; // selected_method, pad_len
        stream.read_exact(&mut resp_meta).await?;
        recv_cipher.process(&mut resp_meta);

        let selected_method = resp_meta[0];
        if selected_method != ENM_OBFUSCATION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported obfuscation method selected by peer: {selected_method}"),
            ));
        }

        let resp_pad_len = resp_meta[1] as usize;
        if resp_pad_len > 0 {
            let mut pad_buf = vec![0u8; resp_pad_len];
            stream.read_exact(&mut pad_buf).await?;
            recv_cipher.process(&mut pad_buf);
        }

        Ok(Self {
            inner: stream,
            send_cipher: Some(send_cipher),
            recv_cipher: Some(recv_cipher),
            read_prefix: Vec::new(),
            read_prefix_offset: 0,
            write_buf: Vec::new(),
            write_offset: 0,
        })
    }

    /// Performs an outbound eD2K client-to-server obfuscation handshake
    /// (eMule `ECS_PENDING_SERVER` / `ONS_BASIC_SERVER_*`):
    ///
    /// - Client → `[SemiRandomMarker (1B), G^A (96B), PadLen (1B), RandomPadding]` (unencrypted)
    /// - Server → `[G^B (96B)]` (unencrypted) then `[MAGICVALUE_SYNC (4B), MethodsSupported (1B), MethodPreferred (1B), PadLen (1B), Padding]` (encrypted)
    /// - Client → `[MAGICVALUE_SYNC (4B), MethodSelected (1B), PadLen (1B), Padding]` (encrypted)
    ///
    /// Keys: `SendKey = MD5(S[96] ‖ 34)`, `RecvKey = MD5(S[96] ‖ 203)` where
    /// `S` is the DH768 shared secret; RC4-drop1024 both ways.
    pub async fn connect_server_outgoing(
        mut stream: S,
        max_padding: u8,
    ) -> Result<Self, io::Error> {
        let (key_pair, marker, pad_len, padding) = {
            let mut rng = rand::rng();
            let kp = Dh768KeyPair::generate(&mut rng);
            let marker = generate_semi_random_marker(&mut rng);
            // eMule pads with 0-15 bytes on the server handshake.
            let pad_len = if max_padding > 0 {
                rng.random_range(0..=max_padding.min(15))
            } else {
                0
            };
            let mut padding = Vec::with_capacity(pad_len as usize);
            for _ in 0..pad_len {
                padding.push(rng.random());
            }
            (kp, marker, pad_len, padding)
        };

        // Unencrypted: marker + our 96-byte DH public key + padding.
        let mut out = Vec::with_capacity(98 + pad_len as usize);
        out.push(marker);
        out.extend_from_slice(&key_pair.public_key);
        out.push(pad_len);
        out.extend_from_slice(&padding);
        stream.write_all(&out).await?;

        // Unencrypted: server's 96-byte DH public key.
        let mut remote_pub = [0u8; 96];
        stream.read_exact(&mut remote_pub).await?;
        let shared = key_pair.compute_shared_secret(&remote_pub);
        let (mut send_cipher, mut recv_cipher) = derive_server_ciphers(&shared);

        // Encrypted: [MAGICVALUE_SYNC][supported][preferred][padlen][padding]
        let mut resp_head = [0u8; 7];
        stream.read_exact(&mut resp_head).await?;
        recv_cipher.process(&mut resp_head);
        let magic = u32::from_le_bytes(resp_head[..4].try_into().unwrap());
        if magic != MAGICVALUE_SYNC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid obfuscation magic sync from server: 0x{magic:08X}"),
            ));
        }
        if resp_head[5] != ENM_OBFUSCATION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Unsupported obfuscation method from server: {}",
                    resp_head[5]
                ),
            ));
        }
        let resp_pad_len = resp_head[6] as usize;
        if resp_pad_len > 0 {
            let mut pad_buf = vec![0u8; resp_pad_len];
            stream.read_exact(&mut pad_buf).await?;
            recv_cipher.process(&mut pad_buf);
        }

        // Encrypted answer: [MAGICVALUE_SYNC][selected method][padlen][padding]
        // (eMule delays it to the first payload; sending it immediately is
        // equivalent on the wire).
        let mut ans = Vec::with_capacity(7 + padding.len());
        ans.extend_from_slice(&MAGICVALUE_SYNC.to_le_bytes());
        ans.push(ENM_OBFUSCATION);
        ans.push(pad_len);
        ans.extend_from_slice(&padding);
        send_cipher.process(&mut ans);
        stream.write_all(&ans).await?;

        Ok(Self {
            inner: stream,
            send_cipher: Some(send_cipher),
            recv_cipher: Some(recv_cipher),
            read_prefix: Vec::new(),
            read_prefix_offset: 0,
            write_buf: Vec::new(),
            write_offset: 0,
        })
    }

    /// Performs an inbound eD2K client-to-client obfuscation handshake when an incoming connection
    /// starts with a non-standard protocol byte.
    ///
    /// The caller passes `first_byte` (already read from the socket).
    /// Responder reads the remaining 4 bytes of `random_key_part` and derives:
    /// - RecvKey = MD5(LocalUserHash || 34 || RandomKeyPart)
    /// - SendKey = MD5(LocalUserHash || 203 || RandomKeyPart)
    ///
    /// It then receives initiator's encrypted sync + parameters, and responds with its own encrypted sync + padding.
    pub async fn accept_peer_incoming(
        mut stream: S,
        _first_byte: u8,
        local_user_hash: &[u8; 16],
        max_padding: u8,
    ) -> Result<Self, io::Error> {
        // 1. Read remaining 4 bytes of RandomKeyPart
        let mut random_key_part_bytes = [0u8; 4];
        stream.read_exact(&mut random_key_part_bytes).await?;
        let random_key_part = u32::from_le_bytes(random_key_part_bytes);

        // 2. Derive ciphers
        let (send_cipher, mut recv_cipher) =
            derive_responder_peer_ciphers(local_user_hash, random_key_part);
        let mut send_cipher = send_cipher;

        // 3. Read initiator's encrypted sync and options
        let mut magic_bytes = [0u8; 4];
        stream.read_exact(&mut magic_bytes).await?;
        recv_cipher.process(&mut magic_bytes);
        let magic = u32::from_le_bytes(magic_bytes);
        if magic != MAGICVALUE_SYNC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid obfuscation magic sync from incoming peer: 0x{magic:08X}"),
            ));
        }

        let mut meta = [0u8; 3]; // supported, preferred, pad_len
        stream.read_exact(&mut meta).await?;
        recv_cipher.process(&mut meta);

        let req_pad_len = meta[2] as usize;
        if req_pad_len > 0 {
            let mut pad_buf = vec![0u8; req_pad_len];
            stream.read_exact(&mut pad_buf).await?;
            recv_cipher.process(&mut pad_buf);
        }

        // 4. Send responder's encrypted answer
        let (pad_len, padding) = {
            let mut rng = rand::rng();
            let pad_len = if max_padding > 0 {
                rng.random_range(0..=max_padding)
            } else {
                0
            };
            let mut padding = Vec::with_capacity(pad_len as usize);
            for _ in 0..pad_len {
                padding.push(rng.random());
            }
            (pad_len, padding)
        };
        let mut resp = Vec::with_capacity(6 + pad_len as usize);
        resp.extend_from_slice(&MAGICVALUE_SYNC.to_le_bytes());
        resp.push(ENM_OBFUSCATION);
        resp.push(pad_len);
        resp.extend_from_slice(&padding);

        send_cipher.process(&mut resp);
        stream.write_all(&resp).await?;

        Ok(Self {
            inner: stream,
            send_cipher: Some(send_cipher),
            recv_cipher: Some(recv_cipher),
            read_prefix: Vec::new(),
            read_prefix_offset: 0,
            write_buf: Vec::new(),
            write_offset: 0,
        })
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for ObfuscatedStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // 1. Drain any pre-read prefix bytes first
        if self.read_prefix_offset < self.read_prefix.len() {
            let available = self.read_prefix.len() - self.read_prefix_offset;
            let to_copy = available.min(buf.remaining());
            buf.put_slice(
                &self.read_prefix[self.read_prefix_offset..self.read_prefix_offset + to_copy],
            );
            self.read_prefix_offset += to_copy;
            return Poll::Ready(Ok(()));
        }

        // 2. Read from inner stream
        let before_len = buf.filled().len();
        match Pin::new(&mut self.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let after_len = buf.filled().len();
                let newly_read = after_len - before_len;
                if newly_read > 0 {
                    if let Some(ref mut cipher) = self.recv_cipher {
                        let filled = buf.filled_mut();
                        cipher.process(&mut filled[before_len..after_len]);
                    }
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for ObfuscatedStream<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = &mut *self;
        // 1. Flush any pending encrypted bytes from previous partial write
        while this.write_offset < this.write_buf.len() {
            let pending_slice = &this.write_buf[this.write_offset..];
            match Pin::new(&mut this.inner).poll_write(cx, pending_slice) {
                Poll::Ready(Ok(n)) => {
                    if n == 0 {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to write buffered data to inner stream",
                        )));
                    }
                    this.write_offset += n;
                }
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }
        this.write_buf.clear();
        this.write_offset = 0;

        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        // 2. If plaintext, forward directly to inner stream
        if this.send_cipher.is_none() {
            return Pin::new(&mut this.inner).poll_write(cx, buf);
        }

        // 3. Encrypted write: take up to 8192 bytes, encrypt and send/buffer
        let chunk_size = buf.len().min(8192);
        let mut chunk = buf[..chunk_size].to_vec();
        if let Some(ref mut cipher) = this.send_cipher {
            cipher.process(&mut chunk);
        }

        match Pin::new(&mut this.inner).poll_write(cx, &chunk) {
            Poll::Ready(Ok(n)) => {
                if n < chunk.len() {
                    this.write_buf = chunk;
                    this.write_offset = n;
                }
                Poll::Ready(Ok(chunk_size))
            }
            Poll::Pending => {
                this.write_buf = chunk;
                this.write_offset = 0;
                Poll::Ready(Ok(chunk_size))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = &mut *self;
        while this.write_offset < this.write_buf.len() {
            let pending_slice = &this.write_buf[this.write_offset..];
            match Pin::new(&mut this.inner).poll_write(cx, pending_slice) {
                Poll::Ready(Ok(n)) => {
                    if n == 0 {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to flush buffered data",
                        )));
                    }
                    this.write_offset += n;
                }
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }
        this.write_buf.clear();
        this.write_offset = 0;
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = &mut *self;
        while this.write_offset < this.write_buf.len() {
            let pending_slice = &this.write_buf[this.write_offset..];
            match Pin::new(&mut this.inner).poll_write(cx, pending_slice) {
                Poll::Ready(Ok(n)) => {
                    if n == 0 {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "failed to flush buffered data during shutdown",
                        )));
                    }
                    this.write_offset += n;
                }
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }
        this.write_buf.clear();
        this.write_offset = 0;
        Pin::new(&mut this.inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_obfuscated_stream_bidirectional_handshake_and_crypto() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let server_user_hash = [0x55u8; 16];

        let client_task = tokio::spawn(async move {
            let mut stream =
                ObfuscatedStream::connect_peer_outgoing(client_io, &server_user_hash, 8)
                    .await
                    .expect("client connect_peer_outgoing failed");
            assert!(stream.is_obfuscated());

            // Write test message to server
            stream.write_all(b"PING_FROM_CLIENT").await.unwrap();
            stream.flush().await.unwrap();

            // Read answer from server
            let mut buf = vec![0u8; 16];
            stream.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"PONG_FROM_SERVER");
        });

        let server_task = tokio::spawn(async move {
            let mut server_io = server_io;
            let mut first_byte = [0u8; 1];
            server_io.read_exact(&mut first_byte).await.unwrap();

            let mut stream = ObfuscatedStream::accept_peer_incoming(
                server_io,
                first_byte[0],
                &server_user_hash,
                8,
            )
            .await
            .expect("server accept_peer_incoming failed");
            assert!(stream.is_obfuscated());

            // Read message from client
            let mut buf = vec![0u8; 16];
            stream.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"PING_FROM_CLIENT");

            // Write answer back to client
            stream.write_all(b"PONG_FROM_SERVER").await.unwrap();
            stream.flush().await.unwrap();
        });

        let (c_res, s_res) = tokio::join!(client_task, server_task);
        c_res.unwrap();
        s_res.unwrap();
    }

    #[tokio::test]
    async fn test_plaintext_stream_with_prefix() {
        let (mut client_io, server_io) = tokio::io::duplex(1024);

        tokio::spawn(async move {
            client_io.write_all(b"WORLD").await.unwrap();
        });

        let mut stream = ObfuscatedStream::plaintext_with_prefix(server_io, b"HELLO ".to_vec());
        assert!(!stream.is_obfuscated());

        let mut buf = vec![0u8; 11];
        stream.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"HELLO WORLD");
    }

    /// Loopback test of the eD2K client↔server DH768+RC4 handshake
    /// (`connect_server_outgoing`) with a mock server performing the
    /// responder side exactly as eMule `ONS_BASIC_SERVER_*` does.
    #[tokio::test]
    async fn test_server_obfuscation_handshake_and_crypto() {
        let (client_io, mut server_io) = tokio::io::duplex(8192);

        let client_task = tokio::spawn(async move {
            let mut stream = ObfuscatedStream::connect_server_outgoing(client_io, 8)
                .await
                .expect("client server handshake failed");
            assert!(stream.is_obfuscated());
            stream.write_all(b"PING_TO_SERVER").await.unwrap();
            stream.flush().await.unwrap();
            let mut buf = vec![0u8; 15];
            stream.read_exact(&mut buf).await.unwrap();
            assert_eq!(&buf, b"PONG_FROM_SRV!!");
        });

        let server_task = tokio::spawn(async move {
            // Unencrypted client hello: [marker(1)][G^A(96)][padlen(1)][pad]
            let mut marker = [0u8; 1];
            server_io.read_exact(&mut marker).await.unwrap();
            let mut client_pub = [0u8; 96];
            server_io.read_exact(&mut client_pub).await.unwrap();
            let mut pad_len = [0u8; 1];
            server_io.read_exact(&mut pad_len).await.unwrap();
            let mut pad = vec![0u8; pad_len[0] as usize];
            server_io.read_exact(&mut pad).await.unwrap();

            let server_kp = Dh768KeyPair::generate(&mut rand::rng());
            let shared = server_kp.compute_shared_secret(&client_pub);
            server_io.write_all(&server_kp.public_key).await.unwrap();

            // Server keys are the mirror of the client's
            // (client send = MD5(S‖34), client recv = MD5(S‖203)).
            let (client_send, client_recv) = derive_server_ciphers(&shared);
            let mut send_cipher = client_recv;
            let mut recv_cipher = client_send;

            // Encrypted server answer: [magic(4)][supported][requested][padlen][pad]
            let mut ans = Vec::new();
            ans.extend_from_slice(&MAGICVALUE_SYNC.to_le_bytes());
            ans.push(ENM_OBFUSCATION);
            ans.push(ENM_OBFUSCATION);
            ans.push(0u8);
            send_cipher.process(&mut ans);
            server_io.write_all(&ans).await.unwrap();

            // Encrypted client confirm: [magic(4)][method][padlen][pad]
            let mut conf = [0u8; 6];
            server_io.read_exact(&mut conf).await.unwrap();
            recv_cipher.process(&mut conf);
            assert_eq!(
                u32::from_le_bytes(conf[..4].try_into().unwrap()),
                MAGICVALUE_SYNC
            );
            assert_eq!(conf[4], ENM_OBFUSCATION);
            let conf_pad = conf[5] as usize;
            if conf_pad > 0 {
                let mut p = vec![0u8; conf_pad];
                server_io.read_exact(&mut p).await.unwrap();
                recv_cipher.process(&mut p);
            }

            // Encrypted data exchange.
            let mut buf = vec![0u8; 14];
            server_io.read_exact(&mut buf).await.unwrap();
            recv_cipher.process(&mut buf);
            assert_eq!(&buf, b"PING_TO_SERVER");
            let mut pong = b"PONG_FROM_SRV!!".to_vec();
            send_cipher.process(&mut pong);
            server_io.write_all(&pong).await.unwrap();
        });

        let (c_res, s_res) = tokio::join!(client_task, server_task);
        c_res.unwrap();
        s_res.unwrap();
    }

    /// A non-obfuscating peer answering the DH probe with plaintext garbage
    /// must produce a clean handshake failure (server falls back to plaintext).
    #[tokio::test]
    async fn test_server_obfuscation_rejects_plaintext_answer() {
        let (client_io, mut server_io) = tokio::io::duplex(8192);
        let client_task =
            tokio::spawn(
                async move { ObfuscatedStream::connect_server_outgoing(client_io, 0).await },
            );
        let server_task = tokio::spawn(async move {
            // Read the client hello, answer with a syntactically valid DH
            // public key but follow it with a PLAINTEXT answer — the client's
            // decrypted magic check must fail cleanly.
            let mut hello = [0u8; 98];
            server_io.read_exact(&mut hello).await.unwrap();
            let mut client_pub = [0u8; 96];
            client_pub.copy_from_slice(&hello[1..97]);
            let server_kp = Dh768KeyPair::generate(&mut rand::rng());
            server_io.write_all(&server_kp.public_key).await.unwrap();
            // Unencrypted junk where the encrypted magic value is expected.
            server_io.write_all(&[0xDE; 7]).await.unwrap();
        });
        let (c_res, s_res) = tokio::join!(client_task, server_task);
        s_res.unwrap();
        assert!(c_res.unwrap().is_err());
    }
}
