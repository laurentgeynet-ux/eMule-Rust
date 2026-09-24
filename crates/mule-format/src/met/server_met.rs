// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::io::{Read, Write};
use std::net::Ipv4Addr;
use std::path::Path;

use crate::met::error::MetError;
use crate::met::reader::CountingReader;
use crate::met::tags::*;

pub const SERVER_MET_VERSION: u8 = 0xE0;
pub const MAX_SERVER_COUNT: usize = 10_000;
pub const MAX_SERVER_TAG_COUNT: usize = 100;

/// A parsed entry from a `server.met` file representing an eD2K server.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerEntry {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub name: String,
    pub description: String,
    pub ping: u32,
    pub preference: u32,
    pub failed_count: u32,
    pub dyn_ip: Option<String>,
    pub version: Option<String>,
    pub tags: Vec<Tag>,
}

impl ServerEntry {
    pub fn new(ip: Ipv4Addr, port: u16, name: impl Into<String>) -> Self {
        Self {
            ip,
            port,
            name: name.into(),
            description: String::new(),
            ping: 0,
            preference: 0,
            failed_count: 0,
            dyn_ip: None,
            version: None,
            tags: Vec::new(),
        }
    }
}

/// A parsed `server.met` file containing a list of eD2K servers.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerMetFile {
    pub version: u8,
    pub servers: Vec<ServerEntry>,
}

impl ServerMetFile {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, MetError> {
        let mut file = crate::met::reader::open_met_file(path)?;
        Self::read_from(&mut file)
    }

    pub fn read_from<R: Read>(reader: &mut R) -> Result<Self, MetError> {
        let mut cr = CountingReader::new(reader);

        let header_offset = cr.offset();
        let version = cr.read_u8("version header")?;
        if version != SERVER_MET_VERSION {
            return Err(MetError::UnsupportedHeader {
                offset: header_offset,
                value: version,
            });
        }

        let count_offset = cr.offset();
        let count = cr.read_u32_le("server count")? as usize;
        if count > MAX_SERVER_COUNT {
            return Err(MetError::InvalidLength {
                offset: count_offset,
                declared: count as u64,
                maximum: MAX_SERVER_COUNT as u64,
                context: "server count",
            });
        }

        let mut servers = Vec::with_capacity(count.min(1000));

        for i in 0..count {
            let ip_u32 = cr.read_u32_le("server ip")?;
            // eMule stores IP in host or network byte order depending on endianness; standard eMule Ipv4Addr is from_bits(u32::from_be(ip_u32))
            // In little endian wire format, bytes are b0, b1, b2, b3.
            let ip_bytes = ip_u32.to_le_bytes();
            let ip = Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);

            let port = cr.read_u16_le("server port")?;
            let tag_count_offset = cr.offset();
            let tag_count = cr.read_u32_le("tag count")? as usize;

            if tag_count > MAX_SERVER_TAG_COUNT {
                return Err(MetError::InvalidLength {
                    offset: tag_count_offset,
                    declared: tag_count as u64,
                    maximum: MAX_SERVER_TAG_COUNT as u64,
                    context: "tag count",
                });
            }

            let mut name = String::new();
            let mut description = String::new();
            let mut ping = 0u32;
            let mut preference = 0u32;
            let mut failed_count = 0u32;
            let mut dyn_ip = None;
            let mut version_str = None;
            let mut tags = Vec::with_capacity(tag_count);

            for _ in 0..tag_count {
                let tag = cr.read_tag()?;
                match tag.name_id {
                    SN_NAME => {
                        if let MetTagValue::String(ref s) = tag.value {
                            name = s.clone();
                        }
                    }
                    SN_DESCRIPTION => {
                        if let MetTagValue::String(ref s) = tag.value {
                            description = s.clone();
                        }
                    }
                    SN_PING => {
                        if let MetTagValue::Uint64(v) = tag.value {
                            ping = v as u32;
                        }
                    }
                    SN_PREFERENCE => {
                        if let MetTagValue::Uint64(v) = tag.value {
                            preference = v as u32;
                        }
                    }
                    SN_FAIL => {
                        if let MetTagValue::Uint64(v) = tag.value {
                            failed_count = v as u32;
                        }
                    }
                    SN_DYNIP => {
                        if let MetTagValue::String(ref s) = tag.value {
                            dyn_ip = Some(s.clone());
                        }
                    }
                    SN_VERSION => {
                        if let MetTagValue::String(ref s) = tag.value {
                            version_str = Some(s.clone());
                        }
                    }
                    _ => {}
                }
                tags.push(tag);
            }

            if name.is_empty() {
                name = format!("Server-{:03}", i + 1);
            }

            servers.push(ServerEntry {
                ip,
                port,
                name,
                description,
                ping,
                preference,
                failed_count,
                dyn_ip,
                version: version_str,
                tags,
            });
        }

