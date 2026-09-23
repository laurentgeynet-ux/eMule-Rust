//! Integration tests verifying local interoperability with reference-compliant eD2K peers on loopback.
//! Phase 6.5: Zero public network, strictly TCP 127.0.0.1. Zero data leakage.

use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use mule_protocol::ed2k::opcodes::{
    OP_FILESTATUS, OP_HELLO, OP_HELLOANSWER, OP_REQUESTPARTS_EMULE, OP_SENDINGPART,
    OP_SETREQFILEID_EMULE,
};
use mule_protocol::ed2k::{
    expected_mask_bytes, opcode_to_str, DivergenceReport, Ed2kCompatibilityProfile,
    EmulatedReferencePeer, PartAvailabilityMask, ReferenceInteropClient, ReferencePeerConfig,
    TestPeerMode, TraceDirection, TraceRecord,
};

#[test]
fn test_reference_peer_config_non_loopback_rejected() {
    let public_addr: SocketAddr = "8.8.8.8:39735".parse().unwrap();
    let res = ReferencePeerConfig::new(public_addr);
    assert!(
        res.is_err(),
        "Non-loopback IP 8.8.8.8 must be rejected immediately"
    );

    let lan_addr: SocketAddr = "192.168.1.50:39735".parse().unwrap();
    let res_lan = ReferencePeerConfig::new(lan_addr);
    assert!(
        res_lan.is_err(),
        "LAN IP 192.168.1.50 must be rejected immediately"
    );

    let zero_port: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let res_zero_port = ReferencePeerConfig::new(zero_port);
    assert!(
        res_zero_port.is_err(),
        "Port 0 must be rejected for reference peer"
    );

    let valid_loopback: SocketAddr = "127.0.0.1:39735".parse().unwrap();
    let res_valid = ReferencePeerConfig::new(valid_loopback);
    assert!(res_valid.is_ok());
}

#[tokio::test]
async fn test_reference_interop_handshake_nominal_and_anonymized_traces() {
    let test_peer = EmulatedReferencePeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();
    let (peer_handle, peer_stopped) = test_peer.spawn_service();

    let config = ReferencePeerConfig::new(peer_addr)
        .expect("valid loopback config")
        .with_handshake_timeout(Duration::from_millis(1500));

    let local_hash = [0x11; 16];
    let (interop_client, peer_info) =
        ReferenceInteropClient::connect_and_handshake(config, local_hash, "eMuleRustRefTest")
            .await
            .expect("handshake with reference test peer succeeds");

    assert!(peer_info.name.is_some());
    assert!(peer_info.version.is_some());

    let traces = interop_client.traces();
    assert_eq!(traces.len(), 2);
    assert_eq!(traces[0].direction, TraceDirection::Outbound);
    assert_eq!(traces[0].opcode, OP_HELLO);
    assert_eq!(traces[1].direction, TraceDirection::Inbound);
    assert_eq!(traces[1].opcode, OP_HELLOANSWER);

    let report = interop_client.finish().await;
    assert!(
        report.validate_anonymity().is_ok(),
        "Handshake traces must be fully anonymized"
    );
    assert!(report.opcodes_observed.contains(&OP_HELLO));
    assert!(report.opcodes_observed.contains(&OP_HELLOANSWER));

    peer_stopped.store(true, std::sync::atomic::Ordering::SeqCst);
    peer_handle.abort();
}

#[tokio::test]
async fn test_reference_interop_file_status_exchange_and_bitmask() {
    let test_peer = EmulatedReferencePeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();

    let file_hash = [0xAA; 16];
    let part_count = 18;
    // Parts 0, 1, 7, 8, 17 available
    let available_parts = vec![0, 1, 7, 8, 17];
    let mask = PartAvailabilityMask::from_parts(&available_parts, part_count);

    test_peer
        .add_shared_file(file_hash, part_count, mask.clone())
        .await;

    let (peer_handle, peer_stopped) = test_peer.spawn_service();

    let config = ReferencePeerConfig::new(peer_addr).expect("valid config");
    let local_hash = [0x22; 16];
    let (mut interop_client, _peer_info) = ReferenceInteropClient::connect_and_handshake(
        config,
        local_hash,
        "eMuleRustFileStatusTest",
    )
    .await
    .expect("handshake succeeds");

    let status = interop_client
        .request_file_status(file_hash)
        .await
        .expect("file status request succeeds");

    assert_eq!(status.file_hash, file_hash);
    assert_eq!(status.part_count, part_count);
    assert_eq!(
        status.availability_mask.raw_bytes().len(),
        expected_mask_bytes(part_count)
    );
    assert_eq!(status.availability_mask, mask);

    for p in 0..part_count {
        let expected_available = available_parts.contains(&p);
        assert_eq!(
            status.availability_mask.has_part(p),
            expected_available,
            "Part {p} availability mismatch"
        );
    }

    let report = interop_client.finish().await;
    assert!(report.validate_anonymity().is_ok());
    assert!(report.opcodes_observed.contains(&OP_SETREQFILEID_EMULE));
    assert!(report.opcodes_observed.contains(&OP_FILESTATUS));
    assert!(report.canonical_file_status_req_detected);

    peer_stopped.store(true, std::sync::atomic::Ordering::SeqCst);
    peer_handle.abort();
}

