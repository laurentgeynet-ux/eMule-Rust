// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

pub mod aich;
pub mod ed2k;

pub use aich::verify_aich_block;
pub use ed2k::verify_part_hash;
