// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! eD2K/eMule protocol tags, peer identifiers, and standard opcodes.

use std::fmt;

/// Standard eD2K protocol identifier byte (`0xE3`).
pub const PR_ED2K: u8 = 0xE3;

/// Extended eMule protocol identifier byte (`0xC5`).
pub const PR_EMULE: u8 = 0xC5;

/// Recognized wire protocol identifier for eMule compressed payloads (`0xD4`).
///
/// NOTE: Framing support recognizes and frames these packets at the transport boundary;
/// compliant decompression and post-decompression safety limits (decompression ratio caps,
/// message-type length ceilings, and decompression bomb mitigation) are implemented in a
/// dedicated decompression stage.
pub const PR_EMULE_COMPRESSED: u8 = 0xD4;

/// Alias for [`PR_EMULE_COMPRESSED`] (`0xD4`).
pub const PR_ZLIB: u8 = PR_EMULE_COMPRESSED;

/// Laboratory and deterministic loopback testbed protocol identifier (`0xEE`).
///
/// Explicitly separated from historical eD2K wire formats to prevent accidental leakage
/// or premature claims of external eMule network interoperation.
pub const PR_LAB: u8 = 0xEE;

/// Verifies whether the given byte is a recognized protocol magic byte.
#[inline]
pub fn is_valid_protocol(protocol: u8) -> bool {
    matches!(protocol, PR_ED2K | PR_EMULE | PR_EMULE_COMPRESSED | PR_LAB)
}

/// 16-byte Peer Identifier (User Hash).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerId(pub [u8; 16]);

impl PeerId {
    /// Creates a new `PeerId` from an array of 16 bytes.
    #[inline]
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Creates a `PeerId` from a byte slice if its length is exactly 16 bytes.
    pub fn from_slice(slice: &[u8]) -> Option<Self> {
        if slice.len() == 16 {
            let mut arr = [0u8; 16];
            arr.copy_from_slice(slice);
            Some(Self(arr))
        } else {
            None
        }
    }

    /// Parses a 32-character hexadecimal string into a `PeerId`.
    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(s, &mut bytes)?;
        Ok(Self(bytes))
    }

    /// Formats the `PeerId` as a 32-character lowercase hexadecimal string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// Returns a reference to the underlying 16 bytes.
    #[inline]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

// ---------------------------------------------------------------------------
// Standard eD2K Client-to-Client Opcodes (Protocol 0xE3)
// ---------------------------------------------------------------------------

/// Peer handshake initialization.
pub const OP_HELLO: u8 = 0x01;
/// Response to OP_HELLO.
pub const OP_HELLO_ANSWER: u8 = 0x4C;
/// Request file name by hash.
pub const OP_REQUESTFILENAME: u8 = 0x59;
/// File name response.
pub const OP_REQFILENAME_ANSWER: u8 = 0x59;
/// Tell peer which file hash we are requesting.
pub const OP_SETREQFILEID: u8 = 0x47;
/// Response indicating requested file is not shared.
pub const OP_FILEREQANSNOFILE: u8 = 0x48;
/// Request remote file status (parts availability bitmask).
pub const OP_FILESTATUSREQ: u8 = 0x49;
/// Parts availability status bitmask response.
pub const OP_FILESTATUS: u8 = 0x50;
/// Request to enter upload queue.
pub const OP_STARTUPLOADREQ: u8 = 0x54;
/// Peer accepted upload request; slot is ready.
pub const OP_ACCEPTUPLOADREQ: u8 = 0x55;
/// Cancel active transfer or remove from upload queue.
pub const OP_CANCELTRANSFER: u8 = 0x56;
/// Inform peer that we no longer need the parts.
pub const OP_OUTOFPARTS: u8 = 0x57;
/// Request specific block range within a part.
pub const OP_REQUESTPARTS: u8 = 0x4B;
/// Data block payload response.
pub const OP_SENDINGPART: u8 = 0x46;
/// eMule: signal that client completed the download of this file (`0x49`).
/// Same wire value as legacy `OP_FILESTATUSREQ` above.
pub const OP_END_OF_DOWNLOAD: u8 = 0x49;
/// eMule: client-ID change notification (`0x4D`) —
/// `<u32 newUserID><u32 newServerIP>` (`ListenSocket.cpp:590`).
pub const OP_CHANGE_CLIENT_ID: u8 = 0x4D;
/// Request hashset for multi-part file.
pub const OP_HASHSETREQUEST: u8 = 0x51;
/// Hashset payload answer.
pub const OP_HASHSETANSWER: u8 = 0x52;

// ---------------------------------------------------------------------------
// Extended eMule Opcodes (Protocol 0xC5)
// ---------------------------------------------------------------------------

/// Extended eMule handshake capabilities exchange.
pub const OP_EMULE_INFO: u8 = 0x01;
/// Extended eMule handshake capabilities answer.
pub const OP_EMULE_INFO_ANSWER: u8 = 0x02;
/// Request AICH root hash.
pub const OP_AICH_REQUEST: u8 = 0x12;
/// AICH root hash answer.
pub const OP_AICH_ANSWER: u8 = 0x13;
/// Request AICH recovery hashset / proof tree for corrupted part.
pub const OP_AICHFILEHASHREQ: u8 = 0x14;
/// AICH recovery hashset / proof tree response.
pub const OP_AICHFILEHASHRES: u8 = 0x15;