#[tokio::test]
async fn test_reference_interop_block_transfer_canonical_opcodes() {
    let test_peer = EmulatedReferencePeer::bind(TestPeerMode::Nominal)
        .await
        .expect("bind test peer");
    let peer_addr = test_peer.local_addr();

    let file_hash = [0xBB; 16];
    let total_size = 500_000u64;

    // Generate deterministic test pattern
    let mut file_data = Vec::with_capacity(total_size as usize);
    for i in 0..total_size {
        file_data.push((i % 256) as u8);
    }
    let fixture_bytes = Bytes::from(file_data);

    test_peer
        .add_shared_file(file_hash, 1, PartAvailabilityMask::from_parts(&[0], 1))
        .await;
    test_peer
        .add_shared_file_data(file_hash, fixture_bytes.clone())
        .await;

    let (peer_handle, peer_stopped) = test_peer.spawn_service();

    let config = ReferencePeerConfig::new(peer_addr).expect("valid config");
    let local_hash = [0x33; 16];
    let (mut interop_client, _peer_info) =
        ReferenceInteropClient::connect_and_handshake(config, local_hash, "eMuleRustTransferTest")
            .await
            .expect("handshake succeeds");

    // Request 1st block: [0, 184320)
    let part1 = interop_client
        .request_and_receive_block(file_hash, 0, 184_320, total_size)
        .await
        .expect("receive part 1");

    assert_eq!(part1.file_hash, file_hash);
    assert_eq!(part1.start_offset, 0);
    assert_eq!(part1.end_offset, 184_320);
    assert_eq!(part1.data.len(), 184_320);
    assert_eq!(part1.data.as_ref(), &fixture_bytes[0..184_320]);

    // Request 2nd block: [184320, 368640)
    let part2 = interop_client
        .request_and_receive_block(file_hash, 184_320, 368_640, total_size)
        .await
        .expect("receive part 2");

    assert_eq!(part2.file_hash, file_hash);
    assert_eq!(part2.start_offset, 184_320);
    assert_eq!(part2.end_offset, 368_640);
    assert_eq!(part2.data.len(), 184_320);
    assert_eq!(part2.data.as_ref(), &fixture_bytes[184_320..368_640]);

    // Request 3rd (residual) block: [368640, 500000) (131360 bytes)
    let part3 = interop_client
        .request_and_receive_block(file_hash, 368_640, 500_000, total_size)
        .await
        .expect("receive part 3");

    assert_eq!(part3.file_hash, file_hash);
    assert_eq!(part3.start_offset, 368_640);
    assert_eq!(part3.end_offset, 500_000);
    assert_eq!(part3.data.len(), 131_360);
    assert_eq!(part3.data.as_ref(), &fixture_bytes[368_640..500_000]);

    let report = interop_client.finish().await;
    assert!(report.validate_anonymity().is_ok());
    assert!(report.canonical_request_parts_detected);
    assert!(report.opcodes_observed.contains(&OP_REQUESTPARTS_EMULE));
    assert!(report.opcodes_observed.contains(&OP_SENDINGPART));

    peer_stopped.store(true, std::sync::atomic::Ordering::SeqCst);
    peer_handle.abort();
}

