//! eD2K Protocol Obfuscation implementation conforming to official eMule specifications.
//!
//! Provides:
//! - [`Rc4Cipher`]: High-performance RC4 stream cipher with `drop_bytes(1024)` support.
//! - [`dh`]: 768-bit MODP Diffie-Hellman key agreement for server connections.
//! - [`keys`]: MD5 key derivation for peer-to-peer and server connections.
//! - [`framing`]: Wire packet encoders/decoders for obfuscation handshakes.

pub mod dh;
pub mod framing;
pub mod keys;
pub mod rc4;

pub use dh::{Dh768KeyPair, DH768_G, DH768_P};
pub use framing::{
    generate_semi_random_marker, is_standard_protocol_marker, ClientInitiatorPacket,
    ClientResponderPacket, ENM_OBFUSCATION, MAGICVALUE_SYNC,
};
pub use keys::{
    derive_initiator_peer_ciphers, derive_initiator_peer_raw_keys, derive_responder_peer_ciphers,
    derive_responder_peer_raw_keys, derive_server_ciphers, MAGICVALUE_REQUESTER, MAGICVALUE_SERVER,
};
pub use rc4::Rc4Cipher;

use thiserror::Error;

/// Errors arising during protocol obfuscation handshakes.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ObfuscationError {
    /// Remote side sent an invalid sync magic value (expected `0x835E6FC4`).
    #[error("Invalid magic sync value: expected 0x835E6FC4, got 0x{0:08X}")]
    InvalidMagicSync(u32),

    /// Remote side selected or requested an unsupported encryption method.
    #[error("Unsupported encryption method: {0}")]
    UnsupportedMethod(u8),

    /// Initiating an obfuscated peer connection requires knowing the peer's UserHash.
    #[error("Missing target client user hash for obfuscated connection")]
    MissingUserHash,

    /// Socket closed before all expected handshake bytes were received.
    #[error("Premature EOF during obfuscation handshake (expected {expected} bytes, got {got})")]
    UnexpectedEof {
        /// Number of bytes expected.
        expected: usize,
        /// Number of bytes actually received before EOF.
        got: usize,
    },

    /// Lower-level I/O error occurred during handshake transmission.
    #[error("I/O error during obfuscation handshake: {0}")]
    Io(String),

    /// Handshake timed out before completion.
    #[error("Handshake timed out")]
    Timeout,
}
