// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Framing and packet structures for eMule protocol obfuscation handshakes.

use rand::RngCore;

use crate::ed2k::{PR_ED2K, PR_EMULE, PR_EMULE_COMPRESSED};

/// Synchronization magic value to verify that encryption works (`0x835E6FC4`).
pub const MAGICVALUE_SYNC: u32 = 0x835E6FC4;

/// Encryption method identifier for basic RC4 protocol obfuscation (`0x00`).
pub const ENM_OBFUSCATION: u8 = 0x00;

/// Returns `true` if `byte` is a standard unencrypted eD2K protocol marker.
#[inline]
pub fn is_standard_protocol_marker(byte: u8) -> bool {
    byte == PR_ED2K || byte == PR_EMULE || byte == PR_EMULE_COMPRESSED
}

/// Generates a semi-random byte that is guaranteed NOT to collide with standard protocol markers.
pub fn generate_semi_random_marker<R: RngCore>(rng: &mut R) -> u8 {
    loop {
        let b = (rng.next_u32() & 0xFF) as u8;
        if !is_standard_protocol_marker(b) {
            return b;
        }
    }
}

/// Initiator packet sent by Client A to Client B (unencrypted header + encrypted payload).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientInitiatorPacket {
    /// Unencrypted marker byte (not `0xE3`, `0xC5`, `0xD4`).
    pub marker: u8,
    /// Unencrypted random key part (4 bytes).
    pub random_key_part: u32,
    /// Encrypted sync magic value (`0x835E6FC4`).
    pub magic_sync: u32,
    /// Encrypted supported encryption methods (0x00).
    pub supported_methods: u8,
    /// Encrypted preferred encryption method (0x00).
    pub preferred_method: u8,
    /// Encrypted random padding bytes.
    pub padding: Vec<u8>,
}

impl ClientInitiatorPacket {
    /// Builds a new client initiator packet with random padding up to `max_padding` bytes.
    pub fn new<R: RngCore>(rng: &mut R, max_padding: u8) -> Self {
        let marker = generate_semi_random_marker(rng);
        let random_key_part = rng.next_u32();
        let pad_len = if max_padding > 0 {
            (rng.next_u32() % (max_padding as u32 + 1)) as u8
        } else {
            0
        };
        let mut padding = vec![0u8; pad_len as usize];
        rng.fill_bytes(&mut padding);

        Self {
            marker,
            random_key_part,
            magic_sync: MAGICVALUE_SYNC,
            supported_methods: ENM_OBFUSCATION,
            preferred_method: ENM_OBFUSCATION,
            padding,
        }
    }

    /// Serializes the unencrypted header (`marker` + `random_key_part`, 5 bytes).
    pub fn serialize_unencrypted_header(&self) -> [u8; 5] {
        let mut out = [0u8; 5];
        out[0] = self.marker;
        out[1..5].copy_from_slice(&self.random_key_part.to_le_bytes());
        out
    }

    /// Serializes the unencrypted payload before RC4 encryption:
    /// `magic_sync` (4B) + `supported` (1B) + `preferred` (1B) + `pad_len` (1B) + `padding` (NB).
    pub fn serialize_payload_plain(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(7 + self.padding.len());
        out.extend_from_slice(&self.magic_sync.to_le_bytes());
        out.push(self.supported_methods);
        out.push(self.preferred_method);
        out.push(self.padding.len() as u8);
        out.extend_from_slice(&self.padding);
        out
    }
}

/// Responder packet sent by Client B to Client A (entirely encrypted).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientResponderPacket {
    /// Encrypted sync magic value (`0x835E6FC4`).
    pub magic_sync: u32,
    /// Encrypted selected encryption method (`0x00`).
    pub selected_method: u8,
    /// Encrypted random padding bytes.
    pub padding: Vec<u8>,
}

impl ClientResponderPacket {
    /// Builds a new client responder packet with random padding up to `max_padding` bytes.
    pub fn new<R: RngCore>(rng: &mut R, max_padding: u8) -> Self {
        let pad_len = if max_padding > 0 {
            (rng.next_u32() % (max_padding as u32 + 1)) as u8
        } else {
            0
        };
        let mut padding = vec![0u8; pad_len as usize];
        rng.fill_bytes(&mut padding);

        Self {
            magic_sync: MAGICVALUE_SYNC,
            selected_method: ENM_OBFUSCATION,
            padding,
        }
    }

    /// Serializes the responder packet before RC4 encryption:
    /// `magic_sync` (4B) + `selected_method` (1B) + `pad_len` (1B) + `padding` (NB).
    pub fn serialize_payload_plain(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(6 + self.padding.len());
        out.extend_from_slice(&self.magic_sync.to_le_bytes());
        out.push(self.selected_method);
        out.push(self.padding.len() as u8);
        out.extend_from_slice(&self.padding);
        out
    }
}
