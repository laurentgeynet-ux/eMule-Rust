use std::fmt;
use url::Url;

use crate::ed2k::hash::Ed2kHash;
use crate::ed2k::source::PeerSource;
use crate::error::FormatError;

const MAX_LINK_LENGTH: usize = 8192;
const MAX_SOURCES_COUNT: usize = 256;
const MAX_PART_HASHES_COUNT: usize = 65536;

/// Parsed eD2K file link (`ed2k://|file|...`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ed2kFileLink {
    pub file_name: String,
    pub size: u64,
    pub ed2k_hash: Ed2kHash,
    pub aich_root: Option<[u8; 20]>,
    pub part_hashes: Vec<[u8; 16]>,
    pub web_sources: Vec<Url>,
    pub peer_sources: Vec<PeerSource>,
    pub unknown_fields: Vec<String>,
}

impl Ed2kFileLink {
    /// Parses an eD2K link string with defensive limits.
    pub fn parse(input: &str) -> Result<Self, FormatError> {
        let trimmed = input.trim();
        if trimmed.len() > MAX_LINK_LENGTH {
            return Err(FormatError::InputTooLarge(trimmed.len()));
        }

        // Must start with ed2k://
        let lower = trimmed.to_ascii_lowercase();
        if !lower.starts_with("ed2k://") {
            return Err(FormatError::InvalidLink(
                "Missing 'ed2k://' scheme prefix".to_string(),
            ));
        }

        let content = &trimmed[7..]; // Strip scheme
        if !content.starts_with('|') {
            return Err(FormatError::InvalidLink(
                "Expected '|' separator after scheme".to_string(),
            ));
        }

        // Split into main sections separated by "/|"
        // e.g. "|file|name|size|hash|h=...|/|sources,ip:port|/"
        let sections: Vec<&str> = content.split("/|").collect();
        let main_section = sections[0];

        // Process main file section
        let tokens: Vec<&str> = main_section.split('|').collect();
        // tokens[0] is empty because content started with '|'
        if tokens.len() < 5 {
            return Err(FormatError::InvalidLink(
                "Insufficient fields in file section".to_string(),
            ));
        }

        if tokens[1] != "file" {
            return Err(FormatError::InvalidLink(format!(
                "Expected link type 'file', found '{}'",
                tokens[1]
            )));
        }

        // Decode percent-encoded filename
        let raw_name = tokens[2];
        let file_name = percent_encoding::percent_decode_str(raw_name)
            .decode_utf8()
            .map_err(|e| FormatError::InvalidLink(format!("Invalid UTF-8 filename: {e}")))?
            .to_string();

        if file_name.is_empty() {
            return Err(FormatError::InvalidLink("File name is empty".to_string()));
        }

        let size = tokens[3]
            .parse::<u64>()
            .map_err(|_| FormatError::InvalidLink(format!("Invalid file size '{}'", tokens[3])))?;

        let ed2k_hash = Ed2kHash::from_hex(tokens[4])?;

        let mut aich_root = None;
        let mut part_hashes = Vec::new();
        let mut web_sources = Vec::new();
        let mut unknown_fields = Vec::new();

        // Optional parameters before trailing "/"
        for &token in &tokens[5..] {
            let item = token.trim_end_matches('/');
            if item.is_empty() {
                continue;
            }

            if let Some(aich_str) = item.strip_prefix("h=") {
                let clean_aich = aich_str.trim();
                let mut out = [0u8; 20];
                data_encoding::BASE32_NOPAD
                    .decode_mut(clean_aich.as_bytes(), &mut out)
                    .map_err(|_| FormatError::InvalidAichBase32(clean_aich.to_string()))?;
                aich_root = Some(out);
            } else if let Some(parts_str) = item.strip_prefix("p=") {
                for ph in parts_str.split(':') {
                    if ph.len() == 32 {
                        let mut h = [0u8; 16];
                        if hex::decode_to_slice(ph, &mut h).is_ok()
                            && part_hashes.len() < MAX_PART_HASHES_COUNT
                        {
                            part_hashes.push(h);
                        }
                    }
                }
            } else if let Some(url_str) = item.strip_prefix("s=") {
                if let Ok(parsed_url) = Url::parse(url_str) {
                    if parsed_url.scheme() == "http" || parsed_url.scheme() == "ftp" {
                        web_sources.push(parsed_url);
                    }
                }
            } else {
                unknown_fields.push(item.to_string());
            }
        }

        // Secondary sections: e.g. "sources,IP:PORT,HOST:PORT|/"
        let mut peer_sources = Vec::new();
        for &extra_section in &sections[1..] {
            let cleaned = extra_section.trim_end_matches('/').trim_end_matches('|');
            if let Some(sources_list) = cleaned.strip_prefix("sources,") {
                for src_item in sources_list.split(',') {
                    if let Some(src) = PeerSource::parse(src_item) {
                        if peer_sources.len() < MAX_SOURCES_COUNT {
                            peer_sources.push(src);
                        }
                    }
                }
            } else if !cleaned.is_empty() {
                unknown_fields.push(cleaned.to_string());
            }
        }

        Ok(Self {
            file_name,
            size,
            ed2k_hash,
            aich_root,
            part_hashes,
            web_sources,
            peer_sources,
            unknown_fields,
        })
    }

    /// Creates a new eD2K file link with validated size and filename.
    pub fn create(
        file_name: impl Into<String>,
        size: u64,
        ed2k_hash: Ed2kHash,
    ) -> Result<Self, FormatError> {
        let raw_name = file_name.into();
        if raw_name.trim().is_empty() {
            return Err(FormatError::InvalidLink(
                "File name cannot be empty".to_string(),
            ));
        }
        if size == 0 {
            return Err(FormatError::EmptyInput);
        }

        Ok(Self {
            file_name: raw_name,
            size,
            ed2k_hash,
            aich_root: None,
            part_hashes: Vec::new(),
            web_sources: Vec::new(),
            peer_sources: Vec::new(),
            unknown_fields: Vec::new(),
        })
    }

    /// Sanitizes delimiter characters ('|' and '/') for URI serialization.
    pub fn sanitize_filename(name: &str) -> String {
        let mut out = String::with_capacity(name.len() + 8);
        for c in name.chars() {
            match c {
                '|' => out.push_str("%7C"),
                '/' => out.push_str("%2F"),
                '\r' | '\n' => {}
                other => out.push(other),
            }
        }
        out
    }

    /// Formats the struct back into a valid canonical `ed2k://` link.
    pub fn to_link_string(&self) -> String {
        let safe_name = Self::sanitize_filename(&self.file_name);
        let mut s = format!(
            "ed2k://|file|{}|{}|{}|",
            safe_name,
            self.size,
            self.ed2k_hash.to_hex()
        );

        if let Some(aich) = &self.aich_root {
            s.push_str(&format!("h={}|", data_encoding::BASE32_NOPAD.encode(aich)));
        }

        if !self.part_hashes.is_empty() {
            let hashes_str = self
                .part_hashes
                .iter()
                .map(hex::encode)
                .collect::<Vec<_>>()
                .join(":");
            s.push_str(&format!("p={hashes_str}|"));
        }

        for ws in &self.web_sources {
            s.push_str(&format!("s={ws}|"));
        }

        s.push('/');

        if !self.peer_sources.is_empty() {
            let sources_str = self
                .peer_sources
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(",");
            s.push_str(&format!("|sources,{sources_str}|/"));
        }

        s
    }
}

impl fmt::Display for Ed2kFileLink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_link_string())
    }
}
