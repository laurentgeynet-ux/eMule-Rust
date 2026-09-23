//! Peer metadata and capability representation negotiated during the eD2K handshake.

use super::packet::{Ed2kEmuleInfo, Ed2kHello, Ed2kHelloAnswer};
use super::tags::{
    Ed2kTag, RawTag, TagName, TagValue, TAG_SPECIAL_CT_EMULE_VERSION,
    TAG_SPECIAL_EMULE_MISCOPTIONS1, TAG_SPECIAL_EMULE_MISCOPTIONS2, TAG_SPECIAL_EMULE_UDPPORTS,
    TAG_SPECIAL_EMULE_VERSION, TAG_SPECIAL_MISCOPTIONS1, TAG_SPECIAL_MISCOPTIONS2,
    TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
};

/// Consolidated information and capabilities for a connected eD2K remote peer.
#[derive(Debug, Clone, PartialEq)]
pub struct Ed2kPeerInfo {
    /// 16-byte MD4 User Hash of the remote peer.
    pub user_hash: [u8; 16],
    /// eD2K Client ID (HighID or LowID).
    pub client_id: u32,
    /// Listening TCP port of the remote peer.
    pub tcp_port: u16,
    /// Human-readable client name, if declared in tags.
    pub name: Option<String>,
    /// Protocol / client software version.
    pub version: Option<u32>,
    /// eMule-specific version code, if present.
    pub emule_version: Option<u32>,
    /// Miscellaneous options bitfield 1 (eMule protocol extensions).
    pub misc_options1: Option<u32>,
    /// Miscellaneous options bitfield 2.
    pub misc_options2: Option<u32>,
    /// Remote peer's extended eMule UDP port, if announced in tags.
    pub emule_udp_port: Option<u16>,
    /// IPv4 address of the peer's connected eD2K server (or 0 if none).
    pub server_ip: u32,
    /// Port of the peer's connected eD2K server (or 0 if none).
    pub server_port: u16,
    /// All recognized metadata tags exchanged in the handshake.
    pub tags: Vec<Ed2kTag>,
    /// Preserved unparsed or raw tags.
    pub unknown_tags: Vec<RawTag>,
    /// Extended eMule protocol announcement (`OP_EMULEINFO`), if received.
    pub emule_info: Option<Ed2kEmuleInfo>,
}

impl Ed2kPeerInfo {
    /// Constructs peer info from an incoming `Ed2kHello` message.
    pub fn from_hello(hello: &Ed2kHello) -> Self {
        Self::from_parts(
            hello.user_hash,
            hello.client_id,
            hello.tcp_port,
            hello.server_ip,
            hello.server_port,
            hello.tags.clone(),
            hello.unknown_tags.clone(),
        )
    }

    /// Constructs peer info from an incoming `Ed2kHelloAnswer` message.
    pub fn from_hello_answer(answer: &Ed2kHelloAnswer) -> Self {
        Self::from_parts(
            answer.user_hash,
            answer.client_id,
            answer.tcp_port,
            answer.server_ip,
            answer.server_port,
            answer.tags.clone(),
            answer.unknown_tags.clone(),
        )
    }

