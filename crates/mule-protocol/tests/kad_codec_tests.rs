use mule_protocol::kad::*;

#[test]
fn test_kad_ping_pong_roundtrip() {
    let kad_id = [0x42u8; 16];
    let udp_port = 39737;

    let encoded_ping = KadCodec::encode_ping(&kad_id, udp_port);
    assert_eq!(encoded_ping.len(), 2 + 16 + 2);

    let decoded_ping = KadCodec::decode(&encoded_ping).expect("should decode ping");
    match decoded_ping {
        KadMessage::Ping(p) => {
            assert_eq!(p.kad_id, kad_id);
            assert_eq!(p.udp_port, udp_port);
        }
        _ => panic!("expected Ping message"),
    }

    let client_port = 55123;
    let encoded_pong = KadCodec::encode_pong(client_port);
    assert_eq!(encoded_pong.len(), 2 + 2);

    let decoded_pong = KadCodec::decode(&encoded_pong).expect("should decode pong");
    match decoded_pong {
        KadMessage::Pong(p) => {
            assert_eq!(p.client_port, client_port);
        }
        _ => panic!("expected Pong message"),
    }
}

#[test]
fn test_kad_hello_roundtrip() {
    let kad_id = [0xAAu8; 16];
    let udp_port = 39737;
    let version = 2;
    let tcp_port = 39735;

    let encoded_hello_req = KadCodec::encode_hello_req(&kad_id, udp_port, version, tcp_port);
    let decoded_hello_req = KadCodec::decode(&encoded_hello_req).expect("should decode hello req");
    match decoded_hello_req {
        KadMessage::HelloReq(h) => {
            assert_eq!(h.kad_id, kad_id);
            assert_eq!(h.udp_port, udp_port);
            assert_eq!(h.version, version);
            assert_eq!(h.tcp_port, tcp_port);
        }
        _ => panic!("expected HelloReq message"),
    }

    let responder_id = [0xBBu8; 16];
    let encoded_hello_res = KadCodec::encode_hello_res(&responder_id, udp_port, version, tcp_port);
    let decoded_hello_res = KadCodec::decode(&encoded_hello_res).expect("should decode hello res");
    match decoded_hello_res {
        KadMessage::HelloRes(h) => {
            assert_eq!(h.kad_id, responder_id);
            assert_eq!(h.udp_port, udp_port);
            assert_eq!(h.version, version);
            assert_eq!(h.tcp_port, tcp_port);
        }
        _ => panic!("expected HelloRes message"),
    }
}

#[test]
fn test_kad_bootstrap_and_find_node_roundtrip() {
    let kad_id = [0x11u8; 16];
    let udp_port = 39737;

    let encoded_boot_req = KadCodec::encode_bootstrap_req(&kad_id, udp_port);
    assert_eq!(encoded_boot_req.len(), 2);
    let decoded_boot_req = KadCodec::decode(&encoded_boot_req).expect("decode bootstrap req");
    match decoded_boot_req {
        KadMessage::BootstrapReq(b) => {
            assert_eq!(b.kad_id, [0u8; 16]);
            assert_eq!(b.udp_port, 0);
        }
        _ => panic!("expected BootstrapReq message"),
    }

    let contacts = vec![
        KadContactWire {
            kad_id: [0x22u8; 16],
            ip: [127, 0, 0, 1],
            udp_port: 39737,
            tcp_port: 39735,
            version: 2,
        },
        KadContactWire {
            kad_id: [0x33u8; 16],
            ip: [127, 0, 0, 2],
            udp_port: 4673,
            tcp_port: 0,
            version: 2,
        },
    ];

    let encoded_boot_res = KadCodec::encode_bootstrap_res(&kad_id, udp_port, 2, &contacts);
    let decoded_boot_res = KadCodec::decode(&encoded_boot_res).expect("decode bootstrap res");
    match decoded_boot_res {
        KadMessage::BootstrapRes(b) => {
            assert_eq!(b.kad_id, kad_id);
            assert_eq!(b.contacts.len(), 2);
            assert_eq!(b.contacts[0].kad_id, [0x22u8; 16]);
            assert_eq!(b.contacts[1].ip, [127, 0, 0, 2]);
        }
        _ => panic!("expected BootstrapRes message"),
    }

    let target_id = [0x99u8; 16];
    let encoded_find_node = KadCodec::encode_find_node_req(&target_id, &kad_id);
    let decoded_find_node = KadCodec::decode(&encoded_find_node).expect("decode find node req");
    match decoded_find_node {
        KadMessage::Req(r) => {
            assert_eq!(r.req_type, KADEMLIA_FIND_NODE);
            assert_eq!(r.target_id, target_id);
            assert_eq!(r.sender_id, kad_id);
        }
        _ => panic!("expected Req message"),
    }

    let encoded_find_res = KadCodec::encode_find_node_res(&target_id, &contacts);
    let decoded_find_res = KadCodec::decode(&encoded_find_res).expect("decode find node res");
    match decoded_find_res {
        KadMessage::BootstrapRes(b) => {
            assert_eq!(b.kad_id, target_id);
            assert_eq!(b.contacts.len(), 2);
            assert_eq!(b.contacts[0].kad_id, [0x22u8; 16]);
            assert_eq!(b.contacts[0].ip, [127, 0, 0, 1]);
            assert_eq!(b.contacts[1].kad_id, [0x33u8; 16]);
            assert_eq!(b.contacts[1].ip, [127, 0, 0, 2]);
            assert_eq!(b.contacts[1].udp_port, 4673);
        }
        _ => panic!("expected BootstrapRes message for FindNode response"),
    }
}

