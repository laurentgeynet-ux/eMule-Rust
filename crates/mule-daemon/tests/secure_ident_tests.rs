use md4::{Digest, Md4};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};
use mule_protocol::ed2k::{
    Ed2kDecoder, Ed2kEncoder, Ed2kHello, Ed2kHelloAnswer, Ed2kPublicKey, Ed2kRequestParts,
    Ed2kSecIdentState, Ed2kSignature, OP_ACCEPTUPLOADREQ, OP_HELLO, OP_HELLO_ANSWER, OP_PUBLICKEY,
    OP_QUEUERANKING, OP_SECIDENTSTATE, OP_SENDINGPART, OP_SIGNATURE, OP_STARTUPLOADREQ, PR_ED2K,
    PR_EMULE,
};
use mule_protocol::framing::PacketCodec;

async fn setup_secure_ident_test_state() -> (AppState, tempfile::TempDir, String) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "test_token_secure_ident".to_string();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    let file_hash_hex = "41d6cfe0d16ae931b73c59d7e0c089c1".to_string();

    let mut authenticated = true;
    let share_req = make_req(
        1,
        "shares.add",
        Some(json!({
            "fileHash": file_hash_hex,
            "fileName": "secure_shared_data.bin",
            "fileSize": 18874368,
            "filePath": "C:\\Shared\\secure_shared_data.bin"
        })),
    );
    let resp = handle_request(&state, share_req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());

    (state, dir, file_hash_hex)
}

fn make_req(id: u64, method: &str, params: Option<Value>) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(id)),
        method: method.to_string(),
        params,
    }
}

