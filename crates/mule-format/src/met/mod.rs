// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

pub mod cancelled_met;
pub mod emfriends;
pub mod error;
pub mod known_met;
pub mod part_met;
pub mod preferences_dat;
pub mod reader;
pub mod server_met;
pub mod tags;

pub use cancelled_met::{cancelled_mask, CancelledEntry, CancelledMetFile, CANCELLED_VERSION};
pub use emfriends::{EmfriendsMetFile, EmfriendsRecord};
pub use error::MetError;
pub use known_met::{
    KnownFileRecord, KnownMetFile, MAX_KNOWN_RECORDS, MAX_PART_HASH_COUNT, MET_HEADER,
    MET_HEADER_I64TAGS, PR_AUTO, PR_HIGH, PR_LOW, PR_NORMAL, PR_VERYHIGH, PR_VERYLOW,
};
pub use preferences_dat::{is_bad_hash, PreferencesDat, PREFFILE_VERSION};
pub use part_met::{
    PartMetFile, PARTFILE_SPLITTEDVERSION, PARTFILE_VERSION, PARTFILE_VERSION_LARGEFILE,
};
pub use reader::{read_tag, CountingReader};
pub use server_met::{ServerEntry, ServerMetFile, MAX_SERVER_COUNT, SERVER_MET_VERSION};
pub use tags::*;
