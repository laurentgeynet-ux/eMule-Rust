use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::aich::hash_tree::AichHash;
use crate::ed2k::hash::Ed2kHash;
use crate::met::error::MetError;
use crate::met::reader::CountingReader;
use crate::met::tags::*;

pub const PARTFILE_VERSION: u8 = 0xe0;
pub const PARTFILE_SPLITTEDVERSION: u8 = 0xe1;
pub const PARTFILE_VERSION_LARGEFILE: u8 = 0xe2;

pub const MAX_PART_COUNT: usize = 65_536;
pub const MAX_TAG_COUNT: usize = 10_000;

/// Parsed eMule `.part.met` file tracking a partial download.
#[derive(Debug, Clone, PartialEq)]
pub struct PartMetFile {
    pub version: u8,
    pub mod_date: u32,
    pub file_hash: Ed2kHash,
    pub part_hashes: Vec<[u8; 16]>,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
    pub transferred: u64,
    pub aich_hash: Option<AichHash>,
    pub gaps: Vec<(u64, u64)>,
    pub tags: Vec<Tag>,
}

impl PartMetFile {
    /// Reads and validates a `.part.met` file from disk.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, MetError> {
        let mut file = File::open(path).map_err(|e| MetError::Io {
            offset: 0,
            message: e.to_string(),
        })?;
        Self::read_from(&mut file)
    }

    /// Deserializes a `.part.met` structure from any reader with byte-level offset tracking.
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, MetError> {
        let mut cr = CountingReader::new(reader);

        let header_offset = cr.offset();
        let version = cr.read_u8("version header")?;
        if version != PARTFILE_VERSION
            && version != PARTFILE_SPLITTEDVERSION
            && version != PARTFILE_VERSION_LARGEFILE
        {
            return Err(MetError::UnsupportedHeader {
                offset: header_offset,
                value: version,
            });
        }

        let mod_date = cr.read_u32_le("mod date")?;

        let hash_bytes = cr.read_exact_bytes(16, "file hash")?;
        let mut hash_buf = [0u8; 16];
        hash_buf.copy_from_slice(&hash_bytes);
        let file_hash = Ed2kHash(hash_buf);

        let count_offset = cr.offset();
        let hash_count = cr.read_u16_le("part hash count")? as usize;
        if hash_count > MAX_PART_COUNT {
            return Err(MetError::InvalidLength {
                offset: count_offset,
                declared: hash_count as u64,
                maximum: MAX_PART_COUNT as u64,
                context: "part hash count",
            });
        }

        let mut part_hashes = Vec::with_capacity(hash_count);
        for _ in 0..hash_count {
            let part_bytes = cr.read_exact_bytes(16, "part hash")?;
            let mut part_buf = [0u8; 16];
            part_buf.copy_from_slice(&part_bytes);
            part_hashes.push(part_buf);
        }

        let tag_count_offset = cr.offset();
        let tag_count = cr.read_u32_le("tag count")? as usize;
        if tag_count > MAX_TAG_COUNT {
            return Err(MetError::InvalidLength {
                offset: tag_count_offset,
                declared: tag_count as u64,
                maximum: MAX_TAG_COUNT as u64,
                context: "tag count",
            });
        }

        let mut tags = Vec::with_capacity(tag_count);
        let mut file_name = None;
        let mut file_size: Option<u64> = None;
        let mut file_size_hi = 0u64;
        let mut transferred = 0u64;
        let mut aich_hash = None;
        let mut gap_starts = Vec::new();
        let mut gap_ends = Vec::new();

        for _ in 0..tag_count {
            let tag = cr.read_tag()?;

            match tag.name_id {
                FT_FILENAME => {
                    if let Some(name) = tag.as_str() {
                        file_name = Some(name.to_string());
                    }
                }
                FT_FILESIZE => {
                    if let Some(sz) = tag.as_u64() {
                        file_size = Some(sz);
                    }
                }
                FT_FILESIZE_HI => {
                    if let Some(hi) = tag.as_u64() {
                        file_size_hi = hi << 32;
                    }
                }
                FT_TRANSFERRED => {
                    if let Some(tr) = tag.as_u64() {
                        transferred = tr;
                    }
                }
                FT_AICH_HASH => {
                    if let Some(s) = tag.as_str() {
                        if let Ok(h) = AichHash::from_base32(s) {
                            aich_hash = Some(h);
                        }
                    }
                }
                FT_GAPSTART => {
                    if let Some(start) = tag.as_u64() {
                        gap_starts.push(start);
                    }
                }
                FT_GAPEND => {
                    if let Some(end) = tag.as_u64() {
                        gap_ends.push(end);
                    }
                }
                _ => {}
            }

            tags.push(tag);
        }

        if let Some(sz) = file_size.as_mut() {
            *sz += file_size_hi;
        }

        let mut gaps = Vec::new();
        for (start, end) in gap_starts.into_iter().zip(gap_ends) {
            gaps.push((start, end));
        }

        Ok(Self {
            version,
            mod_date,
            file_hash,
            part_hashes,
            file_name,
            file_size,
            transferred,
            aich_hash,
            gaps,
            tags,
        })
    }

    /// Returns the total expected number of 9,728,000-byte parts for this file.
    pub fn part_count(&self) -> usize {
        match self.file_size {
            Some(0) | None => 0,
            Some(sz) => {
                let part_sz = crate::ed2k::hash::ED2K_PART_SIZE as u64;
                sz.div_ceil(part_sz) as usize
            }
        }
    }

    /// Computes the completed (acquired) byte ranges by inverting the missing gaps.
    ///
    /// Validates that all gaps satisfy `0 <= start <= end <= total_size`.
    /// Returns sorted, disjoint half-open byte intervals `[start, end)`.
    pub fn compute_completed_ranges(&self) -> Result<Vec<(u64, u64)>, MetError> {
        let total_size = match self.file_size {
            Some(sz) => sz,
            None => return Ok(Vec::new()),
        };

        if total_size == 0 {
            return Ok(Vec::new());
        }

        // If no gaps recorded:
        // When eMule has not started or downloaded anything, it either writes a gap for the full file
        // or has transferred == 0. If transferred == 0 and gaps is empty, nothing has been acquired.
        // If transferred > 0 and gaps is empty, the entire file is complete.
        if self.gaps.is_empty() {
            if self.transferred == 0 {
                return Ok(Vec::new());
            } else {
                return Ok(vec![(0, total_size)]);
            }
        }

        // Validate gaps
        let mut normalized_gaps = Vec::with_capacity(self.gaps.len());
        for &(start, end) in &self.gaps {
            if start > end || end > total_size {
                return Err(MetError::InconsistentMetadata {
                    field: "gaps",
                    reason: format!(
                        "gap interval [{}, {}] exceeds file total size {}",
                        start, end, total_size
                    ),
                });
            }
            if start < end {
                normalized_gaps.push((start, end));
            }
        }

        // Sort and merge overlapping/contiguous gaps
        normalized_gaps.sort_by_key(|g| g.0);
        let mut merged_gaps: Vec<(u64, u64)> = Vec::new();
        for (s, e) in normalized_gaps {
            if let Some(last) = merged_gaps.last_mut() {
                if s <= last.1 {
                    last.1 = last.1.max(e);
                    continue;
                }
            }
            merged_gaps.push((s, e));
        }

        // Invert gaps on [0, total_size)
        let mut completed = Vec::new();
        let mut cursor = 0u64;

        for (g_start, g_end) in merged_gaps {
            if g_start > cursor {
                completed.push((cursor, g_start));
            }
            cursor = cursor.max(g_end);
        }

        if cursor < total_size {
            completed.push((cursor, total_size));
        }

        Ok(completed)
    }
}