#[tokio::test]
async fn test_inbound_secure_ident_handshake_and_authentication() {
    let (state, _dir, hash) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    // 1. Start listener
    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    // 2. Connect client and perform standard OP_HELLO
    let mut client_socket = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    let hello = Ed2kHello {
        user_hash: [0x77u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    let mut received_hello_ans = false;
    while !received_hello_ans {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO_ANSWER {
                received_hello_ans = true;
                break;
            }
        }
    }

    // 3. Send OP_PUBLICKEY (80 bytes key material)
    let mock_pubkey = vec![0x99u8; 80];
    let pubkey_msg = Ed2kPublicKey {
        key_bytes: bytes::Bytes::from(mock_pubkey.clone()),
    };
    let pk_pkt = encoder.encode_public_key(&pubkey_msg).unwrap();
    out.clear();
    out.push(pk_pkt.protocol);
    out.extend_from_slice(&(pk_pkt.frame_len() as u32).to_le_bytes());
    out.push(pk_pkt.opcode);
    out.extend_from_slice(&pk_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // 4. Expect OP_SECIDENTSTATE (Challenge = 1)
    let mut received_sec_challenge = false;
    while !received_sec_challenge {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                let sec_st = decoder
                    .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                assert_eq!(sec_st.state, 1, "Expected challenge state");
                received_sec_challenge = true;
                break;
            }
        }
    }

    // 5. Send OP_SIGNATURE
    let sig_msg = Ed2kSignature {
        signature_bytes: bytes::Bytes::from(vec![0xEEu8; 64]),
    };
    let sig_pkt = encoder.encode_signature(&sig_msg).unwrap();
    out.clear();
    out.push(sig_pkt.protocol);
    out.extend_from_slice(&(sig_pkt.frame_len() as u32).to_le_bytes());
    out.push(sig_pkt.opcode);
    out.extend_from_slice(&sig_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // 6. Expect OP_SECIDENTSTATE (Rejected = 0)
    // Security lock: real RSA verification is not implemented yet (v0.1.9-A1),
    // so every signature is rejected and never grants credit_multiplier.
    let mut received_auth_ok = false;
    while !received_auth_ok {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                let sec_st = decoder
                    .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                assert_eq!(
                    sec_st.state, 0,
                    "Unverified signature must be rejected with state 0"
                );
                received_auth_ok = true;
                break;
            }
        }
    }

    // 7. Request parts on authenticated peer
    let hash_bytes: [u8; 16] = hex::decode(&hash).unwrap().try_into().unwrap();
    let req_parts = Ed2kRequestParts {
        file_hash: hash_bytes,
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
            start: 0,
            end: 184320,
        }],
    };
    let rp_pkt = encoder.encode_request_parts(&req_parts).unwrap();
    out.clear();
    out.push(rp_pkt.protocol);
    out.extend_from_slice(&(rp_pkt.frame_len() as u32).to_le_bytes());
    out.push(rp_pkt.opcode);
    out.extend_from_slice(&rp_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // 8. Expect OP_SENDINGPART
    let mut received_part = false;
    while !received_part {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_SENDINGPART {
                received_part = true;
                break;
            }
        }
    }

    // 9. Verify no credit record was persisted (signature was rejected)
    let pk_hash = hex::encode(Md4::digest(&mock_pubkey));
    let credit_opt = state
        .downloads
        .lock()
        .await
        .get_peer_credit(&pk_hash)
        .unwrap();
    assert!(
        credit_opt.is_none(),
        "No credit record must be created for rejected peer"
    );

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_inbound_secure_ident_verified_with_real_rsa() {
    let (state, _dir, hash) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    // Daemon's own public key DER = the challenger key the peer must sign over.
    let daemon_pubkey_der = {
        let id = state.identity.lock().await;
        id.public_key_bytes().to_vec()
    };

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    // Real RSA-384 peer identity.
    let peer_key = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 384).unwrap();
    use rsa::pkcs1::EncodeRsaPublicKey;
    let peer_pubkey_der = rsa::RsaPublicKey::from(&peer_key)
        .to_pkcs1_der()
        .unwrap()
        .as_bytes()
        .to_vec();
    assert!(peer_pubkey_der.len() <= 80);

    let mut client = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let mut out = Vec::new();

    let hello = Ed2kHello {
        user_hash: [0x55u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hp = encoder.encode_hello(&hello).unwrap();
    out.extend_from_slice(&[hp.protocol]);
    out.extend_from_slice(&(hp.frame_len() as u32).to_le_bytes());
    out.push(hp.opcode);
    out.extend_from_slice(&hp.payload);
    client.write_all(&out).await.unwrap();

    let mut hello_ok = false;
    while !hello_ok {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO_ANSWER {
                hello_ok = true;
                break;
            }
        }
    }

    // Send real public key.
    let pk_pkt = encoder
        .encode_public_key(&Ed2kPublicKey {
            key_bytes: bytes::Bytes::from(peer_pubkey_der.clone()),
        })
        .unwrap();
    out.clear();
    out.push(pk_pkt.protocol);
    out.extend_from_slice(&(pk_pkt.frame_len() as u32).to_le_bytes());
    out.push(pk_pkt.opcode);
    out.extend_from_slice(&pk_pkt.payload);
    client.write_all(&out).await.unwrap();

    // Receive challenge.
    let mut challenge = None;
    while challenge.is_none() {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                let st = decoder
                    .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                assert_eq!(st.state, 1);
                challenge = Some(st.challenge);
            }
        }
    }

    // Sign V1 payload: challenger_pubkey || challenge_le.
    let payload = mule_crypto::legacy_sui::build_sui_signing_payload(
        mule_crypto::legacy_sui::SecureIdentVersion::V1,
        &daemon_pubkey_der,
        challenge.unwrap(),
        None,
        None,
    )
    .unwrap();
    let sig = mule_crypto::legacy_sui::sign_sui_payload(&peer_key, &payload).unwrap();
    assert_eq!(sig.len(), 48);

    let sig_pkt = encoder
        .encode_signature(&Ed2kSignature {
            signature_bytes: bytes::Bytes::from(sig),
        })
        .unwrap();
    out.clear();
    out.push(sig_pkt.protocol);
    out.extend_from_slice(&(sig_pkt.frame_len() as u32).to_le_bytes());
    out.push(sig_pkt.opcode);
    out.extend_from_slice(&sig_pkt.payload);
    client.write_all(&out).await.unwrap();

    // eMule sends no reply on success: observable proof is that a credit record
    // is created for the verified pubkey and uploads are credited to it.
    let hash_bytes: [u8; 16] = hex::decode(&hash).unwrap().try_into().unwrap();
    let rp_pkt = encoder
        .encode_request_parts(&Ed2kRequestParts {
            file_hash: hash_bytes,
            ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
                start: 0,
                end: 184320,
            }],
        })
        .unwrap();
    out.clear();
    out.push(rp_pkt.protocol);
    out.extend_from_slice(&(rp_pkt.frame_len() as u32).to_le_bytes());
    out.push(rp_pkt.opcode);
    out.extend_from_slice(&rp_pkt.payload);
    client.write_all(&out).await.unwrap();

    let mut got_part = false;
    while !got_part {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_SENDINGPART {
                got_part = true;
            }
        }
    }

    let pk_hash = hex::encode(Md4::digest(&peer_pubkey_der));
    let credit = state
        .downloads
        .lock()
        .await
        .get_peer_credit(&pk_hash)
        .unwrap();
    let credit = credit.expect("Verified peer must have a credit record pinned to its pubkey");
    assert_eq!(credit.uploaded_bytes, 184320);

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_credit_weighted_queue_priority() {
    let (state, _dir, hash) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    // Multi-slot uploads (MAX_ACTIVE_UPLOADS) would grant client 1 an active
    // slot instead of a queue rank — cap at 1 active slot for this test.
    state.inbound_listener.upload_manager().set_max_active(1);

    let hash_bytes: [u8; 16] = hex::decode(&hash).unwrap().try_into().unwrap();

    // Pre-populate credits for high-credit peer
    let high_credit_pubkey = vec![0x11u8; 80];
    let high_credit_hash = hex::encode(Md4::digest(&high_credit_pubkey));
    {
        let mut dls = state.downloads.lock().await;
        // Peer gave us 100 MB of downloads, so multiplier is 10.0x
        dls.record_credit_download(
            &high_credit_hash,
            &hex::encode(&high_credit_pubkey),
            104857600,
        )
        .unwrap();
    }

    // Start listener
    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();

    // 1. Client 0 connects and takes the 1 active upload slot
    let mut client0 = TcpStream::connect(&listen_addr_str).await.unwrap();
    let hello0 = Ed2kHello {
        user_hash: [0x00u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let h0_pkt = encoder.encode_hello(&hello0).unwrap();
    let mut out = Vec::new();
    out.push(h0_pkt.protocol);
    out.extend_from_slice(&(h0_pkt.frame_len() as u32).to_le_bytes());
    out.push(h0_pkt.opcode);
    out.extend_from_slice(&h0_pkt.payload);
    client0.write_all(&out).await.unwrap();

    let mut raw_buf = [0u8; 8192];
    let mut read_buf0 = bytes::BytesMut::with_capacity(65536);
    let mut h0_ok = false;
    while !h0_ok {
        let n = client0.read(&mut raw_buf).await.unwrap();
        read_buf0.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf0) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO_ANSWER {
                h0_ok = true;
                break;
            }
        }
    }

    let req_parts0 = Ed2kRequestParts {
        file_hash: hash_bytes,
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
            start: 0,
            end: 184320,
        }],
    };
    let rp_pkt0 = encoder.encode_request_parts(&req_parts0).unwrap();
    out.clear();
    out.push(rp_pkt0.protocol);
    out.extend_from_slice(&(rp_pkt0.frame_len() as u32).to_le_bytes());
    out.push(rp_pkt0.opcode);
    out.extend_from_slice(&rp_pkt0.payload);
    client0.write_all(&out).await.unwrap();

    // Verify client 0 is active
    let mut c0_active = false;
    while !c0_active {
        let n = client0.read(&mut raw_buf).await.unwrap();
        read_buf0.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf0) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_SENDINGPART {
                c0_active = true;
                break;
            }
        }
    }

    // 2. Client 1 (Uncredited, multiplier 1.0x) connects and joins queue
    let mut client1 = TcpStream::connect(&listen_addr_str).await.unwrap();
    let hello1 = Ed2kHello {
        user_hash: [0x11u8; 16],
        client_id: 0x0100007F,
        tcp_port: 4663,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let h1_pkt = encoder.encode_hello(&hello1).unwrap();
    out.clear();
    out.push(h1_pkt.protocol);
    out.extend_from_slice(&(h1_pkt.frame_len() as u32).to_le_bytes());
    out.push(h1_pkt.opcode);
    out.extend_from_slice(&h1_pkt.payload);
    client1.write_all(&out).await.unwrap();

    let mut read_buf1 = bytes::BytesMut::with_capacity(65536);
    let mut h1_ok = false;
    while !h1_ok {
        let n = client1.read(&mut raw_buf).await.unwrap();
        read_buf1.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf1) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO_ANSWER {
                h1_ok = true;
                break;
            }
        }
    }

    let req_parts1 = Ed2kRequestParts {
        file_hash: hash_bytes,
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
            start: 0,
            end: 184320,
        }],
    };
    let rp_pkt1 = encoder.encode_request_parts(&req_parts1).unwrap();
    out.clear();
    out.push(rp_pkt1.protocol);
    out.extend_from_slice(&(rp_pkt1.frame_len() as u32).to_le_bytes());
    out.push(rp_pkt1.opcode);
    out.extend_from_slice(&rp_pkt1.payload);
    client1.write_all(&out).await.unwrap();

    // Client 1 gets queue ranking 1
    let mut c1_rank = 0;
    while c1_rank == 0 {
        let n = client1.read(&mut raw_buf).await.unwrap();
        read_buf1.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf1) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_QUEUERANKING {
                c1_rank = u32::from_le_bytes(pkt.payload[0..4].try_into().unwrap());
                break;
            }
        }
    }
    assert_eq!(c1_rank, 1);

    // 3. Client 2 (High credit 10.0x) connects with OP_PUBLICKEY and joins queue
    let mut client2 = TcpStream::connect(&listen_addr_str).await.unwrap();
    let hello2 = Ed2kHello {
        user_hash: [0x22u8; 16],
        client_id: 0x0100007F,
        tcp_port: 4664,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let h2_pkt = encoder.encode_hello(&hello2).unwrap();
    out.clear();
    out.push(h2_pkt.protocol);
    out.extend_from_slice(&(h2_pkt.frame_len() as u32).to_le_bytes());
    out.push(h2_pkt.opcode);
    out.extend_from_slice(&h2_pkt.payload);
    client2.write_all(&out).await.unwrap();

    let mut read_buf2 = bytes::BytesMut::with_capacity(65536);
    let mut h2_ok = false;
    while !h2_ok {
        let n = client2.read(&mut raw_buf).await.unwrap();
        read_buf2.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf2) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO_ANSWER {
                h2_ok = true;
                break;
            }
        }
    }

    // Authenticate client 2 with high credit public key
    let pubkey_msg2 = Ed2kPublicKey {
        key_bytes: bytes::Bytes::from(high_credit_pubkey.clone()),
    };
    let pk_pkt2 = encoder.encode_public_key(&pubkey_msg2).unwrap();
    out.clear();
    out.push(pk_pkt2.protocol);
    out.extend_from_slice(&(pk_pkt2.frame_len() as u32).to_le_bytes());
    out.push(pk_pkt2.opcode);
    out.extend_from_slice(&pk_pkt2.payload);
    client2.write_all(&out).await.unwrap();

    // Read SecIdentState challenge and answer with Signature
    let mut got_sec_st = false;
    while !got_sec_st {
        let n = client2.read(&mut raw_buf).await.unwrap();
        read_buf2.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf2) {
            if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                got_sec_st = true;
                break;
            }
        }
    }

    let sig_msg2 = Ed2kSignature {
        signature_bytes: bytes::Bytes::from(vec![0x11u8; 64]),
    };
    let sig_pkt2 = encoder.encode_signature(&sig_msg2).unwrap();
    out.clear();
    out.push(sig_pkt2.protocol);
    out.extend_from_slice(&(sig_pkt2.frame_len() as u32).to_le_bytes());
    out.push(sig_pkt2.opcode);
    out.extend_from_slice(&sig_pkt2.payload);
    client2.write_all(&out).await.unwrap();

    // Now request parts on client 2 -> due to 10x credit multiplier, client 2 gets superior rank 1
    let req_parts2 = Ed2kRequestParts {
        file_hash: hash_bytes,
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
            start: 0,
            end: 184320,
        }],
    };
    let rp_pkt2 = encoder.encode_request_parts(&req_parts2).unwrap();
    out.clear();
    out.push(rp_pkt2.protocol);
    out.extend_from_slice(&(rp_pkt2.frame_len() as u32).to_le_bytes());
    out.push(rp_pkt2.opcode);
    out.extend_from_slice(&rp_pkt2.payload);
    client2.write_all(&out).await.unwrap();

    let mut c2_rank = 0;
    while c2_rank == 0 {
        let n = client2.read(&mut raw_buf).await.unwrap();
        read_buf2.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf2) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_QUEUERANKING {
                c2_rank = u32::from_le_bytes(pkt.payload[0..4].try_into().unwrap());
                break;
            }
        }
    }
    // Credit multiplier is only granted after real RSA SUI verification (v0.1.9-A1);
    // until then every peer queues at 1.0x, so client 2 stays behind client 1.
    assert_eq!(
        c2_rank, 2,
        "Unverified peer must not earn credit multiplier priority"
    );

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_outbound_secure_ident_answers_peer_challenge() {
    let (state, _dir, hash) = setup_secure_ident_test_state().await;
    let mut authenticated = true;
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);

    // The file must exist as a download (not just a share) for sources.connect.
    let link = format!("ed2k://|file|outbound_sui.bin|9728001|{hash}|/");
    let resp = handle_request(
        &state,
        make_req(0, "downloads.addEd2kLink", Some(json!({ "link": link }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());

    // Mock eMule peer: RSA-384 identity, sends OP_PUBLICKEY + OP_SECIDENTSTATE
    // challenge (state=2 → expects our pubkey then signature).
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();

    let peer_key = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 384).unwrap();
    let peer_pub = rsa::RsaPublicKey::from(&peer_key);
    use rsa::pkcs1::EncodeRsaPublicKey;
    let peer_pub_der = peer_pub.to_pkcs1_der().unwrap().as_bytes().to_vec();

    let hash_bytes: [u8; 16] = hex::decode(&hash).unwrap().try_into().unwrap();
    let challenge_val: u32 = 0xDEADBEEF;

    let peer_pub_der_clone = peer_pub_der.clone();
    let mock = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut read_buf = bytes::BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let encoder = Ed2kEncoder::default();
        let decoder = Ed2kDecoder::default();
        let mut out = Vec::new();
        let mut ed2k_pkts = 0usize;

        // 1. Wait for OP_HELLO
        let mut got_hello = false;
        while !got_hello {
            let n = socket.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO {
                    got_hello = true;
                }
            }
        }

        // 2. Send HELLO_ANSWER + OP_PUBLICKEY + OP_SECIDENTSTATE(state=2)
        let ha = encoder
            .encode_hello_answer(&Ed2kHelloAnswer {
                user_hash: [0x66u8; 16],
                client_id: 0x0100007F,
                tcp_port: peer_addr.port(),
                tags: vec![],
                unknown_tags: vec![],
                server_ip: 0,
                server_port: 0,
            })
            .unwrap();
        out.extend_from_slice(&[ha.protocol]);
        out.extend_from_slice(&(ha.frame_len() as u32).to_le_bytes());
        out.push(ha.opcode);
        out.extend_from_slice(&ha.payload);
        socket.write_all(&out).await.unwrap();

        // Wait until the daemon proceeds past the handshake (first ED2K packet
        // such as OP_REQFILENAME) before issuing the SUI exchange — a real
        // eMule challenger also acts post-handshake.
        let mut daemon_moved_on = false;
        while !daemon_moved_on {
            let n = socket.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K {
                    ed2k_pkts += 1;
                    daemon_moved_on = true;
                }
            }
        }

        out.clear();
        let pk_pkt = encoder
            .encode_public_key(&Ed2kPublicKey {
                key_bytes: bytes::Bytes::from(peer_pub_der_clone.clone()),
            })
            .unwrap();
        out.push(pk_pkt.protocol);
        out.extend_from_slice(&(pk_pkt.frame_len() as u32).to_le_bytes());
        out.push(pk_pkt.opcode);
        out.extend_from_slice(&pk_pkt.payload);
        let st_pkt = encoder
            .encode_secident_state(&Ed2kSecIdentState {
                state: 2,
                challenge: challenge_val,
            })
            .unwrap();
        out.push(st_pkt.protocol);
        out.extend_from_slice(&(st_pkt.frame_len() as u32).to_le_bytes());
        out.push(st_pkt.opcode);
        out.extend_from_slice(&st_pkt.payload);
        socket.write_all(&out).await.unwrap();

        // 3. Loop: answer file-status req, capture daemon OP_PUBLICKEY +
        //    OP_SIGNATURE, accept upload, serve one part.
        let mut daemon_pubkey: Option<Vec<u8>> = None;
        let mut daemon_sig: Option<Vec<u8>> = None;
        let mut sent_filestatus = false;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while std::time::Instant::now() < deadline {
            let n = match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                socket.read(&mut raw_buf),
            )
            .await
            {
                Ok(Ok(n)) if n > 0 => n,
                _ => break,
            };
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_EMULE && pkt.opcode == OP_PUBLICKEY {
                    let pk = decoder
                        .decode_public_key(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                        .unwrap();
                    daemon_pubkey = Some(pk.key_bytes.to_vec());
                    continue;
                }
                if pkt.protocol == PR_EMULE && pkt.opcode == OP_SIGNATURE {
                    let sig = decoder
                        .decode_signature(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                        .unwrap();
                    daemon_sig = Some(sig.signature_bytes.to_vec());
                    continue;
                }
                if pkt.protocol == PR_ED2K {
                    ed2k_pkts += 1;
                    if !sent_filestatus && ed2k_pkts >= 2 {
                        sent_filestatus = true;
                        let fs = encoder
                            .encode_file_status(&mule_protocol::ed2k::Ed2kFileStatus {
                                file_hash: hash_bytes,
                                part_count: 2,
                                availability_mask:
                                    mule_protocol::ed2k::PartAvailabilityMask::from_parts(
                                        &[0, 1],
                                        2,
                                    ),
                            })
                            .unwrap();
                        out.clear();
                        out.push(fs.protocol);
                        out.extend_from_slice(&(fs.frame_len() as u32).to_le_bytes());
                        out.push(fs.opcode);
                        out.extend_from_slice(&fs.payload);
                        socket.write_all(&out).await.unwrap();
                    }
                    if pkt.opcode == OP_STARTUPLOADREQ {
                        out.clear();
                        out.push(PR_ED2K);
                        out.extend_from_slice(&1u32.to_le_bytes());
                        out.push(OP_ACCEPTUPLOADREQ);
                        socket.write_all(&out).await.unwrap();
                    }
                    if pkt.opcode == 0x47 || pkt.opcode == 0x58 {
                        // OP_REQUESTPARTS / EMULE variant → send a small part
                        let sp = encoder
                            .encode_sending_part(&mule_protocol::ed2k::Ed2kSendingPart::new(
                                hash_bytes,
                                0,
                                1024,
                                bytes::Bytes::from(vec![0xABu8; 1024]),
                            ))
                            .unwrap();
                        out.clear();
                        out.push(sp.protocol);
                        out.extend_from_slice(&(sp.frame_len() as u32).to_le_bytes());
                        out.push(sp.opcode);
                        out.extend_from_slice(&sp.payload);
                        socket.write_all(&out).await.unwrap();
                    }
                }
            }
            if daemon_pubkey.is_some() && daemon_sig.is_some() {
                break;
            }
        }
        (daemon_pubkey, daemon_sig)
    });

    // Connect outbound to the mock peer.
    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_none(),
        "sources.connect failed: {:?}",
        resp.error
    );

    let (daemon_pubkey, daemon_sig) = mock.await.unwrap();
    let daemon_pubkey = daemon_pubkey.expect("Daemon must send OP_PUBLICKEY on state=2 challenge");
    let daemon_sig = daemon_sig.expect("Daemon must send OP_SIGNATURE answering the challenge");

    // The signature must verify against the peer's pubkey over
    // `peer_pubkey || challenge_le` and must have been made by OUR key.
    let payload = mule_crypto::legacy_sui::build_sui_signing_payload(
        mule_crypto::legacy_sui::SecureIdentVersion::V1,
        &peer_pub_der,
        challenge_val,
        None,
        None,
    )
    .unwrap();
    let daemon_pubkey_parsed =
        mule_crypto::legacy_sui::parse_peer_public_key(&daemon_pubkey).unwrap();
    mule_crypto::legacy_sui::verify_sui_signature(&daemon_pubkey_parsed, &payload, &daemon_sig)
        .expect("Daemon signature must verify under its own public key");

    // The pubkey sent must be the daemon's actual identity key.
    let daemon_real_pub = {
        let id = state.identity.lock().await;
        id.public_key_bytes().to_vec()
    };
    assert_eq!(daemon_pubkey, daemon_real_pub);
}

