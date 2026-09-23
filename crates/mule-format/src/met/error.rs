// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use thiserror::Error;

/// Granular, offset-aware errors for parsing eMule binary metadata files (.met).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetError {
    #[error("unsupported .met header 0x{value:02x} at offset {offset}")]
    UnsupportedHeader { offset: u64, value: u8 },

    #[error("unexpected end of file at offset {offset} while reading {context}")]
    UnexpectedEof { offset: u64, context: &'static str },

    #[error("invalid tag type 0x{tag_type:02x} at offset {offset}")]
    InvalidTagType { offset: u64, tag_type: u8 },

    #[error(
        "declared length {declared} exceeds maximum {maximum} at offset {offset} for {context}"
    )]
    InvalidLength {
        offset: u64,
        declared: u64,
        maximum: u64,
        context: &'static str,
    },

    #[error("invalid UTF-8 at offset {offset} for {context}")]
    InvalidUtf8 { offset: u64, context: &'static str },

    #[error("integer overflow at offset {offset} while reading {context}")]
    IntegerOverflow { offset: u64, context: &'static str },

    #[error("inconsistent metadata for {field}: {reason}")]
    InconsistentMetadata { field: &'static str, reason: String },

    #[error("I/O error at offset {offset}: {message}")]
    Io { offset: u64, message: String },
}
