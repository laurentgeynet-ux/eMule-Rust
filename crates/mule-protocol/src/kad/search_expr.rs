// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Kad restrictive search-expression trees.
//!
//! Implements the wire format and evaluation semantics of eMule
//! `CreateSearchExpressionTree` (`KademliaUDPListener.cpp:883`) and
//! `CKeyEntry::StartSearchTermsMatch` (`Entry.cpp:224`):
//!
//! - `0x00` + bool op (`0x00` AND / `0x01` OR / `0x02` NOT) + two child
//!   expressions. `NOT` means *left and not right*.
//! - `0x01` + UTF-8 string — filename term: the string is lowercased and
//!   pre-tokenized (quoted parts preserved); every token must be a substring
//!   of the lowercased file name (`"a b"` behaves as `a AND b`).
//! - `0x02` + UTF-8 value + `u16` tag-name len + tag-name bytes — case
//!   insensitive equality against a stored string tag; `TAG_FILEFORMAT`
//!   (`0x04`) compares against the file-name extension instead.
//! - `0x03`/`0x08` + `u32`/`u64` LE value + `u8` comparison op + `u16`
//!   tag-name len + tag-name bytes — integer relation against a stored int
//!   tag; `TAG_FILESIZE` (`0x02`) is a virtual tag resolved to the entry's
//!   file size (eMule `CEntry::GetIntTagValue` with `bIncludeVirtualTags`).

use crate::ed2k::tags::{Ed2kTag, TagName, TagValue};
use crate::ed2k::{Ed2kErrorKind, Ed2kParseError};
use crate::kad::types::KadKeywordEntryWire;

/// Maximum expression tree depth — eMule rejects trees at `iLevel >= 24`
/// (`KademliaUDPListener.cpp:887`).
pub const MAX_SEARCH_EXPR_DEPTH: usize = 24;

/// Tag id compared against the file name extension without being stored
/// (eMule `TAG_FILEFORMAT` special case, `Entry.cpp:260`).
const TAG_ID_FILEFORMAT: u8 = 0x04;

/// Tag id resolved to the entry's `file_size` field
/// (eMule `TAG_FILESIZE` virtual tag, `Entry.cpp:101`).
const TAG_ID_FILESIZE: u8 = 0x02;

/// Delimiter set splitting unquoted filename terms
/// (eMule `INV_KAD_KEYWORD_CHARS`, `SearchManager.cpp:54`).
const INV_KAD_KEYWORD_CHARS: [char; 20] = [
    ' ', '(', ')', '[', ']', '{', '}', '<', '>', ',', '.', '_', '-', '!', '?', ':', ';', '\\', '/',
    '"',
];

/// Expression leaf node kinds parsed from the wire.
#[derive(Debug, Clone, PartialEq)]
pub enum KadSearchTerm {
    /// Filename term — every pre-tokenized word must be a substring of the
    /// lowercased file name (eMule `SSearchTerm::String`).
    Str(Vec<String>),
    /// Metadata string equality (eMule `SSearchTerm::MetaTag`).
    MetaStr {
        /// Raw tag name bytes (1 byte = tag id, longer = name string).
        name: Vec<u8>,
        /// Lowercased expected value.
        value: String,
    },
    /// Integer metadata relation (eMule `OpEqual`…`OpNotEqual` over
    /// `CKadTagUInt64`; float expression terms are never produced by
    /// `CreateSearchExpressionTree`, so only int terms exist on the wire).
    Num {
        /// Raw tag name bytes (1 byte = tag id, longer = name string).
        name: Vec<u8>,
        /// Comparison operator (wire codes 0x00–0x05).
        op: KadSearchNumOp,
        /// Right-hand comparison value.
        value: u64,
    },
}

/// Integer comparison operators (`mmop` wire codes, `Entry.cpp:284`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KadSearchNumOp {
    /// `0x00` — equal.
    Equal,
    /// `0x01` — strictly greater.
    Greater,
    /// `0x02` — strictly less.
    Less,
    /// `0x03` — greater or equal.
    GreaterEqual,
    /// `0x04` — less or equal.
    LessEqual,
    /// `0x05` — not equal.
    NotEqual,
}

/// Parsed restrictive search-expression tree.
#[derive(Debug, Clone, PartialEq)]
pub enum KadSearchExpression {
    /// `AND` — both children must match.
    And(Box<KadSearchExpression>, Box<KadSearchExpression>),
    /// `OR` — at least one child must match.
    Or(Box<KadSearchExpression>, Box<KadSearchExpression>),
    /// `NOT` — left child must match and right child must not match.
    Not(Box<KadSearchExpression>, Box<KadSearchExpression>),
    /// Leaf term.
    Term(KadSearchTerm),
}

