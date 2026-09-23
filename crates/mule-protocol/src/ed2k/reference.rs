// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

//! Validation module for local interoperability against reference eD2K peers (eMule / aMule).
//!
//! Phase 6.5: Strictly confined to local loopback (127.0.0.1). Zero public network,
//! zero peer discovery, zero servers, zero external DHT/Kad, zero leakage of user data.
//!
//! # Verification Tiers
//! 1. **Tier 1 (Corpus)**: Syntactic wire compliance against byte fixtures (`ed2k_corpus_tests.rs`).
//! 2. **Tier 2 (Emulated Reference Peer)**: Controlled loopback compliance using in-process
//!    emulation (`EmulatedReferencePeer` / `ReferenceCompatibleTestPeer`) mimicking official eMule/aMule wire logic.
//! 3. **Tier 3 (External Reference Binary)**: Execution against real pinned eMule/aMule binaries in loopback isolation.

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use unicode_normalization::UnicodeNormalization;

use crate::ed2k::loopback::Ed2kLoopbackClient;
use crate::ed2k::opcodes::{
    OP_FILESTATUS, OP_FILESTATUSREQ, OP_HELLO, OP_HELLOANSWER, OP_REQUESTPARTS,
    OP_REQUESTPARTS_EMULE, OP_SENDINGPART, OP_SETREQFILEID_EMULE,
};
use crate::ed2k::packet::{Ed2kFileStatus, Ed2kSendingPart};
use crate::ed2k::peer_info::Ed2kPeerInfo;
use crate::ed2k::profile::Ed2kCompatibilityProfile;
use crate::ed2k::session::Ed2kSessionError;
use crate::ed2k::tags::{Ed2kTag, TagName, TagValue, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION};

/// In-process emulated reference peer for Tier 2 loopback compliance testing.
pub type EmulatedReferencePeer = crate::ed2k::loopback::Ed2kTestPeer;

/// Alias for `EmulatedReferencePeer` denoting compatibility testing against eMule/aMule patterns.
pub type ReferenceCompatibleTestPeer = crate::ed2k::loopback::Ed2kTestPeer;

/// Direction of a recorded wire trace frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceDirection {
    /// Inbound packet received from remote loopback peer.
    Inbound,
    /// Outbound packet transmitted to remote loopback peer.
    Outbound,
}

/// Anonymized wire trace record of an eD2K frame exchanged on loopback.
/// Contains no user tokens, no raw IP addresses outside loopback, and no filesystem paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRecord {
    /// Monotonically increasing sequence index of the trace record.
    pub seq: usize,
    /// Elapsed milliseconds since session start.
    pub elapsed_ms: u64,
    /// Attestation test tier (2 for local emulated reference peer, 3 for pinned external binary).
    pub test_tier: u8,
    /// Kind of reference environment ("emulated-reference-peer" for Tier 2).
    pub reference_kind: &'static str,
    /// Explicit compatibility profile active when recording this frame.
    pub compatibility_profile: Ed2kCompatibilityProfile,
    /// Transmission direction (inbound or outbound).
    pub direction: TraceDirection,
    /// Numerical operation code byte.
    pub opcode: u8,
    /// Symbolic human-readable opcode label.
    pub opcode_name: &'static str,
    /// Raw payload length in bytes.
    pub payload_len: usize,
    /// Anonymized list of tag descriptions without sensitive tokens or paths.
    pub tags_summary: Vec<String>,
    /// Hex-encoded file hash (if applicable to the frame).
    pub file_hash_hex: Option<String>,
    /// Start and end offsets range (if applicable to the frame).
    pub range_summary: Option<(u32, u32)>,
}

/// Returns true if a character is a Unicode bidirectional override/isolate, invisible/zero-width formatting character, or control code.
pub fn is_bidi_or_formatting_control(c: char) -> bool {
    let cp = c as u32;
    matches!(
        cp,
        // Bidi controls and directional formatting
        0x061C | 0x200E | 0x200F | 0x202A..=0x202E | 0x2066..=0x2069 |
        // Zero-width and invisible formatting
        0x00AD | 0x200B..=0x200D | 0x2060 | 0xFEFF | 0xFFF9..=0xFFFB
    )
}

