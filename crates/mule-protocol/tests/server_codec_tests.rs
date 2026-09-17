use mule_protocol::ed2k::{
    decode_server_response, Ed2kLimits, GetSourcesRequest, SearchQuery, SearchResult,
    ServerIdChange, ServerLoginRequest, ServerResponse, ServerStatus, OP_FOUNDSOURCES,
    OP_FOUNDSOURCES_OBFU, OP_GETSOURCES, OP_GETSOURCES_OBFU, OP_IDCHANGE, OP_LOGINREQUEST, OP_SEARCHREQUEST, OP_SEARCH_RESULT,
    OP_SERVERMESSAGE, OP_SERVERSTATUS, TAG_SPECIAL_FILESIZE, TAG_SPECIAL_NAME,
    TAG_SPECIAL_VERSION,
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

    let resp_low = decode_server_response(PR_ED2K, OP_IDCHANGE, &payload_low, &limits)
        .expect("decode LowID");
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
    let packet_simple = simple_query.encode(&limits).expect("encode simple search query");
    assert_eq!(packet_simple.protocol, PR_ED2K);
    assert_eq!(packet_simple.opcode, OP_SEARCHREQUEST);
    assert_eq!(packet_simple.payload[0], 0x01);
    let term_len = u16::from_le_bytes([packet_simple.payload[1], packet_simple.payload[2]]) as usize;
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

    let resp = decode_server_response(PR_ED2K, mule_protocol::ed2k::OP_SERVERIDENT, &payload, &limits)
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

    let resp = decode_server_response(PR_ED2K, mule_protocol::ed2k::OP_SERVERIDENT, &payload, &limits)
        .expect("decode server ident with fixed str tag 0x19");

    match resp {
        ServerResponse::Ident(ident) => {
            assert_eq!(ident.server_hash, server_hash);
            assert_eq!(ident.name().as_deref(), Some("Lugdunum9"));
        }
        _ => panic!("unexpected response type"),
    }
}