#[test]
fn test_kad_packet_size_limit() {
    let mut oversized = vec![PR_KADEMLIA, KADEMLIA2_PING];
    oversized.resize(MAX_KAD_UDP_PACKET_SIZE + 10, 0);

    let err = KadCodec::decode(&oversized).unwrap_err();
    assert!(err.to_string().contains("exceeds limit"));
}

#[test]
fn test_kad_invalid_protocol_rejected() {
    let bad_proto = vec![0x12, KADEMLIA2_PING, 0, 0];
    let err = KadCodec::decode(&bad_proto).unwrap_err();
    assert!(err.to_string().contains("unsupported protocol"));
}

#[test]
fn test_kad_truncated_payload_rejected() {
    let truncated = vec![PR_KADEMLIA, KADEMLIA2_PING, 1, 2, 3];
    let err = KadCodec::decode(&truncated).unwrap_err();
    assert!(err.to_string().contains("unexpected EOF"));
}

#[test]
fn test_kad_search_and_publish_source_roundtrip() {
    let file_hash = [0x77u8; 16];
    let sender_id = [0x88u8; 16];
    let start_pos = 0;
    let file_size = 10_485_760;

    // Official format: [target_hash 16] [start_pos u16] [file_size u64] — no sender_id
    let enc_search_req = KadCodec::encode_search_source_req(&file_hash, start_pos, file_size);
    let dec_search_req = KadCodec::decode(&enc_search_req).expect("decode search source req");
    match dec_search_req {
        KadMessage::SearchSourceReq(r) => {
            assert_eq!(r.target_file_hash, file_hash);
            assert_eq!(r.start_pos, start_pos);
            assert_eq!(r.file_size, file_size);
        }
        _ => panic!("expected SearchSourceReq"),
    }

    let sources = vec![
        KadSourceWire {
            kad_id: [0x11u8; 16],
            ip: [127, 0, 0, 1],
            udp_port: 39737,
            tcp_port: 39735,
            encryption: 0,
        },
        KadSourceWire {
            kad_id: [0x22u8; 16],
            ip: [127, 0, 0, 2],
            udp_port: 4673,
            tcp_port: 4663,
            encryption: 1,
        },
    ];

    let enc_search_res = KadCodec::encode_search_source_res(&file_hash, &sources);
    let dec_search_res = KadCodec::decode(&enc_search_res).expect("decode search source res");
    match dec_search_res {
        KadMessage::SearchSourceRes(r) => {
            assert_eq!(r.target_file_hash, file_hash);
            assert_eq!(r.sources.len(), 2);
            assert_eq!(r.sources[0].kad_id, [0x11u8; 16]);
            assert_eq!(r.sources[1].tcp_port, 4663);
            assert_eq!(r.sources[1].encryption, 1);
        }
        _ => panic!("expected SearchSourceRes"),
    }

    let contact = KadContactWire {
        kad_id: sender_id,
        ip: [127, 0, 0, 1],
        udp_port: 39737,
        tcp_port: 39735,
        version: 2,
    };

    let enc_pub_req = KadCodec::encode_publish_source_req(&file_hash, &contact, file_size);
    let dec_pub_req = KadCodec::decode(&enc_pub_req).expect("decode publish source req");
    match dec_pub_req {
        KadMessage::PublishSourceReq(r) => {
            assert_eq!(r.target_file_hash, file_hash);
            assert_eq!(r.contact.kad_id, sender_id);
            assert_eq!(r.file_size, file_size);
        }
        _ => panic!("expected PublishSourceReq"),
    }

    let enc_pub_res = KadCodec::encode_publish_source_res(&file_hash, 0);
    let dec_pub_res = KadCodec::decode(&enc_pub_res).expect("decode publish source res");
    match dec_pub_res {
        KadMessage::PublishSourceRes(r) => {
            assert_eq!(r.target_file_hash, file_hash);
            assert_eq!(r.status, 0);
        }
        _ => panic!("expected PublishSourceRes"),
    }
}

