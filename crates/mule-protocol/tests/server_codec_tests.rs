// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use mule_protocol::ed2k::{
    decode_server_response, Ed2kLimits, GetSourcesRequest, SearchQuery, SearchResult,
    ServerIdChange, ServerLoginRequest, ServerResponse, ServerStatus, OP_FOUNDSOURCES,
    OP_FOUNDSOURCES_OBFU, OP_GETSOURCES, OP_GETSOURCES_OBFU, OP_IDCHANGE, OP_LOGINREQUEST,
    OP_SEARCHREQUEST, OP_SEARCH_RESULT, OP_SERVERMESSAGE, OP_SERVERSTATUS, TAG_SPECIAL_FILESIZE,
    TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
};
use mule_protocol::PR_ED2K;

#[test]
fn test_encode_server_login_request() {
    let limits = Ed2kLimits::default();
    let user_hash = [1u8; 16];
    let req = ServerLoginRequest::new(user_hash, 39735, "TestUser", 0x3c);
    let packet = req.encode(&limits).expect("encode login request");

    assert_eq!(packet.protocol, PR_ED2K);
    assert_eq!(packet.opcode, OP_LOGINREQUEST);
    // payload starts with user_hash
    assert_eq!(&packet.payload[0..16], &user_hash);
}

/// Vecteur filaire eMule 0.72a (`ServerConnect.cpp:170-198`) : le login
/// contient exactement 4 tags dans l'ordre `CT_NAME`, `CT_VERSION`,
/// `CT_SERVER_FLAGS`, `CT_EMULE_VERSION` ; les flags de base sont
/// `SRVCAP_ZLIB | SRVCAP_NEWTAGS | SRVCAP_LARGEFILES | SRVCAP_UNICODE` et la
/// version eMule est encodée `(mjr << 17) | (min << 10) | (upd << 7)`.
#[test]
fn test_encode_server_login_request_emule_wire_layout() {
    use mule_protocol::ed2k::tags::{
        SRVCAP_LARGEFILES, SRVCAP_NEWTAGS, SRVCAP_REQUESTCRYPT, SRVCAP_SUPPORTCRYPT,
        SRVCAP_UNICODE, SRVCAP_ZLIB,
    };
    use mule_protocol::ed2k::ANNOUNCED_EMULE_VERSION_TAG;

    let limits = Ed2kLimits::default();
    let user_hash = [0xABu8; 16];
    let req = ServerLoginRequest::new(user_hash, 4662, "Nick", 0x3c);
    let packet = req.encode(&limits).expect("encode login");
    let p = &packet.payload;

    // user_hash + client_id + tcp_port + tagcount.
    assert_eq!(&p[0..16], &user_hash);
    assert_eq!(&p[16..20], &0u32.to_le_bytes());
    assert_eq!(&p[20..22], &4662u16.to_le_bytes());
    assert_eq!(&p[22..26], &4u32.to_le_bytes());

    let mut pos = 26;
    // Tag 1 — CT_NAME (string, id 0x01).
    assert_eq!(&p[pos..pos + 4], &[0x02, 0x01, 0x00, 0x01]);
    pos += 4;
    let name_len = u16::from_le_bytes([p[pos], p[pos + 1]]) as usize;
    pos += 2;
    assert_eq!(&p[pos..pos + name_len], b"Nick");
    pos += name_len;
    // Tag 2 — CT_VERSION (uint32, id 0x11) = EDONKEYVERSION 0x3C.
    assert_eq!(&p[pos..pos + 4], &[0x03, 0x01, 0x00, 0x11]);
    pos += 4;
    assert_eq!(&p[pos..pos + 4], &0x3cu32.to_le_bytes());
    pos += 4;
    // Tag 3 — CT_SERVER_FLAGS (uint32, id 0x20).
    assert_eq!(&p[pos..pos + 4], &[0x03, 0x01, 0x00, 0x20]);
    pos += 4;
    let flags = u32::from_le_bytes([p[pos], p[pos + 1], p[pos + 2], p[pos + 3]]);
    assert_eq!(flags, SRVCAP_ZLIB | SRVCAP_NEWTAGS | SRVCAP_LARGEFILES | SRVCAP_UNICODE);
    pos += 4;
    // Tag 4 — CT_EMULE_VERSION (uint32, id 0xFB) = 0.70a eMule packing.
    assert_eq!(&p[pos..pos + 4], &[0x03, 0x01, 0x00, 0xFB]);
    pos += 4;
    assert_eq!(&p[pos..pos + 4], &ANNOUNCED_EMULE_VERSION_TAG.to_le_bytes());
    pos += 4;
    assert_eq!(pos, p.len(), "no trailing bytes after the 4 login tags");

    // `with_server_flags` OR-merges crypt capabilities into tag 3.
    let req_crypt = ServerLoginRequest::new(user_hash, 4662, "Nick", 0x3c)
        .with_server_flags(SRVCAP_SUPPORTCRYPT | SRVCAP_REQUESTCRYPT);
    let packet = req_crypt.encode(&limits).expect("encode login crypt");
    let p = &packet.payload;
    // tag1 = 4 hdr + 2 len + 4 "Nick" = 10 ; tag2 = 4 hdr + 4 = 8 ;
    // tag3 header at 26+10+8, value 4 bytes later.
    let base = 26 + 10 + 8 + 4;
    let flags = u32::from_le_bytes([p[base], p[base + 1], p[base + 2], p[base + 3]]);
    assert_eq!(
        flags,
        SRVCAP_ZLIB | SRVCAP_NEWTAGS | SRVCAP_LARGEFILES | SRVCAP_UNICODE
            | SRVCAP_SUPPORTCRYPT | SRVCAP_REQUESTCRYPT
    );
}

