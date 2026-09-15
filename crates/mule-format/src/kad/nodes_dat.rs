use std::io::{Read, Write};
use std::net::Ipv4Addr;
use crate::error::FormatError;
use crate::met::error::MetError;
use crate::met::reader::CountingReader;
use crate::met::tags::{
    Tag, TagValue, TAGTYPE_BLOB, TAGTYPE_BOOL, TAGTYPE_BSOB, TAGTYPE_FLOAT32, TAGTYPE_HASH16,
    TAGTYPE_STRING, TAGTYPE_UINT16, TAGTYPE_UINT32, TAGTYPE_UINT64, TAGTYPE_UINT8,
};

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
            tags: Vec::new(),
        }
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
            let ip = Ipv4Addr::from(ip_raw.to_be());

            let udp_port = cr.read_u16_le("node udp_port")?;
            let tcp_port = cr.read_u16_le("node tcp_port")?;

            let version = if file_version >= 1 {
                cr.read_u8("node version")?
            } else {
                0
            };

            let mut tags = Vec::new();
            if file_version >= 2 {
                if let Ok(tag_count) = cr.read_u32_le("node tag count") {
                    let tag_count = tag_count as usize;
                    if tag_count > MAX_TAGS_PER_NODE {
                        return Err(FormatError::Met(MetError::InvalidLength {
                            offset: cr.offset(),
                            declared: tag_count as u64,
                            maximum: MAX_TAGS_PER_NODE as u64,
                            context: "node tag count",
                        }));
                    }
                    for _ in 0..tag_count {
                        let tag = cr.read_tag()?;
                        tags.push(tag);
                    }
                }
            }

            contacts.push(KadContactEntry {
                client_id,
                ip,
                udp_port,
                tcp_port,
                version,
                tags,
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
            let ip_u32 = u32::from(contact.ip).to_be();
            writer.write_all(&ip_u32.to_le_bytes())?;
            writer.write_all(&contact.udp_port.to_le_bytes())?;
            writer.write_all(&contact.tcp_port.to_le_bytes())?;

            if self.version >= 1 {
                writer.write_all(&[contact.version])?;
            }

            if self.version >= 2 {
                writer.write_all(&(contact.tags.len() as u32).to_le_bytes())?;
                for tag in &contact.tags {
                    write_tag(&mut writer, tag)?;
                }
            }
        }

        writer.flush()?;
        Ok(())
    }
}

fn write_tag<W: Write>(writer: &mut W, tag: &Tag) -> Result<(), FormatError> {
    match &tag.value {
        TagValue::String(s) => {
            if tag.name_id != 0 {
                writer.write_all(&[TAGTYPE_STRING | 0x80])?;
                writer.write_all(&[tag.name_id])?;
            } else {
                writer.write_all(&[TAGTYPE_STRING])?;
                let name_bytes = tag.name.as_bytes();
                writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
                writer.write_all(name_bytes)?;
            }
            let s_bytes = s.as_bytes();
            writer.write_all(&(s_bytes.len() as u16).to_le_bytes())?;
            writer.write_all(s_bytes)?;
        }
        TagValue::Uint64(v) => {
            let (raw_type, val_bytes): (u8, Vec<u8>) = if *v <= u8::MAX as u64 {
                (TAGTYPE_UINT8, vec![*v as u8])
            } else if *v <= u16::MAX as u64 {
                (TAGTYPE_UINT16, (*v as u16).to_le_bytes().to_vec())
            } else if *v <= u32::MAX as u64 {
                (TAGTYPE_UINT32, (*v as u32).to_le_bytes().to_vec())
            } else {
                (TAGTYPE_UINT64, v.to_le_bytes().to_vec())
            };

            if tag.name_id != 0 {
                writer.write_all(&[raw_type | 0x80])?;
                writer.write_all(&[tag.name_id])?;
            } else {
                writer.write_all(&[raw_type])?;
                let name_bytes = tag.name.as_bytes();
                writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
                writer.write_all(name_bytes)?;
            }
            writer.write_all(&val_bytes)?;
        }
        TagValue::Float32(f) => {
            if tag.name_id != 0 {
                writer.write_all(&[TAGTYPE_FLOAT32 | 0x80])?;
                writer.write_all(&[tag.name_id])?;
            } else {
                writer.write_all(&[TAGTYPE_FLOAT32])?;
                let name_bytes = tag.name.as_bytes();
                writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
                writer.write_all(name_bytes)?;
            }
            writer.write_all(&f.to_le_bytes())?;
        }
        TagValue::Bool(b) => {
            if tag.name_id != 0 {
                writer.write_all(&[TAGTYPE_BOOL | 0x80])?;
                writer.write_all(&[tag.name_id])?;
            } else {
                writer.write_all(&[TAGTYPE_BOOL])?;
                let name_bytes = tag.name.as_bytes();
                writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
                writer.write_all(name_bytes)?;
            }
            writer.write_all(&[if *b { 1 } else { 0 }])?;
        }
        TagValue::Hash16(h) => {
            if tag.name_id != 0 {
                writer.write_all(&[TAGTYPE_HASH16 | 0x80])?;
                writer.write_all(&[tag.name_id])?;
            } else {
                writer.write_all(&[TAGTYPE_HASH16])?;
                let name_bytes = tag.name.as_bytes();
                writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
                writer.write_all(name_bytes)?;
            }
            writer.write_all(&h.0)?;
        }
        TagValue::Blob(b) => {
            if tag.name_id != 0 {
                writer.write_all(&[TAGTYPE_BLOB | 0x80])?;
                writer.write_all(&[tag.name_id])?;
            } else {
                writer.write_all(&[TAGTYPE_BLOB])?;
                let name_bytes = tag.name.as_bytes();
                writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
                writer.write_all(name_bytes)?;
            }
            writer.write_all(&(b.len() as u32).to_le_bytes())?;
            writer.write_all(b)?;
        }
        TagValue::Bsob(b) => {
            if tag.name_id != 0 {
                writer.write_all(&[TAGTYPE_BSOB | 0x80])?;
                writer.write_all(&[tag.name_id])?;
            } else {
                writer.write_all(&[TAGTYPE_BSOB])?;
                let name_bytes = tag.name.as_bytes();
                writer.write_all(&(name_bytes.len() as u16).to_le_bytes())?;
                writer.write_all(name_bytes)?;
            }
            writer.write_all(&[b.len() as u8])?;
            writer.write_all(b)?;
        }
    }
    Ok(())
}
