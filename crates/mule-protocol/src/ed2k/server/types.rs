// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Core types and structures for eD2K server communication.

use crate::ed2k::encoder::Ed2kEncoder;
use crate::ed2k::error::{Ed2kEncodeError, Ed2kErrorKind};
use crate::ed2k::limits::Ed2kLimits;
use crate::ed2k::opcodes::{OP_CALLBACKREQUEST, OP_LOGINREQUEST, OP_OFFERFILES};
use crate::ed2k::protocol::PR_ED2K;
use crate::ed2k::tags::{
    Ed2kTag, RawTag, TagName, TagValue, SRVCAP_LARGEFILES, SRVCAP_NEWTAGS, SRVCAP_UNICODE,
    TAG_SPECIAL_CT_EMULE_VERSION, TAG_SPECIAL_EMULE_MISCOPTIONS1, TAG_SPECIAL_EMULE_VERSION,
    TAG_SPECIAL_FILESIZE, TAG_SPECIAL_FILESIZE_HI, TAG_SPECIAL_FILETYPE, TAG_SPECIAL_NAME,
    TAG_SPECIAL_SERVER_FLAGS, TAG_SPECIAL_VERSION,
};
use crate::framing::RawPacket;
use bytes::Bytes;

/// Client login request sent to an eD2K server (`OP_LOGINREQUEST` = `0x01` under `PR_ED2K`).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerLoginRequest {
    /// 16-byte MD4 User Hash.
    pub user_hash: [u8; 16],
    /// Current client ID (usually 0 before server assigns one).
    pub client_id: u32,
    /// Local listening TCP port.
    pub tcp_port: u16,
    /// Client tags (e.g. Nickname, Version, Flags).
    pub tags: Vec<Ed2kTag>,
}

impl ServerLoginRequest {
    /// Creates a new default login request with user hash, port, and standard modern nickname and version tags (eMule 0.60d compatible).
    pub fn new(user_hash: [u8; 16], tcp_port: u16, nickname: &str, version: u32) -> Self {
        let emule_ver_packed: u32 = (60 << 10) | (4 << 7);
        let emule_ver_ext: u32 = (0x3c << 24) | (60 << 17) | 4;

        let tags = vec![
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_NAME),
                value: TagValue::String(nickname.to_string()),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_VERSION),
                value: TagValue::UInt32(version),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_SERVER_FLAGS),
                value: TagValue::UInt32(SRVCAP_NEWTAGS | SRVCAP_UNICODE | SRVCAP_LARGEFILES),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_EMULE_VERSION),
                value: TagValue::UInt32(emule_ver_ext),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_CT_EMULE_VERSION),
                value: TagValue::UInt32(emule_ver_packed),
            },
            Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_EMULE_MISCOPTIONS1),
                value: TagValue::UInt32(0x01),
            },
        ];
        Self {
            user_hash,
            client_id: 0,
            tcp_port,
            tags,
        }
    }

    /// Encodes the login request into a `RawPacket`.
    pub fn encode(&self, limits: &Ed2kLimits) -> Result<RawPacket, Ed2kEncodeError> {
        let encoder = Ed2kEncoder::with_limits(limits.clone());
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.user_hash);
        buf.extend_from_slice(&self.client_id.to_le_bytes());
        buf.extend_from_slice(&self.tcp_port.to_le_bytes());
        buf.extend_from_slice(&(self.tags.len() as u32).to_le_bytes());

        for tag in &self.tags {
            encoder.encode_tag(OP_LOGINREQUEST, tag, &mut buf)?;
        }

        let payload_len = buf.len() + 1;
        if payload_len > limits.max_frame_bytes {
            return Err(Ed2kEncodeError::new(
                OP_LOGINREQUEST,
                "login_request_payload",
                Ed2kErrorKind::FrameTooLarge {
                    size: payload_len,
                    limit: limits.max_frame_bytes,
                },
            ));
        }

        Ok(RawPacket::with_vec(PR_ED2K, OP_LOGINREQUEST, buf))
    }
}

/// Server message containing server greetings or MOTD (`OP_SERVERMESSAGE` = `0x38`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerMessage {
    /// Message string sent by the server.
    pub message: String,
}

/// Server ID change response (`OP_IDCHANGE` = `0x40`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerIdChange {
    /// Assigned Client ID.
    /// If `>= 16777216`, client is assigned HighID.
    /// If `< 16777216`, client is behind NAT/firewall and assigned LowID.
    pub client_id: u32,
    /// Server TCP flags / capabilities (historical).
    pub tcp_flags: u32,
}