#[test]
fn test_reference_interop_trace_anonymity_enforcement() {
    let clean_record = TraceRecord {
        seq: 0,
        elapsed_ms: 10,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
        direction: TraceDirection::Outbound,
        opcode: OP_HELLO,
        opcode_name: opcode_to_str(OP_HELLO),
        payload_len: 20,
        tags_summary: vec![
            "TAG_NAME:Anonymous".to_string(),
            "TAG_VERSION:60".to_string(),
        ],
        file_hash_hex: None,
        range_summary: None,
    };
    assert!(clean_record.is_clean());

    let normalized_json = clean_record.to_normalized_json("accepted");
    assert!(normalized_json.contains("\"schemaVersion\":1"));
    assert!(normalized_json.contains("\"testTier\":2"));
    assert!(normalized_json.contains("\"referenceKind\":\"emulated-reference-peer\""));
    assert!(normalized_json.contains("\"compatibilityProfile\":\"Emule050a\""));
    assert!(normalized_json.contains("\"opcode\":\"0x01\""));

    let dirty_record = TraceRecord {
        seq: 1,
        elapsed_ms: 20,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
        direction: TraceDirection::Outbound,
        opcode: OP_HELLO,
        opcode_name: opcode_to_str(OP_HELLO),
        payload_len: 20,
        tags_summary: vec!["PATH:C:\\Users\\Bob\\Downloads".to_string()],
        file_hash_hex: None,
        range_summary: None,
    };
    assert!(!dirty_record.is_clean());

    let report = DivergenceReport {
        profile: Ed2kCompatibilityProfile::Emule050a,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        traces: vec![clean_record, dirty_record],
        opcodes_observed: vec![OP_HELLO],
        canonical_request_parts_detected: false,
        canonical_file_status_req_detected: false,
        notes: vec![],
    };
    assert!(report.validate_anonymity().is_err());
}

