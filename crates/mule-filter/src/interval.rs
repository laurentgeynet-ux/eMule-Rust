// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

/// An IPv4 address represented as an inclusive range `[start, end]`.
/// IP values are stored as big-endian `u32` (network order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterRange {
    pub start: u32,
    pub end: u32,
    pub level: u32,
    pub description: String,
}

impl FilterRange {
    pub fn new(start: Ipv4Addr, end: Ipv4Addr, level: u32, description: impl Into<String>) -> Self {
        let start_u32 = u32::from(start);
        let end_u32 = u32::from(end);
        let (s, e) = if start_u32 <= end_u32 {
            (start_u32, end_u32)
        } else {
            (end_u32, start_u32)
        };
        Self {
            start: s,
            end: e,
            level,
            description: description.into(),
        }
    }

    pub fn contains(&self, ip: u32) -> bool {
        ip >= self.start && ip <= self.end
    }

    pub fn start_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.start)
    }

    pub fn end_ip(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.end)
    }
}

/// Merges sorted intervals where possible.
/// If two ranges overlap or are adjacent AND have the same level, they are merged.
/// If they have different levels or descriptions, descriptions can be appended or kept.
pub fn coalesce_ranges(mut ranges: Vec<FilterRange>) -> (Vec<FilterRange>, usize) {
    if ranges.len() <= 1 {
        return (ranges, 0);
    }

    // Sort by start ascending, then end ascending
    ranges.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)));

    let mut merged: Vec<FilterRange> = Vec::with_capacity(ranges.len());
    let mut merged_count = 0usize;

    for r in ranges {
        if let Some(last) = merged.last_mut() {
            // Check if current range overlaps or is immediately adjacent (last.end + 1 == r.start)
            // Be careful with u32 overflow: last.end < u32::MAX
            let is_adjacent = last.end < u32::MAX && (last.end + 1 >= r.start);
            let is_overlapping = r.start <= last.end;

            if (is_adjacent || is_overlapping) && last.level == r.level {
                // Merge into last
                if r.end > last.end {
                    last.end = r.end;
                }
                if !r.description.is_empty() && !last.description.contains(&r.description) {
                    if last.description.is_empty() {
                        last.description = r.description;
                    } else {
                        last.description.push_str(" | ");
                        last.description.push_str(&r.description);
                    }
                }
                merged_count += 1;
            } else {
                merged.push(r);
            }
        } else {
            merged.push(r);
        }
    }

    merged.shrink_to_fit();
    (merged, merged_count)
}