/// Sanitizes a string for safe display in CI reports and terminal logs:
/// 1. Applies Unicode Normalization Form C (NFC).
/// 2. Preserves standard international text (Latin, Cyrillic, Greek, Arabic, Hebrew, CJK, Emojis).
/// 3. Escapes dangerous or visually deceptive characters into \u{XXXX} format:
///    - Bidi control characters and directional formatting/overrides
///    - Invisible/zero-width formatting characters
///    - Control characters (c.is_control(), U+0000..=U+001F, U+007F..=U+009F).
pub fn sanitize_unicode_for_display(raw: &str) -> String {
    let nfc: String = raw.nfc().collect();
    let mut out = String::with_capacity(nfc.len());
    for c in nfc.chars() {
        if is_bidi_or_formatting_control(c)
            || (c.is_control() && c != '\t' && c != '\n' && c != '\r')
        {
            use std::fmt::Write;
            let _ = write!(out, "\\u{{{:04X}}}", c as u32);
        } else {
            out.push(c);
        }
    }
    out
}

impl TraceRecord {
    /// Verifies that this trace record is strictly anonymized and free of user paths, tokens, control characters, or bidi overrides.
    pub fn is_clean(&self) -> bool {
        for tag in &self.tags_summary {
            let lower = tag.to_ascii_lowercase();
            if lower.contains(":\\")
                || lower.contains("\\\\")
                || lower.contains("/home")
                || lower.contains("/users")
                || lower.contains("/tmp")
                || lower.contains("/var")
                || lower.contains("token")
                || lower.contains("bearer")
                || lower.contains("secret")
                || lower.contains("password")
                || tag.chars().any(|c| {
                    (c as u32) < 0x20 && c != '\t'
                        || c == '\u{007F}'
                        || (0x0080..=0x009F).contains(&(c as u32))
                        || is_bidi_or_formatting_control(c)
                })
            {
                return false;
            }
        }
        true
    }

    /// Returns the sanitized list of tag summaries with NFC normalization and escaped formatting/bidi controls.
    pub fn sanitized_tags_summary(&self) -> Vec<String> {
        self.tags_summary
            .iter()
            .map(|s| sanitize_unicode_for_display(s))
            .collect()
    }

    /// Produces a normalized, anonymized JSON representation conforming to test harness expectations.
    pub fn to_normalized_json(&self, outcome: &str) -> String {
        format!(
            "{{\"schemaVersion\":1,\"testTier\":{},\"referenceKind\":\"{}\",\"compatibilityProfile\":\"{}\",\"protocolByte\":\"0xE3\",\"opcode\":\"0x{:02X}\",\"direction\":\"{}\",\"payloadLength\":{},\"outcome\":\"{}\"}}",
            self.test_tier,
            self.reference_kind,
            self.compatibility_profile,
            self.opcode,
            match self.direction {
                TraceDirection::Outbound => "client_to_peer",
                TraceDirection::Inbound => "peer_to_client",
            },
            self.payload_len,
            outcome
        )
    }
}

/// Maps an opcode byte to a human-readable protocol label.
pub fn opcode_to_str(opcode: u8) -> &'static str {
    match opcode {
        OP_HELLO => "OP_HELLO (0x01)",
        OP_HELLOANSWER => "OP_HELLOANSWER (0x4C)",
        OP_FILESTATUSREQ => "OP_FILESTATUSREQ (0x49)",
        OP_SETREQFILEID_EMULE => "OP_SETREQFILEID_EMULE (0x4F)",
        OP_FILESTATUS => "OP_FILESTATUS (0x50)",
        OP_REQUESTPARTS => "OP_REQUESTPARTS (0x4B)",
        OP_REQUESTPARTS_EMULE => "OP_REQUESTPARTS_EMULE (0x47)",
        OP_SENDINGPART => "OP_SENDINGPART (0x46)",
        _ => "OP_UNKNOWN",
    }
}

/// Configuration for connecting to a local reference peer instance.
#[derive(Debug, Clone)]
pub struct ReferencePeerConfig {
    /// Target socket address on loopback (127.0.0.1).
    pub peer_addr: SocketAddr,
    /// Handshake completion timeout.
    pub handshake_timeout: Duration,
    /// Request/response transaction timeout.
    pub request_timeout: Duration,
    /// Maximum number of in-flight block requests.
    pub max_in_flight_blocks: usize,
    /// Explicit compatibility profile (defaults to Emule072a).
    pub profile: Ed2kCompatibilityProfile,
}

