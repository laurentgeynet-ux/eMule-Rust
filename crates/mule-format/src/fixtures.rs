// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use std::cmp::min;
use std::io::Read;

/// Canonical repeating 0..=255 pattern.
pub const REPEATING_0_TO_255: [u8; 256] = {
    let mut arr = [0u8; 256];
    let mut i = 0;
    while i < 256 {
        arr[i] = i as u8;
        i += 1;
    }
    arr
};

/// A streaming deterministic byte reader with zero RAM allocation overhead.
pub struct PatternReader<'a> {
    pattern: &'a [u8],
    total_len: u64,
    pos: u64,
}

impl<'a> PatternReader<'a> {
    pub fn new(pattern: &'a [u8], total_len: u64) -> Self {
        Self {
            pattern,
            total_len,
            pos: 0,
        }
    }
}

impl<'a> Read for PatternReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.total_len || buf.is_empty() || self.pattern.is_empty() {
            return Ok(0);
        }

        let remaining = (self.total_len - self.pos) as usize;
        let to_read = min(buf.len(), remaining);

        let mut written = 0;
        while written < to_read {
            let pat_offset = ((self.pos + written as u64) % (self.pattern.len() as u64)) as usize;
            let available_in_pat = self.pattern.len() - pat_offset;
            let chunk = min(to_read - written, available_in_pat);

            buf[written..written + chunk]
                .copy_from_slice(&self.pattern[pat_offset..pat_offset + chunk]);
            written += chunk;
        }

        self.pos += written as u64;
        Ok(written)
    }
}

/// Description of an independent byte pattern fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytePatternFixture {
    pub name: &'static str,
    pub length: u64,
    pub expected_ed2k_hex: &'static str,
    pub expected_part_hashes_count: usize,
    pub expected_aich_root_base32: Option<&'static str>,
}

/// Pre-calculated, independent test fixtures for repeating 0..=255.
pub const FIXTURE_1_BYTE: BytePatternFixture = BytePatternFixture {
    name: "pattern_1_byte",
    length: 1,
    expected_ed2k_hex: "47c61a0fa8738ba77308a8a600f88e4b",
    expected_part_hashes_count: 1,
    expected_aich_root_base32: Some("FD4SSTG4T5CYE7A3J54T7RERUVM32U3Q"),
};

pub const FIXTURE_JUST_UNDER_PART: BytePatternFixture = BytePatternFixture {
    name: "pattern_9727999",
    length: 9_727_999,
    expected_ed2k_hex: "4f2dc92b1fd24bde92dd01d85154b06c",
    expected_part_hashes_count: 1,
    expected_aich_root_base32: None,
};

pub const FIXTURE_EXACT_ONE_PART: BytePatternFixture = BytePatternFixture {
    name: "pattern_9728000",
    length: 9_728_000,
    expected_ed2k_hex: "d815a0409aaa0b4279dd86e580bb043f",
    expected_part_hashes_count: 2, // part_0 + empty
    expected_aich_root_base32: None,
};

pub const FIXTURE_ONE_PART_PLUS_ONE: BytePatternFixture = BytePatternFixture {
    name: "pattern_9728001",
    length: 9_728_001,
    expected_ed2k_hex: "695b81b1727666818988524bd7fc823c",
    expected_part_hashes_count: 2,
    expected_aich_root_base32: None,
};

pub const FIXTURE_EXACT_TWO_PARTS: BytePatternFixture = BytePatternFixture {
    name: "pattern_19456000",
    length: 19_456_000,
    expected_ed2k_hex: "7fdd19736858b9f24c65da0e0e0adba3",
    expected_part_hashes_count: 3, // part_0 + part_1 + empty
    expected_aich_root_base32: None,
};
