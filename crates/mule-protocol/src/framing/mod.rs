//! Packet framing, serialization, and stream decoding.

pub mod codec;
pub mod packet;

pub use codec::PacketCodec;
pub use packet::RawPacket;
