use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::net::IpAddr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use crate::interval::FilterRange;
use crate::parser::{parse_ipfilter, ParseError, ParseReport, MAX_FILE_SIZE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterStats {
    pub enabled: bool,
    pub range_count: usize,
    pub hits_count: u64,
    pub last_import_report: Option<ParseReport>,
}

/// Thread-safe in-memory IP filter based on sorted disjoint intervals.
/// Lookup is \(O(\log N)\) using binary search.
pub struct IpFilter {
    enabled: AtomicBool,
    ranges: RwLock<Vec<FilterRange>>,
    hits: std::sync::atomic::AtomicU64,
    last_report: RwLock<Option<ParseReport>>,
}

impl Default for IpFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl IpFilter {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            ranges: RwLock::new(Vec::new()),
            hits: std::sync::atomic::AtomicU64::new(0),
            last_report: RwLock::new(None),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn len(&self) -> usize {
        self.ranges.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        let mut w = self.ranges.write().unwrap();
        w.clear();
        *self.last_report.write().unwrap() = None;
    }

    /// Load and parse filter ranges from a local file path.
    pub fn load_from_file<P: AsRef<Path>>(&self, path: P) -> Result<ParseReport, ParseError> {
        let path_ref = path.as_ref();
        let metadata = std::fs::metadata(path_ref)?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::FileTooLarge(metadata.len()));
        }

        let file = File::open(path_ref)?;
        let reader = BufReader::new(file);
        let (ranges, report) = parse_ipfilter(reader)?;

        {
            let mut w = self.ranges.write().unwrap();
            *w = ranges;
        }
        {
            let mut lr = self.last_report.write().unwrap();
            *lr = Some(report.clone());
        }

        Ok(report)
    }

    /// Load and parse filter ranges from an in-memory string or byte slice.
    pub fn load_from_str(&self, content: &str) -> Result<ParseReport, ParseError> {
        if content.len() as u64 > MAX_FILE_SIZE {
            return Err(ParseError::FileTooLarge(content.len() as u64));
        }
        let reader = BufReader::new(Cursor::new(content.as_bytes()));
        let (ranges, report) = parse_ipfilter(reader)?;

        {
            let mut w = self.ranges.write().unwrap();
            *w = ranges;
        }
        {
            let mut lr = self.last_report.write().unwrap();
            *lr = Some(report.clone());
        }

        Ok(report)
    }

    /// Checks whether an IP address is blocked.
    /// Supports native IPv4 as well as IPv4-mapped IPv6 (`::ffff:a.b.c.d`).
    /// Native IPv6 (not mapped to v4) is currently not blocked by standard `ipfilter.dat` unless configured.
    pub fn check_ip(&self, ip: IpAddr) -> Option<FilterRange> {
        if !self.is_enabled() {
            return None;
        }

        let v4_opt = match ip {
            IpAddr::V4(v4) => Some(v4),
            IpAddr::V6(v6) => v6.to_ipv4_mapped(),
        };

        let v4 = v4_opt?;
        let ip_u32 = u32::from(v4);

        let guard = self.ranges.read().unwrap();
        let ranges = guard.as_slice();

        // Binary search on ranges. Since ranges are sorted by start ascending and merged (disjoint):
        // We binary search for the first range whose start <= ip_u32
        let idx = match ranges.binary_search_by(|r| {
            if r.start <= ip_u32 && ip_u32 <= r.end {
                std::cmp::Ordering::Equal
            } else if r.start > ip_u32 {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Less
            }
        }) {
            Ok(found_idx) => found_idx,
            Err(_) => return None,
        };

        self.hits.fetch_add(1, Ordering::Relaxed);
        Some(ranges[idx].clone())
    }

    pub fn stats(&self) -> FilterStats {
        FilterStats {
            enabled: self.is_enabled(),
            range_count: self.len(),
            hits_count: self.hits.load(Ordering::Relaxed),
            last_import_report: self.last_report.read().unwrap().clone(),
        }
    }
}