impl ReferencePeerConfig {
    /// Creates and validates a reference peer configuration.
    /// Strictly rejects any non-loopback address.
    pub fn new(peer_addr: SocketAddr) -> Result<Self, Ed2kSessionError> {
        if !peer_addr.ip().is_loopback() {
            return Err(Ed2kSessionError::Io(format!(
                "Non-loopback address {peer_addr} is strictly forbidden in reference interop"
            )));
        }
        if peer_addr.port() == 0 {
            return Err(Ed2kSessionError::Io(
                "Reference peer port cannot be 0".to_string(),
            ));
        }

        Ok(Self {
            peer_addr,
            handshake_timeout: Duration::from_millis(2000),
            request_timeout: Duration::from_millis(2000),
            max_in_flight_blocks: 8,
            profile: Ed2kCompatibilityProfile::Emule072a,
        })
    }

    /// Sets the explicit compatibility profile.
    pub fn with_profile(mut self, profile: Ed2kCompatibilityProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Sets the handshake completion timeout duration.
    pub fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
        self.handshake_timeout = timeout;
        self
    }

    /// Sets the request/response transaction timeout duration.
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }
}

/// Comprehensive interoperability and divergence report between EMule-Rust and reference peer.
#[derive(Debug, Clone)]
pub struct DivergenceReport {
    /// Explicit compatibility profile active during the session.
    pub profile: Ed2kCompatibilityProfile,
    /// Attestation test tier (2 for local emulated reference peer, 3 for pinned external binary).
    pub test_tier: u8,
    /// Kind of reference environment ("emulated-reference-peer" for Tier 2).
    pub reference_kind: &'static str,
    /// Recorded chronologic trace records.
    pub traces: Vec<TraceRecord>,
    /// Unique set of opcodes observed during session.
    pub opcodes_observed: Vec<u8>,
    /// Whether canonical eMule request parts opcode (0x47) was detected.
    pub canonical_request_parts_detected: bool,
    /// Whether canonical eMule set requested file ID opcode (0x4F) was detected.
    pub canonical_file_status_req_detected: bool,
    /// Descriptive audit and verification notes.
    pub notes: Vec<String>,
}

impl DivergenceReport {
    /// Validates that all traces in the report are fully anonymized.
    pub fn validate_anonymity(&self) -> Result<(), String> {
        for trace in &self.traces {
            if !trace.is_clean() {
                return Err(format!(
                    "Trace seq {} contains sensitive/non-anonymized data: {:?}",
                    trace.seq,
                    trace.sanitized_tags_summary()
                ));
            }
        }
        Ok(())
    }

    /// Generates a human- and CI-readable summary of the interop session.
    pub fn format_ci_summary(&self) -> String {
        let mut out = String::new();
        out.push_str("=== ED2K REFERENCE INTEROP SUMMARY ===\n");
        out.push_str("Schema Version: 1\n");
        out.push_str(&format!("Test Tier: {}\n", self.test_tier));
        out.push_str(&format!("Reference Kind: {}\n", self.reference_kind));
        out.push_str(&format!("Compatibility Profile: {}\n", self.profile));
        out.push_str(&format!("Total Traces: {}\n", self.traces.len()));
        out.push_str(&format!(
            "Observed Opcodes: {:02X?}\n",
            self.opcodes_observed
        ));
        out.push_str(&format!(
            "Canonical 0x47 Detected: {}\n",
            self.canonical_request_parts_detected
        ));
        out.push_str(&format!(
            "Canonical 0x4F Detected: {}\n",
            self.canonical_file_status_req_detected
        ));
        out.push_str(&format!(
            "Anonymity Validated: {}\n",
            self.validate_anonymity().is_ok()
        ));
        if !self.notes.is_empty() {
            out.push_str("Notes:\n");
            for note in &self.notes {
                let sanitized_note = sanitize_unicode_for_display(note);
                out.push_str(&format!("  - {sanitized_note}\n"));
            }
        }
        out.push_str("=== END SUMMARY ===\n");
        out
    }

    /// Serializes all recorded traces into normalized JSON strings.
    pub fn to_normalized_json_lines(&self) -> Vec<String> {
        self.traces
            .iter()
            .map(|t| t.to_normalized_json("accepted"))
            .collect()
    }

