// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! `preferences.dat` user-hash extraction — eMule `CPreferences::Init`
//! reads a packed `Preferences_Ext_Struct` (`Preferences.h:78-86`,
//! `Preferences.cpp:542-551`):
//!
//! ```text
//! [u8 version][userhash 16][WINDOWPLACEMENT …]
//! ```
//!
//! Only the first 17 bytes matter for identity migration — eMule never
//! validates `version` on load, so any value is accepted as long as the
//! file holds a full `Preferences_Ext_Struct` prefix and the hash is not
//! a degenerate marker (`isbadhash`, `OtherFunctions.h:380-383`, which
//! `CreateUserHash` would have regenerated anyway).

use std::io::Read;
use std::path::Path;

use super::error::MetError;
use super::reader::{open_met_file, CountingReader};

/// `PREFFILE_VERSION` (`Opcodes.h:33`) — informational; eMule accepts
/// any stored version on read.
pub const PREFFILE_VERSION: u8 = 0x14;

/// Parsed `preferences.dat` identity fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreferencesDat {
    /// Stored `Preferences_Ext_Struct.version`.
    pub version: u8,
    /// `m_abyUserHash` — the persisted random user hash that keeps
    /// eMule credits (`CreateUserHash`, `Preferences.cpp:1484-1492`).
    pub user_hash: [u8; 16],
}

/// `isbadhash` (`OtherFunctions.h:380-383`) — a hash is degenerate when
/// every byte eDonkey-family clients leave zero is zero:
/// `!(q0 & 0xffff00ffffffffff) && !(q1 & 0xff00ffffffffffff)`.
pub fn is_bad_hash(hash: &[u8; 16]) -> bool {
    let q0 = u64::from_le_bytes(hash[0..8].try_into().unwrap_or_default());
    let q1 = u64::from_le_bytes(hash[8..16].try_into().unwrap_or_default());
    q0 & 0xffff_00ff_ffff_ffff == 0 && q1 & 0xff00_ffff_ffff_ffff == 0
}

impl PreferencesDat {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, MetError> {
        let mut file = open_met_file(path)?;
        Self::read_from(&mut file)
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, MetError> {
        let mut cr = CountingReader::new(reader);
        let version = cr.read_u8("preferences.dat version")?;
        let mut user_hash = [0u8; 16];
        user_hash.copy_from_slice(&cr.read_exact_bytes(16, "preferences.dat userhash")?);
        if is_bad_hash(&user_hash) {
            // `CreateUserHash` regenerates bad hashes — a stored one means
            // the file is corrupt, not a real identity.
            return Err(MetError::InconsistentMetadata {
                field: "preferences.dat userhash",
                reason: "degenerate hash (isbadhash)".to_string(),
            });
        }
        Ok(Self { version, user_hash })
    }
}
