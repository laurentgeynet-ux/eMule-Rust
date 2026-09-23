// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

//! Kad UDP protocol codecs and definitions.

pub mod codec;
pub mod decoder;
pub mod encoder;
pub mod opcodes;
pub mod types;
pub mod udp_crypt;

pub use codec::KadCodec;
pub use decoder::KadDecoder;
pub use encoder::KadEncoder;
pub use opcodes::*;
pub use types::*;
pub use udp_crypt::{decrypt_kad_udp_packet, DecryptedKadUdp, MAGICVALUE_UDP_SYNC_CLIENT};
