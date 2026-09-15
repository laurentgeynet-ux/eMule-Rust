use mule_protocol::ed2k::{
    decode_server_response, Ed2kLimits, ServerIdChange, ServerLoginRequest, ServerResponse,
    ServerStatus, OP_IDCHANGE, OP_LOGINREQUEST, OP_SERVERMESSAGE, OP_SERVERSTATUS,
};
use mule_protocol::PR_ED2K;

#[test]
fn test_encode_server_login_request() {
    let limits = Ed2kLimits::default();
    let user_hash = [1u8; 16];
    let req = ServerLoginRequest::new(user_hash, 4662, "TestUser", 0x3c);
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