#[test]
fn test_decode_server_reject() {
    use mule_protocol::ed2k::OP_REJECT;
    let limits = Ed2kLimits::default();
    // eMule `OP_REJECT` — payload `(null)`; any bytes present are ignored.
    let resp = decode_server_response(PR_ED2K, OP_REJECT, &[], &limits).expect("decode OP_REJECT");
    assert!(matches!(resp, ServerResponse::Rejected));
    let resp = decode_server_response(PR_ED2K, OP_REJECT, &[0xAA, 0xBB], &limits)
        .expect("decode OP_REJECT with stray payload");
    assert!(matches!(resp, ServerResponse::Rejected));
}

#[test]
fn test_decode_server_message() {
    let limits = Ed2kLimits::default();
    let msg_str = "Welcome to eD2K Server!";
    let mut payload = Vec::new();
    payload.extend_from_slice(&(msg_str.len() as u16).to_le_bytes());
    payload.extend_from_slice(msg_str.as_bytes());

    let resp = decode_server_response(PR_ED2K, OP_SERVERMESSAGE, &payload, &limits)
        .expect("decode server message");
    match resp {
        ServerResponse::Message(msg) => {
            assert_eq!(msg.message, msg_str);
        }
        _ => panic!("unexpected response type"),
    }
}

#[test]
fn test_decode_server_id_change() {
    let limits = Ed2kLimits::default();

    // HighID: >= 16_777_216
    let mut payload_high = Vec::new();
    payload_high.extend_from_slice(&20_000_000u32.to_le_bytes());
    payload_high.extend_from_slice(&1u32.to_le_bytes());

    let resp = decode_server_response(PR_ED2K, OP_IDCHANGE, &payload_high, &limits)
        .expect("decode HighID");
    if let ServerResponse::IdChange(ServerIdChange {
        client_id,
        tcp_flags,
    }) = resp
    {
        assert_eq!(client_id, 20_000_000);
        assert_eq!(tcp_flags, 1);
        assert!(resp_is_high_id(client_id));
    } else {
        panic!("unexpected response type");
    }

    // LowID: < 16_777_216
    let mut payload_low = Vec::new();
    payload_low.extend_from_slice(&500_000u32.to_le_bytes());
    payload_low.extend_from_slice(&0u32.to_le_bytes());

    let resp_low =
        decode_server_response(PR_ED2K, OP_IDCHANGE, &payload_low, &limits).expect("decode LowID");
    if let ServerResponse::IdChange(ServerIdChange { client_id, .. }) = resp_low {
        assert_eq!(client_id, 500_000);
        assert!(!resp_is_high_id(client_id));
    } else {
        panic!("unexpected response type");
    }
}

