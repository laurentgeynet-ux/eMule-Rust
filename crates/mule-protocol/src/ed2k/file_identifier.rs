// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! `FileIdentifier` — identification composite d'un fichier introduite par eMule 0.50
//! et généralisée en 0.72a (`FileIdentifier.h` / `FileIdentifier.cpp`).
//!
//! Format filaire :
//!
//! ```text
//! <options u8>[<md4 hash 16>][<file size u64>][<aich root hash 20>]
//! ```
//!
//! Octet d'options (`opcodes.h` / `FileIdentifier.cpp:165`) :
//! - bits 0..=3 (`0x0F`) : présence des champs — bit0 = MD4, bit1 = taille u64,
//!   bit2 = hash AICH. Ces bits sont **requis** par eMule (`bRequiredOptions = 0x0F`) :
//!   toute option inconnue de cette moitié provoque un rejet.
//! - bits 4..=7 (`0xF0`) : options optionnelles réservées — ignorées par eMule
//!   (`bOptionalSubOptions = 0xF0`).
//!
//! En pratique eMule exige toujours le hash MD4 sauf contexte Kad spécifique
//! (`CFileIdentifierSA::ReadIdentifier(…, bAllowEmptyMD4Hash)`). La taille u64
//! est également toujours présente dans le trafic moderne.
//!
//! Référence : `srchybrid/FileIdentifier.cpp:125-250`, `opcodes.h:286-289`.

use super::error::{Ed2kErrorKind, Ed2kParseError};
use super::tags::TagReader;

/// Option bit0 — hash MD4 présent (`FileIdentifier.cpp` `0x01`).
pub const FILE_IDENTIFIER_OPT_MD4: u8 = 0x01;
/// Option bit1 — taille de fichier `u64` présente (`0x02`).
pub const FILE_IDENTIFIER_OPT_SIZE: u8 = 0x02;
/// Option bit2 — hash racine AICH (20 octets) présent (`0x04`).
pub const FILE_IDENTIFIER_OPT_AICH: u8 = 0x04;
/// Masque des options obligatoires : eMule rejette tout bit inconnu ici
/// (`bRequiredOptions = 0x0F`, `FileIdentifier.cpp:185`).
pub const FILE_IDENTIFIER_REQUIRED_MASK: u8 = 0x0F;
/// Masque des options optionnelles : bits réservés, ignorés à la réception
/// (`bOptionalSubOptions = 0xF0`, `FileIdentifier.cpp:187`).
pub const FILE_IDENTIFIER_OPTIONAL_MASK: u8 = 0xF0;

/// Taille du hash MD4 eD2K.
pub const MD4_HASH_LEN: usize = 16;
/// Taille du hash racine AICH (SHA-1).
pub const AICH_HASH_LEN: usize = 20;

/// Identifiant composite d'un fichier (`FileIdentifier`, eMule 0.72a).
///
/// Sérialisé en tête de `OP_HASHSETREQUEST2`/`OP_HASHSETANSWER2` et
/// `OP_MULTIPACKET_EXT2`/`OP_MULTIPACKETANSWER_EXT2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kFileIdentifier {
    /// Hash MD4 du fichier. `None` uniquement dans les contextes Kad
    /// (`bAllowEmptyMD4Hash`) ; sur TCP eMule exige sa présence.
    pub md4_hash: Option<[u8; MD4_HASH_LEN]>,
    /// Taille du fichier en octets.
    pub file_size: Option<u64>,
    /// Hash racine AICH (SHA-1 de l'arbre de hachage).
    pub aich_hash: Option<[u8; AICH_HASH_LEN]>,
    /// Bits optionnels supérieurs (`0xF0`) lus sur le fil, préservés tels quels
    /// pour un round-trip déterministe.
    pub optional_bits: u8,
}

impl Ed2kFileIdentifier {
    /// Identifiant standard TCP : hash MD4 + taille (le minimum exigé par eMule).
    pub fn new(md4_hash: [u8; MD4_HASH_LEN], file_size: u64) -> Self {
        Self {
            md4_hash: Some(md4_hash),
            file_size: Some(file_size),
            aich_hash: None,
            optional_bits: 0,
        }
    }

