// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use serde::{Deserialize, Serialize};

/// Connectivity profile defining address boundary restrictions and protocol capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkProfile {
    /// Strict lab confinement: only loopback addresses (127.0.0.0/8, ::1) are allowed.
    LocalLab,
    /// Controlled preview: allows connection to public eD2K servers and peers.
    /// Strictly rejects private LAN, loopback, link-local, multicast, etc. (anti-SSRF).
    PublicEd2kPreview,
    /// Controlled preview with UDP support for Kademlia.
    /// Same address filtering rules as PublicEd2kPreview.
    #[default]
    PublicEd2kKadPreview,
}

impl std::fmt::Display for NetworkProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LocalLab => write!(f, "local-lab"),
            Self::PublicEd2kPreview => write!(f, "public-ed2k-preview"),
            Self::PublicEd2kKadPreview => write!(f, "public-ed2k-kad-preview"),
        }
    }
}

impl std::str::FromStr for NetworkProfile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "local-lab" | "locallab" | "lab" => Ok(Self::LocalLab),
            "public-ed2k-preview" | "ed2k-preview" => Ok(Self::PublicEd2kPreview),
            "public-ed2k-kad-preview" | "kad-preview" => Ok(Self::PublicEd2kKadPreview),
            other => Err(format!("Unknown network profile: '{other}'")),
        }
    }
}

impl NetworkProfile {
    /// Whether this profile restricts all traffic exclusively to loopback addresses.
    pub fn is_loopback_only(&self) -> bool {
        matches!(self, Self::LocalLab)
    }

    /// Whether this profile permits UDP operations for Kademlia routing.
    pub fn allows_udp_kad(&self) -> bool {
        matches!(self, Self::LocalLab | Self::PublicEd2kKadPreview)
    }

    /// Whether this profile is an external preview profile.
    pub fn is_preview(&self) -> bool {
        matches!(self, Self::PublicEd2kPreview | Self::PublicEd2kKadPreview)
    }
}