impl ServerIdChange {
    /// Returns true if the client ID qualifies as HighID (`>= 16_777_216`).
    #[inline]
    pub fn is_high_id(&self) -> bool {
        self.client_id >= 16_777_216
    }
}

/// Server status report (`OP_SERVERSTATUS` = `0x14`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerStatus {
    /// Total active users connected to the server.
    pub current_users: u32,
    /// Total files currently indexed by the server.
    pub current_files: u32,
}

/// Server identification response (`OP_SERVERIDENT` = `0x41`).
#[derive(Debug, Clone, PartialEq)]
pub struct ServerIdent {
    /// 16-byte server hash.
    pub server_hash: [u8; 16],
    /// Server IPv4 address in network order.
    pub ip: u32,
    /// Server listening port.
    pub port: u16,
    /// Server tags (name, description, etc.).
    pub tags: Vec<Ed2kTag>,
    /// Unknown / raw tags.
    pub unknown_tags: Vec<RawTag>,
}

impl ServerIdent {
    /// Helper to find server name in tags.
    pub fn name(&self) -> Option<String> {
        self.tags.iter().find_map(|t| match (&t.name, &t.value) {
            (TagName::Id(TAG_SPECIAL_NAME), TagValue::String(s)) => Some(s.clone()),
            _ => None,
        })
    }

    /// eMule `ST_TCPPORTOBFUSCATION` (`0x97`, uint16) — dedicated TCP port
    /// accepting obfuscated (DH768+RC4) server connections.
    pub fn obfuscation_tcp_port(&self) -> Option<u16> {
        self.server_tag_u16(0x97)
    }

    /// eMule `ST_UDPPORTOBFUSCATION` (`0x98`, uint16) — dedicated UDP port
    /// accepting obfuscated server datagrams.
    pub fn obfuscation_udp_port(&self) -> Option<u16> {
        self.server_tag_u16(0x98)
    }

    fn server_tag_u16(&self, tag_id: u8) -> Option<u16> {
        for t in &self.tags {
            if t.name == TagName::Id(tag_id) {
                match t.value {
                    TagValue::UInt16(v) => return Some(v),
                    TagValue::UInt32(v) => return u16::try_from(v).ok(),
                    _ => {}
                }
            }
        }
        // Fall back to raw tags (unrecognized typed decode): a uint16 tag is
        // 2 little-endian bytes.
        self.unknown_tags.iter().find_map(|t| {
            if t.name == TagName::Id(tag_id) && t.raw_bytes.len() == 2 {
                Some(u16::from_le_bytes([t.raw_bytes[0], t.raw_bytes[1]]))
            } else {
                None
            }
        })
    }
}

/// eMule `SRV_TCPFLG_COMPRESSION`: server accepts the 0xFBFB/0xFCFC offer
/// markers and zlib-compressed server packets.
pub const SRV_TCPFLG_COMPRESSION: u32 = 0x0000_0001;
/// eMule `SRV_TCPFLG_LARGEFILES`: server supports >4 GiB file entries.
pub const SRV_TCPFLG_LARGEFILES: u32 = 0x0000_0100;

/// Marker client ID sent in `OP_OFFERFILES` entries when the server supports
/// TCP compression: file is complete.
pub const OFFER_COMPLETE_CLIENT_ID: u32 = 0xFBFBFBFB;
/// Marker client port paired with `OFFER_COMPLETE_CLIENT_ID`.
pub const OFFER_COMPLETE_CLIENT_PORT: u16 = 0xFBFB;
/// Marker client ID for an incomplete (part) file.
pub const OFFER_INCOMPLETE_CLIENT_ID: u32 = 0xFCFCFCFC;
/// Marker client port paired with `OFFER_INCOMPLETE_CLIENT_ID`.
pub const OFFER_INCOMPLETE_CLIENT_PORT: u16 = 0xFCFC;

