// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! `clients.met` parser — eMule `CClientCreditsList::LoadList`
//! (`ClientCredits.cpp:156-233`).
//!
//! File layout: `[u8 version][u32 count]` then `count` packed
//! `CreditStruct` records. Two versions exist (`Opcodes.h:39-40`):
//!
//! - `0x12` (`CREDITFILE_VERSION`) — 119-byte `CreditStruct`
//!   (`ClientCredits.h:41-52`):
//!   `abyKey[16] | nUploadedLo u32 | nDownloadedLo u32 | tLastSeen u32 |
//!    nUploadedHi u32 | nDownloadedHi u32 | nReserved3 u16 |
//!    nKeySize u8 | abySecureIdent[80]`
//! - `0x11` (`CREDITFILE_VERSION_29`) — 38-byte `CreditStruct_29a`
//!   (`ClientCredits.h:28-37`), same fields minus `nKeySize`/pubkey.
//!
//! Any other version → eMule refuses the whole file.

use std::io::Read;
use std::path::Path;

use super::error::MetError;
use super::reader::{open_met_file, CountingReader};

/// `CREDITFILE_VERSION` (`Opcodes.h:39`).
pub const CREDITFILE_VERSION: u8 = 0x12;
/// `CREDITFILE_VERSION_29` (`Opcodes.h:40`) — 0.29a layout.
pub const CREDITFILE_VERSION_29: u8 = 0x11;
/// `MAXPUBKEYSIZE` (`ClientCredits.h:21`).
pub const MAXPUBKEYSIZE: usize = 80;
/// Packed `CreditStruct` size (version 0x12).
const CREDIT_STRUCT_LEN: usize = 119;
/// Packed `CreditStruct_29a` size (version 0x11).
const CREDIT_STRUCT_29A_LEN: usize = 38;
/// Safety bound on the record count — eMule credit lists are user-scale
/// (thousands, not millions).
pub const MAX_CLIENTS_RECORDS: usize = 5_000_000;

/// One `clients.met` credit record.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientsMetRecord {
    /// `abyKey` — the client's eD2K user hash (credits key in eMule).
    pub key: [u8; 16],
    /// `nUploadedHi:nUploadedLo` — bytes uploaded TO the peer.
    pub uploaded: u64,
    /// `nDownloadedHi:nDownloadedLo` — bytes downloaded FROM the peer.
    pub downloaded: u64,
    /// `tLastSeen` — unix seconds; eMule drops entries older than 150
    /// days at load (`ClientCredits.cpp:207-213`).
    pub last_seen: u32,
    /// `abySecureIdent[..nKeySize]` — RSA public key when the peer
    /// completed secure identification; `None` for `nKeySize == 0`.
    pub pubkey: Option<Vec<u8>>,
}

/// Parsed `clients.met` content.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientsMetFile {
    /// File version (`0x12` or `0x11`).
    pub version: u8,
    /// `true` when records carried no public key (`0x11` layout).
    pub legacy_format: bool,
    /// Records in file order.
    pub records: Vec<ClientsMetRecord>,
}

impl ClientsMetFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, MetError> {
        let mut file = open_met_file(path)?;
        Self::read_from(&mut file)
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, MetError> {
        let mut cr = CountingReader::new(reader);
        let version = cr.read_u8("clients.met version")?;
        let (record_len, legacy_format) = match version {
            CREDITFILE_VERSION => (CREDIT_STRUCT_LEN, false),
            CREDITFILE_VERSION_29 => (CREDIT_STRUCT_29A_LEN, true),
            other => {
                return Err(MetError::UnsupportedHeader {
                    offset: 0,
                    value: other,
                })
            }
        };
        let count = cr.read_u32_le("clients.met record count")? as usize;
        if count > MAX_CLIENTS_RECORDS {
            return Err(MetError::InvalidLength {
                offset: cr.offset(),
                declared: count as u64,
                maximum: MAX_CLIENTS_RECORDS as u64,
                context: "clients.met record count",
            });
        }
        let mut records = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            let raw = cr.read_exact_bytes(record_len, "clients.met credit record")?;
            records.push(parse_credit_struct(&raw, legacy_format));
        }
        Ok(Self {
            version,
            legacy_format,
            records,
        })
    }
}

fn parse_credit_struct(raw: &[u8], legacy: bool) -> ClientsMetRecord {
    // Packed field offsets (`ClientCredits.h:41-52`):
    // abyKey@0..16 | nUploadedLo@16 | nDownloadedLo@20 | tLastSeen@24 |
    // nUploadedHi@28 | nDownloadedHi@32 | nReserved3@36 u16 |
    // nKeySize@38 u8 | abySecureIdent@39..119
    let mut key = [0u8; 16];
    key.copy_from_slice(&raw[0..16]);
    let u32_at = |off: usize| u32::from_le_bytes(raw[off..off + 4].try_into().unwrap_or_default());
    let uploaded = ((u32_at(28) as u64) << 32) | u32_at(16) as u64;
    let downloaded = ((u32_at(32) as u64) << 32) | u32_at(20) as u64;
    let last_seen = u32_at(24);
    let pubkey = if legacy {
        None
    } else {
        let key_size = raw[38] as usize;
        (key_size > 0 && key_size <= MAXPUBKEYSIZE)
            .then(|| raw[39..39 + key_size].to_vec())
    };
    ClientsMetRecord {
        key,
        uploaded,
        downloaded,
        last_seen,
        pubkey,
    }
}