fn resp_is_high_id(client_id: u32) -> bool {
    client_id >= 16_777_216
}

#[test]
fn test_decode_server_status() {
    let limits = Ed2kLimits::default();
    let mut payload = Vec::new();
    payload.extend_from_slice(&12345u32.to_le_bytes());
    payload.extend_from_slice(&67890u32.to_le_bytes());

    let resp = decode_server_response(PR_ED2K, OP_SERVERSTATUS, &payload, &limits)
        .expect("decode server status");
    if let ServerResponse::Status(ServerStatus {
        current_users,
        current_files,
    }) = resp
    {
        assert_eq!(current_users, 12345);
        assert_eq!(current_files, 67890);
    } else {
        panic!("unexpected response type");
    }
}

#[test]
fn test_encode_search_query() {
    let limits = Ed2kLimits::default();

    // Simple search (no filters): starts with 0x01 (NAME), u16 len, then term
    let simple_query = SearchQuery::new("windows");
    let packet_simple = simple_query
        .encode(&limits)
        .expect("encode simple search query");
    assert_eq!(packet_simple.protocol, PR_ED2K);
    assert_eq!(packet_simple.opcode, OP_SEARCHREQUEST);
    assert_eq!(packet_simple.payload[0], 0x01);
    let term_len =
        u16::from_le_bytes([packet_simple.payload[1], packet_simple.payload[2]]) as usize;
    assert_eq!(term_len, 7);
    assert_eq!(&packet_simple.payload[3..3 + term_len], b"windows");

    // Filtered search: Query Tree binary operators 0x00, 0x00 (AND)
    let mut query = SearchQuery::new("debian");
    query.min_size = Some(1024);
    query.extension = Some("iso".to_string());

    let packet = query.encode(&limits).expect("encode search query");
    assert_eq!(packet.protocol, PR_ED2K);
    assert_eq!(packet.opcode, OP_SEARCHREQUEST);
    assert_eq!(packet.payload[0], 0x00); // BOOL
    assert_eq!(packet.payload[1], 0x00); // AND
    assert!(packet.payload.len() > 15);
}

#[test]
fn test_decode_search_result() {
    let limits = Ed2kLimits::default();
    let mut payload = Vec::new();
    // 1 file matched
    payload.extend_from_slice(&1u32.to_le_bytes());

    // File 1:
    let file_hash = [42u8; 16];
    payload.extend_from_slice(&file_hash);
    payload.extend_from_slice(&100u32.to_le_bytes()); // client id
    payload.extend_from_slice(&39735u16.to_le_bytes()); // client port

    // 2 tags: name and size
    payload.extend_from_slice(&2u32.to_le_bytes()); // tag count

    // Tag 1: Name (String)
    let filename = "ubuntu-24.04.iso";
    payload.push(0x02); // TAGTYPE_STRING
    payload.extend_from_slice(&1u16.to_le_bytes()); // name len = 1 (special)
    payload.push(TAG_SPECIAL_NAME);
    payload.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    payload.extend_from_slice(filename.as_bytes());

    // Tag 2: Size (UInt32)
    payload.push(0x03); // TAGTYPE_UINT32
    payload.extend_from_slice(&1u16.to_le_bytes()); // name len = 1 (special)
    payload.push(TAG_SPECIAL_FILESIZE);
    payload.extend_from_slice(&2_000_000_000u32.to_le_bytes());

    let resp = decode_server_response(PR_ED2K, OP_SEARCH_RESULT, &payload, &limits)
        .expect("decode search result");

    if let ServerResponse::SearchResult(SearchResult { files }) = resp {
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_hash, file_hash);
        assert_eq!(files[0].name().as_deref(), Some("ubuntu-24.04.iso"));
        assert_eq!(files[0].size(), Some(2_000_000_000));
    } else {
        panic!("unexpected response type");
    }
}