#[test]
fn test_reference_interop_normalized_json_snapshots_and_ci_report() {
    let traces = vec![
        TraceRecord {
            seq: 0,
            elapsed_ms: 5,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
            direction: TraceDirection::Outbound,
            opcode: OP_HELLO,
            opcode_name: opcode_to_str(OP_HELLO),
            payload_len: 30,
            tags_summary: vec!["TAG_NAME:Anonymous".to_string()],
            file_hash_hex: None,
            range_summary: None,
        },
        TraceRecord {
            seq: 1,
            elapsed_ms: 12,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
            direction: TraceDirection::Inbound,
            opcode: OP_HELLOANSWER,
            opcode_name: opcode_to_str(OP_HELLOANSWER),
            payload_len: 25,
            tags_summary: vec!["TAG_NAME:eMuleReferencePeer".to_string()],
            file_hash_hex: None,
            range_summary: None,
        },
        TraceRecord {
            seq: 2,
            elapsed_ms: 18,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
            direction: TraceDirection::Outbound,
            opcode: OP_SETREQFILEID_EMULE,
            opcode_name: opcode_to_str(OP_SETREQFILEID_EMULE),
            payload_len: 16,
            tags_summary: vec![],
            file_hash_hex: Some("11223344556677889900aabbccddeeff".to_string()),
            range_summary: None,
        },
        TraceRecord {
            seq: 3,
            elapsed_ms: 25,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
            direction: TraceDirection::Inbound,
            opcode: OP_FILESTATUS,
            opcode_name: opcode_to_str(OP_FILESTATUS),
            payload_len: 19,
            tags_summary: vec![],
            file_hash_hex: Some("11223344556677889900aabbccddeeff".to_string()),
            range_summary: None,
        },
        TraceRecord {
            seq: 4,
            elapsed_ms: 32,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
            direction: TraceDirection::Outbound,
            opcode: OP_REQUESTPARTS_EMULE,
            opcode_name: opcode_to_str(OP_REQUESTPARTS_EMULE),
            payload_len: 40,
            tags_summary: vec![],
            file_hash_hex: Some("11223344556677889900aabbccddeeff".to_string()),
            range_summary: Some((0, 184_320)),
        },
        TraceRecord {
            seq: 5,
            elapsed_ms: 45,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
            direction: TraceDirection::Inbound,
            opcode: OP_SENDINGPART,
            opcode_name: opcode_to_str(OP_SENDINGPART),
            payload_len: 184_344,
            tags_summary: vec![],
            file_hash_hex: Some("11223344556677889900aabbccddeeff".to_string()),
            range_summary: Some((0, 184_320)),
        },
    ];

    let report = DivergenceReport {
        profile: Ed2kCompatibilityProfile::Emule050a,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        traces,
        opcodes_observed: vec![
            OP_HELLO,
            OP_HELLOANSWER,
            OP_SETREQFILEID_EMULE,
            OP_FILESTATUS,
            OP_REQUESTPARTS_EMULE,
            OP_SENDINGPART,
        ],
        canonical_request_parts_detected: true,
        canonical_file_status_req_detected: true,
        notes: vec!["Hermetic loopback reference peer run successful".to_string()],
    };

    // 1. Verify anonymity
    assert!(report.validate_anonymity().is_ok());

    // 2. Verify CI-readable report format
    let ci_summary = report.format_ci_summary();
    assert!(ci_summary.contains("=== ED2K REFERENCE INTEROP SUMMARY ==="));
    assert!(ci_summary.contains("Schema Version: 1"));
    assert!(ci_summary.contains("Test Tier: 2"));
    assert!(ci_summary.contains("Reference Kind: emulated-reference-peer"));
    assert!(ci_summary.contains("Compatibility Profile: Emule050a"));
    assert!(ci_summary.contains("Total Traces: 6"));
    assert!(ci_summary.contains("Canonical 0x47 Detected: true"));
    assert!(ci_summary.contains("Canonical 0x4F Detected: true"));
    assert!(ci_summary.contains("Anonymity Validated: true"));
    assert!(ci_summary.contains("Hermetic loopback reference peer run successful"));
    assert!(ci_summary.contains("=== END SUMMARY ==="));

    // 3. Verify normalized JSON records conform to required schema
    let json_lines = report.to_normalized_json_lines();
    assert_eq!(json_lines.len(), 6);

    // Verify wire trace schema on record 4 (RequestParts 0x47)
    let trace_4_json: serde_json::Value =
        serde_json::from_str(&json_lines[4]).expect("valid JSON for trace 4");
    assert_eq!(trace_4_json["schemaVersion"], 1);
    assert_eq!(trace_4_json["testTier"], 2);
    assert_eq!(trace_4_json["referenceKind"], "emulated-reference-peer");
    assert_eq!(trace_4_json["compatibilityProfile"], "Emule050a");
    assert_eq!(trace_4_json["protocolByte"], "0xE3");
    assert_eq!(trace_4_json["opcode"], "0x47");
    assert_eq!(trace_4_json["direction"], "client_to_peer");
    assert_eq!(trace_4_json["payloadLength"], 40);
    assert_eq!(trace_4_json["outcome"], "accepted");

    // Verify wire trace schema on record 2 (SetReqFileId 0x4F)
    let trace_2_json: serde_json::Value =
        serde_json::from_str(&json_lines[2]).expect("valid JSON for trace 2");
    assert_eq!(trace_2_json["schemaVersion"], 1);
    assert_eq!(trace_2_json["testTier"], 2);
    assert_eq!(trace_2_json["referenceKind"], "emulated-reference-peer");
    assert_eq!(trace_2_json["compatibilityProfile"], "Emule050a");
    assert_eq!(trace_2_json["opcode"], "0x4F");
    assert_eq!(trace_2_json["direction"], "client_to_peer");
    assert_eq!(trace_2_json["payloadLength"], 16);

    // Verify wire trace schema on record 5 (SendingPart 0x46)
    let trace_5_json: serde_json::Value =
        serde_json::from_str(&json_lines[5]).expect("valid JSON for trace 5");
    assert_eq!(trace_5_json["schemaVersion"], 1);
    assert_eq!(trace_5_json["testTier"], 2);
    assert_eq!(trace_5_json["referenceKind"], "emulated-reference-peer");
    assert_eq!(trace_5_json["compatibilityProfile"], "Emule050a");
    assert_eq!(trace_5_json["opcode"], "0x46");
    assert_eq!(trace_5_json["direction"], "peer_to_client");
    assert_eq!(trace_5_json["payloadLength"], 184_344);
}