impl KadSearchExpression {
    /// Parses a trailing search-expression tree from `data`
    /// (`CreateSearchExpressionTree`). Trailing bytes after a complete tree
    /// are ignored, like eMule reading from a stream. Returns an
    /// `Ed2kParseError` on malformed or truncated input, unknown operators,
    /// or trees deeper than `MAX_SEARCH_EXPR_DEPTH` — the caller must drop
    /// the datagram, like eMule's `try/catch` in
    /// `Process_KADEMLIA2_SEARCH_KEY_REQ`.
    pub fn parse(protocol: u8, opcode: u8, data: &[u8]) -> Result<Self, Ed2kParseError> {
        let mut cur = Cursor {
            data,
            pos: 0,
            protocol,
            opcode,
        };
        parse_term(&mut cur, 0)
    }

    /// Evaluates the expression against a stored keyword entry
    /// (`CKeyEntry::StartSearchTermsMatch`). The entry's lowercased file name
    /// is computed once by the caller via `KadKeywordSearchView`.
    pub fn matches(&self, view: &KadKeywordSearchView<'_>) -> bool {
        match self {
            Self::And(l, r) => l.matches(view) && r.matches(view),
            Self::Or(l, r) => l.matches(view) || r.matches(view),
            Self::Not(l, r) => l.matches(view) && !r.matches(view),
            Self::Term(t) => t.matches(view),
        }
    }
}

/// Evaluation context for one stored keyword entry: lowercased file name,
/// file size and the published tag list (eMule `CKeyEntry` fields
/// `m_strSearchTermCacheCommonFileNameLowerCase`, `m_uSize`, `m_listTag`).
#[derive(Debug)]
pub struct KadKeywordSearchView<'a> {
    /// File name lowercased once per evaluation (eMule caches it on the
    /// entry for the duration of the match).
    file_name_lower: String,
    /// Entry file size — virtual `TAG_FILESIZE` value.
    file_size: u64,
    /// Published metadata tag list.
    tags: &'a [Ed2kTag],
}

impl<'a> KadKeywordSearchView<'a> {
    /// Builds the evaluation view of a stored keyword entry.
    pub fn new(entry: &'a KadKeywordEntryWire) -> Self {
        Self {
            file_name_lower: entry.file_name.to_lowercase(),
            file_size: entry.file_size,
            tags: &entry.tags,
        }
    }
}

impl KadSearchTerm {
    fn matches(&self, view: &KadKeywordSearchView<'_>) -> bool {
        match self {
            Self::Str(tokens) => {
                if tokens.is_empty() {
                    return false;
                }
                tokens
                    .iter()
                    .all(|t| view.file_name_lower.contains(t.as_str()))
            }
            Self::MetaStr { name, value } => {
                if name.as_slice() == [TAG_ID_FILEFORMAT] {
                    // eMule: `fileformat` is part of the file name — compare
                    // the extension (after the last '.') case-insensitively.
                    if let Some(ext) = view.file_name_lower.rsplit('.').next() {
                        if view.file_name_lower.contains('.') && ext == value {
                            return true;
                        }
                    }
                }
                view.tags.iter().any(|tag| {
                    tag_name_matches(&tag.name, name)
                        && matches!(&tag.value, TagValue::String(s) if s.to_lowercase() == *value)
                })
            }
            Self::Num { name, op, value } => {
                // eMule `GetIntTagValue`: first matching int tag by exact
                // name; `TAG_FILESIZE` resolves to the entry size when no
                // stored tag matches.
                let entry_value = view
                    .tags
                    .iter()
                    .find_map(|tag| match (&tag.name, &tag.value) {
                        (n, v) if tag_name_matches(n, name) => tag_int_value(v),
                        _ => None,
                    })
                    .or_else(|| {
                        (name.as_slice() == [TAG_ID_FILESIZE]).then_some(view.file_size)
                    });
                let Some(entry_value) = entry_value else {
                    return false;
                };
                match op {
                    KadSearchNumOp::Equal => entry_value == *value,
                    KadSearchNumOp::Greater => entry_value > *value,
                    KadSearchNumOp::Less => entry_value < *value,
                    KadSearchNumOp::GreaterEqual => entry_value >= *value,
                    KadSearchNumOp::LessEqual => entry_value <= *value,
                    KadSearchNumOp::NotEqual => entry_value != *value,
                }
            }
        }
    }
}

