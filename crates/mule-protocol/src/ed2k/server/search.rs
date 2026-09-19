//! Search queries and results for eD2K servers.

use crate::ed2k::error::{Ed2kEncodeError, Ed2kErrorKind};
use crate::ed2k::limits::Ed2kLimits;
use crate::ed2k::opcodes::OP_SEARCHREQUEST;
use crate::ed2k::protocol::PR_ED2K;
use crate::ed2k::tags::{Ed2kTag, TagName, TagValue, TAG_SPECIAL_FILESIZE, TAG_SPECIAL_NAME};
use crate::framing::RawPacket;

/// Request to search files on an eD2K server (`OP_SEARCHREQUEST` = `0x16`).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchQuery {
    /// Text search query string.
    pub term: String,
    /// Optional minimum file size filter in bytes.
    pub min_size: Option<u64>,
    /// Optional maximum file size filter in bytes.
    pub max_size: Option<u64>,
    /// Optional file extension / format filter (e.g. "iso", "zip").
    pub extension: Option<String>,
}

impl SearchQuery {
    /// Creates a simple search query by keyword.
    pub fn new(term: impl Into<String>) -> Self {
        Self {
            term: term.into(),
            min_size: None,
            max_size: None,
            extension: None,
        }
    }

    /// Encodes the search request into a `RawPacket` as a canonical eD2K Query Tree.
    pub fn encode(&self, limits: &Ed2kLimits) -> Result<RawPacket, Ed2kEncodeError> {
        let term_bytes = self.term.as_bytes();
        if term_bytes.len() > limits.max_string_bytes {
            return Err(Ed2kEncodeError::new(
                OP_SEARCHREQUEST,
                "term",
                Ed2kErrorKind::OversizedString {
                    size: term_bytes.len(),
                    limit: limits.max_string_bytes,
                },
            ));
        }

        // 1. Encode term leaf: 0x01 (EDONKEY_SEARCH_NAME) + u16 len + term bytes
        let mut term_leaf = Vec::with_capacity(3 + term_bytes.len());
        term_leaf.push(0x01);
        term_leaf.extend_from_slice(&(term_bytes.len() as u16).to_le_bytes());
        term_leaf.extend_from_slice(term_bytes);

        // 2. Prepare filter leaves
        let mut filter_leaves = Vec::new();

        if let Some(min) = self.min_size {
            let mut leaf = Vec::with_capacity(9);
            leaf.push(0x03);
            let val = (min.min(u32::MAX as u64)) as u32;
            leaf.extend_from_slice(&val.to_le_bytes());
            leaf.push(0x01); // MIN
            leaf.extend_from_slice(&1u16.to_le_bytes());
            leaf.push(TAG_SPECIAL_FILESIZE);
            filter_leaves.push(leaf);
        }

        if let Some(max) = self.max_size {
            let mut leaf = Vec::with_capacity(9);
            leaf.push(0x03);
            let val = (max.min(u32::MAX as u64)) as u32;
            leaf.extend_from_slice(&val.to_le_bytes());
            leaf.push(0x02); // MAX
            leaf.extend_from_slice(&1u16.to_le_bytes());
            leaf.push(TAG_SPECIAL_FILESIZE);
            filter_leaves.push(leaf);
        }

        if let Some(ext) = &self.extension {
            let ext_bytes = ext.as_bytes();
            let mut leaf = Vec::with_capacity(6 + ext_bytes.len());
            leaf.push(0x02); // META
            leaf.extend_from_slice(&(ext_bytes.len() as u16).to_le_bytes());
            leaf.extend_from_slice(ext_bytes);
            leaf.extend_from_slice(&1u16.to_le_bytes());
            leaf.push(0x04); // EDONKEY_STAG_FORMAT
            filter_leaves.push(leaf);
        }

        // 3. Assemble binary tree in preorder traversal:
        let mut current_tree = term_leaf;
        for filter in filter_leaves {
            let mut new_tree = Vec::with_capacity(2 + current_tree.len() + filter.len());
            new_tree.push(0x00); // EDONKEY_SEARCH_BOOL
            new_tree.push(0x00); // EDONKEY_SEARCH_AND
            new_tree.extend_from_slice(&current_tree);
            new_tree.extend_from_slice(&filter);
            current_tree = new_tree;
        }

        let buf = current_tree;
        let payload_len = buf.len() + 1;
        if payload_len > limits.max_frame_bytes {
            return Err(Ed2kEncodeError::new(
                OP_SEARCHREQUEST,
                "search_query",
                Ed2kErrorKind::FrameTooLarge {
                    size: payload_len,
                    limit: limits.max_frame_bytes,
                },
            ));
        }

        Ok(RawPacket::with_vec(PR_ED2K, OP_SEARCHREQUEST, buf))
    }
}

/// A single file result returned by the server search.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResultFile {
    /// 16-byte file hash.
    pub file_hash: [u8; 16],
    /// Client ID of the publisher or 0.
    pub client_id: u32,
    /// Client port of the publisher or 0.
    pub client_port: u16,
    /// File tags (Name, Size, Sources count, etc.).
    pub tags: Vec<Ed2kTag>,
}

impl SearchResultFile {
    /// Helper to extract filename from tags.
    pub fn name(&self) -> Option<String> {
        self.tags.iter().find_map(|t| match (&t.name, &t.value) {
            (TagName::Id(TAG_SPECIAL_NAME), TagValue::String(s)) => Some(s.clone()),
            _ => None,
        })
    }

    /// Helper to extract file size in bytes from tags.
    pub fn size(&self) -> Option<u64> {
        self.tags.iter().find_map(|t| match (&t.name, &t.value) {
            (TagName::Id(TAG_SPECIAL_FILESIZE), TagValue::UInt32(v)) => Some(*v as u64),
            (TagName::Id(TAG_SPECIAL_FILESIZE), TagValue::UInt64(v)) => Some(*v),
            _ => None,
        })
    }

    /// Helper to extract source count from tags.
    pub fn source_count(&self) -> Option<u32> {
        self.tags.iter().find_map(|t| {
            let is_source_tag = match &t.name {
                TagName::Id(0x15) | TagName::Id(0x30) => true,
                TagName::Text(txt) => {
                    let lower = txt.to_lowercase();
                    lower.contains("source")
                        || lower.contains("availability")
                        || lower.contains("complete")
                }
                _ => false,
            };
            if is_source_tag {
                match &t.value {
                    TagValue::UInt8(v) => Some(*v as u32),
                    TagValue::UInt16(v) => Some(*v as u32),
                    TagValue::UInt32(v) => Some(*v),
                    TagValue::UInt64(v) => Some(*v as u32),
                    _ => None,
                }
            } else {
                None
            }
        })
    }
}

/// Search result packet payload (`OP_SEARCH_RESULT` = `0x33`).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// List of matched files.
    pub files: Vec<SearchResultFile>,
}