#[test]
fn test_reference_interop_anonymization_adversarial_patterns() {
    // 1. Valid Unicode (emojis, accents, Asian script, RTL) without leak must be CLEAN
    let unicode_clean_record = TraceRecord {
        seq: 0,
        elapsed_ms: 10,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
        direction: TraceDirection::Outbound,
        opcode: OP_HELLO,
        opcode_name: opcode_to_str(OP_HELLO),
        payload_len: 50,
        tags_summary: vec![
            "NAME:Émule-Sion 🌟 現代的なクライアント 🚀".to_string(),
            "DESC:تجربة آمنة وموثوقة".to_string(),
            "VERSION:60".to_string(),
        ],
        file_hash_hex: None,
        range_summary: None,
    };
    assert!(
        unicode_clean_record.is_clean(),
        "valid international Unicode without paths must be allowed"
    );

    // 2. Adversarial leak patterns must all be rejected by is_clean()
    let leak_samples = [
        ("Windows Drive", "PATH:C:\\secret\\file.dat"),
        ("Windows UNC", "SHARE:\\\\nas_storage\\shares\\music"),
        ("Unix /home", "CONFIG:/home/developer/.ssh/id_rsa"),
        ("Unix /Users", "PROFILE:/Users/administrator/secret.key"),
        ("Unix /tmp", "DUMP:/tmp/system_dump.raw"),
        ("Unix /var", "LOG:/var/log/messages"),
        ("Token lowercase", "AUTH:token=abcd1234efgh5678"),
        ("Token uppercase", "HEADER:TOKEN=xyz987"),
        ("Bearer Token", "AUTHORIZATION:Bearer eyJhbGciOi..."),
        ("Secret phrase", "API_KEY:secret_production_key"),
        ("Password", "CRED:password=Hunter2"),
        ("Control Null", "PAYLOAD:hello\0world"),
        ("Control Escape", "PAYLOAD:escape\x1b[31mcolor"),
    ];

    for (desc, leak_tag) in leak_samples {
        let dirty_record = TraceRecord {
            seq: 1,
            elapsed_ms: 20,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
            direction: TraceDirection::Outbound,
            opcode: OP_HELLO,
            opcode_name: opcode_to_str(OP_HELLO),
            payload_len: 30,
            tags_summary: vec![leak_tag.to_string()],
            file_hash_hex: None,
            range_summary: None,
        };
        assert!(
            !dirty_record.is_clean(),
            "leak sample '{desc}' ({leak_tag}) must be caught as dirty"
        );

        let report = DivergenceReport {
            profile: Ed2kCompatibilityProfile::Emule050a,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            traces: vec![dirty_record],
            opcodes_observed: vec![OP_HELLO],
            canonical_request_parts_detected: false,
            canonical_file_status_req_detected: false,
            notes: vec![],
        };
        assert!(
            report.validate_anonymity().is_err(),
            "validate_anonymity must fail for leak sample '{desc}'"
        );
    }
}

#[test]
fn test_safe_ci_artifact_writer_in_tempdir() {
    let clean_record = TraceRecord {
        seq: 0,
        elapsed_ms: 10,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
        direction: TraceDirection::Outbound,
        opcode: OP_HELLO,
        opcode_name: opcode_to_str(OP_HELLO),
        payload_len: 20,
        tags_summary: vec!["TAG_NAME:Anonymous".to_string()],
        file_hash_hex: None,
        range_summary: None,
    };

    let clean_report = DivergenceReport {
        profile: Ed2kCompatibilityProfile::Emule050a,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        traces: vec![clean_record],
        opcodes_observed: vec![OP_HELLO],
        canonical_request_parts_detected: false,
        canonical_file_status_req_detected: false,
        notes: vec!["Hermetic execution confirmed".to_string()],
    };

    // 1. Clean report writing into tempdir succeeds
    let temp_dir = tempfile::tempdir().expect("tempdir created");
    let (summary_path, traces_path) = clean_report
        .write_ci_report_secure(temp_dir.path())
        .expect("write_ci_report_secure succeeds on clean report");

    assert!(summary_path.exists());
    assert!(traces_path.exists());

    let summary_content = std::fs::read_to_string(&summary_path).expect("read summary");
    assert!(summary_content.contains("=== ED2K REFERENCE INTEROP SUMMARY ==="));
    assert!(summary_content.contains("Test Tier: 2"));
    assert!(summary_content.contains("Reference Kind: emulated-reference-peer"));

    let traces_content = std::fs::read_to_string(&traces_path).expect("read traces");
    assert!(traces_content.contains("\"schemaVersion\":1"));
    assert!(traces_content.contains("\"testTier\":2"));
    assert!(traces_content.contains("\"referenceKind\":\"emulated-reference-peer\""));

    // 2. Dirty report writing into tempdir is strictly rejected and creates NO files
    let dirty_record = TraceRecord {
        seq: 1,
        elapsed_ms: 20,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
        direction: TraceDirection::Outbound,
        opcode: OP_HELLO,
        opcode_name: opcode_to_str(OP_HELLO),
        payload_len: 20,
        tags_summary: vec!["PATH:C:\\Users\\admin".to_string()],
        file_hash_hex: None,
        range_summary: None,
    };

    let dirty_report = DivergenceReport {
        profile: Ed2kCompatibilityProfile::Emule050a,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        traces: vec![dirty_record],
        opcodes_observed: vec![OP_HELLO],
        canonical_request_parts_detected: false,
        canonical_file_status_req_detected: false,
        notes: vec![],
    };

    let dirty_temp_dir = tempfile::tempdir().expect("dirty tempdir created");
    let write_res = dirty_report.write_ci_report_secure(dirty_temp_dir.path());
    assert!(
        write_res.is_err(),
        "writing dirty report must fail immediately"
    );

    // Guarantee zero files written
    let file_count = std::fs::read_dir(dirty_temp_dir.path())
        .expect("read_dir")
        .count();
    assert_eq!(
        file_count, 0,
        "no files must ever be written when anonymity validation fails"
    );
}

