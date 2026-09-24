// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! eMule `cancelled.met` reader — `CKnownFileList::LoadCancelledFiles`
//! (`KnownFileList.cpp:122-178`).
//!
//! Layout (current version):
//! ```text
//! [u8  header]            MET_HEADER_I64TAGS (0x0F)
//! [u8  version]           CANCELLED_VERSION (≤ 0x01)
//! [u32 seed]              m_dwCancelledFilesSeed
//! [u32 record_count]
//! per record: [16B masked_hash] [u8 tag_count] [tags]
//! ```
//!
//! Privacy model (`KnownFileList.cpp:384-411`): the stored hash is NOT
//! the file hash — it is `MD5(seed_le || file_hash)` so the cancelled
//! list never leaks what was downloaded then cancelled. The deprecated
//! `0x0E` format stored real file hashes; eMule converts them at load
//! with a fresh random seed — we expose them as `Real` so the importer
//! can mask them under ITS OWN seed.

use md5::Digest;
use std::io::Read;
use std::path::Path;

use crate::met::error::MetError;
use crate::met::known_met::{MET_HEADER, MET_HEADER_I64TAGS};
use crate::met::reader::CountingReader;

/// `CANCELLED_VERSION` (`KnownFileList.cpp:46`) — only version eMule
/// accepts for the `0x0F` layout.
pub const CANCELLED_VERSION: u8 = 0x01;

/// Bounded record count — a cancelled list stays small; the cap only
/// guards against corrupt count fields.
const MAX_CANCELLED_RECORDS: usize = 500_000;
/// Bounded tag count per record — eMule writes zero; the reader skips
/// tags for forward compatibility (`for compatibility with future
/// versions`, `KnownFileList.cpp:167`).
const MAX_CANCELLED_TAGS: usize = 1_000;

/// One cancelled entry. Whether the hash is already masked depends on
/// the file version — see [`CancelledMetFile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelledEntry {
    /// `0x0F` format — `MD5(seed_le || file_hash)`, seed from the file.
    Masked([u8; 16]),
    /// Deprecated `0x0E` format — the real eD2K file hash; the importer
    /// must mask it under its own seed (eMule does the same at load).
    Real([u8; 16]),
}

/// Parsed `cancelled.met` content.
#[derive(Debug, Clone, PartialEq)]
pub struct CancelledMetFile {
    /// `m_dwCancelledFilesSeed` — present only for the `0x0F` layout.
    /// `0` is impossible in valid files (eMule regenerates it).
    pub seed: Option<u32>,
    /// Records in file order.
    pub entries: Vec<CancelledEntry>,
    /// `true` when the file used the deprecated `0x0E` layout — entries
    /// are all `Real` hashes.
    pub legacy_format: bool,
}

impl CancelledMetFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, MetError> {
        let mut file = crate::met::reader::open_met_file(path)?;
        Self::read_from(&mut file)
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, MetError> {
        let mut cr = CountingReader::new(reader);
        let header_offset = cr.offset();
        let header = cr.read_u8("cancelled.met header")?;
        let legacy_format = match header {
            MET_HEADER_I64TAGS => false,
            MET_HEADER => true,
            other => {
                return Err(MetError::UnsupportedHeader {
                    offset: header_offset,
                    value: other,
                })
            }
        };
        let seed = if legacy_format {
            None
        } else {
            let version = cr.read_u8("cancelled version")?;
            // `> CANCELLED_VERSION` → eMule refuses the file.
            if version > CANCELLED_VERSION {
                return Err(MetError::UnsupportedHeader {
                    offset: cr.offset(),
                    value: version,
                });
            }
            Some(cr.read_u32_le("cancelled seed")?)
        };
        let record_count = cr.read_u32_le("record count")? as usize;
        if record_count > MAX_CANCELLED_RECORDS {
            return Err(MetError::InvalidLength {
                offset: cr.offset(),
                declared: record_count as u64,
                maximum: MAX_CANCELLED_RECORDS as u64,
                context: "cancelled record count",
            });
        }
        let mut entries = Vec::with_capacity(record_count.min(4096));
        for _ in 0..record_count {
            let mut hash = [0u8; 16];
            hash.copy_from_slice(&cr.read_exact_bytes(16, "cancelled hash")?);
            let tag_count = cr.read_u8("tag count")? as usize;
            if tag_count > MAX_CANCELLED_TAGS {
                return Err(MetError::InvalidLength {
                    offset: cr.offset(),
                    declared: tag_count as u64,
                    maximum: MAX_CANCELLED_TAGS as u64,
                    context: "cancelled tag count",
                });
            }
            // Future-version fields are skipped, like eMule's tag loop.
            for _ in 0..tag_count {
                let _ = cr.read_tag()?;
            }
            entries.push(if legacy_format {
                CancelledEntry::Real(hash)
            } else {
                CancelledEntry::Masked(hash)
            });
        }
        Ok(Self {
            seed,
            entries,
            legacy_format,
        })
    }
}

/// eMule `AddCancelledFileID`/`IsCancelledFileByID`
/// (`KnownFileList.cpp:384-411`) — masked hash =
/// `MD5(seed_u32_le || file_hash)`.
pub fn cancelled_mask(seed: u32, file_hash: &[u8; 16]) -> [u8; 16] {
    let mut buf = [0u8; 20];
    buf[..4].copy_from_slice(&seed.to_le_bytes());
    buf[4..].copy_from_slice(file_hash);
    let digest = md5::Md5::digest(buf);
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}
