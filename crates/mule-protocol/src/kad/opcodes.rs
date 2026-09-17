//! Kademlia UDP protocol magic bytes, opcodes, and constants.

/// Standard Kademlia UDP protocol magic byte (0xE4).
pub const PR_KADEMLIA: u8 = 0xE4;

/// Packed Kademlia UDP protocol magic byte (0xE5).
pub const PR_KADEMLIA_PACKED: u8 = 0xE5;

/// Reserved UDP protocol magic byte (0x8D).
pub const PR_KADEMLIA_RESERVED: u8 = 0x8D;

// --- Kad v1.0 Opcodes (Legacy) ---
/// Bootstrap request (0x01).
pub const KADEMLIA_BOOTSTRAP_REQ: u8 = 0x01;
/// Bootstrap response (0x09).
pub const KADEMLIA_BOOTSTRAP_RES: u8 = 0x09;
/// Hello request (0x10).
pub const KADEMLIA_HELLO_REQ: u8 = 0x10;
/// Hello response (0x18).
pub const KADEMLIA_HELLO_RES: u8 = 0x18;

// --- Kad v2.0 Opcodes (Official eMule / aMule / Wireshark spec) ---
/// Kad2 Bootstrap request (0x01).
pub const KADEMLIA2_BOOTSTRAP_REQ: u8 = 0x01;
/// Kad2 Bootstrap response (0x09).
pub const KADEMLIA2_BOOTSTRAP_RES: u8 = 0x09;
/// Kad2 Hello request (0x11).
pub const KADEMLIA2_HELLO_REQ: u8 = 0x11;
/// Kad2 Hello response (0x19).
pub const KADEMLIA2_HELLO_RES: u8 = 0x19;
/// Kad2 Hello response ACK (0x22).
pub const KADEMLIA2_HELLO_RES_ACK: u8 = 0x22;
/// Kad2 Find Node / Value request (0x21).
pub const KADEMLIA2_REQ: u8 = 0x21;
/// Kad2 Find Node / Value response (0x29).
pub const KADEMLIA2_RES: u8 = 0x29;
/// Kad2 Search Keyword request (0x33).
pub const KADEMLIA2_SEARCH_KEY_REQ: u8 = 0x33;
/// Kad2 Search Source request (0x34).
pub const KADEMLIA2_SEARCH_SOURCE_REQ: u8 = 0x34;
/// Kad2 Search Notes request (0x35).
pub const KADEMLIA2_SEARCH_NOTES_REQ: u8 = 0x35;
/// Kad2 Search response (0x3B).
pub const KADEMLIA2_SEARCH_RES: u8 = 0x3B;
/// Kad2 Search Source response (0x3B).
pub const KADEMLIA2_SEARCH_SOURCE_RES: u8 = 0x3B;
/// Kad2 Search Keyword request (0x33).
pub const KADEMLIA2_SEARCH_KEYWORD_REQ: u8 = 0x33;
/// Kad2 Search Keyword response (0x2C).
pub const KADEMLIA2_SEARCH_KEYWORD_RES: u8 = 0x2C;
/// Kad2 Publish Keyword request (0x43).
pub const KADEMLIA2_PUBLISH_KEY_REQ: u8 = 0x43;
/// Kad2 Publish Source request (0x44).
pub const KADEMLIA2_PUBLISH_SOURCE_REQ: u8 = 0x44;
/// Kad2 Publish Notes request (0x45).
pub const KADEMLIA2_PUBLISH_NOTES_REQ: u8 = 0x45;
/// Kad2 Publish response (0x4B).
pub const KADEMLIA2_PUBLISH_RES: u8 = 0x4B;
/// Kad2 Publish Source response (0x4B).
pub const KADEMLIA2_PUBLISH_SOURCE_RES: u8 = 0x4B;
/// Kad2 Publish Keyword request (0x43).
pub const KADEMLIA2_PUBLISH_KEYWORD_REQ: u8 = 0x43;
/// Kad2 Publish Keyword response (0x2D).
pub const KADEMLIA2_PUBLISH_KEYWORD_RES: u8 = 0x2D;
/// Kad2 Ping request (0x60).
pub const KADEMLIA2_PING: u8 = 0x60;
/// Kad2 Pong response (0x61).
pub const KADEMLIA2_PONG: u8 = 0x61;
/// Kad2 Firewall UDP test (0x62).
pub const KADEMLIA2_FIREWALLUDP: u8 = 0x62;

/// Maximum allowable search keywords returned in a single packet.
pub const MAX_KAD_SEARCH_KEYWORDS: usize = 50;

/// Maximum allowable UDP datagram payload size in bytes (1472 bytes, standard Ethernet MTU 1500 - 20 IP - 8 UDP).
pub const MAX_KAD_UDP_PACKET_SIZE: usize = 1472;

/// Checks if the protocol byte is a recognized Kademlia UDP protocol magic.
#[inline]
pub fn is_valid_kad_protocol(protocol: u8) -> bool {
    matches!(protocol, PR_KADEMLIA | PR_KADEMLIA_PACKED | PR_KADEMLIA_RESERVED)
}