#[test]
fn test_kad_search_and_publish_keyword_roundtrip() {
    let keyword_hash = [0x55u8; 16];
    let sender_id = [0xAAu8; 16];

    // 1. Search Keyword Req
    let enc_req = KadCodec::encode_search_keyword_req(&keyword_hash, 0, &sender_id);
    let dec_req = KadCodec::decode(&enc_req).expect("decode search keyword req");
    match dec_req {
        KadMessage::SearchKeywordReq(r) => {
            assert_eq!(r.keyword_hash, keyword_hash);
            assert_eq!(r.start_pos, 0);
        }
        _ => panic!("expected SearchKeywordReq"),
    }

    // 2. Search Keyword Res
    let entries = vec![
        mule_protocol::kad::codec::KadKeywordEntryWire {
            file_hash: [0x11u8; 16],
            file_name: "ubuntu-22.04-live-server.iso".to_string(),
            file_size: 1_500_000_000,
            source_count: 42,
        },
        mule_protocol::kad::codec::KadKeywordEntryWire {
            file_hash: [0x22u8; 16],
            file_name: "debian-12.0.0-amd64-netinst.iso".to_string(),
            file_size: 650_000_000,
            source_count: 12,
        },
    ];

    let enc_res = KadCodec::encode_search_keyword_res(&keyword_hash, &entries);
    let dec_res = KadCodec::decode(&enc_res).expect("decode search keyword res");
    match dec_res {
        KadMessage::SearchKeywordRes(r) => {
            assert_eq!(r.keyword_hash, keyword_hash);
            assert_eq!(r.entries.len(), 2);
            assert_eq!(r.entries[0].file_name, "ubuntu-22.04-live-server.iso");
            assert_eq!(r.entries[0].file_size, 1_500_000_000);
            assert_eq!(r.entries[0].source_count, 42);
            assert_eq!(r.entries[1].file_name, "debian-12.0.0-amd64-netinst.iso");
        }
        _ => panic!("expected SearchKeywordRes"),
    }

    // 3. Publish Keyword Req
    let enc_pub_req = KadCodec::encode_publish_keyword_req(&keyword_hash, &entries);
    let dec_pub_req = KadCodec::decode(&enc_pub_req).expect("decode publish keyword req");
    match dec_pub_req {
        KadMessage::PublishKeywordReq(r) => {
            assert_eq!(r.keyword_hash, keyword_hash);
            assert_eq!(r.entries.len(), 2);
            assert_eq!(r.entries[0].file_hash, [0x11u8; 16]);
        }
        _ => panic!("expected PublishKeywordReq"),
    }

    // 4. Publish Keyword Res (encodes 0x4B, the unified publish response)
    let enc_pub_res = KadCodec::encode_publish_keyword_res(&keyword_hash, 0);
    let dec_pub_res = KadCodec::decode(&enc_pub_res).expect("decode publish keyword res");
    match dec_pub_res {
        KadMessage::PublishSourceRes(r) => {
            assert_eq!(r.target_file_hash, keyword_hash);
            assert_eq!(r.status, 0);
        }
        KadMessage::PublishKeywordRes(r) => {
            assert_eq!(r.keyword_hash, keyword_hash);
            assert_eq!(r.status, 0);
        }
        _ => panic!("expected PublishSourceRes or PublishKeywordRes for 0x4B"),
    }
}

