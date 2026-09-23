// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Protocol magic bytes and identifiers for historical eD2K networks.

/// Standard eDonkey2000 protocol identifier byte (`0xE3`).
pub const PR_ED2K: u8 = 0xE3;

/// Extended eMule protocol identifier byte (`0xC5`).
pub const PR_EMULE: u8 = 0xC5;

/// Compressed eMule protocol identifier byte (`0xD4`).
pub const PR_EMULE_COMPRESSED: u8 = 0xD4;

/// Verifies whether the given byte is a recognized eD2K protocol magic byte.
#[inline]
pub fn is_valid_ed2k_protocol(protocol: u8) -> bool {
    matches!(protocol, PR_ED2K | PR_EMULE | PR_EMULE_COMPRESSED)
}