/// eMule `pTag->m_name.Compare(name) == 0` — exact byte comparison: a 1-byte
/// expression name matches `TagName::Id`, longer names match `TagName::Text`
/// byte for byte.
fn tag_name_matches(tag_name: &TagName, expr_name: &[u8]) -> bool {
    match tag_name {
        TagName::Id(id) => expr_name == [*id].as_slice(),
        TagName::Text(s) => expr_name == s.as_bytes(),
    }
}

/// Integer value of a stored tag (eMule `CKadTag::IsInt()`).
fn tag_int_value(value: &TagValue) -> Option<u64> {
    match value {
        TagValue::UInt64(v) => Some(*v),
        TagValue::UInt32(v) => Some(u64::from(*v)),
        TagValue::UInt16(v) => Some(u64::from(*v)),
        TagValue::UInt8(v) => Some(u64::from(*v)),
        _ => None,
    }
}

/// Bounded cursor over the expression byte stream.
struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    protocol: u8,
    opcode: u8,
}

impl<'a> Cursor<'a> {
    fn err(&self, field: &'static str, kind: Ed2kErrorKind) -> Ed2kParseError {
        Ed2kParseError::new(self.protocol, Some(self.opcode), self.pos, field, kind)
    }

    fn eof(&self, field: &'static str, needed: usize) -> Ed2kParseError {
        self.err(
            field,
            Ed2kErrorKind::UnexpectedEof {
                needed,
                available: self.data.len().saturating_sub(self.pos),
            },
        )
    }