/// Outbound verifier role: when the peer announces SecIdent support in its
/// HELLO_ANSWER (MiscOptions1 bits 16-19), the daemon issues an
/// OP_SECIDENTSTATE challenge after the handshake — mirroring eMule
/// `InfoPacketsReceived` / `SendSecIdentStatePacket` (state=2 = need key+sig).
#[tokio::test]
async fn test_outbound_secure_ident_challenges_peer_when_advertised() {
    let (state, _dir, hash) = setup_secure_ident_test_state().await;
    let mut authenticated = true;
    state
        .network_policy
        .set_profile(mule_network_policy::NetworkProfile::LocalLab);

    let link = format!("ed2k://|file|outbound_sui_chall.bin|9728001|{hash}|/");
    let resp = handle_request(
        &state,
        make_req(0, "downloads.addEd2kLink", Some(json!({ "link": link }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(resp.error.is_none());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer_addr = listener.local_addr().unwrap();
    let hash_bytes: [u8; 16] = hex::decode(&hash).unwrap().try_into().unwrap();

    let mock = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut read_buf = bytes::BytesMut::with_capacity(65536);
        let mut raw_buf = [0u8; 8192];
        let codec = PacketCodec::new();
        let encoder = Ed2kEncoder::default();
        let decoder = Ed2kDecoder::default();
        let mut out = Vec::new();

        // Wait for OP_HELLO; it must advertise SecIdent=3 (bits 16-19).
        let mut hello_secident = 0u8;
        while hello_secident == 0 {
            let n = socket.read(&mut raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO {
                    let hello = decoder
                        .decode_hello(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                        .unwrap();
                    let info = mule_protocol::ed2k::Ed2kPeerInfo::from_hello(&hello);
                    hello_secident = info.secure_ident_support();
                }
            }
        }

        // HELLO_ANSWER advertising SecIdent=3 in MiscOptions1.
        let misc1: u32 = (1 << 29)
            | (1 << 28)
            | (4 << 24)
            | (1 << 20)
            | (3 << 16)
            | (4 << 12)
            | (2 << 8)
            | (1 << 4)
            | (1 << 1)
            | 1;
        let ha = encoder
            .encode_hello_answer(&Ed2kHelloAnswer {
                user_hash: [0x77u8; 16],
                client_id: 0x0100007F,
                tcp_port: peer_addr.port(),
                tags: vec![mule_protocol::ed2k::tags::Ed2kTag {
                    name: mule_protocol::ed2k::tags::TagName::Id(
                        mule_protocol::ed2k::tags::TAG_SPECIAL_EMULE_MISCOPTIONS1,
                    ),
                    value: mule_protocol::ed2k::tags::TagValue::UInt32(misc1),
                }],
                unknown_tags: vec![],
                server_ip: 0,
                server_port: 0,
            })
            .unwrap();
        out.push(ha.protocol);
        out.extend_from_slice(&(ha.frame_len() as u32).to_le_bytes());
        out.push(ha.opcode);
        out.extend_from_slice(&ha.payload);
        socket.write_all(&out).await.unwrap();

        // Capture the daemon's OP_SECIDENTSTATE challenge; keep the session
        // minimally alive (file status) so the loop keeps reading.
        let mut got_challenge: Option<mule_protocol::ed2k::Ed2kSecIdentState> = None;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        while got_challenge.is_none() && std::time::Instant::now() < deadline {
            let n = match tokio::time::timeout(
                std::time::Duration::from_secs(10),
                socket.read(&mut raw_buf),
            )
            .await
            {
                Ok(Ok(n)) if n > 0 => n,
                _ => break,
            };
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
                if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                    got_challenge = Some(
                        decoder
                            .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                            .unwrap(),
                    );
                    continue;
                }
                if pkt.protocol == PR_ED2K && pkt.opcode == OP_STARTUPLOADREQ {
                    out.clear();
                    out.push(PR_ED2K);
                    out.extend_from_slice(&1u32.to_le_bytes());
                    out.push(OP_ACCEPTUPLOADREQ);
                    socket.write_all(&out).await.unwrap();
                }
                if pkt.protocol == PR_ED2K && (pkt.opcode == 0x4F || pkt.opcode == 0x50) {
                    let fs = encoder
                        .encode_file_status(&mule_protocol::ed2k::Ed2kFileStatus {
                            file_hash: hash_bytes,
                            part_count: 2,
                            availability_mask:
                                mule_protocol::ed2k::PartAvailabilityMask::from_parts(&[0, 1], 2),
                        })
                        .unwrap();
                    out.clear();
                    out.push(fs.protocol);
                    out.extend_from_slice(&(fs.frame_len() as u32).to_le_bytes());
                    out.push(fs.opcode);
                    out.extend_from_slice(&fs.payload);
                    socket.write_all(&out).await.unwrap();
                }
            }
        }
        (hello_secident, got_challenge)
    });

    let req = make_req(
        1,
        "sources.connect",
        Some(json!({ "fileHash": hash, "address": peer_addr.to_string() })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_none(),
        "sources.connect failed: {:?}",
        resp.error
    );

    let (hello_secident, got_challenge) = mock.await.unwrap();
    assert_eq!(hello_secident, 3, "daemon HELLO must advertise SecIdent=3");
    let challenge =
        got_challenge.expect("daemon must issue OP_SECIDENTSTATE to a SecIdent-capable peer");
    assert_eq!(
        challenge.state, 2,
        "challenge must request key+signature (IS_KEYANDSIGNEEDED)"
    );
    assert_ne!(challenge.challenge, 0, "challenge must be non-zero");
}

/// Tier-1 daemon corpus: oversized pubkey ignored, signature without challenge
/// rejected, valid signature accepted, replay rejected (one signature per IP —
/// eMule marks `m_dwLastSignatureIP` once VerifyIdent is reached).
/// NB: a wrong-challenge attempt consumes the IP's single signature slot, so it
/// lives in its own test below.
#[tokio::test]
async fn test_inbound_sui_tier1_malformed_replay_and_wrong_challenge() {
    let (state, _dir, _hash) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    let daemon_pubkey_der = {
        let id = state.identity.lock().await;
        id.public_key_bytes().to_vec()
    };

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    let hello_pkt = encoder
        .encode_hello(&Ed2kHello {
            user_hash: [0x42u8; 16],
            client_id: 0x0100007F,
            tcp_port: 39735,
            tags: vec![],
            unknown_tags: vec![],
            server_ip: 0,
            server_port: 0,
        })
        .unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client.write_all(&out).await.unwrap();

    let mut hello_ok = false;
    while !hello_ok {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO_ANSWER {
                hello_ok = true;
            }
        }
    }

    async fn send_frame(client: &mut TcpStream, protocol: u8, opcode: u8, payload: &[u8]) {
        let mut out = Vec::new();
        out.push(protocol);
        out.extend_from_slice(&((payload.len() as u32 + 1).to_le_bytes()));
        out.push(opcode);
        out.extend_from_slice(payload);
        client.write_all(&out).await.unwrap();
    }

    async fn read_state(
        client: &mut TcpStream,
        read_buf: &mut bytes::BytesMut,
        raw_buf: &mut [u8; 8192],
        codec: &PacketCodec,
        decoder: &Ed2kDecoder,
    ) -> u8 {
        loop {
            let n = client.read(raw_buf).await.unwrap();
            assert!(n > 0);
            read_buf.extend_from_slice(&raw_buf[..n]);
            while let Ok(Some(pkt)) = codec.decode(read_buf) {
                if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                    return decoder
                        .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                        .unwrap()
                        .state;
                }
            }
        }
    }

    // A. Oversized public key (>80 bytes) is ignored → no challenge issued;
    //    a signature then hits "no active challenge" → state 0.
    let mut oversized = vec![200u8];
    oversized.extend(std::iter::repeat(0x77u8).take(200));
    send_frame(&mut client, PR_EMULE, OP_PUBLICKEY, &oversized).await;
    let mut sig_payload = vec![48u8];
    sig_payload.extend(std::iter::repeat(0xEEu8).take(48));
    send_frame(&mut client, PR_EMULE, OP_SIGNATURE, &sig_payload).await;
    assert_eq!(
        read_state(&mut client, &mut read_buf, &mut raw_buf, &codec, &decoder).await,
        0,
        "signature without valid challenge must be rejected"
    );

    // B. Real RSA-384 peer: valid pubkey → challenge issued.
    let peer_key = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 384).unwrap();
    use rsa::pkcs1::EncodeRsaPublicKey;
    let peer_pub_der = rsa::RsaPublicKey::from(&peer_key)
        .to_pkcs1_der()
        .unwrap()
        .as_bytes()
        .to_vec();
    let pk_pkt = encoder
        .encode_public_key(&Ed2kPublicKey {
            key_bytes: bytes::Bytes::from(peer_pub_der.clone()),
        })
        .unwrap();
    send_frame(&mut client, pk_pkt.protocol, pk_pkt.opcode, &pk_pkt.payload).await;

    let mut challenge = None;
    while challenge.is_none() {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                let st = decoder
                    .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                assert_eq!(st.state, 1);
                challenge = Some(st.challenge);
            }
        }
    }
    let challenge = challenge.unwrap();

    // C. Correct signature → verified (eMule sends no reply; proof is the
    //    credit record pinned to the pubkey).
    let good_payload = mule_crypto::legacy_sui::build_sui_signing_payload(
        mule_crypto::legacy_sui::SecureIdentVersion::V1,
        &daemon_pubkey_der,
        challenge,
        None,
        None,
    )
    .unwrap();
    let good_sig = mule_crypto::legacy_sui::sign_sui_payload(&peer_key, &good_payload).unwrap();
    let sp2 = encoder
        .encode_signature(&Ed2kSignature {
            signature_bytes: bytes::Bytes::from(good_sig.clone()),
        })
        .unwrap();
    send_frame(&mut client, sp2.protocol, sp2.opcode, &sp2.payload).await;

    // Credit record pinned = success was applied.
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let pk_hash = hex::encode(Md4::digest(&peer_pub_der));
    assert!(
        state
            .downloads
            .lock()
            .await
            .get_peer_credit(&pk_hash)
            .unwrap()
            .is_some(),
        "verified peer must have a credit record"
    );

    // E. Replay: a second signature from the same IP is flood-rejected.
    send_frame(&mut client, sp2.protocol, sp2.opcode, &sp2.payload).await;
    assert_eq!(
        read_state(&mut client, &mut read_buf, &mut raw_buf, &codec, &decoder).await,
        0,
        "replayed signature must be flood-rejected"
    );

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_peer_credit_persistence_and_multiplier_formula() {
    let (state, _dir, _hash) = setup_secure_ident_test_state().await;

    let pubkey_a = vec![0xAAu8; 80];
    let hash_a = hex::encode(Md4::digest(&pubkey_a));
    let pubkey_a_hex = hex::encode(&pubkey_a);

    // Initial state: no record
    {
        let dls = state.downloads.lock().await;
        assert!(dls.get_peer_credit(&hash_a).unwrap().is_none());
    }

    // Record upload 1 MB
    {
        let mut dls = state.downloads.lock().await;
        dls.record_credit_upload(&hash_a, &pubkey_a_hex, 1048576)
            .unwrap();
    }

    // Ratio when download is 0 is 1.0x
    {
        let dls = state.downloads.lock().await;
        let c = dls.get_peer_credit(&hash_a).unwrap().unwrap();
        assert_eq!(c.uploaded_bytes, 1048576);
        assert_eq!(c.downloaded_bytes, 0);
        assert_eq!(c.credit_multiplier(), 1.0);
    }

    // Record download 3 MB -> ratio: 2 * 3 / (1 + 1) = 6.0x
    {
        let mut dls = state.downloads.lock().await;
        dls.record_credit_download(&hash_a, &pubkey_a_hex, 3145728)
            .unwrap();
    }

    {
        let dls = state.downloads.lock().await;
        let c = dls.get_peer_credit(&hash_a).unwrap().unwrap();
        assert_eq!(c.uploaded_bytes, 1048576);
        assert_eq!(c.downloaded_bytes, 3145728);
        assert!((c.credit_multiplier() - 6.0).abs() < 0.01);
    }

    // Record huge download -> ratio capped at 10.0x
    {
        let mut dls = state.downloads.lock().await;
        dls.record_credit_download(&hash_a, &pubkey_a_hex, 100000000)
            .unwrap();
    }

    {
        let dls = state.downloads.lock().await;
        let c = dls.get_peer_credit(&hash_a).unwrap().unwrap();
        assert_eq!(c.credit_multiplier(), 10.0);
    }
}

