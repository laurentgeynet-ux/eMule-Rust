// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Constructeurs de fixtures binaires `.met` partagés entre les tests
//! d'intégration (`known.met`, `clients.met`, `emfriends.met`…).
//!
//! Ces helpers sérialisent **à la main** le layout `CTag::WriteTagToFile` /
//! `CreditStruct` afin que les tests des readers restent indépendants des
//! encodeurs de production (un bug symétrique writer/reader ne peut pas
//! passer inaperçu).

use mule_format::met::tags::{TAGTYPE_HASH16, TAGTYPE_STRING, TAGTYPE_UINT32, TAGTYPE_UINT64};

/// Écrit l'en-tête d'un tag à nom court : `[type][u16 namelen=1][u8 name_id]`.
pub fn push_tag_id(buf: &mut Vec<u8>, tag_type: u8, name_id: u8) {
    buf.push(tag_type);
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.push(name_id);
}

/// Écrit un tag `TAGTYPE_STRING` à nom court.
pub fn push_string_tag(buf: &mut Vec<u8>, name_id: u8, value: &str) {
    push_tag_id(buf, TAGTYPE_STRING, name_id);
    buf.extend_from_slice(&(value.len() as u16).to_le_bytes());
    buf.extend_from_slice(value.as_bytes());
}

/// Écrit un tag `TAGTYPE_UINT32` à nom court.
pub fn push_u32_tag(buf: &mut Vec<u8>, name_id: u8, value: u32) {
    push_tag_id(buf, TAGTYPE_UINT32, name_id);
    buf.extend_from_slice(&value.to_le_bytes());
}

/// Écrit un tag `TAGTYPE_UINT64` à nom court.
pub fn push_u64_tag(buf: &mut Vec<u8>, name_id: u8, value: u64) {
    push_tag_id(buf, TAGTYPE_UINT64, name_id);
    buf.extend_from_slice(&value.to_le_bytes());
}

/// Écrit un tag `TAGTYPE_HASH16` à nom court.
pub fn push_hash16_tag(buf: &mut Vec<u8>, name_id: u8, value: &[u8; 16]) {
    push_tag_id(buf, TAGTYPE_HASH16, name_id);
    buf.extend_from_slice(value);
}

/// Packed `CreditStruct` (0x12, 119 B) - `ClientCredits.h:41-52`.
pub fn credit_record(
    key: &[u8; 16],
    uploaded: u64,
    downloaded: u64,
    last_seen: u32,
    pubkey: Option<&[u8]>,
) -> Vec<u8> {
    let mut rec = Vec::new();
    rec.extend_from_slice(key);
    rec.extend_from_slice(&(uploaded as u32).to_le_bytes()); // nUploadedLo
    rec.extend_from_slice(&(downloaded as u32).to_le_bytes()); // nDownloadedLo
    rec.extend_from_slice(&last_seen.to_le_bytes()); // tLastSeen
    rec.extend_from_slice(&((uploaded >> 32) as u32).to_le_bytes()); // nUploadedHi
    rec.extend_from_slice(&((downloaded >> 32) as u32).to_le_bytes()); // nDownloadedHi
    rec.extend_from_slice(&0u16.to_le_bytes()); // nReserved3
    let pubkey = pubkey.unwrap_or(&[]);
    rec.push(pubkey.len() as u8); // nKeySize
    rec.extend_from_slice(pubkey); // abySecureIdent[..nKeySize]
    rec.extend(std::iter::repeat_n(0u8, 80 - pubkey.len()));
    rec
}

/// Sérialise un fichier `clients.met` : `[version u8][count u32][records]`.
pub fn clients_met(version: u8, records: &[Vec<u8>]) -> Vec<u8> {
    let mut buf = vec![version];
    buf.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for r in records {
        buf.extend_from_slice(r);
    }
    buf
}
