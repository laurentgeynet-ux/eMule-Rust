// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Kad keyword index models and utilities.

use super::KadId;
use md4::{Digest, Md4};

/// An indexed keyword-file association in Kad DHT.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KadKeywordEntry {
    /// 32-char lowercase hex of the keyword's MD4 hash.
    pub keyword_hash: String,
    /// 32-char lowercase hex of the file's MD4 hash.
    pub file_hash: String,
    /// File name.
    pub file_name: String,
    /// File size in bytes.
    pub file_size: u64,
    /// Number of sources advertising this file.
    pub source_count: u32,
    /// Timestamp when discovered.
    pub discovered_at_unix_ms: i64,
    /// Expiration timestamp (TTL).
    pub expires_at_unix_ms: i64,
}

/// Normalizes a keyword string (lowercased, trimmed) and computes its 128-bit MD4 KadId.
pub fn hash_kad_keyword(keyword: &str) -> (String, KadId) {
    let normalized = keyword.trim().to_lowercase();
    let mut hasher = Md4::new();
    hasher.update(normalized.as_bytes());
    let digest: [u8; 16] = hasher.finalize().into();
    (normalized, KadId::from_hash_be(&digest))
}

/// Extracts search keywords (length >= 3) from a file name and returns pairs of (word, KadId).
pub fn extract_kad_keywords(file_name: &str) -> Vec<(String, KadId)> {
    let mut results = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Split on typical word delimiters
    let words = file_name.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '.' | '_'
                    | '-'
                    | '['
                    | ']'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '+'
                    | ','
                    | ';'
                    | '!'
                    | '@'
                    | '#'
                    | '$'
                    | '%'
                    | '^'
                    | '&'
                    | '*'
                    | '='
                    | '~'
                    | '`'
                    | '|'
                    | '/'
                    | '\\'
            )
    });

    for word in words {
        let trimmed = word.trim();
        if trimmed.len() >= 3 {
            let (normalized, kad_id) = hash_kad_keyword(trimmed);
            if seen.insert(normalized.clone()) {
                results.push((normalized, kad_id));
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_kad_keywords() {
        let name = "Ubuntu-22.04.3-live-server-amd64.iso";
        let keywords = extract_kad_keywords(name);
        let words: Vec<&str> = keywords.iter().map(|(w, _)| w.as_str()).collect();

        assert!(words.contains(&"ubuntu"));
        assert!(words.contains(&"live"));
        assert!(words.contains(&"server"));
        assert!(words.contains(&"amd64"));
        assert!(words.contains(&"iso"));
        // Numbers like "22" or "04" (< 3 chars) are excluded
        assert!(!words.contains(&"22"));
    }

    #[test]
    fn test_hash_kad_keyword() {
        let (norm, id) = hash_kad_keyword("Linux");
        assert_eq!(norm, "linux");
        let (norm2, id2) = hash_kad_keyword("  LINUX  ");
        assert_eq!(norm2, "linux");
        assert_eq!(id, id2);
    }
}