#[test]
fn test_encode_get_sources_and_decode_found_sources() {
    let limits = Ed2kLimits::default();
    let file_hash = [99u8; 16];
    let req = GetSourcesRequest::new(file_hash, 50_000_000);
    let packet = req.encode(&limits).expect("encode get sources");

    assert_eq!(packet.protocol, PR_ED2K);
    assert_eq!(packet.opcode, OP_GETSOURCES);
    assert_eq!(&packet.payload[0..16], &file_hash);

    // Mock found sources response
    let mut payload = Vec::new();
    payload.extend_from_slice(&file_hash);
    payload.push(2u8); // 2 sources

    // Source 1: 192.168.1.10:39735 (little endian IP: 10, 1, 168, 192)
    let ip1: u32 = 192 | (168 << 8) | (1 << 16) | (10 << 24);
    payload.extend_from_slice(&ip1.to_le_bytes());
    payload.extend_from_slice(&39735u16.to_le_bytes());

    // Source 2: 10.0.0.5:39735
    let ip2: u32 = 10 | (5 << 24);
    payload.extend_from_slice(&ip2.to_le_bytes());
    payload.extend_from_slice(&39737u16.to_le_bytes());

    let resp = decode_server_response(PR_ED2K, OP_FOUNDSOURCES, &payload, &limits)
        .expect("decode found sources");

    match resp {
        ServerResponse::FoundSources(fs) => {
            assert_eq!(fs.file_hash, file_hash);
            assert_eq!(fs.sources.len(), 2);
            assert_eq!(fs.sources[0].to_address_string(), "192.168.1.10:39735");
            assert_eq!(fs.sources[1].to_address_string(), "10.0.0.5:39737");
        }
        _ => panic!("unexpected response type"),
    }
}

#[test]
fn test_encode_get_sources_obfu() {
    let limits = Ed2kLimits::default();
    let file_hash = [0x77u8; 16];
    let req = GetSourcesRequest::with_obfuscation(file_hash, 100_000_000, true);
    let packet = req.encode(&limits).expect("encode get sources obfu");

    assert_eq!(packet.protocol, PR_ED2K);
    assert_eq!(packet.opcode, OP_GETSOURCES_OBFU);
    assert_eq!(&packet.payload[0..16], &file_hash);
    assert_eq!(
        u32::from_le_bytes(packet.payload[16..20].try_into().unwrap()),
        100_000_000
    );
}

#[test]
fn test_decode_server_found_sources_obfu() {
    let limits = Ed2kLimits::default();
    let file_hash = [0xABu8; 16];
    let user_hash_peer1 = [0x55u8; 16];

    let mut payload = Vec::new();
    payload.extend_from_slice(&file_hash);
    payload.push(2); // 2 sources

    // Source 1: 87.221.131.17:4662, obf_settings: 0x83 (supports=1, requests=2, hash_present=0x80)
    let ip1: u32 = 87 | (221 << 8) | (131 << 16) | (17 << 24);
    payload.extend_from_slice(&ip1.to_le_bytes());
    payload.extend_from_slice(&4662u16.to_le_bytes());
    payload.push(0x83); // bit 0 (support), bit 1 (request), bit 7 (hash present)
    payload.extend_from_slice(&user_hash_peer1);

    // Source 2: 31.190.77.29:4662, obf_settings: 0x05 (supports=1, requires=4, no hash)
    let ip2: u32 = 31 | (190 << 8) | (77 << 16) | (29 << 24);
    payload.extend_from_slice(&ip2.to_le_bytes());
    payload.extend_from_slice(&4662u16.to_le_bytes());
    payload.push(0x05); // bit 0 (support), bit 2 (require), no hash

    let resp = decode_server_response(PR_ED2K, OP_FOUNDSOURCES_OBFU, &payload, &limits)
        .expect("decode found sources obfu");

    match resp {
        ServerResponse::FoundSourcesObfu(fs) => {
            assert_eq!(fs.file_hash, file_hash);
            assert_eq!(fs.sources.len(), 2);

            let s1 = &fs.sources[0];
            assert_eq!(s1.to_address_string(), "87.221.131.17:4662");
            assert!(s1.supports_crypt());
            assert!(s1.requests_crypt());
            assert!(!s1.requires_crypt());
            assert_eq!(s1.user_hash, Some(user_hash_peer1));

            let s2 = &fs.sources[1];
            assert_eq!(s2.to_address_string(), "31.190.77.29:4662");
            assert!(s2.supports_crypt());
            assert!(!s2.requests_crypt());
            assert!(s2.requires_crypt());
            assert_eq!(s2.user_hash, None);
        }
        _ => panic!("unexpected response type"),
    }
}

