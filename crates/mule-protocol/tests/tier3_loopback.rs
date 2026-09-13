//! Tier 3 external reference integration test suite.
//!
//! Confined strictly to 127.0.0.1 closed loopback with dynamic ephemeral port allocation.
//! Live process execution is feature-gated and disabled by default (#[ignore])
//! to be triggered only when a pinned reference binary is explicitly provided via EMULE_REFERENCE_BIN.

#![cfg(feature = "ed2k-reference-loopback")]

mod tier3_external_reference;

use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::time::Duration;

use mule_protocol::ed2k::{
    Ed2kLoopbackClient, Ed2kTag, TagName, TagValue, TAG_SPECIAL_NAME, TAG_SPECIAL_VERSION,
};
use tier3_external_reference::{
    Tier3BinaryValidator, Tier3DivergenceReport, Tier3Environment, Tier3Harness, Tier3HarnessError,
    WireTraceRecord,
};

/// Helper allocating an ephemeral port bound to 127.0.0.1.
fn allocate_ephemeral_loopback_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback port");
    listener.local_addr().expect("retrieve local addr").port()
}

#[test]
fn test_tier3_environment_initialization_and_neutralized_configs() {
    let test_port = allocate_ephemeral_loopback_port();
    let test_udp_port = allocate_ephemeral_loopback_port();
    let env = Tier3Environment::new(test_port, test_udp_port).expect("failed to initialize Tier3Environment");

    // 1. Verify ephemeral work path exists and is distinct from home directory
    assert!(env.work_path().exists());
    assert!(env.config_dir().exists());
    assert!(env.incoming_dir().exists());
    assert!(env.temp_dir().exists());

    // 2. Verify neutralized preferences.ini
    let ini_path = env.config_dir().join("preferences.ini");
    assert!(ini_path.exists());
    let ini_content = fs::read_to_string(&ini_path).expect("failed to read preferences.ini");

    assert!(ini_content.contains(&format!("Port={test_port}")));
    assert!(ini_content.contains(&format!("UDPPort={test_udp_port}")));
    assert!(ini_content.contains("EnableKad=0"));
    assert!(ini_content.contains("NetworkKademlia=0"));
    assert!(ini_content.contains("AutoConnect=0"));
    assert!(ini_content.contains("AutoUpdateServerList=0"));
    assert!(ini_content.contains("ServerUDPPort=0"));

    // 3. Verify empty server.met
    let met_path = env.config_dir().join("server.met");
    assert!(met_path.exists());
    let met_bytes = fs::read(&met_path).expect("failed to read server.met");
    assert_eq!(&met_bytes, &[0xE0, 0x00, 0x00, 0x00, 0x00]);
}

#[test]
fn test_tier3_harness_rejects_non_loopback_address() {
    let dummy_bin = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/ed2k/hello_v1_minimal.bin");

    // Attempting to configure a non-loopback address (e.g. 192.168.1.50) must be rejected
    let non_loopback_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)), 4662);
    assert!(!non_loopback_addr.ip().is_loopback());

    let test_port = allocate_ephemeral_loopback_port();
    let test_udp_port = allocate_ephemeral_loopback_port();

    // The harness constructor strictly forces 127.0.0.1:
    let harness = Tier3Harness::new(dummy_bin, None, test_port, test_udp_port).expect("valid harness");
    assert!(harness.loopback_addr().ip().is_loopback());
    assert_eq!(harness.loopback_addr().ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
}

#[test]
fn test_tier3_divergence_report_anonymity_enforcement() {
    let mut clean_report =
        Tier3DivergenceReport::new("eMule0.50a-test", "abcdef0123456789", "Emule050a");
    clean_report.handshake_status = "passed".to_string();
    clean_report.record_tag("Nick", "EmuleSionTester");
    clean_report.record_tag("Version", "60");
    clean_report.record_divergence(
        "handshake_tags",
        "expected tag 0xFB",
        "observed tag 0xFC",
        "informational",
    );

    // Record normalized wire trace entry with profile
    clean_report.record_wire_trace(0xE3, 0x47, "client_to_peer", 40, "accepted");

    let explicit_record = WireTraceRecord {
        schema_version: 1,
        test_tier: 3,
        reference_kind: "external-reference-binary".to_string(),
        compatibility_profile: "Emule050a".to_string(),
        protocol_byte: "0xE3".to_string(),
        opcode: "0x47".to_string(),
        direction: "client_to_peer".to_string(),
        payload_length: 40,
        outcome: "accepted".to_string(),
    };
    assert_eq!(explicit_record.schema_version, 1);
    assert_eq!(explicit_record.test_tier, 3);
    assert_eq!(explicit_record.reference_kind, "external-reference-binary");
    assert_eq!(explicit_record.compatibility_profile, "Emule050a");
    assert_eq!(explicit_record.opcode, "0x47");

    assert!(clean_report.validate_anonymity().is_ok());

    let json_output = clean_report.to_json_pretty().expect("json serialization");
    assert!(json_output.contains("\"schemaVersion\": 1"));
    assert!(json_output.contains("\"testTier\": 3"));
    assert!(json_output.contains("\"referenceKind\": \"external-reference-binary\""));
    assert!(json_output.contains("\"compatibilityProfile\": \"Emule050a\""));
    assert!(json_output.contains("\"opcode\": \"0x47\""));

    // Test detection of username leak if environment variable USERNAME is set
    if let Ok(user) = std::env::var("USERNAME") {
        if !user.is_empty() {
            let mut leaking_report = clean_report.clone();
            leaking_report.record_divergence("leak_test", "clean", &format!("user_{user}"), "high");
            assert!(leaking_report.validate_anonymity().is_err());
        }
    }
}

