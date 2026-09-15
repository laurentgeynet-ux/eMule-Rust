use std::io::Cursor;
use std::net::Ipv4Addr;
use mule_format::kad::{KadContactEntry, NodesDatFile, MAX_NODES_COUNT};
use mule_format::met::tags::{Tag, TagValue};

#[test]
fn test_nodes_dat_v0_roundtrip() {
    let contacts = vec![
        KadContactEntry::new([1u8; 16], Ipv4Addr::new(192, 168, 1, 10), 4672, 4662, 0),
        KadContactEntry::new([2u8; 16], Ipv4Addr::new(80, 80, 80, 80), 1234, 5678, 0),
    ];
    let file = NodesDatFile::new(0, contacts.clone());

    let mut buf = Vec::new();
    file.write(&mut buf).unwrap();

    let parsed = NodesDatFile::parse(Cursor::new(buf)).unwrap();
    assert_eq!(parsed.version, 0);
    assert_eq!(parsed.contacts.len(), 2);
    assert_eq!(parsed.contacts[0].client_id, [1u8; 16]);
    assert_eq!(parsed.contacts[0].ip, Ipv4Addr::new(192, 168, 1, 10));
    assert_eq!(parsed.contacts[0].udp_port, 4672);
    assert_eq!(parsed.contacts[0].tcp_port, 4662);
    assert_eq!(parsed.contacts[1].client_id, [2u8; 16]);
    assert_eq!(parsed.contacts[1].ip, Ipv4Addr::new(80, 80, 80, 80));
}

#[test]
fn test_nodes_dat_v2_with_tags_roundtrip() {
    let mut contact = KadContactEntry::new([0xAA; 16], Ipv4Addr::new(1, 2, 3, 4), 4000, 4001, 2);
    contact.tags.push(Tag::new_id(0x01, TagValue::String("eMule v0.50a".into())));

    let file = NodesDatFile::new(2, vec![contact]);

    let mut buf = Vec::new();
    file.write(&mut buf).unwrap();

    let parsed = NodesDatFile::parse(Cursor::new(buf)).unwrap();
    assert_eq!(parsed.version, 2);
    assert_eq!(parsed.contacts.len(), 1);
    assert_eq!(parsed.contacts[0].client_id, [0xAA; 16]);
    assert_eq!(parsed.contacts[0].ip, Ipv4Addr::new(1, 2, 3, 4));
    assert_eq!(parsed.contacts[0].version, 2);
    assert_eq!(parsed.contacts[0].tags.len(), 1);
    assert_eq!(parsed.contacts[0].tags[0].name_id, 0x01);
    assert_eq!(parsed.contacts[0].tags[0].as_str(), Some("eMule v0.50a"));
}

#[test]
fn test_nodes_dat_corrupted_header_fails_gracefully() {
    let raw = vec![0u8; 3]; // too short for count
    let res = NodesDatFile::parse(Cursor::new(raw));
    assert!(res.is_err());
}

#[test]
fn test_nodes_dat_oversized_count_rejected() {
    let mut buf = Vec::new();
    let excessive_count = (MAX_NODES_COUNT + 100) as u32;
    // v2 header with excessive count
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&2u32.to_le_bytes());
    buf.extend_from_slice(&excessive_count.to_le_bytes());

    let res = NodesDatFile::parse(Cursor::new(buf));
    assert!(res.is_err());
}