/// Maps a filename to its eD2K `FT_FILETYPE` search term
/// (eMule `GetED2KFileTypeSearchTerm`): `"Audio"`, `"Video"`, `"Image"`,
/// `"Doc"`, `"Pro"` — or `""` (don't send a type tag).
pub fn ed2k_file_type_term(filename: &str) -> &'static str {
    let ext = filename
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "aac" | "aif" | "aiff" | "au" | "flac" | "mid" | "midi" | "mp3" | "m4a" | "ogg" | "ogm"
        | "ra" | "ram" | "wav" | "wma" | "ape" | "mka" | "opus" => "Audio",
        "3gp" | "asf" | "avi" | "divx" | "flv" | "m1v" | "m2v" | "mkv" | "mov" | "mp1" | "mp2"
        | "mp4" | "mpe" | "mpeg" | "mpg" | "mpv" | "mpv2" | "ogv" | "qt" | "rm" | "rmvb" | "rv"
        | "ts" | "vcd" | "vob" | "webm" | "wmv" => "Video",
        "bmp" | "gif" | "jpeg" | "jpg" | "png" | "psd" | "tif" | "tiff" | "webp" => "Image",
        "doc" | "docx" | "htm" | "html" | "nfo" | "odt" | "pdf" | "pps" | "ppt" | "pptx"
        | "rtf" | "txt" | "xls" | "xlsx" | "epub" | "mobi" => "Doc",
        // eMule publishes archives and CD images as "Pro" for indexing.
        "7z" | "ace" | "alz" | "arj" | "bz2" | "cab" | "gz" | "lha" | "rar" | "tar" | "tgz"
        | "zip" | "zoo" | "bin" | "bwi" | "ccd" | "cue" | "img" | "iso" | "mds" | "nrg" | "sub"
        | "exe" | "msi" | "apk" | "dmg" | "com" | "bat" | "dll" => "Pro",
        _ => "",
    }
}

/// One file entry in an `OP_OFFERFILES` packet
/// (eMule `CSharedFileList::CreateOfferedFilePacket` for servers).
#[derive(Debug, Clone, PartialEq)]
pub struct OfferedFile {
    /// 16-byte MD4 file hash.
    pub file_hash: [u8; 16],
    /// Client ID field: our HighID, 0 when LowID, or the `0xFBFBFBFB` /
    /// `0xFCFCFCFC` markers when the server supports TCP compression.
    pub client_id: u32,
    /// Listening TCP port (or `0xFBFB`/`0xFCFC` marker).
    pub client_port: u16,
    /// Metadata tags (`FT_FILENAME`, `FT_FILESIZE`[, `FT_FILESIZE_HI`],
    /// `FT_FILETYPE`).
    pub tags: Vec<Ed2kTag>,
}

impl OfferedFile {
    /// Builds an offered-file entry for a shared file, following eMule's
    /// server publishing rules (`CreateOfferedFilePacket`): when the server
    /// supports `SRV_TCPFLG_COMPRESSION`, incomplete files (part files) carry
    /// the `0xFCFCFCFC`/`0xFCFC` marker pair and complete files the
    /// `0xFBFBFBFB`/`0xFBFB` pair; otherwise a HighID publishes its real
    /// ID+port (`0` when LowID). Large files get the split
    /// `FT_FILESIZE`/`FT_FILESIZE_HI` pair only when the server supports them.
    pub fn for_server(
        file_hash: [u8; 16],
        file_name: &str,
        file_size: u64,
        our_client_id: u32,
        our_tcp_port: u16,
        server_tcp_flags: u32,
        is_partial: bool,
    ) -> Option<Self> {
        const LARGE_FILE: u64 = 0xFFFF_FFFF;
        let is_large = file_size > LARGE_FILE;
        if is_large && (server_tcp_flags & SRV_TCPFLG_LARGEFILES) == 0 {
            // eMule skips large files when the server lacks large-file TCP support.
            return None;
        }

        let (client_id, client_port) = if server_tcp_flags & SRV_TCPFLG_COMPRESSION != 0 {
            if is_partial {
                (OFFER_INCOMPLETE_CLIENT_ID, OFFER_INCOMPLETE_CLIENT_PORT)
            } else {
                (OFFER_COMPLETE_CLIENT_ID, OFFER_COMPLETE_CLIENT_PORT)
            }
        } else if our_client_id >= 16_777_216 {
            // HighID: publish our real client ID + TCP port.
            (our_client_id, our_tcp_port)
        } else {
            (0, 0)
        };

        let mut tags = vec![Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_NAME),
            value: TagValue::String(file_name.to_string()),
        }];
        if is_large {
            tags.push(Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_FILESIZE),
                value: TagValue::UInt32(file_size as u32),
            });
            tags.push(Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_FILESIZE_HI),
                value: TagValue::UInt32((file_size >> 32) as u32),
            });
        } else {
            tags.push(Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_FILESIZE),
                value: TagValue::UInt32(file_size as u32),
            });
        }
        let ft = ed2k_file_type_term(file_name);
        if !ft.is_empty() {
            tags.push(Ed2kTag {
                name: TagName::Id(TAG_SPECIAL_FILETYPE),
                value: TagValue::String(ft.to_string()),
            });
        }
        Some(Self {
            file_hash,
            client_id,
            client_port,
            tags,
        })
    }
}

