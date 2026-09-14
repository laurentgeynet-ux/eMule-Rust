//! Peer metadata and capability representation negotiated during the eD2K handshake.

use super::packet::{Ed2kEmuleInfo, Ed2kHello, Ed2kHelloAnswer};
use super::tags::{
    Ed2kTag, RawTag, TagName, TagValue, TAG_SPECIAL_EMULE_MISCOPTIONS1,
    TAG_SPECIAL_EMULE_MISCOPTIONS2, TAG_SPECIAL_EMULE_UDPPORTS, TAG_SPECIAL_EMULE_VERSION,
    TAG_SPECIAL_MISCOPTIONS1, TAG_SPECIAL_MISCOPTIONS2, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
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
                TagName::Id(TAG_SPECIAL_EMULE_VERSION) => {
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
        self.tags.iter().find(|t| t.name.display_name() == display_name)
    }

    /// Returns true if this peer identified itself as an eMule client.
    pub fn is_emule(&self) -> bool {
        self.emule_version.is_some()
    }

    /// Returns true if compression capability was announced in OP_EMULEINFO.
    pub fn has_compression_capability(&self) -> bool {
        self.emule_info
            .as_ref()
            .map_or(false, |info| info.has_compression_capability())
    }

    /// Returns true if source exchange capability was announced in OP_EMULEINFO.
    pub fn has_source_exchange_capability(&self) -> bool {
        self.emule_info
            .as_ref()
            .map_or(false, |info| info.has_source_exchange_capability())
    }
}