#[test]
fn test_unicode_nfc_and_visual_sanitization_for_ci_reports() {
    use mule_protocol::ed2k::{is_bidi_or_formatting_control, sanitize_unicode_for_display};

    // 1. Legitimate international text is preserved and NFC-normalized
    let valid_samples = [
        "Nom de fichier français : été, naïf, cœur",
        "Deutsche Umlaute: Übergrößenträger",
        "日本語のファイル名: テスト.part",
        "ملف عربي تجريبي: وثيقة.txt",
        "קובץ עברי: מסמך.pdf",
        "Emoji in name: 🚀_archive_v1.tar.gz 📦",
    ];

    for sample in valid_samples {
        let sanitized = sanitize_unicode_for_display(sample);
        assert_eq!(
            sanitized, sample,
            "standard international Unicode must not be altered"
        );

        let rec = TraceRecord {
            seq: 0,
            elapsed_ms: 0,
            test_tier: 2,
            reference_kind: "emulated-reference-peer",
            compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
            direction: TraceDirection::Inbound,
            opcode: OP_HELLO,
            opcode_name: opcode_to_str(OP_HELLO),
            payload_len: 10,
            tags_summary: vec![sample.to_string()],
            file_hash_hex: None,
            range_summary: None,
        };
        assert!(
            rec.is_clean(),
            "valid international sample must be recognized as clean"
        );
    }

    // 2. Bidi overrides and isolates must be escaped to prevent terminal/CI spoofing
    let bidi_sample = "innocent_document\u{202E}fdp.exe";
    assert!(is_bidi_or_formatting_control('\u{202E}'));
    let sanitized_bidi = sanitize_unicode_for_display(bidi_sample);
    assert_eq!(sanitized_bidi, "innocent_document\\u{202E}fdp.exe");
    assert!(!sanitized_bidi.contains('\u{202E}'));

    let bidi_rec = TraceRecord {
        seq: 1,
        elapsed_ms: 1,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
        direction: TraceDirection::Inbound,
        opcode: OP_HELLO,
        opcode_name: opcode_to_str(OP_HELLO),
        payload_len: 10,
        tags_summary: vec![bidi_sample.to_string()],
        file_hash_hex: None,
        range_summary: None,
    };
    assert!(
        !bidi_rec.is_clean(),
        "bidi override must be flagged as dirty"
    );
    assert_eq!(
        bidi_rec.sanitized_tags_summary(),
        vec!["innocent_document\\u{202E}fdp.exe"]
    );

    // 3. Invisible / zero-width formatting characters must be escaped
    let zero_width_sample = "stealth\u{200B}secret\u{FEFF}file\u{00AD}test";
    let sanitized_zero = sanitize_unicode_for_display(zero_width_sample);
    assert_eq!(
        sanitized_zero,
        "stealth\\u{200B}secret\\u{FEFF}file\\u{00AD}test"
    );

    let zero_rec = TraceRecord {
        seq: 2,
        elapsed_ms: 2,
        test_tier: 2,
        reference_kind: "emulated-reference-peer",
        compatibility_profile: Ed2kCompatibilityProfile::Emule050a,
        direction: TraceDirection::Inbound,
        opcode: OP_HELLO,
        opcode_name: opcode_to_str(OP_HELLO),
        payload_len: 10,
        tags_summary: vec![zero_width_sample.to_string()],
        file_hash_hex: None,
        range_summary: None,
    };
    assert!(
        !zero_rec.is_clean(),
        "invisible characters must be flagged as dirty"
    );

    // 4. Control characters must be escaped
    let ctrl_sample = "payload\u{0000}with\u{001B}[31mred";
    let sanitized_ctrl = sanitize_unicode_for_display(ctrl_sample);
    assert_eq!(sanitized_ctrl, "payload\\u{0000}with\\u{001B}[31mred");
}
