// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Cryptographic primitives for eMule-Rust.
//!
//! Boundary rules:
//! - This crate owns all key generation, signing and verification logic.
//! - It performs no network I/O and knows nothing about eD2K session state.
//! - Legacy eMule SUI (RSA-384, RSASSA-PKCS1v15-SHA1) is a compatibility
//!   mechanism only. It must never be used to encrypt payloads or protect
//!   modern identities. See `docs/ed2k/SUI_WIRE_FORMAT.md`.

pub mod cryptkey;
pub mod legacy_sui;