    fn from_parts(
        user_hash: [u8; 16],
        client_id: u32,
        tcp_port: u16,
        server_ip: u32,
        server_port: u16,
        tags: Vec<Ed2kTag>,
        unknown_tags: Vec<RawTag>,
    ) -> Self {
        let mut name = None;
        let mut version = None;
        let mut emule_version = None;
        let mut misc_options1 = None;
        let mut misc_options2 = None;
        let mut emule_udp_port = None;

        for tag in &tags {
            match &tag.name {
                TagName::Id(TAG_SPECIAL_NAME) => {
                    if let TagValue::String(s) = &tag.value {
                        name = Some(s.clone());
                    }
                }
                TagName::Id(TAG_SPECIAL_VERSION) => {
                    if let TagValue::UInt32(v) = &tag.value {
                        version = Some(*v);
                    }
                }
                // eMule clients announce their version under CT_EMULE_VERSION
                // (0xFB); the legacy 0x5A id (SO_EMULEVERSION) is kept as a
                // fallback for older dialects.
                TagName::Id(TAG_SPECIAL_EMULE_VERSION | TAG_SPECIAL_CT_EMULE_VERSION) => {
                    if let TagValue::UInt32(v) = &tag.value {
                        emule_version = Some(*v);
                    }
                }
                TagName::Id(TAG_SPECIAL_MISCOPTIONS1 | TAG_SPECIAL_EMULE_MISCOPTIONS1) => {
                    if let TagValue::UInt32(v) = &tag.value {
                        misc_options1 = Some(*v);
                    }
                }
                TagName::Id(TAG_SPECIAL_MISCOPTIONS2 | TAG_SPECIAL_EMULE_MISCOPTIONS2) => {
                    if let TagValue::UInt32(v) = &tag.value {
                        misc_options2 = Some(*v);
                    }
                }
                TagName::Id(TAG_SPECIAL_EMULE_UDPPORTS) => {
                    if let TagValue::UInt32(v) = &tag.value {
                        emule_udp_port = Some(*v as u16);
                    } else if let TagValue::UInt16(v) = &tag.value {
                        emule_udp_port = Some(*v);
                    }
                }
                TagName::Text(t) if t.eq_ignore_ascii_case("name") => {
                    if let TagValue::String(s) = &tag.value {
                        name = Some(s.clone());
                    }
                }
                _ => {}
            }
        }

        Self {
            user_hash,
            client_id,
            tcp_port,
            name,
            version,
            emule_version,
            misc_options1,
            misc_options2,
            emule_udp_port,
            server_ip,
            server_port,
            tags,
            unknown_tags,
            emule_info: None,
        }
    }

    /// Finds a tag by its display name.
    pub fn find_tag(&self, display_name: &str) -> Option<&Ed2kTag> {
        self.tags
            .iter()
            .find(|t| t.name.display_name() == display_name)
    }

    /// Returns true if this peer identified itself as an eMule client.
    pub fn is_emule(&self) -> bool {
        self.emule_version.is_some()
    }

    /// Client software identifier from the high byte of `CT_EMULE_VERSION`
    /// (eMule `CompatibleClientID`: 0 = eMule, 1 = cDonkey, 2 = xMule/lxMule,
    /// 3 = aMule, 4 = Shareaza, 5 = eMule Plus, 6 = HydraNode).
    pub fn software_id(&self) -> Option<u8> {
        self.emule_version.map(|v| (v >> 24) as u8)
    }

