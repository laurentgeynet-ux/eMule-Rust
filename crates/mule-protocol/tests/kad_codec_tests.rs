use mule_protocol::kad::*;

#[test]
fn test_kad_ping_pong_roundtrip() {
    let kad_id = [0x42u8; 16];
    let udp_port = 4672;

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
    let udp_port = 4672;
    let version = 2;
    let tcp_port = 4662;

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
    let udp_port = 4672;

    let encoded_boot_req = KadCodec::encode_bootstrap_req(&kad_id, udp_port);
    let decoded_boot_req = KadCodec::decode(&encoded_boot_req).expect("decode bootstrap req");
    match decoded_boot_req {
        KadMessage::BootstrapReq(b) => {
            assert_eq!(b.kad_id, kad_id);
            assert_eq!(b.udp_port, udp_port);
        }
        _ => panic!("expected BootstrapReq message"),
    }

    let contacts = vec![
        KadContactWire {
            kad_id: [0x22u8; 16],
            ip: [127, 0, 0, 1],
            udp_port: 4672,
            tcp_port: 4662,
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
            assert_eq!(r.req_type, 0);
            assert_eq!(r.target_id, target_id);
            assert_eq!(r.sender_id, kad_id);
        }
        _ => panic!("expected Req message"),
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