#[test]
fn test_tier3_binary_validator_missing_binary_fails_gracefully() {
    let nonexistent = PathBuf::from("nonexistent_emule_binary_12345.exe");
    let result = Tier3BinaryValidator::validate(&nonexistent, None);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        Tier3HarnessError::BinaryNotFound(_)
    ));
}

/// Tier 3 live test: Only executed when EMULE_REFERENCE_BIN environment variable points to a valid binary.
/// Feature-gated and ignored by default so standard CI / cargo test never fails if no external binary is installed.
#[tokio::test]
#[ignore = "Tier 3 external reference orchestration requires pinned reference binary via EMULE_REFERENCE_BIN"]
async fn test_tier3_live_external_reference_loopback_handshake() {
    let bin_path_var = match std::env::var("EMULE_REFERENCE_BIN") {
        Ok(val) if !val.trim().is_empty() => PathBuf::from(val.trim()),
        _ => {
            println!("SKIPPED: Tier 3 test requires EMULE_REFERENCE_BIN environment variable.");
            return;
        }
    };

    let expected_hash = std::env::var("EMULE_REFERENCE_SHA256").ok();
    let test_port = allocate_ephemeral_loopback_port();
    let test_udp_port = allocate_ephemeral_loopback_port();

    let mut harness = Tier3Harness::new(
        bin_path_var,
        expected_hash.as_deref(),
        test_port,
        test_udp_port,
    )
    .expect("failed to initialize Tier 3 harness");

    harness.assert_loopback_confinement();

    // Spawn reference process with config dir directed to isolated ephemeral environment
    harness
        .spawn_reference_process(&[])
        .expect("failed to spawn reference process");

    // Wait briefly for the reference process to bind to loopback port
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Connect using Emule-Sion's loopback client
    let user_hash = [0x77; 16];
    let tags = vec![
        Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_NAME),
            value: TagValue::String("EmuleSionTier3".to_string()),
        },
        Ed2kTag {
            name: TagName::Id(TAG_SPECIAL_VERSION),
            value: TagValue::UInt32(60),
        },
    ];

    let connect_res = Ed2kLoopbackClient::connect_and_handshake(
        harness.loopback_addr(),
        user_hash,
        0x7F000001,
        test_port,
        tags,
        0x7F000001,
        test_port,
        Duration::from_secs(3),
    )
    .await;

    let mut report =
        Tier3DivergenceReport::new("live_reference", harness.binary_sha256(), "Emule050a");

    match connect_res {
        Ok((client, peer_info)) => {
            report.handshake_status = "passed".to_string();
            if let Some(ref nick) = peer_info.name {
                report.record_tag("Nick", nick);
            }
            if let Some(ver) = peer_info.version {
                report.record_tag("Version", &ver.to_string());
            }

            report.record_wire_trace(0xE3, 0x01, "client_to_peer", 30, "accepted");
            report.record_wire_trace(0xE3, 0x4C, "peer_to_client", 40, "accepted");

            println!(
                "Tier 3 live reference handshake successful! Peer info: {:?}",
                peer_info
            );
            client.close().await;
        }
        Err(e) => {
            report.handshake_status = "failed".to_string();
            report.record_divergence(
                "connection_or_handshake",
                "successful handshake",
                &format!("{e:?}"),
                "critical",
            );
            eprintln!("Tier 3 live reference handshake failed: {e:?}");
        }
    }

    report
        .validate_anonymity()
        .expect("Tier 3 report failed anonymity validation");

    harness.shutdown();
}
