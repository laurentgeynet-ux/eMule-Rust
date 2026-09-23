//! Compatibility profiles for eD2K wire disambiguation.
//!
//! Historical variations across eDonkey2000, eMule, and aMule resulted in overlapping
//! opcodes with completely different wire payloads and semantic meanings (e.g., `0x4B`
//! represents `OP_REQUESTPARTS` in legacy eDonkey but `OP_ASKSHAREDFILESANSWER` in eMule).
//!
//! To prevent heuristic guessing and wire corruption, `Ed2kCompatibilityProfile` mandates
//! explicit layout and opcode disambiguation.

use std::fmt;

/// Explicit client compatibility profile for eD2K wire encoding and decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Ed2kCompatibilityProfile {
    /// Canonical eMule 0.72a (current reference implementation):
    /// - `OP_REQUESTPARTS_EMULE` (`0x47`): 3-slot layout (40 bytes payload).
    /// - `OP_SETREQFILEID_EMULE` (`0x4F`): File status request (16 bytes file hash).
    /// - Rejects `0x4B` as block request (`0x4B` is `OP_ASKSHAREDFILESANSWER` in eMule).
    /// - Rejects `0x49` as file status request (`0x49` is `OP_END_OF_DOWNLOAD` in eMule).
    #[default]
    Emule072a,

    /// Historical eMule 0.50a — wire opcode assignments and layouts are
    /// identical to `Emule072a` (the ambiguous opcodes `0x47`/`0x49`/`0x4B`/`0x4F`
    /// never changed between 0.50a and 0.72a). Kept for replaying historical
    /// fixtures; new code should use `Emule072a`.
    Emule050a,

    /// aMule 2.3.3:
    /// - Follows the same wire opcode assignments and layouts as eMule (`0x47` and `0x4F`).
    Amule233,

    /// Legacy eDonkey2000:
    /// - `OP_REQUESTPARTS` (`0x4B`): Supports both 24-byte single-slot and 40-byte 3-slot layouts.
    /// - `OP_FILESTATUSREQ` (`0x49`): File status request (16 bytes file hash).
    /// - Rejects `0x47` as block request (`0x47` was `OP_SETREQFILEID` in early eDonkey).
    /// - Rejects `0x4F` as file status request.
    LegacyEdonkey,

    /// Strict unknown profile:
    /// - Never disambiguates overloaded opcodes (`0x47`, `0x4B`, `0x49`, `0x4F`).
    /// - Strictly returns `Ed2kErrorKind::AmbiguousOpcodeForProfile`.
    UnknownStrict,
}

impl Ed2kCompatibilityProfile {
    /// Returns a human-readable identifier for this profile.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Emule072a => "Emule072a",
            Self::Emule050a => "Emule050a",
            Self::Amule233 => "Amule233",
            Self::LegacyEdonkey => "LegacyEdonkey",
            Self::UnknownStrict => "UnknownStrict",
        }
    }

    /// Expected opcode for requesting parts under this profile.
    ///
    /// Returns `None` if the profile is `UnknownStrict`.
    pub const fn request_parts_opcode(&self) -> Option<u8> {
        match self {
            Self::Emule072a | Self::Emule050a | Self::Amule233 => {
                Some(super::opcodes::OP_REQUESTPARTS_EMULE)
            } // 0x47
            Self::LegacyEdonkey => Some(super::opcodes::OP_REQUESTPARTS),                    // 0x4B
            Self::UnknownStrict => None,
        }
    }

    /// Expected opcode for requesting file status under this profile.
    ///
    /// Returns `None` if the profile is `UnknownStrict`.
    pub const fn file_status_req_opcode(&self) -> Option<u8> {
        match self {
            Self::Emule072a | Self::Emule050a | Self::Amule233 => {
                Some(super::opcodes::OP_SETREQFILEID_EMULE)
            } // 0x4F
            Self::LegacyEdonkey => Some(super::opcodes::OP_FILESTATUSREQ),                   // 0x49
            Self::UnknownStrict => None,
        }
    }
}

impl fmt::Display for Ed2kCompatibilityProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
