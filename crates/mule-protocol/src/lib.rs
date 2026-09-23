// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! `mule-protocol` provides zero-unsafe, deterministic framing, encoding, decoding,
//! constants, and decoupled scheduling traits for the eD2K wire protocol in EMule-Rust.

pub mod ed2k;
pub mod error;
pub mod framing;
pub mod kad;
#[cfg(feature = "lab-loopback")]
pub mod lab;
pub mod limits;
pub mod traits;
pub mod types;

pub use error::{FramingError, ProtocolError};
pub use framing::{PacketCodec, RawPacket};
pub use kad::*;
#[cfg(feature = "lab-loopback")]
pub use lab::*;
pub use limits::*;
pub use traits::*;
pub use types::*;