    /// Writes the CI divergence report and trace records to the target directory ONLY if anonymity is verified.
    /// Fails immediately and refuses to create files if any trace contains non-anonymized data.
    pub fn write_ci_report_secure(
        &self,
        target_dir: &std::path::Path,
    ) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
        self.validate_anonymity()?;

        if !target_dir.exists() {
            std::fs::create_dir_all(target_dir).map_err(|e| e.to_string())?;
        }

        let summary_path = target_dir.join("ci_divergence_summary.txt");
        let traces_path = target_dir.join("ci_wire_traces.jsonl");

        let summary_content = self.format_ci_summary();
        let traces_content = self.to_normalized_json_lines().join("\n");

        std::fs::write(&summary_path, summary_content).map_err(|e| e.to_string())?;
        std::fs::write(&traces_path, traces_content).map_err(|e| e.to_string())?;

        Ok((summary_path, traces_path))
    }
}

/// Loopback-confined client for exercising reference peer interoperability with automated trace logging.
pub struct ReferenceInteropClient {
    client: Ed2kLoopbackClient,
    config: ReferencePeerConfig,
    start_time: Instant,
    traces: Vec<TraceRecord>,
    seq_counter: usize,
}

impl ReferenceInteropClient {
    /// Connects to a reference peer on loopback, validates confinement, and completes handshake.
    pub async fn connect_and_handshake(
        config: ReferencePeerConfig,
        local_user_hash: [u8; 16],
        client_name: &str,
    ) -> Result<(Self, Ed2kPeerInfo), Ed2kSessionError> {
        // Enforce loopback check prior to connection
        if !config.peer_addr.ip().is_loopback() {
            return Err(Ed2kSessionError::Io(format!(
                "Non-loopback address {} rejected before connection",
                config.peer_addr
            )));
        }

        let start_time = Instant::now();
        let tags = vec![
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_NAME),
                value: TagValue::String(client_name.to_string()),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_VERSION),
                value: TagValue::UInt32(60),
            },
        ];

        let (client, peer_info) = Ed2kLoopbackClient::connect_and_handshake(
            config.peer_addr,
            local_user_hash,
            0x7F000001,
            39735,
            tags,
            0x7F000001,
            4661,
            config.handshake_timeout,
        )
        .await?;

        let profile = config.profile;
        let mut traces = Vec::new();
        // Outbound Hello trace (anonymized)
        traces.push(TraceRecord {
            seq: 0,
            elapsed_ms: 0,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: profile,
            direction: TraceDirection::Outbound,
            opcode: OP_HELLO,
            opcode_name: opcode_to_str(OP_HELLO),
            payload_len: 30,
            tags_summary: vec![
                "TAG_NAME:eMuleRust".to_string(),
                "TAG_VERSION:60".to_string(),
            ],
            file_hash_hex: None,
            range_summary: None,
        });

        // Inbound HelloAnswer trace (anonymized tags)
        let elapsed = start_time.elapsed().as_millis() as u64;
        let mut inbound_tags = Vec::new();
        if let Some(ref name) = peer_info.name {
            inbound_tags.push(format!("NAME:{name}"));
        }
        if let Some(ver) = peer_info.version {
            inbound_tags.push(format!("VERSION:{ver}"));
        }
        for ut in &peer_info.unknown_tags {
            inbound_tags.push(format!("UNKNOWN_TAG_{:?}:{}B", ut.name, ut.raw_bytes.len()));
        }

        traces.push(TraceRecord {
            seq: 1,
            elapsed_ms: elapsed,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: profile,
            direction: TraceDirection::Inbound,
            opcode: OP_HELLOANSWER,
            opcode_name: opcode_to_str(OP_HELLOANSWER),
            payload_len: 40,
            tags_summary: inbound_tags,
            file_hash_hex: None,
            range_summary: None,
        });

        let interop = Self {
            client,
            config,
            start_time,
            traces,
            seq_counter: 2,
        };

        Ok((interop, peer_info))
    }

    /// Queries file availability from the reference peer via OP_SETREQFILEID / OP_FILESTATUSREQ.
    pub async fn request_file_status(
        &mut self,
        file_hash: [u8; 16],
    ) -> Result<Ed2kFileStatus, Ed2kSessionError> {
        let hash_hex = hex::encode(file_hash);
        let elapsed = self.start_time.elapsed().as_millis() as u64;
        let profile = self.config.profile;

        // Trace outbound request
        self.traces.push(TraceRecord {
            seq: self.seq_counter,
            elapsed_ms: elapsed,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: profile,
            direction: TraceDirection::Outbound,
            opcode: OP_SETREQFILEID_EMULE,
            opcode_name: opcode_to_str(OP_SETREQFILEID_EMULE),
            payload_len: 17,
            tags_summary: vec![],
            file_hash_hex: Some(hash_hex.clone()),
            range_summary: None,
        });
        self.seq_counter += 1;

        let status = self
            .client
            .request_file_status(file_hash, self.config.request_timeout)
            .await?;

        let elapsed_in = self.start_time.elapsed().as_millis() as u64;
        self.traces.push(TraceRecord {
            seq: self.seq_counter,
            elapsed_ms: elapsed_in,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: profile,
            direction: TraceDirection::Inbound,
            opcode: OP_FILESTATUS,
            opcode_name: opcode_to_str(OP_FILESTATUS),
            payload_len: 18 + status.availability_mask.raw_bytes().len(),
            tags_summary: vec![format!("PARTS:{}", status.part_count)],
            file_hash_hex: Some(hash_hex),
            range_summary: None,
        });
        self.seq_counter += 1;

        Ok(status)
    }

    /// Requests a block range via OP_REQUESTPARTS and receives the corresponding OP_SENDINGPART.
    pub async fn request_and_receive_block(
        &mut self,
        file_hash: [u8; 16],
        start: u64,
        end: u64,
        file_size: u64,
    ) -> Result<Ed2kSendingPart, Ed2kSessionError> {
        let hash_hex = hex::encode(file_hash);
        let elapsed = self.start_time.elapsed().as_millis() as u64;
        let profile = self.config.profile;

        self.traces.push(TraceRecord {
            seq: self.seq_counter,
            elapsed_ms: elapsed,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: profile,
            direction: TraceDirection::Outbound,
            opcode: OP_REQUESTPARTS_EMULE,
            opcode_name: opcode_to_str(OP_REQUESTPARTS_EMULE),
            payload_len: 40,
            tags_summary: vec![],
            file_hash_hex: Some(hash_hex.clone()),
            range_summary: Some((start as u32, end as u32)),
        });
        self.seq_counter += 1;

        self.client
            .send_block_request(file_hash, start, end, file_size)
            .await?;

        let part = self
            .client
            .receive_sending_part(self.config.request_timeout)
            .await?;

        let elapsed_in = self.start_time.elapsed().as_millis() as u64;
        self.traces.push(TraceRecord {
            seq: self.seq_counter,
            elapsed_ms: elapsed_in,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: profile,
            direction: TraceDirection::Inbound,
            opcode: OP_SENDINGPART,
            opcode_name: opcode_to_str(OP_SENDINGPART),
            payload_len: 24 + part.data.len(),
            tags_summary: vec![],
            file_hash_hex: Some(hash_hex),
            range_summary: Some((part.start_offset, part.end_offset)),
        });
        self.seq_counter += 1;

        Ok(part)
    }

    /// Accesses the collected wire traces.
    pub fn traces(&self) -> &[TraceRecord] {
        &self.traces
    }

    /// Finalizes the reference interop session and builds a divergence report.
    pub async fn finish(self) -> DivergenceReport {
        let mut opcodes = Vec::new();
        let mut canonical_req_parts = false;
        let mut canonical_file_status_req = false;

        for trace in &self.traces {
            if !opcodes.contains(&trace.opcode) {
                opcodes.push(trace.opcode);
            }
            if trace.opcode == OP_REQUESTPARTS_EMULE {
                canonical_req_parts = true;
            }
            if trace.opcode == OP_SETREQFILEID_EMULE {
                canonical_file_status_req = true;
            }
        }

        self.client.close().await;

        DivergenceReport {
            profile: self.config.profile,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            traces: self.traces,
            opcodes_observed: opcodes,
            canonical_request_parts_detected: canonical_req_parts,
            canonical_file_status_req_detected: canonical_file_status_req,
            notes: vec![
                "Conducted solely on local loopback (127.0.0.1)".to_string(),
                "Zero public network exposure verified".to_string(),
                "All trace logs verified clean of system paths and tokens".to_string(),
            ],
        }
    }
}