#[test]
fn test_legacy_kad_bootstrap_req_opcode_0x00() {
    // Exact 27-byte payload observed from live eMule/aMule client
    let raw = [
        0xE4, 0x00, 0xD6, 0x24, 0x69, 0xF6, 0x75, 0x20, 0x5B, 0x69, 0xBA, 0xD1, 0x1F, 0xE5, 0x0A,
        0x43, 0x86, 0x84, 0x24, 0xDA, 0xAA, 0x56, 0x94, 0x1E, 0xBE, 0x25, 0x00,
    ];

    let decoded = KadCodec::decode(&raw).expect("should decode legacy Kad bootstrap req 0x00");
    match decoded {
        KadMessage::BootstrapReq(req) => {
            assert_eq!(
                req.kad_id,
                [
                    0xD6, 0x24, 0x69, 0xF6, 0x75, 0x20, 0x5B, 0x69, 0xBA, 0xD1, 0x1F, 0xE5, 0x0A,
                    0x43, 0x86, 0x84
                ]
            );
        }
        _ => panic!("expected BootstrapReq message"),
    }
}

#[test]
fn test_emule_source_response_with_tags_roundtrip() {
    let file_hash = [0x5Fu8; 16];
    let sources = vec![KadSourceWire {
        kad_id: [0x88u8; 16],
        ip: [88, 171, 182, 196],
        udp_port: 50671,
        tcp_port: 50662,
        encryption: 3,
    }];

    let encoded = KadCodec::encode_search_source_res(&file_hash, &sources);
    let decoded = KadCodec::decode(&encoded).expect("should decode tag-based source response");
    match decoded {
        KadMessage::SearchSourceRes(res) => {
            assert_eq!(res.target_file_hash, file_hash);
            assert_eq!(res.sources.len(), 1);
            let s = &res.sources[0];
            assert_eq!(s.kad_id, [0x88u8; 16]);
            // Verify IP byte order is NOT inverted!
            assert_eq!(s.ip, [88, 171, 182, 196]);
            assert_eq!(s.tcp_port, 50662);
            assert_eq!(s.udp_port, 50671);
            assert_eq!(s.encryption, 3);
        }
        _ => panic!("expected SearchSourceRes"),
    }
}

#[test]
fn test_emule_keyword_response_with_publishinfo_roundtrip() {
    let keyword_hash = [0xAAu8; 16];
    let entries = vec![KadKeywordEntryWire {
        file_hash: [0x11u8; 16],
        file_name: "test_archive.zip".to_string(),
        file_size: 42_000_000,
        source_count: 7,
    }];

    let encoded = KadCodec::encode_search_keyword_res(&keyword_hash, &entries);
    let decoded = KadCodec::decode(&encoded).expect("should decode keyword response");
    match decoded {
        KadMessage::SearchKeywordRes(res) => {
            assert_eq!(res.keyword_hash, keyword_hash);
            assert_eq!(res.entries.len(), 1);
            let e = &res.entries[0];
            assert_eq!(e.file_hash, [0x11u8; 16]);
            assert_eq!(e.file_name, "test_archive.zip");
            assert_eq!(e.file_size, 42_000_000);
            assert_eq!(e.source_count, 7);
        }
        _ => panic!("expected SearchKeywordRes"),
    }
}

#[test]
fn test_kad_firewall_udp_roundtrip() {
    // eMule KADEMLIA2_FIREWALLUDP: [u8 error_code][u16 tested_port]
    let encoded = KadCodec::encode_firewall_udp(0, 4672);
    assert_eq!(encoded.len(), 5);
    assert_eq!(encoded[0], PR_KADEMLIA);
    assert_eq!(encoded[1], KADEMLIA2_FIREWALLUDP);

    let decoded = KadCodec::decode(&encoded).expect("should decode firewall udp");
    match decoded {
        KadMessage::FirewallUdp(fw) => {
            assert_eq!(fw.error_code, 0);
            assert_eq!(fw.udp_port, 4672);
        }
        _ => panic!("expected FirewallUdp message"),
    }

    // error_code non-zero (remote reported biased/already-known result)
    let encoded_err = KadCodec::encode_firewall_udp(1, 13370);
    match KadCodec::decode(&encoded_err).expect("decode") {
        KadMessage::FirewallUdp(fw) => {
            assert_eq!(fw.error_code, 1);
            assert_eq!(fw.udp_port, 13370);
        }
        _ => panic!("expected FirewallUdp message"),
    }
}

#[test]
fn test_kad_firewall_udp_malformed() {
    // Payload shorter than 3 bytes must be rejected.
    let raw = [PR_KADEMLIA, KADEMLIA2_FIREWALLUDP, 0x00];
    assert!(KadCodec::decode(&raw).is_err());
}
