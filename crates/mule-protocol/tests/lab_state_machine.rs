// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use bytes::Bytes;
use mule_protocol::lab::*;
use mule_protocol::{PeerAction, PeerId, ProtocolViolation};

#[test]
fn test_nominal_handshake_flow_pure_state_machine() {
    let client_peer_id = PeerId::new([0x11; 16]);
    let server_peer_id = PeerId::new([0x22; 16]);
    let challenge_nonce = [0xAA; 16];

    // 1. Client creates initiator state machine and initial Hello
    let (mut client_sm, hello) =
        PeerStateMachine::new_initiator(client_peer_id, 1, challenge_nonce);
    assert_eq!(client_sm.state().name(), "AwaitingHelloAck");

    // 2. Server creates listener state machine
    let mut server_sm = PeerStateMachine::new_listener(server_peer_id, 1);
    assert_eq!(server_sm.state().name(), "AwaitingHello");

    // 3. Server receives Hello from client
    let server_actions = server_sm.on_message(hello).expect("server accepts hello");
    assert_eq!(server_sm.state().name(), "Ready");
    assert_eq!(server_actions.len(), 1);

    let ack = match &server_actions[0] {
        PeerAction::SendMessage(msg) => msg.clone(),
        other => panic!("expected SendMessage, got {:?}", other),
    };

    // 4. Client receives HelloAck from server
    let client_actions = client_sm.on_message(ack).expect("client accepts hello ack");
    assert_eq!(client_sm.state().name(), "Ready");
    assert!(client_actions.is_empty());
}

#[test]
fn test_incompatible_version_rejected() {
    let client_peer_id = PeerId::new([0x11; 16]);
    let server_peer_id = PeerId::new([0x22; 16]);

    let mut server_sm = PeerStateMachine::new_listener(server_peer_id, 1);
    let bad_hello = LabMessage::Hello {
        client_version: 99, // Incompatible
        nonce: [0; 16],
        peer_id: client_peer_id,
    };

    let result = server_sm.on_message(bad_hello);
    assert!(matches!(
        result,
        Err(ProtocolViolation::IncompatibleVersion {
            client: 99,
            server: 1
        })
    ));
}

#[test]
fn test_unexpected_hello_ack_rejected() {
    let server_peer_id = PeerId::new([0x22; 16]);
    let mut server_sm = PeerStateMachine::new_listener(server_peer_id, 1);

    // Sending HelloAck to a server awaiting Hello
    let bad_ack = LabMessage::HelloAck {
        server_version: 1,
        nonce_ack: [0; 16],
        peer_id: PeerId::new([0x33; 16]),
    };

    let result = server_sm.on_message(bad_ack);
    assert!(matches!(
        result,
        Err(ProtocolViolation::UnexpectedMessageInState { .. })
    ));
}

#[test]
fn test_nonce_mismatch_rejected() {
    let client_peer_id = PeerId::new([0x11; 16]);
    let (mut client_sm, _) = PeerStateMachine::new_initiator(client_peer_id, 1, [0xAA; 16]);

    // HelloAck echoes the wrong nonce
    let mismatched_ack = LabMessage::HelloAck {
        server_version: 1,
        nonce_ack: [0xBB; 16], // Doesn't match 0xAA
        peer_id: PeerId::new([0x22; 16]),
    };

    let result = client_sm.on_message(mismatched_ack);
    assert!(matches!(result, Err(ProtocolViolation::NonceMismatch)));
}

#[test]
fn test_double_hello_rejected() {
    let client_peer_id = PeerId::new([0x11; 16]);
    let server_peer_id = PeerId::new([0x22; 16]);

    let mut server_sm = PeerStateMachine::new_listener(server_peer_id, 1);
    let hello = LabMessage::Hello {
        client_version: 1,
        nonce: [0xAA; 16],
        peer_id: client_peer_id,
    };

    server_sm.on_message(hello.clone()).unwrap();
    assert!(server_sm.state().is_ready());

    // Second Hello should be rejected
    let result = server_sm.on_message(hello);
    assert!(matches!(result, Err(ProtocolViolation::DoubleHello)));
}

#[test]
fn test_block_transfer_before_ready_rejected() {
    let server_peer_id = PeerId::new([0x22; 16]);
    let mut server_sm = PeerStateMachine::new_listener(server_peer_id, 1);

    let premature_req = LabMessage::RequestBlock {
        file_hash: [0x11; 16],
        start_offset: 0,
        length: 1024,
    };

    let result = server_sm.on_message(premature_req);
    assert!(matches!(
        result,
        Err(ProtocolViolation::PrematureTransferMessage)
    ));
}

#[test]
fn test_goodbye_closes_session() {
    let server_peer_id = PeerId::new([0x22; 16]);
    let mut server_sm = PeerStateMachine::new_listener(server_peer_id, 1);

    let goodbye = LabMessage::Goodbye {
        reason: "User closed connection".into(),
    };

    let actions = server_sm.on_message(goodbye).expect("goodbye accepted");
    assert_eq!(server_sm.state().name(), "Closed");
    assert_eq!(actions, vec![PeerAction::CloseConnection]);
}

#[test]
fn test_lab_message_roundtrip_raw_packet_serialization() {
    let messages = vec![
        LabMessage::Hello {
            client_version: 1,
            nonce: [0x42; 16],
            peer_id: PeerId::new([0x99; 16]),
        },
        LabMessage::HelloAck {
            server_version: 1,
            nonce_ack: [0x42; 16],
            peer_id: PeerId::new([0x88; 16]),
        },
        LabMessage::FileOffer {
            file_hash: [0x77; 16],
            file_size: 9_728_000,
            name: "test_file.iso".into(),
        },
        LabMessage::RequestBlock {
            file_hash: [0x77; 16],
            start_offset: 184_320,
            length: 184_320,
        },
        LabMessage::BlockData {
            file_hash: [0x77; 16],
            start_offset: 184_320,
            data: Bytes::from_static(&[1, 2, 3, 4, 5]),
        },
        LabMessage::BlockRejected {
            file_hash: [0x77; 16],
            start_offset: 0,
            reason: LabRejectReason::FileNotFound,
        },
        LabMessage::Goodbye {
            reason: "normal exit".into(),
        },
        LabMessage::ProtocolError {
            code: 400,
            message: "bad frame".into(),
        },
    ];

    for msg in messages {
        let raw = msg.to_raw_packet().expect("serializes to raw packet");
        let parsed = LabMessage::from_raw_packet(&raw).expect("deserializes back");
        assert_eq!(parsed, msg);
    }
}
