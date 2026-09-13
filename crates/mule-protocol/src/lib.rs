#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! `mule-protocol` provides zero-unsafe, deterministic framing, encoding, decoding,
//! constants, and decoupled scheduling traits for the eD2K wire protocol in Emule-Sion.

pub mod error;
pub mod framing;
pub mod lab;
pub mod limits;
pub mod traits;
pub mod types;

pub use error::{FramingError, ProtocolError};
pub use framing::{PacketCodec, RawPacket};
pub use lab::*;
pub use limits::*;
pub use traits::*;
pub use types::*;
