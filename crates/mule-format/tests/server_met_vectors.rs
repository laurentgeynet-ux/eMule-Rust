// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use mule_format::met::tags::*;
use mule_format::{MetError, ServerEntry, ServerMetFile, SERVER_MET_VERSION};
use std::io::Cursor;
use std::net::Ipv4Addr;

#[test]
fn test_nominal_server_met_roundtrip() {
    let mut entry1 = ServerEntry::new(Ipv4Addr::new(195, 154, 122, 10), 4661, "eMule Security No1");
    entry1.description = "Primary secure server".to_string();
    entry1.ping = 45;
    entry1.preference = 1;
    entry1.tags.push(Tag {
        name_id: SN_NAME,
        name: String::new(),
        value: TagValue::String("eMule Security No1".to_string()),
    });
    entry1.tags.push(Tag {
        name_id: SN_DESCRIPTION,
        name: String::new(),
        value: TagValue::String("Primary secure server".to_string()),
    });
    entry1.tags.push(Tag {
        name_id: SN_PING,
        name: String::new(),
        value: TagValue::Uint64(45),
    });

    let mut entry2 = ServerEntry::new(Ipv4Addr::new(212, 83, 150, 5), 4242, "Peerates.net");
    entry2.tags.push(Tag {
        name_id: SN_NAME,
        name: String::new(),
        value: TagValue::String("Peerates.net".to_string()),
    });

    let file = ServerMetFile {
        version: SERVER_MET_VERSION,
        servers: vec![entry1, entry2],
    };

    let mut buf = Vec::new();
    file.write_to(&mut buf).unwrap();

    let parsed = ServerMetFile::read_from(&mut Cursor::new(&buf)).unwrap();
    assert_eq!(parsed.version, SERVER_MET_VERSION);
    assert_eq!(parsed.servers.len(), 2);

    let s1 = &parsed.servers[0];
    assert_eq!(s1.ip, Ipv4Addr::new(195, 154, 122, 10));
    assert_eq!(s1.port, 4661);
    assert_eq!(s1.name, "eMule Security No1");
    assert_eq!(s1.description, "Primary secure server");
    assert_eq!(s1.ping, 45);

    let s2 = &parsed.servers[1];
    assert_eq!(s2.ip, Ipv4Addr::new(212, 83, 150, 5));
    assert_eq!(s2.port, 4242);
    assert_eq!(s2.name, "Peerates.net");
}

#[test]
fn test_unsupported_server_met_header() {
    let buf = vec![0x11, 0x00, 0x00, 0x00, 0x00];
    let err = ServerMetFile::read_from(&mut Cursor::new(&buf)).unwrap_err();
    assert!(matches!(
        err,
        MetError::UnsupportedHeader {
            offset: 0,
            value: 0x11
        }
    ));
}

#[test]
fn test_server_met_oversized_count_rejected() {
    // Version 0xE0, server count 20_000 (exceeds 10_000)
    let mut buf = vec![0xE0];
    buf.extend_from_slice(&20_000u32.to_le_bytes());
    let err = ServerMetFile::read_from(&mut Cursor::new(&buf)).unwrap_err();
    assert!(matches!(
        err,
        MetError::InvalidLength {
            declared: 20_000,
            maximum: 10_000,
            ..
        }
    ));
}

#[test]
fn test_server_met_unexpected_eof_truncated() {
    // Valid header and count 1, but no data follows
    let mut buf = vec![0xE0];
    buf.extend_from_slice(&1u32.to_le_bytes());
    let err = ServerMetFile::read_from(&mut Cursor::new(&buf)).unwrap_err();
    assert!(matches!(err, MetError::UnexpectedEof { .. }));
}