#[tokio::test]
async fn test_secure_ident_and_credits_rpc_endpoints() {
    let (state, _dir, _hash) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    // 1. Check identity.status
    let id_resp = handle_request(
        &state,
        make_req(1, "identity.status", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(id_resp.error.is_none());
    let id_val = id_resp.result.unwrap();
    assert_eq!(id_val["enabled"], true);
    assert_eq!(id_val["hasKey"], true);
    assert_eq!(id_val["keyBits"], 384);
    assert!(id_val["pubkeyHash"].as_str().is_some());
    // Invariant: NEVER exposes private key in RPC
    assert!(id_val.get("privateKey").is_none());
    assert!(id_val.get("keyBytes").is_none());

    // 2. Populate some credits
    let pk1 = vec![0x11u8; 80];
    let pk2 = vec![0x22u8; 80];
    let h1 = hex::encode(Md4::digest(&pk1));
    let h2 = hex::encode(Md4::digest(&pk2));
    {
        let mut dls = state.downloads.lock().await;
        dls.record_credit_upload(&h1, &hex::encode(&pk1), 5000)
            .unwrap();
        dls.record_credit_download(&h2, &hex::encode(&pk2), 20000)
            .unwrap();
    }

    // 3. Test credits.list
    let list_resp = handle_request(
        &state,
        make_req(2, "credits.list", Some(json!({ "limit": 10 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(list_resp.error.is_none());
    let list_val = list_resp.result.unwrap();
    assert_eq!(list_val["total"], 2);
    let credits_arr = list_val["credits"].as_array().unwrap();
    assert_eq!(credits_arr.len(), 2);

    // 4. Test credits.stats
    let stats_resp = handle_request(
        &state,
        make_req(3, "credits.stats", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(stats_resp.error.is_none());
    let stats_val = stats_resp.result.unwrap();
    assert_eq!(stats_val["totalPeers"], 2);
    assert_eq!(stats_val["totalUploadedBytes"], 5000);
    assert_eq!(stats_val["totalDownloadedBytes"], 20000);
    assert_eq!(stats_val["activeCreditedPeers"], 2);
}

#[tokio::test]
async fn test_dual_sandboxed_nodes_distinct_keys_and_restart_persistence() {
    let (node_a, dir_a, _) = setup_secure_ident_test_state().await;
    let (node_b, dir_b, _) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    // 1. Get identity status for Node A and Node B
    let id_a = handle_request(
        &node_a,
        make_req(1, "identity.status", None),
        &mut authenticated,
    )
    .await
    .unwrap()
    .result
    .unwrap();
    let id_b = handle_request(
        &node_b,
        make_req(1, "identity.status", None),
        &mut authenticated,
    )
    .await
    .unwrap()
    .result
    .unwrap();

    let hash_a = id_a["pubkeyHash"].as_str().unwrap();
    let hash_b = id_b["pubkeyHash"].as_str().unwrap();

    // Invariant: Two independent nodes must have distinct public keys and hashes
    assert_ne!(
        hash_a, hash_b,
        "Two sandboxed nodes must have unique public keys"
    );

    // 2. Simulate Daemon Restart for Node A in the same data directory
    let token = "test_token_secure_ident".to_string();
    let config_a_restarted = DaemonConfig::new(dir_a.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable);
    let downloads_restarted = DownloadService::in_memory(dir_a.path().join("temp2")).unwrap();
    let node_a_restarted = AppState::new(config_a_restarted, downloads_restarted);

    let id_a_restarted = handle_request(
        &node_a_restarted,
        make_req(2, "identity.status", None),
        &mut authenticated,
    )
    .await
    .unwrap()
    .result
    .unwrap();
    let hash_a_restarted = id_a_restarted["pubkeyHash"].as_str().unwrap();

    // Invariant: Daemon restart must preserve identical cryptkey.dat and pubkeyHash
    assert_eq!(
        hash_a, hash_a_restarted,
        "Daemon restart in same directory must retain exact public key"
    );

    // Keep dirs alive until test completes
    let _ = (dir_a, dir_b);
}

#[tokio::test]
async fn test_key_rotation_creates_fresh_identity_and_detaches_credits() {
    let (node, _dir, _) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    // 1. Check initial identity
    let id1 = handle_request(
        &node,
        make_req(1, "identity.status", None),
        &mut authenticated,
    )
    .await
    .unwrap()
    .result
    .unwrap();
    let hash1 = id1["pubkeyHash"].as_str().unwrap().to_string();

    // 2. Populate credits under old identity hash1
    {
        let mut dls = node.downloads.lock().await;
        dls.record_credit_download(&hash1, "mock_pubkey_hex_1", 10000000)
            .unwrap();
        let cred = dls.get_peer_credit(&hash1).unwrap().unwrap();
        assert_eq!(cred.credit_multiplier(), 10.0);
    }

    // 3. Trigger voluntary key rotation
    let rotate_resp = handle_request(
        &node,
        make_req(2, "identity.rotate", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(rotate_resp.error.is_none());
    let id2 = rotate_resp.result.unwrap();
    let hash2 = id2["pubkeyHash"].as_str().unwrap().to_string();

    // Invariant: Rotated key has a different pubkey hash
    assert_ne!(hash1, hash2, "Rotated key must yield new pubkey hash");

    // 4. Verify new identity status
    let id_current = handle_request(
        &node,
        make_req(3, "identity.status", None),
        &mut authenticated,
    )
    .await
    .unwrap()
    .result
    .unwrap();
    assert_eq!(id_current["pubkeyHash"].as_str().unwrap(), hash2);

    // 5. Query credits for new key -> fresh identity has no inherited credits
    {
        let dls = node.downloads.lock().await;
        assert!(dls.get_peer_credit(&hash2).unwrap().is_none());
    }
}

#[tokio::test]
async fn test_invalid_and_corrupt_signature_rejection_and_fallback() {
    let (state, _dir, hash) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client_socket = TcpStream::connect(&listen_addr_str).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();

    // 1. Send OP_HELLO
    let hello = Ed2kHello {
        user_hash: [0x55u8; 16],
        client_id: 0x0100007F,
        tcp_port: 39735,
        tags: vec![],
        unknown_tags: vec![],
        server_ip: 0,
        server_port: 0,
    };
    let hello_pkt = encoder.encode_hello(&hello).unwrap();
    let mut out = Vec::new();
    out.push(hello_pkt.protocol);
    out.extend_from_slice(&(hello_pkt.frame_len() as u32).to_le_bytes());
    out.push(hello_pkt.opcode);
    out.extend_from_slice(&hello_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    let mut received_hello_ans = false;
    while !received_hello_ans {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO_ANSWER {
                received_hello_ans = true;
                break;
            }
        }
    }

    // 2. Send OP_PUBLICKEY
    let mock_pubkey = vec![0x77u8; 80];
    let pubkey_msg = Ed2kPublicKey {
        key_bytes: bytes::Bytes::from(mock_pubkey.clone()),
    };
    let pk_pkt = encoder.encode_public_key(&pubkey_msg).unwrap();
    out.clear();
    out.push(pk_pkt.protocol);
    out.extend_from_slice(&(pk_pkt.frame_len() as u32).to_le_bytes());
    out.push(pk_pkt.opcode);
    out.extend_from_slice(&pk_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // 3. Receive Challenge
    let mut received_challenge = false;
    while !received_challenge {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                let sec_st = decoder
                    .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                assert_eq!(sec_st.state, 1, "Must receive challenge");
                received_challenge = true;
                break;
            }
        }
    }

    // 4. Send Corrupt / Truncated signature (< 16 bytes)
    let corrupt_sig_msg = Ed2kSignature {
        signature_bytes: bytes::Bytes::from(vec![0xAAu8; 4]), // Corrupted short signature
    };
    let sig_pkt = encoder.encode_signature(&corrupt_sig_msg).unwrap();
    out.clear();
    out.push(sig_pkt.protocol);
    out.extend_from_slice(&(sig_pkt.frame_len() as u32).to_le_bytes());
    out.push(sig_pkt.opcode);
    out.extend_from_slice(&sig_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    // 5. Expect rejection (OP_SECIDENTSTATE with state = 0)
    let mut received_rejection = false;
    while !received_rejection {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                let sec_st = decoder
                    .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                assert_eq!(
                    sec_st.state, 0,
                    "Corrupt signature must be rejected with state 0"
                );
                received_rejection = true;
                break;
            }
        }
    }

    // 6. Request parts anyway -> verify peer is served as uncredited peer without crash
    let hash_bytes: [u8; 16] = hex::decode(&hash).unwrap().try_into().unwrap();
    let req_parts = Ed2kRequestParts {
        file_hash: hash_bytes,
        ranges: vec![mule_protocol::ed2k::Ed2kBlockRange {
            start: 0,
            end: 184320,
        }],
    };
    let rp_pkt = encoder.encode_request_parts(&req_parts).unwrap();
    out.clear();
    out.push(rp_pkt.protocol);
    out.extend_from_slice(&(rp_pkt.frame_len() as u32).to_le_bytes());
    out.push(rp_pkt.opcode);
    out.extend_from_slice(&rp_pkt.payload);
    client_socket.write_all(&out).await.unwrap();

    let mut received_part = false;
    while !received_part {
        let n = client_socket.read(&mut raw_buf).await.unwrap();
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_SENDINGPART {
                received_part = true;
                break;
            }
        }
    }

    // 7. Verify no credit record was created for rejected signature peer
    let pk_hash = hex::encode(Md4::digest(&mock_pubkey));
    let credit_opt = state
        .downloads
        .lock()
        .await
        .get_peer_credit(&pk_hash)
        .unwrap();
    assert!(
        credit_opt.is_none(),
        "No credit record should be saved for rejected signature"
    );

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn test_anti_starvation_uncredited_peer_advancement() {
    use mule_daemon::services::upload_queue::QueuedUpload;

    // Simulate two queued peers:
    // Peer A: uncredited (multiplier 1.0x -> 100 bps), has been waiting for 300 seconds (5 mins)
    // Peer B: maximum credit (multiplier 10.0x -> 1000 bps), has just joined (waiting 10 seconds)
    let now_ms = 1000000000000i64;

    let peer_a = QueuedUpload {
        client_key: "127.0.0.1:39735".to_string(),
        client_address: "127.0.0.1:39735".to_string(),
        file_hash: "hash_a".to_string(),
        file_name: "file.bin".to_string(),
        enqueued_at_unix_ms: now_ms - (300 * 1000), // 300s ago
        credit_multiplier_bps: 100,                 // 1.0x
    };

    let peer_b = QueuedUpload {
        client_key: "127.0.0.1:4663".to_string(),
        client_address: "127.0.0.1:4663".to_string(),
        file_hash: "hash_a".to_string(),
        file_name: "file.bin".to_string(),
        enqueued_at_unix_ms: now_ms - (10 * 1000), // 10s ago
        credit_multiplier_bps: 1000,               // 10.0x
    };

    let score_a = peer_a.effective_score(now_ms);
    let score_b = peer_b.effective_score(now_ms);

    // Score A = 300 * 100 = 30,000
    // Score B = 10 * 1000 = 10,000
    assert_eq!(score_a, 30000);
    assert_eq!(score_b, 10000);

    // Invariant: Long waiting uncredited peer beats newly arrived high-credit peer (no starvation)
    assert!(
        score_a > score_b,
        "Uncredited peer waiting 300s must rank above newly joined 10x peer waiting 10s"
    );
}

#[tokio::test]
async fn test_kill_switch_during_secure_handshake_aborts_cleanly() {
    let (state, _dir, _) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    // Start listener
    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr_str = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client_socket = TcpStream::connect(&listen_addr_str).await.unwrap();

    // Trigger Kill Switch mid-handshake
    let kill_resp = handle_request(
        &state,
        make_req(2, "network.disconnectAll", None),
        &mut authenticated,
    )
    .await
    .unwrap();
    assert!(kill_resp.error.is_none());

    // Write should fail or connection close
    let mut raw_buf = [0u8; 1024];
    let res = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client_socket.read(&mut raw_buf),
    )
    .await
    .expect("Socket read should complete or terminate within timeout");
    if let Ok(n) = res {
        assert_eq!(n, 0, "Socket must be closed on kill switch");
    }
}

/// A signature over the wrong challenge reaches VerifyIdent, fails, and consumes
/// this IP's single signature slot (eMule anti-flood) — needs its own AppState.
#[tokio::test]
async fn test_inbound_sui_wrong_challenge_rejected() {
    let (state, _dir, _hash) = setup_secure_ident_test_state().await;
    let mut authenticated = true;

    let daemon_pubkey_der = {
        let id = state.identity.lock().await;
        id.public_key_bytes().to_vec()
    };

    let start_resp = handle_request(
        &state,
        make_req(1, "listener.start", Some(json!({ "port": 0 }))),
        &mut authenticated,
    )
    .await
    .unwrap();
    let listen_addr = start_resp.result.unwrap()["listenAddress"]
        .as_str()
        .unwrap()
        .to_string();

    let mut client = TcpStream::connect(&listen_addr).await.unwrap();
    let mut read_buf = bytes::BytesMut::with_capacity(65536);
    let mut raw_buf = [0u8; 8192];
    let codec = PacketCodec::new();
    let encoder = Ed2kEncoder::default();
    let decoder = Ed2kDecoder::default();
    let mut out = Vec::new();

    let hp = encoder
        .encode_hello(&Ed2kHello {
            user_hash: [0x99u8; 16],
            client_id: 0x0100007F,
            tcp_port: 39735,
            tags: vec![],
            unknown_tags: vec![],
            server_ip: 0,
            server_port: 0,
        })
        .unwrap();
    out.push(hp.protocol);
    out.extend_from_slice(&(hp.frame_len() as u32).to_le_bytes());
    out.push(hp.opcode);
    out.extend_from_slice(&hp.payload);
    client.write_all(&out).await.unwrap();
    let mut hello_ok = false;
    while !hello_ok {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_ED2K && pkt.opcode == OP_HELLO_ANSWER {
                hello_ok = true;
            }
        }
    }

    // Real RSA-384 key + pubkey → challenge.
    let peer_key = rsa::RsaPrivateKey::new(&mut rsa::rand_core::OsRng, 384).unwrap();
    use rsa::pkcs1::EncodeRsaPublicKey;
    let peer_pub_der = rsa::RsaPublicKey::from(&peer_key)
        .to_pkcs1_der()
        .unwrap()
        .as_bytes()
        .to_vec();
    let pk_pkt = encoder
        .encode_public_key(&Ed2kPublicKey {
            key_bytes: bytes::Bytes::from(peer_pub_der),
        })
        .unwrap();
    out.clear();
    out.push(pk_pkt.protocol);
    out.extend_from_slice(&(pk_pkt.frame_len() as u32).to_le_bytes());
    out.push(pk_pkt.opcode);
    out.extend_from_slice(&pk_pkt.payload);
    client.write_all(&out).await.unwrap();

    let mut challenge = None;
    while challenge.is_none() {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                challenge = Some(
                    decoder
                        .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                        .unwrap()
                        .challenge,
                );
            }
        }
    }

    // Sign a DIFFERENT challenge than the one issued.
    let bad_payload = mule_crypto::legacy_sui::build_sui_signing_payload(
        mule_crypto::legacy_sui::SecureIdentVersion::V1,
        &daemon_pubkey_der,
        challenge.unwrap().wrapping_add(1),
        None,
        None,
    )
    .unwrap();
    let bad_sig = mule_crypto::legacy_sui::sign_sui_payload(&peer_key, &bad_payload).unwrap();
    let sp = encoder
        .encode_signature(&Ed2kSignature {
            signature_bytes: bytes::Bytes::from(bad_sig),
        })
        .unwrap();
    out.clear();
    out.push(sp.protocol);
    out.extend_from_slice(&(sp.frame_len() as u32).to_le_bytes());
    out.push(sp.opcode);
    out.extend_from_slice(&sp.payload);
    client.write_all(&out).await.unwrap();

    // Expect rejection (state 0).
    loop {
        let n = client.read(&mut raw_buf).await.unwrap();
        assert!(n > 0);
        read_buf.extend_from_slice(&raw_buf[..n]);
        let mut got = false;
        while let Ok(Some(pkt)) = codec.decode(&mut read_buf) {
            if pkt.protocol == PR_EMULE && pkt.opcode == OP_SECIDENTSTATE {
                let st = decoder
                    .decode_secident_state(pkt.protocol, pkt.opcode, &pkt.payload, 0)
                    .unwrap();
                assert_eq!(st.state, 0, "wrong-challenge signature must be rejected");
                got = true;
            }
        }
        if got {
            break;
        }
    }

    let _ = handle_request(
        &state,
        make_req(2, "listener.stop", None),
        &mut authenticated,
    )
    .await
    .unwrap();
}