/// `OP_OFFERFILES` (`0x15`) packet: `u32 count` + per-file
/// `hash[16]` + `u32 client_id` + `u16 client_port` + tag list.
/// An empty `files` list doubles as the server keep-alive ping.
#[derive(Debug, Clone, PartialEq)]
pub struct OfferFiles {
    /// Files to publish (max ~200 per eMule's soft-file limit).
    pub files: Vec<OfferedFile>,
}

impl OfferFiles {
    /// Encodes the packet into a `RawPacket` (PR_ED2K / `OP_OFFERFILES`).
    pub fn encode(&self, limits: &Ed2kLimits) -> Result<RawPacket, Ed2kEncodeError> {
        let encoder = Ed2kEncoder::with_limits(limits.clone());
        let mut buf = Vec::new();
        buf.extend_from_slice(&(self.files.len() as u32).to_le_bytes());
        for f in &self.files {
            buf.extend_from_slice(&f.file_hash);
            buf.extend_from_slice(&f.client_id.to_le_bytes());
            buf.extend_from_slice(&f.client_port.to_le_bytes());
            buf.extend_from_slice(&(f.tags.len() as u32).to_le_bytes());
            for tag in &f.tags {
                encoder.encode_tag(OP_OFFERFILES, tag, &mut buf)?;
            }
        }
        let payload_len = buf.len() + 1;
        if payload_len > limits.max_frame_bytes {
            return Err(Ed2kEncodeError::new(
                OP_OFFERFILES,
                "offer_files_payload",
                Ed2kErrorKind::FrameTooLarge {
                    size: payload_len,
                    limit: limits.max_frame_bytes,
                },
            ));
        }
        Ok(RawPacket::with_vec(PR_ED2K, OP_OFFERFILES, buf))
    }
}

/// `OP_CALLBACKREQUEST` (`0x1C`, PR_ED2K): asks the server to relay a
/// connection request to a LowID client — the target then connects to us
/// (eMule `CDownloadQueue::...` / server callback mechanism).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackRequest {
    /// LowID client ID of the peer to call back (< 16,777,216).
    pub client_id: u32,
}

impl CallbackRequest {
    /// Encodes the request into a `RawPacket`: bare `u32 client_id` payload.
    pub fn encode(&self) -> RawPacket {
        RawPacket::with_vec(
            PR_ED2K,
            OP_CALLBACKREQUEST,
            self.client_id.to_le_bytes().to_vec(),
        )
    }
}

/// Server-relayed callback notification `OP_CALLBACKREQUESTED` (`0x35`):
/// `<IP 4><PORT 2>` of a client that asked the server for a callback to us,
/// optionally followed by `u8 crypt_options` + `user_hash[16]` (23+ bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackRequested {
    /// Requester IPv4 (little-endian as on the wire).
    pub ip: u32,
    /// Requester TCP port.
    pub port: u16,
    /// Crypt/obfuscation options byte, if present.
    pub crypt_options: Option<u8>,
    /// Requester user hash, if present.
    pub user_hash: Option<[u8; 16]>,
}

/// Decoded server-to-client message.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerResponse {
    /// Server notice / MOTD string.
    Message(ServerMessage),
    /// Assigned ClientID / HighID or LowID.
    IdChange(ServerIdChange),
    /// Current users and files statistics.
    Status(ServerStatus),
    /// Server identification and metadata.
    Ident(ServerIdent),
    /// Search results returned by server.
    SearchResult(super::search::SearchResult),
    /// Sources returned by server for a requested file hash.
    FoundSources(super::sources::FoundSources),
    /// Sources returned by server with obfuscation metadata (OP_FOUNDSOURCES_OBFU).
    FoundSourcesObfu(super::sources::FoundSourcesObfu),
    /// A peer asked the server to relay a callback to us (`OP_CALLBACKREQUESTED`).
    CallbackRequested(CallbackRequested),
    /// The server could not relay our callback request (`OP_CALLBACK_FAIL`).
    CallbackFail {
        /// Target client ID that could not be called back, when echoed.
        client_id: Option<u32>,
    },
    /// Additional servers advertised by the connected server
    /// (`OP_SERVERLIST`): `(ip_le, port)` entries.
    ServerList {
        /// Advertised servers (little-endian IPv4 + TCP port).
        servers: Vec<(u32, u16)>,
    },
    /// Other unparsed packet.
    Other {
        /// Opcode of unparsed packet.
        opcode: u8,
        /// Raw payload bytes.
        payload: Bytes,
    },
}