#[test]
fn test_decode_server_ident_with_compact_tags() {
    let limits = Ed2kLimits::default();
    let server_hash = [42u8; 16];
    let ip = 0x0100007Fu32;
    let port = 6082u16;

    let mut payload = Vec::new();
    payload.extend_from_slice(&server_hash);
    payload.extend_from_slice(&ip.to_le_bytes());
    payload.extend_from_slice(&port.to_le_bytes());
    payload.extend_from_slice(&2u32.to_le_bytes()); // 2 tags

    // Compact Tag 1: String name (type = TAGTYPE_STRING | 0x80 = 0x82, special ID = 0x01 TAG_SPECIAL_NAME)
    payload.push(0x82); // tag_type with 0x80 set
    payload.push(TAG_SPECIAL_NAME); // 1-byte special ID (no 16-bit length!)
    let server_name = "eDonkeyServer No1";
    payload.extend_from_slice(&(server_name.len() as u16).to_le_bytes());
    payload.extend_from_slice(server_name.as_bytes());

    // Compact Tag 2: Uint32 version (type = TAGTYPE_UINT32 | 0x80 = 0x83, special ID = 0x11 TAG_SPECIAL_VERSION)
    payload.push(0x83);
    payload.push(TAG_SPECIAL_VERSION);
    payload.extend_from_slice(&1740u32.to_le_bytes());

    let resp = decode_server_response(
        PR_ED2K,
        mule_protocol::ed2k::OP_SERVERIDENT,
        &payload,
        &limits,
    )
    .expect("decode server ident with compact tags");

    match resp {
        ServerResponse::Ident(ident) => {
            assert_eq!(ident.server_hash, server_hash);
            assert_eq!(ident.name().as_deref(), Some("eDonkeyServer No1"));
            assert_eq!(ident.tags.len(), 2);
        }
        _ => panic!("unexpected response type"),
    }
}

#[test]
fn test_decode_server_ident_with_str_fixed_tags() {
    let limits = Ed2kLimits::default();
    let server_hash = [42u8; 16];
    let ip = 0x0100007Fu32;
    let port = 6082u16;

    let mut payload = Vec::new();
    payload.extend_from_slice(&server_hash);
    payload.extend_from_slice(&ip.to_le_bytes());
    payload.extend_from_slice(&port.to_le_bytes());
    payload.extend_from_slice(&1u32.to_le_bytes()); // 1 tag

    // Tag 1: TAGTYPE_STR9 (0x19), Name = 0x01 (TAG_SPECIAL_NAME), value = "Lugdunum9" (9 bytes, no length prefix)
    payload.push(0x19); // TAGTYPE_STR9
    payload.extend_from_slice(&1u16.to_le_bytes()); // name_len = 1
    payload.push(TAG_SPECIAL_NAME); // special ID 0x01
    payload.extend_from_slice(b"Lugdunum9"); // exactly 9 bytes

    let resp = decode_server_response(
        PR_ED2K,
        mule_protocol::ed2k::OP_SERVERIDENT,
        &payload,
        &limits,
    )
    .expect("decode server ident with fixed str tag 0x19");

    match resp {
        ServerResponse::Ident(ident) => {
            assert_eq!(ident.server_hash, server_hash);
            assert_eq!(ident.name().as_deref(), Some("Lugdunum9"));
        }
        _ => panic!("unexpected response type"),
    }
}

// ── OP_OFFERFILES ────────────────────────────────────────────────────────────

