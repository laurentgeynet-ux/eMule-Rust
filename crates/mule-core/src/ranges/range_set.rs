// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

use std::collections::BTreeMap;
use std::ops::Range;

use crate::error::CoreError;

/// Canonical set of half-open intervals `[start, end)`.
///
/// Invariants:
/// - Every range is non-empty: `start < end`.
/// - Ranges are sorted by `start`.
/// - No two ranges overlap or touch (adjacent ranges are automatically merged).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RangeSet {
    ranges: BTreeMap<u64, u64>,
}

impl RangeSet {
    pub fn new() -> Self {
        Self {
            ranges: BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn ranges(&self) -> Vec<Range<u64>> {
        self.ranges.iter().map(|(&s, &e)| s..e).collect()
    }

    /// Total number of bytes covered across all intervals.
    pub fn covered_len(&self) -> u64 {
        self.ranges.iter().map(|(&s, &e)| e - s).sum()
    }

    /// Inserts a half-open range `[start, end)` into the set, merging overlapping and adjacent intervals.
    pub fn insert(&mut self, range: Range<u64>) -> Result<(), CoreError> {
        let (mut new_start, mut new_end) = (range.start, range.end);
        if new_start >= new_end {
            return Err(CoreError::InvalidRange {
                start: new_start,
                end: new_end,
                reason: "start must be strictly less than end",
            });
        }

        // Find existing ranges that overlap or touch [new_start, new_end].
        // An existing range [s, e) overlaps or touches [new_start, new_end] if:
        // s <= new_end and e >= new_start.
        let mut to_remove = Vec::new();

        // Check the predecessor range (if any) that starts before new_start
        if let Some((&s, &e)) = self.ranges.range(..=new_start).next_back() {
            if e >= new_start {
                new_start = s;
                new_end = new_end.max(e);
                to_remove.push(s);
            }
        }

        // Check all ranges starting between new_start and new_end
        for (&s, &e) in self.ranges.range(new_start..=new_end) {
            new_end = new_end.max(e);
            to_remove.push(s);
        }

        for s in to_remove {
            self.ranges.remove(&s);
        }

        self.ranges.insert(new_start, new_end);
        Ok(())
    }

    /// Checks whether the entire `[start, end)` interval is covered.
    pub fn contains(&self, range: Range<u64>) -> bool {
        if range.start >= range.end {
            return false;
        }

        if let Some((&s, &e)) = self.ranges.range(..=range.start).next_back() {
            s <= range.start && e >= range.end
        } else {
            false
        }
    }

    /// Checks whether any part of `[start, end)` intersects with the set.
    pub fn intersects(&self, range: Range<u64>) -> bool {
        if range.start >= range.end {
            return false;
        }

        // Check predecessor
        if let Some((_, &e)) = self.ranges.range(..=range.start).next_back() {
            if e > range.start {
                return true;
            }
        }

        // Check any range starting inside the query interval
        if let Some((&s, _)) = self.ranges.range(range.start..range.end).next() {
            if s < range.end {
                return true;
            }
        }

        false
    }

    /// Returns the missing gaps within a given bounding interval `limit`.
    pub fn missing_within(&self, limit: Range<u64>) -> Vec<Range<u64>> {
        let (limit_start, limit_end) = (limit.start, limit.end);
        if limit_start >= limit_end {
            return Vec::new();
        }

        let mut gaps = Vec::new();
        let mut cursor = limit_start;

        for (&s, &e) in &self.ranges {
            if e <= cursor {
                continue;
            }
            if s >= limit_end {
                break;
            }

            if s > cursor {
                gaps.push(cursor..s.min(limit_end));
            }
            cursor = cursor.max(e);
            if cursor >= limit_end {
                break;
            }
        }

        if cursor < limit_end {
            gaps.push(cursor..limit_end);
        }

        gaps
    }

    /// Evicts a range `[start, end)` from the set (e.g. upon detecting corruption).
    pub fn remove(&mut self, range: Range<u64>) -> Result<(), CoreError> {
        let (rem_start, rem_end) = (range.start, range.end);
        if rem_start >= rem_end {
            return Err(CoreError::InvalidRange {
                start: rem_start,
                end: rem_end,
                reason: "start must be strictly less than end",
            });
        }

        let mut to_remove = Vec::new();
        let mut to_insert = Vec::new();

        // Check predecessor
        if let Some((&s, &e)) = self.ranges.range(..=rem_start).next_back() {
            if e > rem_start {
                to_remove.push(s);
                if s < rem_start {
                    to_insert.push((s, rem_start));
                }
                if e > rem_end {
                    to_insert.push((rem_end, e));
                }
            }
        }

        // Check ranges starting inside the removed range
        for (&s, &e) in self.ranges.range(rem_start..rem_end) {
            to_remove.push(s);
            if e > rem_end {
                to_insert.push((rem_end, e));
            }
        }

        for s in to_remove {
            self.ranges.remove(&s);
        }
        for (s, e) in to_insert {
            self.ranges.insert(s, e);
        }

        Ok(())
    }
}
