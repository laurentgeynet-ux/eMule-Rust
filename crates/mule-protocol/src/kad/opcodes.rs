//! Kademlia UDP protocol magic bytes, opcodes, and constants.

/// Standard Kademlia UDP protocol magic byte (0xE4).
pub const PR_KADEMLIA: u8 = 0xE4;

/// Packed Kademlia UDP protocol magic byte (0xE5).
pub const PR_KADEMLIA_PACKED: u8 = 0xE5;

// --- Kad v1.0 Opcodes (Legacy) ---
/// Bootstrap request (0x01).
pub const KADEMLIA_BOOTSTRAP_REQ: u8 = 0x01;
/// Bootstrap response (0x09).
pub const KADEMLIA_BOOTSTRAP_RES: u8 = 0x09;
/// Hello request (0x10).
pub const KADEMLIA_HELLO_REQ: u8 = 0x10;
/// Hello response (0x18).
pub const KADEMLIA_HELLO_RES: u8 = 0x18;

// --- Kad v2.0 Opcodes ---
/// Kad2 Ping request (0x60).
pub const KADEMLIA2_PING: u8 = 0x60;
/// Kad2 Pong response (0x61).
pub const KADEMLIA2_PONG: u8 = 0x61;
/// Kad2 Hello request (0x11).
pub const KADEMLIA2_HELLO_REQ: u8 = 0x11;
/// Kad2 Hello response (0x19).
pub const KADEMLIA2_HELLO_RES: u8 = 0x19;
/// Kad2 Bootstrap request (0x21).
pub const KADEMLIA2_BOOTSTRAP_REQ: u8 = 0x21;
/// Kad2 Bootstrap response (0x29).
pub const KADEMLIA2_BOOTSTRAP_RES: u8 = 0x29;
/// Kad2 Find Node / Value request (0x21).
pub const KADEMLIA2_REQ: u8 = 0x21;
/// Kad2 Find Node / Value response (0x29).
pub const KADEMLIA2_RES: u8 = 0x29;

/// Maximum allowable UDP datagram payload size in bytes (1472 bytes, standard Ethernet MTU 1500 - 20 IP - 8 UDP).
pub const MAX_KAD_UDP_PACKET_SIZE: usize = 1472;

/// Checks if the protocol byte is a recognized Kademlia UDP protocol magic.
#[inline]
pub fn is_valid_kad_protocol(protocol: u8) -> bool {
    matches!(protocol, PR_KADEMLIA | PR_KADEMLIA_PACKED)
}
