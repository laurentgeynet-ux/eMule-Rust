// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Kademlia UDP protocol magic bytes, opcodes, and constants.

/// Standard Kademlia UDP protocol magic byte (0xE4).
pub const PR_KADEMLIA: u8 = 0xE4;

/// Packed Kademlia UDP protocol magic byte (0xE5).
pub const PR_KADEMLIA_PACKED: u8 = 0xE5;

/// Reserved UDP protocol magic byte (0x8D).
pub const PR_KADEMLIA_RESERVED: u8 = 0x8D;

// --- Kad v1.0 Opcodes (Legacy) ---
/// Legacy Bootstrap request (0x00).
pub const KADEMLIA_BOOTSTRAP_REQ_OLD: u8 = 0x00;
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

// NOTE on Kad2 search/publish opcodes (official eMule 0.72a reference):
// Requests:  SEARCH_KEY_REQ=0x33, SEARCH_SOURCE_REQ=0x34, SEARCH_NOTES_REQ=0x35
// Response:  SEARCH_RES=0x3B (unified for all search types)
// Requests:  PUBLISH_KEY_REQ=0x43, PUBLISH_SOURCE_REQ=0x44, PUBLISH_NOTES_REQ=0x45
// Response:  PUBLISH_RES=0x4B (unified for all publish types)
/// Kad2 Find Node / Value request (0x21).
pub const KADEMLIA2_REQ: u8 = 0x21;
/// Kad2 Find Node contact count / subtype (0x0B = 11 contacts requested).
pub const KADEMLIA_FIND_NODE: u8 = 0x0B;
/// Kad2 Find Value contact count / subtype (0x02 contacts requested).
pub const KADEMLIA_FIND_VALUE: u8 = 0x02;
/// Kad2 Find Node / Value response (0x29).
pub const KADEMLIA2_RES: u8 = 0x29;
/// Kad2 Search Keyword request (0x33).
pub const KADEMLIA2_SEARCH_KEY_REQ: u8 = 0x33;
/// Kad2 Search Source request (0x34).
pub const KADEMLIA2_SEARCH_SOURCE_REQ: u8 = 0x34;
/// Kad2 Search Notes request (0x35).
pub const KADEMLIA2_SEARCH_NOTES_REQ: u8 = 0x35;
/// Legacy Kad1 Search Notes request (0x32) — `[target:16]` only.
pub const KADEMLIA_SEARCH_NOTES_REQ_LEGACY: u8 = 0x32;
/// Legacy Kad1 Search Notes response (0x3A) — `[target:16][count:u16]`
/// then the same `[answer:16][taglist]` entries as the unified 0x3B form.
/// NOTE: eMule 0.72a answers Kad2 note searches on `KADEMLIA2_SEARCH_RES`
/// (0x3B); 0x3A is only seen from Kad1 nodes.
pub const KADEMLIA_SEARCH_NOTES_RES: u8 = 0x3A;
/// Kad2 Search response (0x3B) — unified response for keyword, source, and notes search.
pub const KADEMLIA2_SEARCH_RES: u8 = 0x3B;
/// Alias: Kad2 Search Source response is the unified KADEMLIA2_SEARCH_RES (0x3B).
pub const KADEMLIA2_SEARCH_SOURCE_RES: u8 = KADEMLIA2_SEARCH_RES;
/// Alias: Kad2 Search Keyword request is KADEMLIA2_SEARCH_KEY_REQ (0x33).
pub const KADEMLIA2_SEARCH_KEYWORD_REQ: u8 = KADEMLIA2_SEARCH_KEY_REQ;
/// Alias: Kad2 Search Keyword response is the unified KADEMLIA2_SEARCH_RES (0x3B).
pub const KADEMLIA2_SEARCH_KEYWORD_RES: u8 = KADEMLIA2_SEARCH_RES;
/// Legacy internal search keyword response opcode (0x2C, preserved for fallback/tests).
pub const KADEMLIA2_SEARCH_KEYWORD_RES_LEGACY: u8 = 0x2C;
/// Kad2 Publish Keyword request (0x43).
pub const KADEMLIA2_PUBLISH_KEY_REQ: u8 = 0x43;
/// Kad2 Publish Source request (0x44).
pub const KADEMLIA2_PUBLISH_SOURCE_REQ: u8 = 0x44;
/// Kad2 Publish Notes request (0x45).
pub const KADEMLIA2_PUBLISH_NOTES_REQ: u8 = 0x45;
/// Kad2 Publish response (0x4B) — unified response for keyword, source, and notes publish.
pub const KADEMLIA2_PUBLISH_RES: u8 = 0x4B;
/// Alias: Kad2 Publish Source response is the unified KADEMLIA2_PUBLISH_RES (0x4B).
pub const KADEMLIA2_PUBLISH_SOURCE_RES: u8 = KADEMLIA2_PUBLISH_RES;
/// Alias: Kad2 Publish Keyword request is KADEMLIA2_PUBLISH_KEY_REQ (0x43).
pub const KADEMLIA2_PUBLISH_KEYWORD_REQ: u8 = KADEMLIA2_PUBLISH_KEY_REQ;
/// Alias: Kad2 Publish Keyword response is the unified KADEMLIA2_PUBLISH_RES (0x4B).
/// NOTE: Previously was 0x2D which is an invented opcode that doesn't exist in official eMule.
pub const KADEMLIA2_PUBLISH_KEYWORD_RES: u8 = KADEMLIA2_PUBLISH_RES;
/// Kad2 Ping request (0x60).
pub const KADEMLIA2_PING: u8 = 0x60;
/// Kad2 Pong response (0x61).
pub const KADEMLIA2_PONG: u8 = 0x61;
/// Kad2 Firewall UDP test (0x62).
pub const KADEMLIA2_FIREWALLUDP: u8 = 0x62;
/// Kad buddy search request (0x51): [buddy_id:16][user_id:16][tcp_port:2].
pub const KADEMLIA_FINDBUDDY_REQ: u8 = 0x51;
/// Kad buddy callback relay request (0x52): [buddy_id:16][file_id:16][tcp_port:2].
pub const KADEMLIA_CALLBACK_REQ: u8 = 0x52;
/// Kad buddy search response (0x5A): [buddy_id:16][client_id:16][tcp_port:2][connect_opts:1].
pub const KADEMLIA_FINDBUDDY_RES: u8 = 0x5A;
/// Kad TCP firewall check request (0x50): `[tcp_port u16]` (legacy, pre-Kad7).
pub const KADEMLIA_FIREWALLED_REQ: u8 = 0x50;
/// Kad TCP firewall check request v2 (0x53, eMule `KADEMLIA_FIREWALLED2_REQ`):
/// `[tcp_port u16][user_hash 16][connect_options u8]` — 19 bytes.
/// Required for Kad version >= 7.
pub const KADEMLIA_FIREWALLED2_REQ: u8 = 0x53;
/// Kad firewall check response (0x58): `[ip u32]` — echoes the requester's IP.
pub const KADEMLIA_FIREWALLED_RES: u8 = 0x58;
/// Kad firewall check ack over UDP (0x59, deprecated by `OP_KAD_FWTCPCHECK_ACK`
/// on TCP since Kad version 7).
pub const KADEMLIA_FIREWALLED_ACK_RES: u8 = 0x59;

/// eMule `TAG_KADMISCOPTIONS` (`0xF2`, uint8) carried in KADEMLIA2_HELLO_*:
/// bit 0 = UDP firewalled, bit 1 = TCP firewalled, bit 2 = requesting
/// `KADEMLIA2_HELLO_RES_ACK`. Required for Kad version >= 8.
pub const TAG_KADMISCOPTIONS: u8 = 0xF2;
/// eMule `TAG_SOURCEUPORT` (`0xFC`, uint16) — advertised Kad UDP port.
pub const TAG_SOURCEUPORT: u8 = 0xFC;

/// Maximum allowable search keywords returned in a single packet.
pub const MAX_KAD_SEARCH_KEYWORDS: usize = 50;

/// Maximum allowable UDP datagram payload size in bytes (maximum IPv4 UDP payload is 65507 bytes).
pub const MAX_KAD_UDP_PACKET_SIZE: usize = 65507;

/// Checks if the protocol byte is a recognized Kademlia UDP protocol magic.
#[inline]
pub fn is_valid_kad_protocol(protocol: u8) -> bool {
    matches!(
        protocol,
        PR_KADEMLIA | PR_KADEMLIA_PACKED | PR_KADEMLIA_RESERVED
    )
}
