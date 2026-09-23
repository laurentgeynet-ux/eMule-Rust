// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Packet framing, serialization, and stream decoding.

pub mod codec;
pub mod packet;

pub use codec::PacketCodec;
pub use packet::RawPacket;
