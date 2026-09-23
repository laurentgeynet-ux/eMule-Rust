// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use crate::error::FormatError;
use crate::met::error::MetError;
use crate::met::reader::CountingReader;
use crate::met::tags::Tag;
use std::io::{Read, Write};
use std::net::Ipv4Addr;

/// Maximum size allowed for a nodes.dat file to prevent DoS (5 MiB).
pub const MAX_NODES_DAT_SIZE: usize = 5 * 1024 * 1024;
/// Maximum number of contacts allowed per nodes.dat file.
pub const MAX_NODES_COUNT: usize = 10_000;
/// Maximum tags allowed per contact entry.
pub const MAX_TAGS_PER_NODE: usize = 50;

#[derive(Debug, Clone, PartialEq)]
pub struct KadContactEntry {
    pub client_id: [u8; 16],
    pub ip: Ipv4Addr,
    pub udp_port: u16,
    pub tcp_port: u16,
    pub version: u8,
    pub kad_udp_key: Option<(u32, u32)>,
    pub verified: bool,
    pub tags: Vec<Tag>,
}

impl KadContactEntry {
    pub fn new(
        client_id: [u8; 16],
        ip: Ipv4Addr,
        udp_port: u16,
        tcp_port: u16,
        version: u8,
    ) -> Self {
        Self {
            client_id,
            ip,
            udp_port,
            tcp_port,
            version,
            kad_udp_key: None,
            verified: false,
            tags: Vec::new(),
        }
    }

    pub fn with_kad_udp_key(mut self, key: u32, ip: u32) -> Self {
        self.kad_udp_key = Some((key, ip));
        self
    }

    pub fn with_verified(mut self, verified: bool) -> Self {
        self.verified = verified;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodesDatFile {
    pub version: u32,
    pub contacts: Vec<KadContactEntry>,
}

impl NodesDatFile {
    pub fn new(version: u32, contacts: Vec<KadContactEntry>) -> Self {
        Self { version, contacts }
    }

    pub fn parse<R: Read>(reader: R) -> Result<Self, FormatError> {
        let mut cr = CountingReader::new(reader);
        let first_u32 = cr.read_u32_le("nodes.dat header count/version")?;

        let (file_version, contact_count) = if first_u32 == 0 {
            let ver = cr.read_u32_le("nodes.dat Kad2 version")?;
            let cnt = cr.read_u32_le("nodes.dat Kad2 count")?;
            (ver, cnt)
        } else if first_u32 <= (MAX_NODES_COUNT as u32) {
            (0, first_u32)
        } else {
            return Err(FormatError::Met(MetError::InconsistentMetadata {
                field: "nodes.dat count",
                reason: format!("Header count {} exceeds maximum", first_u32),
            }));
        };

        if contact_count as usize > MAX_NODES_COUNT {
            return Err(FormatError::Met(MetError::InvalidLength {
                offset: cr.offset(),
                declared: contact_count as u64,
                maximum: MAX_NODES_COUNT as u64,
                context: "nodes.dat contact count",
            }));
        }

        let mut contacts = Vec::with_capacity(contact_count as usize);

        for _ in 0..contact_count {
            let client_id_bytes = cr.read_exact_bytes(16, "node client_id")?;
            let mut client_id = [0u8; 16];
            client_id.copy_from_slice(&client_id_bytes);

            let ip_raw = cr.read_u32_le("node ip")?;
            let ip = Ipv4Addr::from(ip_raw);

            let udp_port = cr.read_u16_le("node udp_port")?;
            let tcp_port = cr.read_u16_le("node tcp_port")?;

            let version = if file_version >= 1 {
                cr.read_u8("node version")?
            } else {
                0
            };

            let (kad_udp_key, verified) = if file_version >= 2 {
                let key = cr.read_u32_le("node kad_udp_key")?;
                let ip_key = cr.read_u32_le("node kad_udp_ip")?;
                let verified_byte = cr.read_u8("node verified")?;
                (Some((key, ip_key)), verified_byte != 0)
            } else {
                (None, false)
            };

            contacts.push(KadContactEntry {
                client_id,
                ip,
                udp_port,
                tcp_port,
                version,
                kad_udp_key,
                verified,
                tags: Vec::new(),
            });
        }

        Ok(Self {
            version: file_version,
            contacts,
        })
    }

    pub fn write<W: Write>(&self, mut writer: W) -> Result<(), FormatError> {
        if self.contacts.len() > MAX_NODES_COUNT {
            return Err(FormatError::Met(MetError::InvalidLength {
                offset: 0,
                declared: self.contacts.len() as u64,
                maximum: MAX_NODES_COUNT as u64,
                context: "nodes.dat write contact count",
            }));
        }

        if self.version >= 2 {
            writer.write_all(&0u32.to_le_bytes())?;
            writer.write_all(&self.version.to_le_bytes())?;
            writer.write_all(&(self.contacts.len() as u32).to_le_bytes())?;
        } else if self.version == 1 {
            writer.write_all(&0u32.to_le_bytes())?;
            writer.write_all(&1u32.to_le_bytes())?;
            writer.write_all(&(self.contacts.len() as u32).to_le_bytes())?;
        } else {
            writer.write_all(&(self.contacts.len() as u32).to_le_bytes())?;
        }

        for contact in &self.contacts {
            writer.write_all(&contact.client_id)?;
            let ip_u32 = u32::from(contact.ip);
            writer.write_all(&ip_u32.to_le_bytes())?;
            writer.write_all(&contact.udp_port.to_le_bytes())?;
            writer.write_all(&contact.tcp_port.to_le_bytes())?;

            if self.version >= 1 {
                writer.write_all(&[contact.version])?;
            }

            if self.version >= 2 {
                let (key, ip_key) = contact.kad_udp_key.unwrap_or((0, 0));
                writer.write_all(&key.to_le_bytes())?;
                writer.write_all(&ip_key.to_le_bytes())?;
                writer.write_all(&[if contact.verified { 1 } else { 0 }])?;
            }
        }

        writer.flush()?;
        Ok(())
    }
}