    /// Human-readable client software name derived from `software_id`.
    pub fn software_name(&self) -> &'static str {
        match self.software_id() {
            Some(0) => "eMule",
            Some(1) => "cDonkey",
            Some(2) => "xMule",
            Some(3) => "aMule",
            Some(4) => "Shareaza",
            Some(5) => "eMule Plus",
            Some(6) => "HydraNode",
            Some(0xAA) => "eMule-compat",
            Some(_) => "unknown",
            None => "ed2k",
        }
    }

    /// Decoded eMule-scale version string from the low 24 bits of
    /// `CT_EMULE_VERSION` (`(mjr<<17)|(min<<10)|(upd<<7)`), e.g. `"0.72a"`.
    pub fn emule_version_string(&self) -> Option<String> {
        self.emule_version.map(|v| {
            let v = v & 0x00FF_FFFF;
            let mjr = (v >> 17) & 0x7F;
            let min = (v >> 10) & 0x7F;
            let upd = (v >> 7) & 0x07;
            format!("{mjr}.{min}{}", (b'a' + upd as u8) as char)
        })
    }

    /// eMule `m_byExtendedRequestsVer` — CT_EMULE_MISCOPTIONS1 bits 8-11
    /// (0 = none, 1 = extended requests, 2 = 64-bit large-file requests).
    pub fn extended_requests_version(&self) -> u8 {
        self.misc_options1
            .map(|m| ((m >> 8) & 0x0F) as u8)
            .unwrap_or(0)
    }

    /// Kad protocol version announced in CT_EMULE_MISCOPTIONS2 bits 0-4.
    pub fn kad_version(&self) -> u8 {
        self.misc_options2.map(|m| (m & 0x0F) as u8).unwrap_or(0)
    }

    /// Returns true if this peer announced >4 GiB file support
    /// (bit 4 of misc_options2, eMule `m_fLargeFiles`).
    pub fn supports_large_files(&self) -> bool {
        self.misc_options2
            .map(|m| (m & (1 << 4)) != 0)
            .unwrap_or(false)
    }

    /// Returns true if compression capability was announced in OP_EMULEINFO
    /// or via `CT_EMULE_MISCOPTIONS1` bits 20-23 (`nDataCompVer`, eMule
    /// `ProcessHelloTypePacket`). Peers advertising it accept
    /// `OP_COMPRESSEDPART` zlib blocks on uploads.
    pub fn has_compression_capability(&self) -> bool {
        self.emule_info
            .as_ref()
            .is_some_and(|info| info.has_compression_capability())
            || self
                .misc_options1
                .map(|m| ((m >> 20) & 0x0F) > 0)
                .unwrap_or(false)
    }

    /// eMule `m_bySourceExchange1Ver` — SX1 version announced in
    /// CT_EMULE_MISCOPTIONS1 bits 12-15 (`BaseClient.cpp:479`). `0` means the
    /// peer did not advertise legacy source exchange.
    pub fn source_exchange1_version(&self) -> u8 {
        self.misc_options1
            .map(|m| ((m >> 12) & 0x0F) as u8)
            .unwrap_or(0)
    }

    /// Returns true if source exchange capability was announced in OP_EMULEINFO or CT_EMULE_MISCOPTIONS1.
    pub fn has_source_exchange_capability(&self) -> bool {
        self.emule_info
            .as_ref()
            .is_some_and(|info| info.has_source_exchange_capability())
            || self
                .misc_options1
                .map(|m| ((m >> 12) & 0x0F) > 0)
                .unwrap_or(false)
    }

    /// Returns true if this peer supports Source Exchange 2 (bit 10 of misc_options2).
    pub fn supports_source_exchange2(&self) -> bool {
        self.misc_options2
            .map(|m| (m & (1 << 10)) != 0)
            .unwrap_or(false)
    }

    /// Returns true if this peer supports either SX1 (via OP_EMULEINFO) or SX2 (via misc_options2).
    pub fn supports_source_exchange(&self) -> bool {
        self.supports_source_exchange2() || self.has_source_exchange_capability()
    }

    /// Returns true if this peer supports AICH (bit 29 of misc_options1,
    /// eMule `m_bySupportAICH` in CT_EMULE_MISCOPTIONS1).
    pub fn supports_aich(&self) -> bool {
        self.misc_options1
            .map(|m| ((m >> 29) & 0x07) > 0)
            .unwrap_or(false)
    }

    /// eMule `m_byUDPVer` — client UDP protocol version announced in
    /// CT_EMULE_MISCOPTIONS1 bits 24-27 (0 = no UDP support).
    pub fn udp_version(&self) -> u8 {
        self.misc_options1
            .map(|m| ((m >> 24) & 0x0F) as u8)
            .unwrap_or(0)
    }

    /// eMule `m_bySupportSecIdent` — Secure Ident support level announced in
    /// CT_EMULE_MISCOPTIONS1 bits 16-19 (0 = none, bit0 = v1, bit1 = v2).
    pub fn secure_ident_support(&self) -> u8 {
        self.misc_options1
            .map(|m| ((m >> 16) & 0x0F) as u8)
            .unwrap_or(0)
    }

    /// Returns true if this peer supports protocol obfuscation (bit 7 of misc_options2).
    pub fn supports_crypt_layer(&self) -> bool {
        self.misc_options2
            .map(|m| (m & (1 << 7)) != 0)
            .unwrap_or(false)
    }

    /// Returns true if this peer requests protocol obfuscation (bit 8 of misc_options2).
    pub fn requests_crypt_layer(&self) -> bool {
        self.misc_options2
            .map(|m| (m & (1 << 8)) != 0)
            .unwrap_or(false)
    }

    /// Returns true if this peer strictly requires protocol obfuscation (bit 9 of misc_options2).
    pub fn requires_crypt_layer(&self) -> bool {
        self.misc_options2
            .map(|m| (m & (1 << 9)) != 0)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sx1_capability_detected_from_misc_options1() {
        let mut info = Ed2kPeerInfo {
            user_hash: [0u8; 16],
            client_id: 123456789,
            tcp_port: 4662,
            name: None,
            version: None,
            emule_version: None,
            misc_options1: Some(4 << 12), // SX1 version = 4
            misc_options2: None,
            emule_udp_port: None,
            server_ip: 0,
            server_port: 0,
            tags: vec![],
            unknown_tags: vec![],
            emule_info: None,
        };

        assert!(info.has_source_exchange_capability());
        assert!(info.supports_source_exchange());
        assert!(!info.supports_source_exchange2());

        // Now set SX2 bit 10 in misc_options2
        info.misc_options2 = Some(1 << 10);
        assert!(info.supports_source_exchange2());
        assert!(info.supports_source_exchange());
    }
}
