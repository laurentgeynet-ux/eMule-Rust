// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

pub mod error;
pub mod part_met;
pub mod reader;
pub mod server_met;
pub mod tags;

pub use error::MetError;
pub use part_met::{
    PartMetFile, PARTFILE_SPLITTEDVERSION, PARTFILE_VERSION, PARTFILE_VERSION_LARGEFILE,
};
pub use reader::{read_tag, CountingReader};
pub use server_met::{ServerEntry, ServerMetFile, MAX_SERVER_COUNT, SERVER_MET_VERSION};
pub use tags::*;