    /// Octet d'options encodé.
    pub fn options_byte(&self) -> u8 {
        let mut opts = self.optional_bits & FILE_IDENTIFIER_OPTIONAL_MASK;
        if self.md4_hash.is_some() {
            opts |= FILE_IDENTIFIER_OPT_MD4;
        }
        if self.file_size.is_some() {
            opts |= FILE_IDENTIFIER_OPT_SIZE;
        }
        if self.aich_hash.is_some() {
            opts |= FILE_IDENTIFIER_OPT_AICH;
        }
        opts
    }

    /// Écrit l'identifiant sur le fil (ordre : options, MD4, taille, AICH).
    pub fn write_to(&self, out: &mut Vec<u8>) {
        out.push(self.options_byte());
        if let Some(h) = &self.md4_hash {
            out.extend_from_slice(h);
        }
        if let Some(s) = self.file_size {
            out.extend_from_slice(&s.to_le_bytes());
        }
        if let Some(h) = &self.aich_hash {
            out.extend_from_slice(h);
        }
    }

    /// Lit un `FileIdentifier` depuis `reader`.
    ///
    /// `allow_empty_md4` correspond à `bAllowEmptyMD4Hash` de
    /// `CFileIdentifierSA::ReadIdentifier` (`FileIdentifier.cpp:198`) : `false`
    /// sur TCP (eMule exige le MD4), `true` possible en contexte Kad.
    ///
    /// Comme eMule (`FileIdentifier.cpp:185-191`), toute option inconnue dans le
    /// masque obligatoire `0x0F` provoque `InvalidField` ; les bits optionnels
    /// `0xF0` sont tolérés et conservés dans `optional_bits`.
    pub fn read_from(
        reader: &mut TagReader<'_>,
        allow_empty_md4: bool,
    ) -> Result<Self, Ed2kParseError> {
        let opts = reader.read_u8("file_identifier_options")?;

        let unknown_required = opts
            & FILE_IDENTIFIER_REQUIRED_MASK
            & !(FILE_IDENTIFIER_OPT_MD4 | FILE_IDENTIFIER_OPT_SIZE | FILE_IDENTIFIER_OPT_AICH);
        if unknown_required != 0 {
            return Err(Ed2kParseError::new(
                reader.protocol(),
                reader.opcode(),
                reader.pos() - 1,
                "file_identifier_options",
                Ed2kErrorKind::MalformedField {
                    message: format!(
                        "file identifier required options unknown: 0x{unknown_required:02X}"
                    ),
                },
            ));
        }

        let md4_hash = if opts & FILE_IDENTIFIER_OPT_MD4 != 0 {
            Some(reader.read_hash("file_identifier_md4")?)
        } else {
            None
        };
        if md4_hash.is_none() && !allow_empty_md4 {
            return Err(Ed2kParseError::new(
                reader.protocol(),
                reader.opcode(),
                reader.pos() - 1,
                "file_identifier_md4_missing",
                Ed2kErrorKind::MalformedField {
                    message: "file identifier without MD4 hash on TCP context".to_string(),
                },
            ));
        }

        let file_size = if opts & FILE_IDENTIFIER_OPT_SIZE != 0 {
            Some(reader.read_u64_le("file_identifier_size")?)
        } else {
            None
        };

        let aich_hash = if opts & FILE_IDENTIFIER_OPT_AICH != 0 {
            let bytes = reader.read_bytes(AICH_HASH_LEN, "file_identifier_aich")?;
            let mut arr = [0u8; AICH_HASH_LEN];
            arr.copy_from_slice(bytes);
            Some(arr)
        } else {
            None
        };

        Ok(Self {
            md4_hash,
            file_size,
            aich_hash,
            optional_bits: opts & FILE_IDENTIFIER_OPTIONAL_MASK,
        })
    }
}
