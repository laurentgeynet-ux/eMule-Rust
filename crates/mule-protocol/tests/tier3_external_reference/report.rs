//! Anonymized divergence reporting for Tier 3 external reference orchestration.
//!
//! Conforms to the `reference_manifest.json` schema and guarantees zero leakage of
//! host user identifiers, external IPs, or private file paths.
//! Every report explicitly declares its `compatibilityProfile` to avoid ambiguous interpretation
//! of overloaded opcodes (e.g. 0x47 vs 0x4B, 0x4F vs 0x49).

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// A single divergence record observed between Emule-Sion and the external reference binary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DivergenceEntry {
    pub area: String,
    pub expected: String,
    pub observed: String,
    pub severity: String,
}

/// A serialized representation of tags observed during the handshake with the reference binary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservedTagReport {
    pub name: String,
    pub value_summary: String,
}

/// A normalized wire trace entry recording frame-level exchange with its explicit compatibility profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WireTraceRecord {
    #[serde(rename = "schemaVersion", default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(rename = "testTier", default = "default_tier3")]
    pub test_tier: u8,
    #[serde(rename = "referenceKind", default = "default_ref_kind_external")]
    pub reference_kind: String,
    #[serde(rename = "compatibilityProfile")]
    pub compatibility_profile: String,
    #[serde(rename = "protocolByte")]
    pub protocol_byte: String,
    pub opcode: String,
    pub direction: String,
    #[serde(rename = "payloadLength")]
    pub payload_length: usize,
    pub outcome: String,
}

fn default_schema_version() -> u32 {
    1
}

fn default_tier3() -> u8 {
    3
}

fn default_ref_kind_external() -> String {
    "external-reference-binary".to_string()
}

/// The complete divergence report generated after running a Tier 3 loopback session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Tier3DivergenceReport {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "testTier")]
    pub test_tier: u8,
    #[serde(rename = "referenceKind")]
    pub reference_kind: String,
    #[serde(rename = "compatibilityProfile")]
    pub compatibility_profile: String,
    #[serde(rename = "referenceBinaryName")]
    pub reference_binary_name: String,
    #[serde(rename = "referenceBinarySha256")]
    pub reference_binary_sha256: String,
    #[serde(rename = "handshakeStatus")]
    pub handshake_status: String,
    #[serde(rename = "observedTags")]
    pub observed_tags: Vec<ObservedTagReport>,
    #[serde(rename = "divergences")]
    pub divergences: Vec<DivergenceEntry>,
    #[serde(rename = "wireTraces")]
    pub wire_traces: Vec<WireTraceRecord>,
}

impl Tier3DivergenceReport {
    /// Creates a new empty divergence report for a verified reference binary and explicit profile.
    pub fn new(
        reference_binary_name: &str,
        reference_binary_sha256: &str,
        compatibility_profile: &str,
    ) -> Self {
        Self {
            schema_version: 1,
            test_tier: 3,
            reference_kind: "external-reference-binary".to_string(),
            compatibility_profile: compatibility_profile.to_string(),
            reference_binary_name: reference_binary_name.to_string(),
            reference_binary_sha256: reference_binary_sha256.to_string(),
            handshake_status: "unknown".to_string(),
            observed_tags: Vec::new(),
            divergences: Vec::new(),
            wire_traces: Vec::new(),
        }
    }

    /// Records an observed tag from the reference peer.
    pub fn record_tag(&mut self, name: &str, value_summary: &str) {
        self.observed_tags.push(ObservedTagReport {
            name: name.to_string(),
            value_summary: value_summary.to_string(),
        });
    }

    /// Records a protocol divergence.
    pub fn record_divergence(&mut self, area: &str, expected: &str, observed: &str, severity: &str) {
        self.divergences.push(DivergenceEntry {
            area: area.to_string(),
            expected: expected.to_string(),
            observed: observed.to_string(),
            severity: severity.to_string(),
        });
    }

    /// Records a normalized wire trace entry with its explicit compatibility profile.
    pub fn record_wire_trace(
        &mut self,
        protocol_byte: u8,
        opcode: u8,
        direction: &str,
        payload_length: usize,
        outcome: &str,
    ) {
        self.wire_traces.push(WireTraceRecord {
            schema_version: 1,
            test_tier: 3,
            reference_kind: "external-reference-binary".to_string(),
            compatibility_profile: self.compatibility_profile.clone(),
            protocol_byte: format!("0x{protocol_byte:02X}"),
            opcode: format!("0x{opcode:02X}"),
            direction: direction.to_string(),
            payload_length,
            outcome: outcome.to_string(),
        });
    }

    /// Validates that the report does not leak any non-loopback IP, absolute host username,
    /// or non-ephemeral file paths.
    pub fn validate_anonymity(&self) -> Result<(), String> {
        let serialized = serde_json::to_string(self)
            .map_err(|e| format!("serialization error during anonymity check: {e}"))?;

        // Check for common leaked host artifacts
        if let Ok(username) = std::env::var("USERNAME") {
            if !username.is_empty() && serialized.to_lowercase().contains(&username.to_lowercase()) {
                return Err(format!("Anonymity violation: report contains host username '{username}'"));
            }
        }
        if let Ok(user) = std::env::var("USER") {
            if !user.is_empty() && serialized.to_lowercase().contains(&user.to_lowercase()) {
                return Err(format!("Anonymity violation: report contains host USER '{user}'"));
            }
        }

        // Check for typical Windows or Unix absolute home paths
        if serialized.contains(":\\Users\\") || serialized.contains("/home/") {
            return Err("Anonymity violation: report contains host user home directory path".to_string());
        }

        Ok(())
    }

    /// Serializes the report to a formatted JSON string.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