        Ok(ServerMetFile { version, servers })
    }

    /// Serializes the server.met structure into a writable stream.
    pub fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&[self.version])?;
        writer.write_all(&(self.servers.len() as u32).to_le_bytes())?;

        for s in &self.servers {
            let octets = s.ip.octets();
            let ip_u32 = u32::from_le_bytes(octets);
            writer.write_all(&ip_u32.to_le_bytes())?;
            writer.write_all(&s.port.to_le_bytes())?;

            // Write tags
            writer.write_all(&(s.tags.len() as u32).to_le_bytes())?;
            for tag in &s.tags {
                // Encode tag
                let is_numeric = tag.name_id != 0;
                let name_bytes = tag.name.as_bytes();
                let type_byte = match &tag.value {
                    MetTagValue::String(_) => TAGTYPE_STRING,
                    MetTagValue::Uint64(v) => {
                        if *v <= u8::MAX as u64 {
                            TAGTYPE_UINT8
                        } else if *v <= u16::MAX as u64 {
                            TAGTYPE_UINT16
                        } else if *v <= u32::MAX as u64 {
                            TAGTYPE_UINT32
                        } else {
                            TAGTYPE_UINT64
                        }
                    }
                    MetTagValue::Float32(_) => TAGTYPE_FLOAT32,
                    MetTagValue::Hash16(_) => TAGTYPE_HASH16,
                    MetTagValue::Blob(_) => TAGTYPE_BLOB,
                    MetTagValue::Bsob(_) => TAGTYPE_BSOB,
                    MetTagValue::Bool(_) => TAGTYPE_BOOL,
                };

                writer.write_all(&[type_byte])?;
                if is_numeric {
                    writer.write_all(&1u16.to_le_bytes())?;
                    writer.write_all(&[tag.name_id])?;
                } else {
                    writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
                    writer.write_all(name_bytes)?;
                }

                match &tag.value {
                    MetTagValue::String(s) => {
                        let bytes = s.as_bytes();
                        writer.write_all(&(bytes.len() as u16).to_le_bytes())?;
                        writer.write_all(bytes)?;
                    }
                    MetTagValue::Uint64(v) => match type_byte {
                        TAGTYPE_UINT8 => writer.write_all(&[*v as u8])?,
                        TAGTYPE_UINT16 => writer.write_all(&(*v as u16).to_le_bytes())?,
                        TAGTYPE_UINT32 => writer.write_all(&(*v as u32).to_le_bytes())?,
                        _ => writer.write_all(&v.to_le_bytes())?,
                    },
                    MetTagValue::Float32(f) => writer.write_all(&f.to_le_bytes())?,
                    MetTagValue::Hash16(h) => writer.write_all(&h.0)?,
                    MetTagValue::Blob(b) | MetTagValue::Bsob(b) => {
                        writer.write_all(&(b.len() as u32).to_le_bytes())?;
                        writer.write_all(b)?;
                    }
                    MetTagValue::Bool(b) => writer.write_all(&[if *b { 1 } else { 0 }])?,
                }
            }
        }

        Ok(())
    }
}
