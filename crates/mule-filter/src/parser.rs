// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::net::Ipv4Addr;
use std::str::FromStr;
use thiserror::Error;

use crate::interval::{coalesce_ranges, FilterRange};

pub const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB limit
pub const MAX_LINE_LEN: usize = 1024; // 1024 bytes per line max
pub const MAX_ENTRIES: usize = 500_000; // 500,000 rules max

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("File size {0} bytes exceeds maximum allowed {MAX_FILE_SIZE} bytes")]
    FileTooLarge(u64),
    #[error("Line {0} exceeds maximum length {MAX_LINE_LEN} bytes")]
    LineTooLong(usize),
    #[error("Entry count exceeds maximum allowed {MAX_ENTRIES}")]
    TooManyEntries,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ParseReport {
    pub total_lines: usize,
    pub comment_or_empty_lines: usize,
    pub valid_entries_parsed: usize,
    pub invalid_lines: usize,
    pub coalesced_count: usize,
    pub final_range_count: usize,
    pub warnings: Vec<String>,
}

/// Parses an `ipfilter.dat` stream defensively.
///
/// Supported formats:
/// 1) Standard eMule / PeerGuardian:
///    `001.000.000.000 - 001.255.255.255 , 000 , Description`
/// 2) Short format:
///    `1.0.0.0 - 1.255.255.255 , Description` (default level 0)
/// 3) CIDR / Single IP (if separated by hyphen or same start-end):
///    `1.2.3.4 - 1.2.3.4 , 100 , Spam`
///
/// Lines starting with `#`, `//`, `;` are treated as comments.
pub fn parse_ipfilter<R: BufRead>(
    reader: R,
) -> Result<(Vec<FilterRange>, ParseReport), ParseError> {
    let mut report = ParseReport::default();
    let mut raw_ranges = Vec::new();

    for (line_idx, line_res) in reader.lines().enumerate() {
        let line_num = line_idx + 1;
        let line = line_res?;

        if line.len() > MAX_LINE_LEN {
            report.invalid_lines += 1;
            if report.warnings.len() < 50 {
                report.warnings.push(format!(
                    "Line {}: exceeded length limit of {} bytes",
                    line_num, MAX_LINE_LEN
                ));
            }
            continue;
        }

        report.total_lines += 1;
        let trimmed = line.trim();

        // Comments or empty line
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with("//")
            || trimmed.starts_with(';')
        {
            report.comment_or_empty_lines += 1;
            continue;
        }

        match parse_line(trimmed) {
            Some(range) => {
                if raw_ranges.len() >= MAX_ENTRIES {
                    return Err(ParseError::TooManyEntries);
                }
                raw_ranges.push(range);
                report.valid_entries_parsed += 1;
            }
            None => {
                report.invalid_lines += 1;
                if report.warnings.len() < 50 {
                    report.warnings.push(format!(
                        "Line {}: invalid ipfilter format: '{}'",
                        line_num, trimmed
                    ));
                }
            }
        }
    }

    let (merged, coalesced) = coalesce_ranges(raw_ranges);
    report.coalesced_count = coalesced;
    report.final_range_count = merged.len();

    Ok((merged, report))
}

fn parse_line(line: &str) -> Option<FilterRange> {
    // Format: "start - end , level , description" or "start - end , description"
    let parts: Vec<&str> = line.splitn(3, ',').collect();
    if parts.is_empty() {
        return None;
    }

    let ip_range_part = parts[0].trim();
    let ips: Vec<&str> = ip_range_part.split('-').collect();
    if ips.len() != 2 {
        return None;
    }

    let start_ip = parse_ipv4(ips[0].trim())?;
    let end_ip = parse_ipv4(ips[1].trim())?;

    let (level, description) = if parts.len() == 1 {
        (0u32, String::new())
    } else if parts.len() == 2 {
        let second = parts[1].trim();
        // Check if second part is numeric level or description
        if let Ok(lvl) = second.parse::<u32>() {
            (lvl, String::new())
        } else {
            (0u32, second.to_string())
        }
    } else {
        // parts.len() == 3
        let lvl = parts[1].trim().parse::<u32>().unwrap_or(0);
        let desc = parts[2].trim().to_string();
        (lvl, desc)
    };

    Some(FilterRange::new(start_ip, end_ip, level, description))
}

/// Parses an IPv4 address, supporting zero-padded octets (e.g., "001.000.000.000").
fn parse_ipv4(s: &str) -> Option<Ipv4Addr> {
    let octets: Vec<&str> = s.split('.').collect();
    if octets.len() != 4 {
        return None;
    }

    let mut b = [0u8; 4];
    for i in 0..4 {
        let octet_str = octets[i].trim();
        // Parse directly as u8 (handles standard base 10 even with leading zeroes, standard FromStr fails if leading 0s on some implementations)
        let val = u8::from_str(octet_str).ok()?;
        b[i] = val;
    }

    Some(Ipv4Addr::new(b[0], b[1], b[2], b[3]))
}