    fn u8(&mut self, field: &'static str) -> Result<u8, Ed2kParseError> {
        if self.pos >= self.data.len() {
            return Err(self.eof(field, 1));
        }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn u16(&mut self, field: &'static str) -> Result<u16, Ed2kParseError> {
        if self.pos + 2 > self.data.len() {
            return Err(self.eof(field, 2));
        }
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn take(&mut self, len: usize, field: &'static str) -> Result<&'a [u8], Ed2kParseError> {
        if self.pos + len > self.data.len() {
            return Err(self.eof(field, len));
        }
        let s = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(s)
    }

    /// `CFileDataIO::ReadStringUTF8` — `u16` raw length + bytes, decoded as
    /// UTF-8 with lossy fallback (eMule falls back to the local codepage).
    fn utf8_string(&mut self, field: &'static str) -> Result<String, Ed2kParseError> {
        let len = self.u16(field)? as usize;
        let bytes = self.take(len, field)?;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

fn parse_term(cur: &mut Cursor<'_>, level: usize) -> Result<KadSearchExpression, Ed2kParseError> {
    // eMule `iLevel >= 24` -> NULL (the caller's level 0 is the root).
    if level >= MAX_SEARCH_EXPR_DEPTH {
        return Err(cur.err(
            "search_expr_depth",
            Ed2kErrorKind::MalformedField {
                message: "search expression tree exceeds depth limit".to_string(),
            },
        ));
    }
    let level = level + 1;
    let op = cur.u8("search_expr_op")?;
    match op {
        0x00 => {
            let bool_op = cur.u8("search_expr_bool_op")?;
            let make = match bool_op {
                0x00 => KadSearchExpression::And,
                0x01 => KadSearchExpression::Or,
                0x02 => KadSearchExpression::Not,
                _ => {
                    return Err(cur.err(
                        "search_expr_bool_op",
                        Ed2kErrorKind::MalformedField {
                            message: "unknown boolean search operator".to_string(),
                        },
                    ))
                }
            };
            let left = parse_term(cur, level)?;
            let right = parse_term(cur, level)?;
            Ok(make(Box::new(left), Box::new(right)))
        }
        0x01 => {
            let raw = cur.utf8_string("search_expr_string")?;
            // eMule `KadTagStrMakeLower` + `TokenizeOptQuotedSearchTerm`.
            let tokens = tokenize_search_term(&raw.to_lowercase());
            Ok(KadSearchExpression::Term(KadSearchTerm::Str(tokens)))
        }
        0x02 => {
            let value = cur.utf8_string("search_expr_meta_value")?.to_lowercase();
            let name = read_tag_name(cur)?;
            Ok(KadSearchExpression::Term(KadSearchTerm::MetaStr {
                name,
                value,
            }))
        }
        0x03 | 0x08 => {
            let width = if op == 0x03 { 4 } else { 8 };
            let bytes = cur.take(width, "search_expr_num_value")?;
            let mut buf = [0u8; 8];
            buf[..width].copy_from_slice(bytes);
            let value = u64::from_le_bytes(buf);
            let mmop = cur.u8("search_expr_num_op")?;
            let op = match mmop {
                0x00 => KadSearchNumOp::Equal,
                0x01 => KadSearchNumOp::Greater,
                0x02 => KadSearchNumOp::Less,
                0x03 => KadSearchNumOp::GreaterEqual,
                0x04 => KadSearchNumOp::LessEqual,
                0x05 => KadSearchNumOp::NotEqual,
                _ => {
                    return Err(cur.err(
                        "search_expr_num_op",
                        Ed2kErrorKind::MalformedField {
                            message: "unknown integer search operator".to_string(),
                        },
                    ))
                }
            };
            let name = read_tag_name(cur)?;
            Ok(KadSearchExpression::Term(KadSearchTerm::Num {
                name,
                op,
                value,
            }))
        }
        _ => Err(cur.err(
            "search_expr_op",
            Ed2kErrorKind::MalformedField {
                message: "unknown search expression opcode".to_string(),
            },
        )),
    }
}

/// `u16` tag-name length + raw name bytes (not UTF-8 — eMule reads them into
/// a `CStringA`).
fn read_tag_name(cur: &mut Cursor<'_>) -> Result<Vec<u8>, Ed2kParseError> {
    let len = cur.u16("search_expr_tag_name_len")? as usize;
    Ok(cur.take(len, "search_expr_tag_name")?.to_vec())
}

/// eMule `TokenizeOptQuotedSearchTerm` (`KademliaUDPListener.cpp:848`):
/// splits on `INV_KAD_KEYWORD_CHARS`; a `"` starts a quoted section taken
/// verbatim until the next `"` (an unterminated quote is skipped); empty
/// tokens are dropped. The input is already lowercased.
fn tokenize_search_term(input: &str) -> Vec<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '"' {
            // Quoted section: take until the next '"'. If none, the start
            // quote is just skipped.
            i += 1;
            let start = i;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            if i < chars.len() {
                if i > start {
                    tokens.push(chars[start..i].iter().collect());
                }
                i += 1;
            }
        } else {
            let start = i;
            while i < chars.len() && !INV_KAD_KEYWORD_CHARS.contains(&chars[i]) {
                i += 1;
            }
            if i > start {
                tokens.push(chars[start..i].iter().collect());
                if i == chars.len() {
                    break;
                }
                if chars[i] == '"' {
                    continue;
                }
            }
            i += 1;
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ed2k::tags::{TagName, TagValue};

    const PROTO: u8 = 0xE4;
    const OP: u8 = 0x33;

    fn str_leaf(s: &str) -> Vec<u8> {
        let mut v = vec![0x01u8];
        v.extend_from_slice(&(s.len() as u16).to_le_bytes());
        v.extend_from_slice(s.as_bytes());
        v
    }

    fn meta_str_leaf(name: &[u8], value: &str) -> Vec<u8> {
        let mut v = vec![0x02u8];
        v.extend_from_slice(&(value.len() as u16).to_le_bytes());
        v.extend_from_slice(value.as_bytes());
        v.extend_from_slice(&(name.len() as u16).to_le_bytes());
        v.extend_from_slice(name);
        v
    }

    fn num_leaf(name: &[u8], op: u8, value: u64, wide: bool) -> Vec<u8> {
        let mut v = vec![if wide { 0x08u8 } else { 0x03u8 }];
        if wide {
            v.extend_from_slice(&value.to_le_bytes());
        } else {
            v.extend_from_slice(&(value as u32).to_le_bytes());
        }
        v.push(op);
        v.extend_from_slice(&(name.len() as u16).to_le_bytes());
        v.extend_from_slice(name);
        v
    }

    fn bool_node(op: u8, l: &[u8], r: &[u8]) -> Vec<u8> {
        let mut v = vec![0x00u8, op];
        v.extend_from_slice(l);
        v.extend_from_slice(r);
        v
    }

    fn entry(name: &str, size: u64, sources: u32, tags: Vec<Ed2kTag>) -> KadKeywordEntryWire {
        KadKeywordEntryWire {
            file_hash: [1u8; 16],
            file_name: name.to_string(),
            file_size: size,
            source_count: sources,
            tags,
        }
    }

    fn eval(expr: &KadSearchExpression, entry: &KadKeywordEntryWire) -> bool {
        expr.matches(&KadKeywordSearchView::new(entry))
    }

    #[test]
    fn str_term_substring_case_insensitive() {
        let e = KadSearchExpression::parse(PROTO, OP, &str_leaf("Movie")).unwrap();
        assert!(eval(&e, &entry("A.MOVIE.2024.x264.mkv", 1, 1, vec![])));
        assert!(!eval(&e, &entry("other.bin", 1, 1, vec![])));
    }

    #[test]
    fn str_term_multiple_words_are_anded() {
        // "a b" behaves as a AND b (substring of the lowercased name).
        let e = KadSearchExpression::parse(PROTO, OP, &str_leaf("movie x264")).unwrap();
        assert!(eval(&e, &entry("a.movie.2024.x264.mkv", 1, 1, vec![])));
        assert!(!eval(&e, &entry("a.movie.2024.mkv", 1, 1, vec![])));
    }

    #[test]
    fn str_term_quoted_part_kept_verbatim() {
        // Quoted "movie x" stays one token; unquoted b is a second token.
        let e = KadSearchExpression::parse(PROTO, OP, &str_leaf("\"movie x\" b")).unwrap();
        assert!(eval(&e, &entry("the movie x b.mkv", 1, 1, vec![])));
        // eMule matches on substrings: "movie x" is a prefix of "movie xb"
        // and "b" appears in "xb" — both tokens match here.
        assert!(eval(&e, &entry("movie xb.mkv", 1, 1, vec![])));
        // No "b" token at all -> no match.
        assert!(!eval(&e, &entry("movie x cd.mkv", 1, 1, vec![])));
    }

    #[test]
    fn meta_str_matches_stored_tag_case_insensitive() {
        let e =
            KadSearchExpression::parse(PROTO, OP, &meta_str_leaf(b"codec", "X264")).unwrap();
        let tag = Ed2kTag {
            name: TagName::Text("codec".to_string()),
            value: TagValue::String("x264".to_string()),
        };
        assert!(eval(&e, &entry("f.mkv", 1, 1, vec![tag])));
        assert!(!eval(&e, &entry("f.mkv", 1, 1, vec![])));
    }

    #[test]
    fn meta_str_fileformat_matches_extension() {
        let e =
            KadSearchExpression::parse(PROTO, OP, &meta_str_leaf(&[0x04], "avi")).unwrap();
        assert!(eval(&e, &entry("clip.AVI", 1, 1, vec![])));
        assert!(!eval(&e, &entry("clip.mkv", 1, 1, vec![])));
        assert!(!eval(&e, &entry("noext", 1, 1, vec![])));
    }

    #[test]
    fn num_filesize_virtual_tag() {
        let e = KadSearchExpression::parse(PROTO, OP, &num_leaf(&[0x02], 0x03, 700, false)).unwrap();
        assert!(eval(&e, &entry("f.bin", 700, 1, vec![])));
        assert!(eval(&e, &entry("f.bin", 800, 1, vec![])));
        assert!(!eval(&e, &entry("f.bin", 699, 1, vec![])));
    }

    #[test]
    fn num_sources_stored_tag() {
        let e = KadSearchExpression::parse(PROTO, OP, &num_leaf(&[0x15], 0x01, 5, false)).unwrap();
        let tag = Ed2kTag {
            name: TagName::Id(0x15),
            value: TagValue::UInt32(6),
        };
        assert!(eval(&e, &entry("f.bin", 1, 1, vec![tag])));
        let tag_low = Ed2kTag {
            name: TagName::Id(0x15),
            value: TagValue::UInt32(5),
        };
        assert!(!eval(&e, &entry("f.bin", 1, 1, vec![tag_low])));
    }

    #[test]
    fn num_operators_all() {
        let tag = Ed2kTag {
            name: TagName::Text("bitrate".to_string()),
            value: TagValue::UInt32(100),
        };
        for (wire_op, expect) in [
            (0x00u8, true),  // = 100
            (0x01, false),   // > 100
            (0x02, false),   // < 100
            (0x03, true),    // >= 100
            (0x04, true),    // <= 100
            (0x05, false),   // <> 100
        ] {
            let e = KadSearchExpression::parse(
                PROTO,
                OP,
                &num_leaf(b"bitrate", wire_op, 100, false),
            )
            .unwrap();
            let t = Ed2kTag {
                name: tag.name.clone(),
                value: tag.value.clone(),
            };
            assert_eq!(eval(&e, &entry("f.bin", 1, 1, vec![t])), expect, "op {wire_op}");
        }
    }

    #[test]
    fn num_64bit_value() {
        let e =
            KadSearchExpression::parse(PROTO, OP, &num_leaf(&[0x02], 0x01, u32::MAX as u64, true))
                .unwrap();
        assert!(eval(&e, &entry("big.iso", u64::from(u32::MAX) + 1, 1, vec![])));
        assert!(!eval(&e, &entry("small.bin", 10, 1, vec![])));
    }

    #[test]
    fn boolean_and_or_not() {
        let a = str_leaf("movie");
        let b = str_leaf("x264");
        let c = str_leaf("cam");

        let and = KadSearchExpression::parse(PROTO, OP, &bool_node(0x00, &a, &b)).unwrap();
        let or = KadSearchExpression::parse(PROTO, OP, &bool_node(0x01, &b, &c)).unwrap();
        let not = KadSearchExpression::parse(PROTO, OP, &bool_node(0x02, &a, &c)).unwrap();

        let hit = entry("a.movie.x264.mkv", 1, 1, vec![]);
        let cam = entry("a.movie.cam.mkv", 1, 1, vec![]);
        assert!(eval(&and, &hit));
        assert!(!eval(&and, &cam));
        assert!(eval(&or, &hit));
        assert!(eval(&or, &cam));
        assert!(eval(&not, &hit));
        assert!(!eval(&not, &cam));
    }

    #[test]
    fn nested_expression() {
        // ("movie" AND "x264") OR ("doc" AND "pdf")
        let l = bool_node(0x00, &str_leaf("movie"), &str_leaf("x264"));
        let r = bool_node(0x00, &str_leaf("doc"), &str_leaf("pdf"));
        let e = KadSearchExpression::parse(PROTO, OP, &bool_node(0x01, &l, &r)).unwrap();
        assert!(eval(&e, &entry("a.movie.x264.mkv", 1, 1, vec![])));
        assert!(eval(&e, &entry("my doc file.pdf", 1, 1, vec![])));
        assert!(!eval(&e, &entry("a.movie.avi", 1, 1, vec![])));
    }

    #[test]
    fn malformed_inputs_rejected() {
        // Empty.
        assert!(KadSearchExpression::parse(PROTO, OP, &[]).is_err());
        // Unknown node op.
        assert!(KadSearchExpression::parse(PROTO, OP, &[0x7F]).is_err());
        // Unknown bool op.
        assert!(KadSearchExpression::parse(PROTO, OP, &[0x00, 0x09, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00]).is_err());
        // Unknown num op.
        let mut bad = vec![0x03u8, 1, 0, 0, 0, 0x09];
        bad.extend_from_slice(&1u16.to_le_bytes());
        bad.push(0x02);
        assert!(KadSearchExpression::parse(PROTO, OP, &bad).is_err());
        // Truncated string.
        assert!(KadSearchExpression::parse(PROTO, OP, &[0x01, 0x05, 0x00, b'a']).is_err());
        // Truncated tag name.
        assert!(KadSearchExpression::parse(PROTO, OP, &[0x02, 0x01, 0x00, b'x', 0x04, 0x00, 0x04]).is_err());
    }

    #[test]
    fn depth_limit_rejected() {
        // Depth 24+ nested AND nodes -> error (eMule iLevel >= 24).
        let mut expr = str_leaf("x");
        for _ in 0..MAX_SEARCH_EXPR_DEPTH + 2 {
            expr = bool_node(0x00, &expr.clone(), &str_leaf("y"));
        }
        assert!(KadSearchExpression::parse(PROTO, OP, &expr).is_err());
        // Depth exactly 23 is accepted.
        let mut ok = str_leaf("x");
        for _ in 0..22 {
            ok = bool_node(0x00, &ok.clone(), &str_leaf("y"));
        }
        assert!(KadSearchExpression::parse(PROTO, OP, &ok).is_ok());
    }

    #[test]
    fn trailing_bytes_ignored() {
        let mut v = str_leaf("x");
        v.extend_from_slice(&[0xAA, 0xBB]);
        assert!(KadSearchExpression::parse(PROTO, OP, &v).is_ok());
    }

    #[test]
    fn empty_token_list_does_not_match() {
        // A string leaf that yields no tokens (delimiters only) never matches
        // (eMule returns false on empty m_pastr).
        let e = KadSearchExpression::parse(PROTO, OP, &str_leaf("...")).unwrap();
        assert!(!eval(&e, &entry("...file.mkv", 1, 1, vec![])));
    }
}
