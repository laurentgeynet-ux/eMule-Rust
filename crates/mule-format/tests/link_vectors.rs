// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

use mule_format::{Ed2kFileLink, Ed2kHash, FormatError, PeerSource};

#[test]
fn test_parse_minimal_link() {
    let link_str = "ed2k://|file|test_file.iso|1048576|31d6cfe0d16ae931b73c59d7e0c089c0|/";
    let link = Ed2kFileLink::parse(link_str).unwrap();

    assert_eq!(link.file_name, "test_file.iso");
    assert_eq!(link.size, 1048576);
    assert_eq!(
        link.ed2k_hash,
        Ed2kHash::from_hex("31d6cfe0d16ae931b73c59d7e0c089c0").unwrap()
    );
    assert!(link.aich_root.is_none());
    assert!(link.peer_sources.is_empty());
}

#[test]
fn test_parse_link_with_url_encoding_and_spaces() {
    let link_str = "ed2k://|file|My%20Cool%20File%20%C3%A9t%C3%A9.mkv|2048000|0123456789abcdef0123456789abcdef|/";
    let link = Ed2kFileLink::parse(link_str).unwrap();

    assert_eq!(link.file_name, "My Cool File été.mkv");
    assert_eq!(link.size, 2048000);
}

#[test]
fn test_parse_link_with_aich_and_sources() {
    // 32 chars base32 = 20 bytes SHA1
    let link_str = "ed2k://|file|sample.dat|9728000|0123456789abcdef0123456789abcdef|h=OBTXIKUQC6F7E4424IWSU5V5MOMN2WZA|/|sources,192.168.1.50:39735,node.network.org:39735|/";
    let link = Ed2kFileLink::parse(link_str).unwrap();

    assert_eq!(link.file_name, "sample.dat");
    assert_eq!(link.size, 9728000);
    assert!(link.aich_root.is_some());

    let aich_b32 = data_encoding::BASE32_NOPAD.encode(&link.aich_root.unwrap());
    assert_eq!(aich_b32, "OBTXIKUQC6F7E4424IWSU5V5MOMN2WZA");

    assert_eq!(link.peer_sources.len(), 2);
    assert_eq!(link.peer_sources[0], PeerSource::new("192.168.1.50", 39735));
    assert_eq!(
        link.peer_sources[1],
        PeerSource::new("node.network.org", 39735)
    );
}

#[test]
fn test_parse_link_with_web_sources() {
    let link_str = "ed2k://|file|doc.pdf|5000|0123456789abcdef0123456789abcdef|s=http://example.com/downloads/doc.pdf|/";
    let link = Ed2kFileLink::parse(link_str).unwrap();

    assert_eq!(link.web_sources.len(), 1);
    assert_eq!(
        link.web_sources[0].as_str(),
        "http://example.com/downloads/doc.pdf"
    );
}

#[test]
fn test_malformed_links_rejected() {
    // Missing ed2k://
    assert!(matches!(
        Ed2kFileLink::parse("http://|file|test|10|0123456789abcdef0123456789abcdef|/").unwrap_err(),
        FormatError::InvalidLink(_)
    ));

    // Invalid hash length
    assert!(matches!(
        Ed2kFileLink::parse("ed2k://|file|test|10|invalid_hash|/").unwrap_err(),
        FormatError::InvalidEd2kHashString(_)
    ));

    // Empty file name
    assert!(matches!(
        Ed2kFileLink::parse("ed2k://|file||10|0123456789abcdef0123456789abcdef|/").unwrap_err(),
        FormatError::InvalidLink(_)
    ));
}