#[test]
fn test_offer_files_wire_layout() {
    use mule_protocol::ed2k::{
        ed2k_file_type_term, OfferFiles, OfferedFile, TagName, TagValue, OP_OFFERFILES,
        SRV_TCPFLG_COMPRESSION, TAG_SPECIAL_FILETYPE,
    };
    let limits = Ed2kLimits::default();

    let hash = [0xABu8; 16];
    let of = OfferedFile::for_server(
        hash,
        "movie.avi",
        1_500_000,
        0,
        4662,
        SRV_TCPFLG_COMPRESSION,
        false,
    )
    .unwrap();
    // Compression-capable server: markers instead of real ID/port.
    assert_eq!(of.client_id, 0xFBFBFBFB);
    assert_eq!(of.client_port, 0xFBFB);
    assert_eq!(of.tags.len(), 3); // name + size + type

    let pkt = OfferFiles { files: vec![of] }.encode(&limits).unwrap();
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_OFFERFILES);
    let p = &pkt.payload;
    assert_eq!(&p[0..4], &1u32.to_le_bytes());
    assert_eq!(&p[4..20], &hash);
    assert_eq!(&p[20..24], &0xFBFBFBFBu32.to_le_bytes());
    assert_eq!(&p[24..26], &0xFBFBu16.to_le_bytes());
    assert_eq!(&p[26..30], &3u32.to_le_bytes()); // 3 tags

    assert_eq!(ed2k_file_type_term("movie.avi"), "Video");
    assert_eq!(ed2k_file_type_term("song.mp3"), "Audio");
    assert_eq!(ed2k_file_type_term("archive.rar"), "Pro");
    assert_eq!(ed2k_file_type_term("noext"), "");
    let _ = (TagName::Id(TAG_SPECIAL_FILETYPE), TagValue::UInt32(0));
}

#[test]
fn test_offer_files_highid_vs_lowid_and_large_file() {
    use mule_protocol::ed2k::{OfferedFile, SRV_TCPFLG_LARGEFILES};

    // HighID on a legacy server: real id+port published.
    let of = OfferedFile::for_server([1u8; 16], "a.bin", 100, 20_000_000, 4662, 0, false).unwrap();
    assert_eq!(of.client_id, 20_000_000);
    assert_eq!(of.client_port, 4662);

    // LowID on a legacy server: zeros.
    let of = OfferedFile::for_server([1u8; 16], "a.bin", 100, 5, 4662, 0, false).unwrap();
    assert_eq!((of.client_id, of.client_port), (0, 0));

    // Large file skipped without LARGEFILES flag, sent with split size tags else.
    let big = 5_000_000_000u64;
    assert!(OfferedFile::for_server([1u8; 16], "big.iso", big, 0, 0, 0, false).is_none());
    let of =
        OfferedFile::for_server([1u8; 16], "big.iso", big, 0, 0, SRV_TCPFLG_LARGEFILES, false)
            .unwrap();
    assert_eq!(of.tags.len(), 4); // name + size lo + size hi + type
}

#[test]
fn test_offer_files_partial_marker() {
    use mule_protocol::ed2k::{OfferedFile, SRV_TCPFLG_COMPRESSION};

    // eMule CreateOfferedFilePacket: incomplete (part) files are published
    // with the 0xFCFCFCFC/0xFCFC marker pair when the server supports TCP
    // compression; on legacy servers they carry the real HighID like
    // complete files.
    let partial = OfferedFile::for_server(
        [2u8; 16],
        "ongoing.bin",
        2_000_000,
        20_000_000,
        4662,
        SRV_TCPFLG_COMPRESSION,
        true,
    )
    .unwrap();
    assert_eq!(partial.client_id, 0xFCFCFCFC);
    assert_eq!(partial.client_port, 0xFCFC);

    let partial_legacy =
        OfferedFile::for_server([2u8; 16], "ongoing.bin", 2_000_000, 20_000_000, 4662, 0, true)
            .unwrap();
    assert_eq!(partial_legacy.client_id, 20_000_000);
    assert_eq!(partial_legacy.client_port, 4662);
}

#[test]
fn test_offer_files_empty_is_keepalive() {
    use mule_protocol::ed2k::{OfferFiles, OP_OFFERFILES};
    let pkt = OfferFiles { files: vec![] }
        .encode(&Ed2kLimits::default())
        .unwrap();
    assert_eq!(pkt.opcode, OP_OFFERFILES);
    assert_eq!(&pkt.payload[..], &0u32.to_le_bytes());
}

