// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum FormatError {
    #[error("I/O error: {0}")]
    Io(String),

    #[error("Invalid eD2K hash: expected 32 hex chars, got '{0}'")]
    InvalidEd2kHashString(String),

    #[error("Empty input is not a valid eD2K download stream")]
    EmptyInput,

    #[error("Invalid eD2K link: {0}")]
    InvalidLink(String),

    #[error("Invalid AICH base32 string: '{0}'")]
    InvalidAichBase32(String),

    #[error("Invalid AICH block index {0}: exceeds expected block count {1}")]
    InvalidAichBlockIndex(usize, usize),

    #[error("AICH Merkle verification failed: {0}")]
    AichVerificationFailed(String),

    #[error(transparent)]
    Met(#[from] crate::met::error::MetError),

    #[error("Input length exceeds safe threshold: {0} bytes")]
    InputTooLarge(usize),
}

impl From<std::io::Error> for FormatError {
    fn from(err: std::io::Error) -> Self {
        FormatError::Io(err.to_string())
    }
}