#[test]
fn test_callback_request_encodes_bare_client_id() {
    use mule_protocol::ed2k::{CallbackRequest, OP_CALLBACKREQUEST};
    let pkt = CallbackRequest {
        client_id: 0x00AB_CD12,
    }
    .encode();
    assert_eq!(pkt.protocol, PR_ED2K);
    assert_eq!(pkt.opcode, OP_CALLBACKREQUEST);
    assert_eq!(&pkt.payload[..], &0x00AB_CD12u32.to_le_bytes());
}

#[test]
fn test_decode_callback_requested_short_and_extended() {
    use mule_protocol::ed2k::{OP_CALLBACKREQUESTED, OP_CALLBACK_FAIL};
    let limits = Ed2kLimits::default();

    // Short form: <IP 4><PORT 2>
    let mut payload = Vec::new();
    payload.extend_from_slice(&0x0A0B0C0Du32.to_le_bytes());
    payload.extend_from_slice(&4662u16.to_le_bytes());
    let resp = decode_server_response(PR_ED2K, OP_CALLBACKREQUESTED, &payload, &limits).unwrap();
    match resp {
        ServerResponse::CallbackRequested(cb) => {
            assert_eq!(cb.ip, 0x0A0B0C0D);
            assert_eq!(cb.port, 4662);
            assert_eq!(cb.crypt_options, None);
            assert_eq!(cb.user_hash, None);
        }
        _ => panic!("unexpected response"),
    }

    // Extended form: + crypt_options u8 + user_hash[16]
    let hash = [0x42u8; 16];
    payload.push(0x03);
    payload.extend_from_slice(&hash);
    let resp = decode_server_response(PR_ED2K, OP_CALLBACKREQUESTED, &payload, &limits).unwrap();
    match resp {
        ServerResponse::CallbackRequested(cb) => {
            assert_eq!(cb.crypt_options, Some(0x03));
            assert_eq!(cb.user_hash, Some(hash));
        }
        _ => panic!("unexpected response"),
    }

    // Truncated
    assert!(decode_server_response(PR_ED2K, OP_CALLBACKREQUESTED, &[0u8; 5], &limits).is_err());

    // OP_CALLBACK_FAIL: empty and echoed-id forms
    let resp = decode_server_response(PR_ED2K, OP_CALLBACK_FAIL, &[], &limits).unwrap();
    assert!(matches!(
        resp,
        ServerResponse::CallbackFail { client_id: None }
    ));
    let resp =
        decode_server_response(PR_ED2K, OP_CALLBACK_FAIL, &1234u32.to_le_bytes(), &limits).unwrap();
    assert!(matches!(
        resp,
        ServerResponse::CallbackFail {
            client_id: Some(1234)
        }
    ));
}

#[test]
fn test_decode_server_list() {
    use mule_protocol::ed2k::OP_SERVERLIST;
    let limits = Ed2kLimits::default();
    let mut payload = Vec::new();
    payload.push(2u8); // count
    payload.extend_from_slice(&0x0102_0304u32.to_le_bytes());
    payload.extend_from_slice(&4661u16.to_le_bytes());
    payload.extend_from_slice(&0x7F00_0001u32.to_le_bytes());
    payload.extend_from_slice(&4665u16.to_le_bytes());

    let resp = decode_server_response(PR_ED2K, OP_SERVERLIST, &payload, &limits)
        .expect("decode OP_SERVERLIST");
    match resp {
        ServerResponse::ServerList { servers } => {
            assert_eq!(servers.len(), 2);
            assert_eq!(servers[0], (0x0102_0304, 4661));
            assert_eq!(servers[1], (0x7F00_0001, 4665));
        }
        _ => panic!("unexpected response type"),
    }

    // Empty list is valid; truncated entry is rejected.
    let resp = decode_server_response(PR_ED2K, OP_SERVERLIST, &[0u8], &limits).unwrap();
    assert!(matches!(resp, ServerResponse::ServerList { ref servers } if servers.is_empty()));
    assert!(decode_server_response(PR_ED2K, OP_SERVERLIST, &[1u8, 0, 0, 0], &limits).is_err());
}
