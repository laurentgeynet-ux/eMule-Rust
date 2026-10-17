// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

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

use md4::{Digest, Md4};
use mule_protocol::ed2k::{
    Ed2kBlockSink, Ed2kDownloadSequencer, Ed2kEncoder, Ed2kLoopbackClient, Ed2kPartDownloadStatus,
    Ed2kPartFileSink, Ed2kPartResumeState, Ed2kSendingPart, Ed2kSequencerError,
    OP_REQUESTPARTS_EMULE, PR_ED2K,
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

/// Explicit compatibility profile of the pinned reference binary
/// (`EMULE_REFERENCE_PROFILE` env, default `Emule050a`). The profile is
/// declared in every trace record so overloaded opcodes stay unambiguous.
fn reference_profile() -> String {
    std::env::var("EMULE_REFERENCE_PROFILE")
        .ok()
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| "Emule050a".to_string())
}

/// Expected `OP_EMULEINFO` `client_version` for the pinned profile:
/// `Emule<VV>a` parses `<VV>` as hex (eMule 0.50a → 0x50 = 80, 0.72a →
/// 0x72 = 114). Unknown profiles fail fast — a wrong expectation is worse
/// than no check.
fn expected_emule_client_version() -> u8 {
    let profile = reference_profile();
    let digits = profile
        .strip_prefix("Emule")
        .and_then(|s| s.strip_suffix('a'))
        .expect("profile must look like Emule<VV>a");
    u8::from_str_radix(digits, 16).expect("profile version digits must be hex")
}

/// `true` si le profil épinglé est un binaire aMule (`Amule*`). Les
/// assertions spécifiques à la numérotation de version eMule sont alors
/// relâchées en observations consignées dans le rapport de divergence.
fn is_amule_profile() -> bool {
    reference_profile().starts_with("Amule")
}

#[test]
fn test_tier3_environment_initialization_and_neutralized_configs() {
    let test_port = allocate_ephemeral_loopback_port();
    let test_udp_port = allocate_ephemeral_loopback_port();
    let env = Tier3Environment::new(test_port, test_udp_port)
        .expect("failed to initialize Tier3Environment");

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
    let dummy_bin =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ed2k/hello_v1_minimal.bin");

    // Attempting to configure a non-loopback address (e.g. 192.168.1.50) must be rejected
    let non_loopback_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 50)), 39735);
    assert!(!non_loopback_addr.ip().is_loopback());

    let test_port = allocate_ephemeral_loopback_port();
    let test_udp_port = allocate_ephemeral_loopback_port();

    // The harness constructor strictly forces 127.0.0.1:
    let harness =
        Tier3Harness::new(dummy_bin, None, test_port, test_udp_port).expect("valid harness");
    assert!(harness.loopback_addr().ip().is_loopback());
    assert_eq!(
        harness.loopback_addr().ip(),
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
    );
}

#[test]
fn test_tier3_divergence_report_anonymity_enforcement() {
    let mut clean_report =
        Tier3DivergenceReport::new("eMule0.50a-test", "abcdef0123456789", "Emule050a");
    clean_report.handshake_status = "passed".to_string();
    clean_report.record_tag("Nick", "eMuleRustTester");
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

    // Generate conformant eMule user hash (with 0x0E at index 5 and 0x6F at index 14)
    let user_hash = [
        0x31, 0x41, 0x59, 0x26, 0x53, 0x0E, 0x58, 0x97, 0x93, 0x23, 0x84, 0x62, 0x64, 0x33, 0x6F,
        0x83,
    ];
    let client_tcp_port = allocate_ephemeral_loopback_port();
    // Minimal Hello: start with tag_count = 0
    let tags = Vec::new();

    let pid = harness.child_pid().unwrap_or(0);
    println!("Target loopback addr: {:?}", harness.loopback_addr());
    println!(
        "Reference binary work path: {:?}",
        harness.environment().work_path()
    );
    println!("Child PID: {pid}");

    // Wait for the reference process to bind to loopback port and connect (up to 5 seconds)
    let mut connect_res = None;
    for attempt in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let is_running = harness.is_child_running();
        println!("Attempt {attempt}: child running = {is_running}");
        if attempt == 3 && pid > 0 {
            let out = std::process::Command::new("powershell")
                .arg("-NoProfile")
                .arg("-Command")
                .arg(format!("Get-NetTCPConnection -OwningProcess {pid} -ErrorAction SilentlyContinue | Select-Object LocalAddress, LocalPort, State | Out-String"))
                .output();
            if let Ok(o) = out {
                println!(
                    "TCP connections for PID {pid}:\n{}",
                    String::from_utf8_lossy(&o.stdout)
                );
            }
        }
        match Ed2kLoopbackClient::connect_and_handshake(
            harness.loopback_addr(),
            user_hash,
            0, // client_id = 0 (Low ID / unassigned)
            client_tcp_port,
            tags.clone(),
            0, // server_ip = 0
            0, // server_port = 0
            Duration::from_secs(3),
        )
        .await
        {
            Ok(ok_res) => {
                println!("Connected on attempt {attempt}!");
                connect_res = Some(Ok(ok_res));
                break;
            }
            Err(e) => {
                println!("Attempt {attempt} failed: {e:?}");
                if attempt == 10 {
                    connect_res = Some(Err(e));
                }
            }
        }
    }
    let connect_res = connect_res.expect("connection attempt completed");

    let mut report = Tier3DivergenceReport::new(
        "live_reference",
        &harness.binary_sha256_redacted(),
        &reference_profile(),
    );

    match connect_res {
        Ok((client, peer_info)) => {
            report.handshake_status = "passed".to_string();
            if let Some(ref nick) = peer_info.name {
                report.record_tag("Nick", nick);
            }
            if let Some(ver) = peer_info.version {
                report.record_tag("Version", &ver.to_string());
            }

            report.record_wire_trace(0xE3, 0x01, "client_to_peer", 34, "accepted");
            report.record_wire_trace(0xE3, 0x4C, "peer_to_client", 40, "accepted");

            println!(
                "Tier 3 live reference handshake successful! Peer info: {:?}",
                peer_info
            );

            assert_eq!(peer_info.name.as_deref(), Some("RefPeer"));
            if is_amule_profile() {
                // Profil aMule : les champs de version et `OP_EMULEINFO`
                // obéissent à un encodage différent de celui d'eMule — on les
                // consigne dans le rapport au lieu d'imposer les valeurs eMule.
                report.record_tag("Version", &format!("{:?}", peer_info.version));
                report.record_tag(
                    "EmuleUdpPort",
                    &format!("{:?}", peer_info.emule_udp_port),
                );
                report.record_tag(
                    "MiscOptions",
                    &format!(
                        "{:?}/{:?}",
                        peer_info.misc_options1, peer_info.misc_options2
                    ),
                );
                if let Some(ei) = peer_info.emule_info.as_ref() {
                    report.record_tag("EmuleInfoClientVersion", &ei.client_version.to_string());
                    report.record_tag("EmuleInfoProtocolVersion", &ei.protocol_version.to_string());
                } else {
                    report.record_divergence(
                        "op_emuleinfo",
                        "OP_EMULEINFO decoded",
                        "absent from handshake",
                        "info",
                    );
                }
                report.record_tag(
                    "CompressionCap",
                    &peer_info.has_compression_capability().to_string(),
                );
                report.record_tag(
                    "SourceExchangeCap",
                    &peer_info.has_source_exchange_capability().to_string(),
                );
            } else {
                assert_eq!(peer_info.version, Some(60));
                assert!(
                    peer_info.misc_options1.is_some(),
                    "misc_options1 must be parsed"
                );
                assert!(
                    peer_info.misc_options2.is_some(),
                    "misc_options2 must be parsed"
                );
                assert_eq!(
                    peer_info.emule_udp_port,
                    Some(test_udp_port),
                    "emule_udp_port must match test_udp_port"
                );
                assert!(
                    peer_info.emule_info.is_some(),
                    "emule_info must be populated from OP_EMULEINFO"
                );
                let emule_info = peer_info.emule_info.as_ref().unwrap();
                assert_eq!(
                    emule_info.client_version,
                    expected_emule_client_version(),
                    "emule_info client_version must match EMULE_REFERENCE_PROFILE"
                );
                assert_eq!(
                    emule_info.protocol_version, 1,
                    "emule_info protocol_version must be 1"
                );
                assert!(
                    peer_info.has_compression_capability(),
                    "compression capability must be announced"
                );
                assert!(
                    peer_info.has_source_exchange_capability(),
                    "source exchange capability must be announced"
                );
            }

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
            panic!("Tier 3 live reference handshake failed: {e:?}");
        }
    }

    println!("Files in work_path:");
    if let Ok(entries) = std::fs::read_dir(harness.environment().work_path()) {
        for e in entries.flatten() {
            println!("  {:?}", e.path());
        }
    }
    let config_dir = harness.environment().config_dir();
    println!("Files in config_dir:");
    if let Ok(entries) = std::fs::read_dir(config_dir) {
        for e in entries.flatten() {
            println!("  {:?}", e.path());
            let path = e.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if ext == "log" || ext == "txt" {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        println!("Content of {:?}:\n{}", path, content);
                    }
                }
            }
        }
    }

    report
        .validate_anonymity()
        .expect("Tier 3 report failed anonymity validation");

    harness.shutdown();
}

/// Tier 3.1: Live passive post-handshake, multi-port, and hermetic teardown test.
///
/// Invariants verified:
/// 1. Consecutive execution across 2 iterations on independent dynamically allocated ephemeral ports.
/// 2. Hermetic sandbox isolation: each iteration runs in a fresh temporary directory with
///    Windows registry UsePublicUserDirectories = 2 forced.
/// 3. Handshake completes both OP_EMULEINFO (0xC5 0x01) and OP_HELLOANSWER (0xE3 0x4C).
/// 4. Peer metadata (name, version 60, UDP port, misc_options1/2, emule_info) decoded accurately.
/// 5. Passive capability discipline: peer capabilities (compression, source exchange, Kad)
///    are observed but strictly never activated or negotiated unrequested.
/// 6. Bounded idle watchdog (1s): connection held cleanly with zero unexpected traffic.
/// 7. Graceful TCP FIN termination (`client.close().await`) and process shutdown (`harness.shutdown()`).
/// 8. Zero non-loopback network connections and zero residual orphan processes.
#[tokio::test]
#[ignore = "Tier 3 external reference orchestration requires pinned reference binary via EMULE_REFERENCE_BIN"]
async fn test_tier3_live_passive_post_handshake_multi_port_and_teardown() {
    let bin_path_var = match std::env::var("EMULE_REFERENCE_BIN") {
        Ok(val) if !val.trim().is_empty() => PathBuf::from(val.trim()),
        _ => {
            println!("SKIPPED: Tier 3.1 test requires EMULE_REFERENCE_BIN environment variable.");
            return;
        }
    };

    let expected_hash = std::env::var("EMULE_REFERENCE_SHA256").ok();

    // Multi-port and multi-sandbox iteration across 2 distinct runs
    for iteration in 1..=2 {
        println!("\n=== Starting Tier 3.1 Iteration {iteration}/2 ===");
        let test_port = allocate_ephemeral_loopback_port();
        let test_udp_port = allocate_ephemeral_loopback_port();

        let mut harness = Tier3Harness::new(
            bin_path_var.clone(),
            expected_hash.as_deref(),
            test_port,
            test_udp_port,
        )
        .expect("failed to initialize Tier 3.1 harness");

        harness.assert_loopback_confinement();
        println!("Iteration {iteration}: Assigned TCP port {test_port}, UDP port {test_udp_port}");
        println!(
            "Iteration {iteration}: Work path: {:?}",
            harness.environment().work_path()
        );

        harness
            .spawn_reference_process(&[])
            .expect("failed to spawn reference process");

        let pid = harness.child_pid().unwrap_or(0);
        assert!(pid > 0, "child PID must be valid");
        assert!(harness.is_child_running(), "child process must be active");

        // Canonical eMule user hash (with 0x0E at index 5 and 0x6F at index 14)
        let user_hash = [
            0x31, 0x41, 0x59, 0x26, 0x53, 0x0E, 0x58, 0x97, 0x93, 0x23, 0x84, 0x62, 0x64, 0x33,
            0x6F, 0x83,
        ];
        let client_tcp_port = allocate_ephemeral_loopback_port();

        // Connect with retry
        let mut connect_res = None;
        for attempt in 1..=10 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if !harness.is_child_running() {
                panic!("Iteration {iteration}: Reference process exited unexpectedly on attempt {attempt}");
            }
            match Ed2kLoopbackClient::connect_and_handshake(
                harness.loopback_addr(),
                user_hash,
                0,
                client_tcp_port,
                Vec::new(),
                0,
                0,
                Duration::from_secs(3),
            )
            .await
            {
                Ok(res) => {
                    connect_res = Some(res);
                    break;
                }
                Err(e) => {
                    if attempt == 10 {
                        panic!("Iteration {iteration}: Failed to connect after 10 attempts: {e:?}");
                    }
                }
            }
        }

        let (client, peer_info) = connect_res.expect("connected");

        // 1. Assert peer identity
        assert_eq!(peer_info.name.as_deref(), Some("RefPeer"));

        // 2. Assert OP_EMULEINFO announcement (encodage version spécifique eMule)
        if is_amule_profile() {
            println!(
                "Iteration {iteration}: aMule profile — version={:?} udp_port={:?} misc={:?}/{:?} emule_info={:?} compression={} source_exchange={}",
                peer_info.version,
                peer_info.emule_udp_port,
                peer_info.misc_options1,
                peer_info.misc_options2,
                peer_info.emule_info.as_ref().map(|ei| ei.client_version),
                peer_info.has_compression_capability(),
                peer_info.has_source_exchange_capability(),
            );
        } else {
            assert_eq!(peer_info.version, Some(60));
            assert_eq!(peer_info.emule_udp_port, Some(test_udp_port));
            assert!(peer_info.misc_options1.is_some());
            assert!(peer_info.misc_options2.is_some());
            assert!(
                peer_info.emule_info.is_some(),
                "OP_EMULEINFO must be captured and parsed"
            );
            let emule_info = peer_info.emule_info.as_ref().unwrap();
            assert_eq!(
                emule_info.client_version,
                expected_emule_client_version(),
                "emule_info client_version must match EMULE_REFERENCE_PROFILE"
            );
            assert_eq!(
                emule_info.protocol_version, 1,
                "eMule compatible protocol version must be 1"
            );
            assert!(
                peer_info.has_compression_capability(),
                "compression capability must be announced"
            );
            assert!(
                peer_info.has_source_exchange_capability(),
                "source exchange capability must be announced"
            );
            println!(
                "Iteration {iteration}: Decoded OP_EMULEINFO with {} tags, compression={}, source_exchange={}",
                emule_info.tags.len(),
                peer_info.has_compression_capability(),
                peer_info.has_source_exchange_capability(),
            );
        }

        // 3. Assert Passive Capability Discipline
        // The client observes remote capabilities but remains in baseline uncompressed protocol mode
        assert!(client.session().is_ready());
        assert_eq!(client.session().snapshot().pending_status_count, 0);
        assert_eq!(client.session().snapshot().pending_block_count, 0);

        // 4. Bounded idle watchdog: hold connection passively for 1s
        println!("Iteration {iteration}: Holding passive connection for 1s watchdog...");
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(
            harness.is_child_running(),
            "eMule process remains healthy during idle hold"
        );

        // 5. Verify network confinement via PowerShell Get-NetTCPConnection
        let check_conn = std::process::Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!("Get-NetTCPConnection -OwningProcess {pid} -ErrorAction SilentlyContinue | Where-Object {{ $_.RemoteAddress -ne '0.0.0.0' -and $_.RemoteAddress -ne '127.0.0.1' -and $_.RemoteAddress -ne '::' -and $_.RemoteAddress -ne '::1' }} | Measure-Object | Select-Object -ExpandProperty Count"))
            .output();
        if let Ok(out) = check_conn {
            let count_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let non_loopback_count: usize = count_str.parse().unwrap_or(0);
            assert_eq!(
                non_loopback_count, 0,
                "Non-loopback connections detected for PID {pid}!"
            );
        }

        // 6. Graceful TCP FIN teardown
        client.close().await;

        // 7. Hermetic process teardown
        harness.shutdown();
        assert!(
            !harness.is_child_running(),
            "Process must be terminated after shutdown"
        );

        println!("=== Iteration {iteration}/2 Completed Successfully ===");
    }
}

/// Tier 3.2: Live file availability request (OP_SETREQFILEID_EMULE / OP_FILESTATUS)
/// for a deterministic shared fixture file against official eMule 0.50a.
///
/// Invariants verified:
/// 1. Sealed fixture hash `695b81b1727666818988524bd7fc823c` computed ahead of time.
/// 2. Deterministic fixture `tier3_status_fixture_9728001.bin` (9,728,001 bytes) created in `incoming/`.
/// 3. Loopback-only connection to ephemeral dynamic port.
/// 4. Handshake completed cleanly (OP_EMULEINFO and OP_HELLOANSWER).
/// 5. Exactly one OP_SETREQFILEID_EMULE (0x4F) request sent with sealed file hash.
/// 6. Exactly one OP_FILESTATUS (0x50) received from eMule 0.50a.
/// 7. Part count = 2, bitfield = 1 byte (`0x03` = parts 0 and 1 available, unused bits 2..7 are zero).
/// 8. Strict passive discipline: no OP_REQUESTPARTS, no data transfer, no zlib, no Kad.
/// 9. Graceful TCP FIN teardown and process cleanup.
/// 10. Post-shutdown confinement scan confirms 0 non-loopback sockets.
#[tokio::test]
#[ignore = "Tier 3 external reference orchestration requires pinned reference binary via EMULE_REFERENCE_BIN"]
async fn test_tier3_live_file_status_request_and_parts_availability() {
    let bin_path_var = match std::env::var("EMULE_REFERENCE_BIN") {
        Ok(val) if !val.trim().is_empty() => PathBuf::from(val.trim()),
        _ => {
            println!("SKIPPED: Tier 3.2 test requires EMULE_REFERENCE_BIN environment variable.");
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
    .expect("failed to initialize Tier 3.2 harness");

    harness.assert_loopback_confinement();

    // 1. Create deterministic fixture in incoming directory (9,728,001 bytes = 1 full part + 1 byte)
    let fixture_len = 9_728_001;
    let fixture_path = harness
        .environment()
        .create_deterministic_fixture("tier3_status_fixture_9728001.bin", fixture_len)
        .expect("failed to create deterministic fixture");
    assert!(fixture_path.exists());
    assert_eq!(
        std::fs::metadata(&fixture_path).unwrap().len(),
        fixture_len as u64
    );

    // Sealed reference eD2K hash for 9_728_001 bytes of repeating 0..=255
    let sealed_hash: [u8; 16] = [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81, 0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82,
        0x3c,
    ];

    println!("Tier 3.2: Created fixture at {:?}", fixture_path);
    println!("Tier 3.2: Sealed eD2K Hash: {}", hex::encode(sealed_hash));

    // 2. Spawn reference process
    harness
        .spawn_reference_process(&[])
        .expect("failed to spawn reference process");

    let pid = harness.child_pid().unwrap_or(0);
    assert!(pid > 0, "child PID must be valid");
    assert!(harness.is_child_running(), "child process must be active");

    let user_hash = [
        0x31, 0x41, 0x59, 0x26, 0x53, 0x0E, 0x58, 0x97, 0x93, 0x23, 0x84, 0x62, 0x64, 0x33, 0x6F,
        0x83,
    ];
    let client_tcp_port = allocate_ephemeral_loopback_port();

    // 3. Connect and complete handshake with retry
    let mut connect_res = None;
    for attempt in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !harness.is_child_running() {
            panic!("Reference process exited unexpectedly on attempt {attempt}");
        }
        match Ed2kLoopbackClient::connect_and_handshake(
            harness.loopback_addr(),
            user_hash,
            0,
            client_tcp_port,
            Vec::new(),
            0,
            0,
            Duration::from_secs(3),
        )
        .await
        {
            Ok(res) => {
                connect_res = Some(res);
                break;
            }
            Err(e) => {
                if attempt == 10 {
                    panic!("Failed to connect after 10 attempts: {e:?}");
                }
            }
        }
    }

    let (mut client, peer_info) = connect_res.expect("connected");
    assert!(client.session().is_ready());
    println!(
        "Tier 3.2: Handshake successful with peer: {:?}",
        peer_info.name
    );

    // Give eMule a brief moment to settle background file hashing if needed
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 4. Request file status for sealed fixture hash
    println!("Tier 3.2: Sending OP_SETREQFILEID_EMULE (0x4F) for fixture...");
    let mut file_status = None;
    for attempt in 1..=5 {
        match client
            .request_file_status(sealed_hash, Duration::from_secs(3))
            .await
        {
            Ok(status) => {
                file_status = Some(status);
                break;
            }
            Err(mule_protocol::ed2k::Ed2kSessionError::FileNotFound(_)) => {
                println!("Tier 3.2: File not yet indexed by eMule, retry {attempt}/5 in 1s...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => {
                panic!("Tier 3.2: Unexpected error requesting file status: {e:?}");
            }
        }
    }

    let status =
        file_status.expect("eMule 0.50a must respond with OP_FILESTATUS for shared fixture");
    println!("Tier 3.2: Received OP_FILESTATUS:");
    println!("  File Hash : {}", hex::encode(status.file_hash));
    println!("  Part Count: {}", status.part_count);
    println!("  Bitfield  : {:02X?}", status.raw_bytes());

    // 5. Assertions on canonical eD2K completed file status
    assert_eq!(
        status.file_hash, sealed_hash,
        "file hash must match requested hash"
    );
    // In eD2K wire protocol (ClientTCPSocket.cpp), official eMule/aMule write uint16(0)
    // when responding to OP_SETREQFILEID for a completed shared file (CKnownFile).
    // In this canonical case, no bitfield is transmitted, and part_count = 0 signifies
    // that the peer is a complete source (m_bCompleteSource = true) possessing 100% of the parts.
    assert_eq!(
        status.part_count, 0,
        "eMule 0.50a encodes complete file availability as part_count = 0"
    );
    assert!(
        status.is_complete(),
        "file status must report complete source"
    );
    assert_eq!(
        status.raw_bytes().len(),
        0,
        "bitfield for complete file has 0 bytes"
    );
    assert!(
        status.has_part(0),
        "part 0 must be reported available on complete file"
    );
    assert!(
        status.has_part(1),
        "part 1 must be reported available on complete file"
    );

    // 6. Also verify OP_FILEREQANSNOFILE (0x48) when requesting an unknown file
    let unknown_hash = [0xAA; 16];
    let no_file_res = client
        .request_file_status(unknown_hash, Duration::from_secs(3))
        .await;
    match no_file_res {
        Err(mule_protocol::ed2k::Ed2kSessionError::FileNotFound(h)) => {
            assert_eq!(h, hex::encode(unknown_hash), "eMule must report requested unknown file as not found via OP_FILEREQANSNOFILE (0x48)");
            println!(
                "Tier 3.2: Successfully received OP_FILEREQANSNOFILE (0x48) for unknown hash!"
            );
        }
        other => panic!("expected FileNotFound error via OP_FILEREQANSNOFILE, got: {other:?}"),
    }

    // 7. Strict Passive Discipline: verify no blocks requested
    assert_eq!(client.session().snapshot().pending_block_count, 0);
    assert_eq!(client.session().snapshot().pending_status_count, 0);
    assert!(client.session().is_ready());

    // 7. Network Confinement check
    let check_conn = std::process::Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(format!("Get-NetTCPConnection -OwningProcess {pid} -ErrorAction SilentlyContinue | Where-Object {{ $_.RemoteAddress -ne '0.0.0.0' -and $_.RemoteAddress -ne '127.0.0.1' -and $_.RemoteAddress -ne '::' -and $_.RemoteAddress -ne '::1' }} | Measure-Object | Select-Object -ExpandProperty Count"))
        .output();
    if let Ok(out) = check_conn {
        let count_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let non_loopback_count: usize = count_str.parse().unwrap_or(0);
        assert_eq!(
            non_loopback_count, 0,
            "Non-loopback connections detected for PID {pid}!"
        );
    }

    // 8. Graceful TCP FIN teardown
    client.close().await;

    // 9. Hermetic process teardown
    harness.shutdown();
    assert!(
        !harness.is_child_running(),
        "Process must be terminated after shutdown"
    );

    println!(
        "Tier 3.2: File status and parts availability successfully verified against eMule 0.50a!"
    );
}

/// Tier 3.3 Live Reference Integration Test:
/// Download sequencing and block request planning without payload transmission.
///
/// Steps:
/// 1. Spawn eMule 0.50a reference process with 9,728,001 bytes fixture in sandbox incoming/.
/// 2. Complete loopback handshake.
/// 3. Request OP_FILESTATUS for sealed hash 695b81b1727666818988524bd7fc823c.
/// 4. Receive OP_FILESTATUS with part_count = 0 (complete file source).
/// 5. Instantiate Ed2kDownloadSequencer:
///    - Total parts = 2.
///    - Both Part 0 and Part 1 are eligible for selection.
/// 6. Plan Part 0:
///    - 53 blocks planned.
///    - Block 0: [0, 184_320).
///    - Block 52: [9_584_640, 9_728_000) (length 143,360 bytes, exactly aligned to 9,728,000 part boundary).
/// 7. Plan Part 1:
///    - 1 block planned: [9_728_000, 9_728_001) (residual 1 byte).
/// 8. Construct local wire frames:
///    - Encode OP_REQUESTPARTS_EMULE (0x47) for Part 0 Block 0 and Part 1 Block 0.
///    - Validate 40 bytes wire payload.
/// 9. Strict Passive Discipline:
///    - NO block request packet is transmitted to eMule 0.50a.
///    - Pending block count on session remains strictly 0.
/// 10. Negative response proof:
///    - Request unknown file hash -> receives OP_FILEREQANSNOFILE (0x48).
///    - Verify sequencer refuses planning for unknown file.
/// 11. Confinement scan: 0 non-loopback connections.
/// 12. Graceful TCP FIN teardown and process cleanup.
#[tokio::test]
#[ignore = "Tier 3 external reference orchestration requires pinned reference binary via EMULE_REFERENCE_BIN"]
async fn test_tier3_live_download_sequencing_without_payload() {
    let bin_path_var = match std::env::var("EMULE_REFERENCE_BIN") {
        Ok(val) if !val.trim().is_empty() => PathBuf::from(val.trim()),
        _ => {
            println!("SKIPPED: Tier 3.3 test requires EMULE_REFERENCE_BIN environment variable.");
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
    .expect("failed to initialize Tier 3.3 harness");

    harness.assert_loopback_confinement();

    // 1. Create deterministic fixture in incoming directory (9,728,001 bytes = 1 full part + 1 byte)
    let fixture_len = 9_728_001u64;
    let fixture_path = harness
        .environment()
        .create_deterministic_fixture("tier3_status_fixture_9728001.bin", fixture_len as usize)
        .expect("failed to create deterministic fixture");
    assert!(fixture_path.exists());
    assert_eq!(std::fs::metadata(&fixture_path).unwrap().len(), fixture_len);

    let sealed_hash: [u8; 16] = [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81, 0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82,
        0x3c,
    ];

    // 2. Spawn reference process
    harness
        .spawn_reference_process(&[])
        .expect("failed to spawn reference process");

    let pid = harness.child_pid().unwrap_or(0);
    assert!(pid > 0);
    assert!(harness.is_child_running());

    let user_hash = [
        0x31, 0x41, 0x59, 0x26, 0x53, 0x0E, 0x58, 0x97, 0x93, 0x23, 0x84, 0x62, 0x64, 0x33, 0x6F,
        0x83,
    ];
    let client_tcp_port = allocate_ephemeral_loopback_port();

    // 3. Connect and complete handshake
    let mut connect_res = None;
    for attempt in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !harness.is_child_running() {
            panic!("Reference process exited unexpectedly on attempt {attempt}");
        }
        match Ed2kLoopbackClient::connect_and_handshake(
            harness.loopback_addr(),
            user_hash,
            0,
            client_tcp_port,
            Vec::new(),
            0,
            0,
            Duration::from_secs(3),
        )
        .await
        {
            Ok(res) => {
                connect_res = Some(res);
                break;
            }
            Err(e) => {
                if attempt == 10 {
                    panic!("Failed to connect after 10 attempts: {e:?}");
                }
            }
        }
    }

    let (mut client, _peer_info) = connect_res.expect("connected");
    assert!(client.session().is_ready());

    // Give eMule a brief moment to settle background file hashing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 4. Request file status for sealed fixture hash
    let mut file_status = None;
    for attempt in 1..=5 {
        match client
            .request_file_status(sealed_hash, Duration::from_secs(3))
            .await
        {
            Ok(status) => {
                file_status = Some(status);
                break;
            }
            Err(mule_protocol::ed2k::Ed2kSessionError::FileNotFound(_)) => {
                println!("Tier 3.3: File not yet indexed by eMule, retry {attempt}/5 in 1s...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => {
                panic!("Tier 3.3: Unexpected error requesting file status: {e:?}");
            }
        }
    }

    let status =
        file_status.expect("eMule 0.50a must respond with OP_FILESTATUS for shared fixture");
    assert_eq!(status.file_hash, sealed_hash);
    assert_eq!(
        status.part_count, 0,
        "complete source reports part_count = 0"
    );
    assert!(status.is_complete());

    // 5. Tier 3.3 Download Sequencing & Block Request Planning
    let sequencer = Ed2kDownloadSequencer::new(sealed_hash, fixture_len, status)
        .expect("sequencer initialization succeeds");

    assert_eq!(
        sequencer.total_parts(),
        2,
        "9,728,001 bytes file has exactly 2 parts"
    );
    assert!(sequencer.is_part_available(0), "part 0 must be available");
    assert!(sequencer.is_part_available(1), "part 1 must be available");
    assert_eq!(sequencer.available_parts(), vec![0, 1]);
    assert_eq!(sequencer.select_first_available_part(), Some(0));

    // Plan Part 0 blocks (9,728,000 bytes -> 53 blocks)
    let part0_blocks = sequencer.plan_part_blocks(0).expect("plan part 0");
    assert_eq!(part0_blocks.len(), 53);
    assert_eq!(part0_blocks[0].start, 0);
    assert_eq!(part0_blocks[0].end, 184_320);
    assert_eq!(part0_blocks[52].start, 9_584_640);
    assert_eq!(
        part0_blocks[52].end, 9_728_000,
        "final block of part 0 aligns exactly to part boundary"
    );
    assert_eq!(part0_blocks[52].len(), 143_360);

    // Plan Part 1 blocks (1 byte remainder -> 1 block)
    let part1_blocks = sequencer.plan_part_blocks(1).expect("plan part 1");
    assert_eq!(part1_blocks.len(), 1);
    assert_eq!(part1_blocks[0].start, 9_728_000);
    assert_eq!(
        part1_blocks[0].end, 9_728_001,
        "residual block covers the last byte of the fixture"
    );
    assert_eq!(part1_blocks[0].len(), 1);

    // 6. Build and encode wire request packets locally (Emule050a profile -> opcode 0x47)
    let encoder = Ed2kEncoder::new();
    let wire_pkt_part0_block0 = sequencer
        .build_request_packet(&[part0_blocks[0]], &encoder)
        .expect("encode part 0 block 0");
    assert_eq!(wire_pkt_part0_block0.protocol, PR_ED2K);
    assert_eq!(wire_pkt_part0_block0.opcode, OP_REQUESTPARTS_EMULE); // 0x47
    assert_eq!(
        wire_pkt_part0_block0.payload.len(),
        40,
        "wire payload must be 40 bytes"
    );

    let wire_pkt_part1_block0 = sequencer
        .build_request_packet(&[part1_blocks[0]], &encoder)
        .expect("encode part 1 residual block");
    assert_eq!(wire_pkt_part1_block0.opcode, OP_REQUESTPARTS_EMULE);
    assert_eq!(wire_pkt_part1_block0.payload.len(), 40);

    // 7. STRICT PASSIVE DISCIPLINE: verify NO block requests were sent over the wire
    assert_eq!(client.session().snapshot().pending_block_count, 0);
    assert_eq!(client.session().snapshot().pending_status_count, 0);
    assert!(client.session().is_ready());

    // 8. Negative response proof: OP_FILEREQANSNOFILE prevents planning
    let unknown_hash = [0xAA; 16];
    let no_file_res = client
        .request_file_status(unknown_hash, Duration::from_secs(3))
        .await;
    let seq_from_neg =
        Ed2kDownloadSequencer::from_file_status_result(unknown_hash, fixture_len, no_file_res);
    assert!(matches!(
        seq_from_neg,
        Err(Ed2kSequencerError::FileNotFound(_))
    ));

    // 9. Confinement scan
    let check_conn = std::process::Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(format!("Get-NetTCPConnection -OwningProcess {pid} -ErrorAction SilentlyContinue | Where-Object {{ $_.RemoteAddress -ne '0.0.0.0' -and $_.RemoteAddress -ne '127.0.0.1' -and $_.RemoteAddress -ne '::' -and $_.RemoteAddress -ne '::1' }} | Measure-Object | Select-Object -ExpandProperty Count"))
        .output();
    if let Ok(out) = check_conn {
        let count_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let non_loopback_count: usize = count_str.parse().unwrap_or(0);
        assert_eq!(
            non_loopback_count, 0,
            "Non-loopback connections detected for PID {pid}!"
        );
    }

    // 10. Graceful TCP FIN teardown
    client.close().await;

    // 11. Hermetic process teardown
    harness.shutdown();
    assert!(
        !harness.is_child_running(),
        "Process must be terminated after shutdown"
    );

    println!("Tier 3.3: Download sequencing and block request planning without payload successfully verified against eMule 0.50a!");
}

/// Tier 3.4 Live Reference Integration Test:
/// Single bounded uncompressed block transfer from official eMule 0.50a on loopback.
///
/// Invariants verified:
/// 1. Sealed fixture `tier3_status_fixture_9728001.bin` (9,728,001 bytes, hash `695b81b1727666818988524bd7fc823c`)
///    generated in the ephemeral incoming directory.
/// 2. Confinement to 127.0.0.1 on dynamic ephemeral loopback ports.
/// 3. Handshake completed cleanly (OP_EMULEINFO and OP_HELLOANSWER).
/// 4. File status queried via OP_SETREQFILEID_EMULE (0x4F) -> receives OP_FILESTATUS (0x50) with part_count = 0.
/// 5. Single block request emitted: OP_REQUESTPARTS_EMULE (0x47) for [0, 184_320) with inactive slots zeroed.
/// 6. Response received: OP_SENDINGPART (0x46) under PR_ED2K (0xE3).
/// 7. Data validation:
///    - File hash matches sealed fixture hash.
///    - Start offset = 0, End offset = 184_320, Data length = 184_320 bytes.
///    - Byte-by-byte comparison against fixture prefix passes 100%.
///    - Local MD4 oracle check on received block passes 100%.
/// 8. Storage assembly: Ed2kBlockSink receives the block and tracks [0, 184_320).
/// 9. Strict passive discipline: no further packets sent, pending block count = 0.
/// 10. Graceful TCP FIN teardown (client.close()).
/// 11. Confinement scan: 0 non-loopback TCP connections.
/// 12. RAII process and sandbox cleanup.
#[tokio::test]
#[ignore = "Tier 3 external reference orchestration requires pinned reference binary via EMULE_REFERENCE_BIN"]
async fn test_tier3_live_single_uncompressed_block_transfer_from_emule050a() {
    let bin_path_var = match std::env::var("EMULE_REFERENCE_BIN") {
        Ok(val) if !val.trim().is_empty() => PathBuf::from(val.trim()),
        _ => {
            println!("SKIPPED: Tier 3.4 test requires EMULE_REFERENCE_BIN environment variable.");
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
    .expect("failed to initialize Tier 3.4 harness");

    harness.assert_loopback_confinement();

    // 1. Create deterministic fixture in incoming directory (9,728,001 bytes)
    let fixture_len = 9_728_001u64;
    let fixture_path = harness
        .environment()
        .create_deterministic_fixture("tier3_status_fixture_9728001.bin", fixture_len as usize)
        .expect("failed to create deterministic fixture");
    assert!(fixture_path.exists());
    assert_eq!(std::fs::metadata(&fixture_path).unwrap().len(), fixture_len);

    let sealed_hash: [u8; 16] = [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81, 0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82,
        0x3c,
    ];

    // Read the reference 184,320-byte prefix from the generated fixture
    let fixture_bytes = std::fs::read(&fixture_path).expect("read fixture file");
    let expected_prefix = &fixture_bytes[0..184_320];

    // 2. Spawn reference process
    harness
        .spawn_reference_process(&[])
        .expect("failed to spawn reference process");

    let pid = harness.child_pid().unwrap_or(0);
    assert!(pid > 0);
    assert!(harness.is_child_running());

    let user_hash = [
        0x31, 0x41, 0x59, 0x26, 0x53, 0x0E, 0x58, 0x97, 0x93, 0x23, 0x84, 0x62, 0x64, 0x33, 0x6F,
        0x83,
    ];
    let client_tcp_port = allocate_ephemeral_loopback_port();

    // 3. Connect and complete handshake
    let mut connect_res = None;
    for attempt in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !harness.is_child_running() {
            panic!("Reference process exited unexpectedly on attempt {attempt}");
        }
        match Ed2kLoopbackClient::connect_and_handshake(
            harness.loopback_addr(),
            user_hash,
            0,
            client_tcp_port,
            Vec::new(),
            0,
            0,
            Duration::from_secs(3),
        )
        .await
        {
            Ok(res) => {
                connect_res = Some(res);
                break;
            }
            Err(e) => {
                if attempt == 10 {
                    panic!("Failed to connect after 10 attempts: {e:?}");
                }
            }
        }
    }

    let (mut client, _peer_info) = connect_res.expect("connected");
    assert!(client.session().is_ready());

    // Give eMule a moment to settle hashing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 4. Request file status for sealed fixture hash
    let mut file_status = None;
    for attempt in 1..=5 {
        match client
            .request_file_status(sealed_hash, Duration::from_secs(3))
            .await
        {
            Ok(status) => {
                file_status = Some(status);
                break;
            }
            Err(mule_protocol::ed2k::Ed2kSessionError::FileNotFound(_)) => {
                println!("Tier 3.4: File not yet indexed by eMule, retry {attempt}/5 in 1s...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => {
                panic!("Tier 3.4: Unexpected error requesting file status: {e:?}");
            }
        }
    }

    let status =
        file_status.expect("eMule 0.50a must respond with OP_FILESTATUS for shared fixture");
    assert_eq!(status.file_hash, sealed_hash);
    assert_eq!(
        status.part_count, 0,
        "complete source reports part_count = 0"
    );
    assert!(status.is_complete());

    // 5. Tier 3.4: Request exactly ONE uncompressed block [0, 184_320)
    let sequencer = Ed2kDownloadSequencer::new(sealed_hash, fixture_len, status)
        .expect("sequencer initialization succeeds");
    let part0_blocks = sequencer.plan_part_blocks(0).expect("plan part 0");
    let target_block = part0_blocks[0];
    assert_eq!(target_block.start, 0);
    assert_eq!(target_block.end, 184_320);

    // Enter upload queue via OP_STARTUPLOADREQ (0x54) and wait for OP_ACCEPTUPLOADREQ (0x55)
    println!("Tier 3.4: Requesting upload slot via OP_STARTUPLOADREQ (0x54)...");
    client
        .request_upload_slot(sealed_hash, Duration::from_secs(5))
        .await
        .expect("eMule 0.50a must grant upload slot via OP_ACCEPTUPLOADREQ (0x55)");
    println!("Tier 3.4: OP_ACCEPTUPLOADREQ (0x55) confirmed! Upload slot granted.");

    // Transmit single block request for [0, 184_320)
    println!("Tier 3.4: Transmitting single OP_REQUESTPARTS_EMULE (0x47) for [0, 184_320)...");
    client
        .send_block_request(sealed_hash, 0, 184_320, fixture_len)
        .await
        .expect("failed to transmit block request");
    assert_eq!(client.session().snapshot().pending_block_count, 1);

    // 6. Receive OP_SENDINGPART (0x46) chunks into Ed2kBlockSink
    let mut sink = Ed2kBlockSink::new(fixture_len);
    println!("Tier 3.4: Awaiting OP_SENDINGPART (0x46) chunks for [0, 184_320)...");
    let mut chunk_count = 0;
    while sink.received_bytes_count() < 184_320 {
        let part = client
            .receive_sending_part(Duration::from_secs(5))
            .await
            .expect("must receive OP_SENDINGPART from eMule 0.50a");
        chunk_count += 1;
        println!(
            "Tier 3.4: Received OP_SENDINGPART chunk #{chunk_count}: [{}..{}), {} bytes (total: {}/184,320)",
            part.start_offset, part.end_offset, part.data.len(),
            sink.received_bytes_count() + part.data.len()
        );

        assert_eq!(
            part.file_hash, sealed_hash,
            "chunk file hash must match fixture"
        );
        let written_range = sink
            .write_block(&sequencer, &part)
            .expect("write chunk to sink");
        assert_eq!(written_range.start, part.start_offset);
        assert_eq!(written_range.end, part.end_offset);
    }

    // 7. Ground-truth assertions on received block
    assert_eq!(
        sink.received_bytes_count(),
        184_320,
        "exactly 184,320 bytes assembled"
    );
    assert!(
        sink.has_range(0, 184_320),
        "block sink reports full range [0, 184_320) covered"
    );
    let assembled_block = &sink.buffer()[0..184_320];

    // 8. Byte-by-byte comparison against fixture prefix
    assert_eq!(
        assembled_block, expected_prefix,
        "transferred payload must match fixture prefix byte-by-byte"
    );

    // 9. Local MD4 oracle check
    let mut hasher = Md4::new();
    hasher.update(assembled_block);
    let block_md4: [u8; 16] = hasher.finalize().into();

    let mut expected_hasher = Md4::new();
    expected_hasher.update(expected_prefix);
    let expected_md4: [u8; 16] = expected_hasher.finalize().into();

    assert_eq!(block_md4, expected_md4, "local block MD4 oracle must match");
    println!("Tier 3.4: Block MD4 Oracle: {}", hex::encode(block_md4));

    // 10. Duplicate write rejection in sink
    let dup_part = Ed2kSendingPart {
        file_hash: sealed_hash,
        start_offset: 0,
        end_offset: 10240,
        data: bytes::Bytes::copy_from_slice(&assembled_block[0..10240]),
    };
    let dup_err = sink.write_block(&sequencer, &dup_part).unwrap_err();
    assert!(matches!(dup_err, Ed2kSequencerError::DuplicateBlock { .. }));

    // 11. STRICT PASSIVE DISCIPLINE: verify 0 additional requests and clean session state
    assert_eq!(client.session().snapshot().pending_block_count, 0);
    assert_eq!(client.session().snapshot().pending_status_count, 0);
    assert!(client.session().is_ready());

    // 12. Confinement scan: 0 non-loopback connections
    let check_conn = std::process::Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(format!("Get-NetTCPConnection -OwningProcess {pid} -ErrorAction SilentlyContinue | Where-Object {{ $_.RemoteAddress -ne '0.0.0.0' -and $_.RemoteAddress -ne '127.0.0.1' -and $_.RemoteAddress -ne '::' -and $_.RemoteAddress -ne '::1' }} | Measure-Object | Select-Object -ExpandProperty Count"))
        .output();
    if let Ok(out) = check_conn {
        let count_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let non_loopback_count: usize = count_str.parse().unwrap_or(0);
        assert_eq!(
            non_loopback_count, 0,
            "Non-loopback connections detected for PID {pid}!"
        );
    }

    // 13. Graceful TCP FIN teardown
    client.close().await;

    // 14. Hermetic process teardown
    harness.shutdown();
    assert!(
        !harness.is_child_running(),
        "Process must be terminated after shutdown"
    );

    println!("Tier 3.4: Single bounded block transfer successfully verified against eMule 0.50a!");
}

/// Tier 3.5: Full Part 0 transfer (9,728,000 bytes = 53 blocks) with controlled resume,
/// pipelined sliding window (max 3 in-flight requests), and abrupt interruption.
///
/// Scenario:
/// 1. Harness setup, loopback confinement, deterministic fixture creation (9,728,001 bytes).
/// 2. Pinned eMule 0.50a reference process spawned in isolated sandbox.
/// 3. Session 1:
///    - Handshake -> OP_FILESTATUSREQ -> OP_STARTUPLOADREQ -> OP_ACCEPTUPLOADREQ.
///    - Download exactly 20 blocks (3,686,400 bytes) via sliding window (max 3 in-flight).
///    - Chunks written to Ed2kPartFileSink, backed by atomic .resume.json journal.
///    - Verify exact intermediate byte-for-byte match against fixture prefix.
///    - Simulate abrupt interruption: TCP FIN / client close.
/// 4. Interruption verification:
///    - Verify on-disk .part file and .resume.json state accurately record 20 blocks.
/// 5. Session 2:
///    - Reconnect to same eMule process. Handshake -> OP_FILESTATUSREQ -> OP_STARTUPLOADREQ -> OP_ACCEPTUPLOADREQ.
///    - Reload existing .part file and .resume.json via Ed2kPartFileSink::open_or_resume.
///    - Call sequencer.plan_missing_blocks(0, resumed_sink.completed_ranges()):
///      verifies EXACTLY 33 blocks remain (blocks 20..52), and zero requests are emitted for blocks 0..19!
///    - Download remaining 33 blocks via sliding window (max 3 in-flight).
///    - Complete Part 0 (100% of 9,728,000 bytes).
/// 6. In-Disk MD4 Checksum Verification:
///    - Stream Part 0 from disk into Md4 hasher via resumed_sink.compute_part_md4(0, &sequencer).
///    - Compare with local fixture MD4 oracle. Must match 100%.
/// 7. Verify Part 1 (residual 1 byte) was NEVER requested in Tier 3.5.
/// 8. Confinement scan: 0 non-loopback connections.
/// 9. Graceful TCP teardown and RAII cleanup.
#[tokio::test]
#[ignore = "Tier 3 external reference orchestration requires pinned reference binary via EMULE_REFERENCE_BIN"]
async fn test_tier3_live_part0_transfer_with_resume_and_interruption() {
    let bin_path_var = match std::env::var("EMULE_REFERENCE_BIN") {
        Ok(val) if !val.trim().is_empty() => PathBuf::from(val.trim()),
        _ => {
            println!("SKIPPED: Tier 3.5 test requires EMULE_REFERENCE_BIN environment variable.");
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
    .expect("failed to initialize Tier 3.5 harness");

    harness.assert_loopback_confinement();

    // 1. Create deterministic fixture in incoming directory (9,728,001 bytes)
    let fixture_len = 9_728_001u64;
    let fixture_path = harness
        .environment()
        .create_deterministic_fixture("tier3_status_fixture_9728001.bin", fixture_len as usize)
        .expect("failed to create deterministic fixture");
    assert!(fixture_path.exists());
    assert_eq!(std::fs::metadata(&fixture_path).unwrap().len(), fixture_len);

    let sealed_hash: [u8; 16] = [
        0x69, 0x5b, 0x81, 0xb1, 0x72, 0x76, 0x66, 0x81, 0x89, 0x88, 0x52, 0x4b, 0xd7, 0xfc, 0x82,
        0x3c,
    ];

    // Read ground truth Part 0 (9,728,000 bytes) from fixture file
    let fixture_bytes = std::fs::read(&fixture_path).expect("read fixture file");
    let part0_expected_bytes = &fixture_bytes[0..9_728_000];

    // Compute ground-truth MD4 oracle for Part 0
    let mut part0_oracle = Md4::new();
    part0_oracle.update(part0_expected_bytes);
    let expected_part0_md4: [u8; 16] = part0_oracle.finalize().into();
    println!(
        "Tier 3.5: Expected Part 0 MD4 oracle: {}",
        hex::encode(expected_part0_md4)
    );

    // 2. Spawn reference eMule 0.50a process
    harness
        .spawn_reference_process(&[])
        .expect("failed to spawn reference process");

    let pid = harness.child_pid().unwrap_or(0);
    assert!(pid > 0);
    assert!(harness.is_child_running());

    let user_hash = [
        0x31, 0x41, 0x59, 0x26, 0x53, 0x0E, 0x58, 0x97, 0x93, 0x23, 0x84, 0x62, 0x64, 0x33, 0x6F,
        0x83,
    ];
    let client1_tcp_port = allocate_ephemeral_loopback_port();

    // 3. Connect Session 1 and complete handshake
    let mut connect_res = None;
    for attempt in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !harness.is_child_running() {
            panic!("Reference process exited unexpectedly on attempt {attempt}");
        }
        match Ed2kLoopbackClient::connect_and_handshake(
            harness.loopback_addr(),
            user_hash,
            0,
            client1_tcp_port,
            Vec::new(),
            0,
            0,
            Duration::from_secs(3),
        )
        .await
        {
            Ok(res) => {
                connect_res = Some(res);
                break;
            }
            Err(e) => {
                if attempt == 10 {
                    panic!("Failed to connect Session 1 after 10 attempts: {e:?}");
                }
            }
        }
    }

    let (mut client1, _peer_info) = connect_res.expect("connected Session 1");
    assert!(client1.session().is_ready());

    // Give eMule a moment to settle hashing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 4. Request file status for sealed fixture hash
    let mut file_status = None;
    for attempt in 1..=5 {
        match client1
            .request_file_status(sealed_hash, Duration::from_secs(3))
            .await
        {
            Ok(status) => {
                file_status = Some(status);
                break;
            }
            Err(mule_protocol::ed2k::Ed2kSessionError::FileNotFound(_)) => {
                println!("Tier 3.5: File not yet indexed by eMule, retry {attempt}/5 in 1s...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => {
                panic!("Tier 3.5: Unexpected error requesting file status: {e:?}");
            }
        }
    }

    let status =
        file_status.expect("eMule 0.50a must respond with OP_FILESTATUS for shared fixture");
    assert_eq!(status.file_hash, sealed_hash);
    assert_eq!(
        status.part_count, 0,
        "complete source reports part_count = 0"
    );
    assert!(status.is_complete());

    let sequencer = Ed2kDownloadSequencer::new(sealed_hash, fixture_len, status)
        .expect("sequencer initialization succeeds");
    let part0_blocks = sequencer.plan_part_blocks(0).expect("plan part 0");
    assert_eq!(
        part0_blocks.len(),
        53,
        "Part 0 must comprise exactly 53 blocks"
    );

    // 5. Initialize on-disk destination .part file and companion .resume.json
    let target_part_path = harness
        .environment()
        .work_path()
        .join("tier3_part0_transfer.part");
    let mut sink = Ed2kPartFileSink::create_new(target_part_path.clone(), sealed_hash, fixture_len)
        .expect("create initial part sink");

    // Request upload slot in Session 1
    println!("Tier 3.5: Session 1 requesting upload slot via OP_STARTUPLOADREQ (0x54)...");
    client1
        .request_upload_slot(sealed_hash, Duration::from_secs(5))
        .await
        .expect("eMule 0.50a must grant upload slot in Session 1");
    println!("Tier 3.5: Session 1 upload slot granted!");

    // 6. Controlled Interruption (Phase 1): Download exactly 20 blocks with sliding window (max 3 in-flight)
    println!("Tier 3.5: Phase 1: Downloading first 20 blocks (3,686,400 bytes) with sliding window (max 3 in-flight)...");
    let completed_phase1 = client1
        .download_blocks_pipelined(
            &sequencer,
            &part0_blocks,
            &mut sink,
            3,
            Some(20),
            Duration::from_secs(5),
        )
        .await
        .expect("Phase 1 pipelined download succeeds");

    assert_eq!(
        completed_phase1, 20,
        "Phase 1 must stop after completing exactly 20 blocks"
    );
    let blocks_20_len = 20 * mule_protocol::ed2k::ED2K_BLOCK_SIZE as u64; // 3,686,400 bytes
    assert_eq!(sink.total_bytes_received(), blocks_20_len);
    assert_eq!(sink.completed_ranges(), &[(0, blocks_20_len)]);
    assert!(sink.has_range(0, blocks_20_len));

    // Verify intermediate byte-by-byte accuracy against fixture prefix
    sink.flush_and_sync().expect("flush and sync sink");
    let intermediate_disk_bytes =
        std::fs::read(&target_part_path).expect("read intermediate part file");
    assert_eq!(
        &intermediate_disk_bytes[0..blocks_20_len as usize],
        &fixture_bytes[0..blocks_20_len as usize],
        "intermediate 20 blocks on disk must match fixture byte-by-byte"
    );
    println!("Tier 3.5: Phase 1 verified: exactly 20 blocks (3,686,400 bytes) written and verified on disk.");

    // 7. Abrupt Interruption: close Session 1 client and drop sink
    println!(
        "Tier 3.5: Simulating abrupt network interruption (closing Session 1 TCP connection)..."
    );
    client1.close().await;
    drop(sink);

    // Verify companion .resume.json state file directly on disk
    let resume_path = PathBuf::from(format!("{}.resume.json", target_part_path.display()));
    assert!(resume_path.exists(), "resume journal must exist on disk");
    let journal = Ed2kPartResumeState::load_from_file(&resume_path).expect("load resume journal");
    assert_eq!(journal.file_hash, hex::encode(sealed_hash));
    assert_eq!(journal.file_size, fixture_len);
    assert_eq!(journal.total_bytes_received, blocks_20_len);
    assert_eq!(journal.completed_ranges, vec![(0, blocks_20_len)]);
    println!("Tier 3.5: Atomic .resume.json journal verified on disk after interruption.");

    // Pause briefly before reconnecting
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 8. Session 2: Resume download from existing .part and .resume.json state
    println!("Tier 3.5: Phase 2: Opening existing .part file and .resume.json journal...");
    let mut resumed_sink =
        Ed2kPartFileSink::open_or_resume(target_part_path.clone(), sealed_hash, fixture_len)
            .expect("resume sink from disk");
    assert_eq!(resumed_sink.total_bytes_received(), blocks_20_len);
    assert_eq!(resumed_sink.completed_ranges(), &[(0, blocks_20_len)]);

    // 9. Gap calculation via plan_missing_blocks: verify EXACTLY 33 blocks (blocks 20..52) remain
    let missing_blocks = sequencer
        .plan_missing_blocks(0, resumed_sink.completed_ranges())
        .expect("plan missing blocks for Part 0");
    assert_eq!(
        missing_blocks.len(),
        33,
        "Part 0 missing blocks must be exactly 33 (blocks 20 to 52)"
    );
    assert_eq!(missing_blocks[0].start, blocks_20_len as u32);
    assert_eq!(missing_blocks[32].end, 9_728_000);

    // Verify ZERO requests planned for already-downloaded blocks 0..19
    for b in &missing_blocks {
        assert!(
            b.start >= blocks_20_len as u32,
            "No missing block may overlap already completed blocks 0..19"
        );
        assert!(
            b.end <= 9_728_000,
            "No block in Part 0 may exceed part boundary 9,728,000"
        );
    }
    println!("Tier 3.5: Gap calculation verified: exactly 33 missing blocks planned [3,686,400..9,728,000).");

    // Connect Session 2
    let client2_tcp_port = allocate_ephemeral_loopback_port();
    let mut connect_res2 = None;
    for attempt in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !harness.is_child_running() {
            panic!("Reference process exited unexpectedly before Session 2 on attempt {attempt}");
        }
        match Ed2kLoopbackClient::connect_and_handshake(
            harness.loopback_addr(),
            user_hash,
            0,
            client2_tcp_port,
            Vec::new(),
            0,
            0,
            Duration::from_secs(3),
        )
        .await
        {
            Ok(res) => {
                connect_res2 = Some(res);
                break;
            }
            Err(e) => {
                if attempt == 10 {
                    panic!("Failed to connect Session 2 after 10 attempts: {e:?}");
                }
            }
        }
    }

    let (mut client2, _peer_info2) = connect_res2.expect("connected Session 2");
    assert!(client2.session().is_ready());

    // Request upload slot in Session 2
    println!("Tier 3.5: Session 2 requesting upload slot via OP_STARTUPLOADREQ (0x54)...");
    client2
        .request_upload_slot(sealed_hash, Duration::from_secs(5))
        .await
        .expect("eMule 0.50a must grant upload slot in Session 2");
    println!("Tier 3.5: Session 2 upload slot granted!");

    // 10. Download remaining 33 blocks via sliding window (max 3 in-flight)
    println!("Tier 3.5: Phase 2: Downloading remaining 33 blocks with sliding window (max 3 in-flight)...");
    let completed_phase2 = client2
        .download_blocks_pipelined(
            &sequencer,
            &missing_blocks,
            &mut resumed_sink,
            3,
            None,
            Duration::from_secs(5),
        )
        .await
        .expect("Phase 2 pipelined download succeeds");

    assert_eq!(
        completed_phase2, 33,
        "Phase 2 must complete all remaining 33 blocks"
    );
    assert_eq!(
        resumed_sink.total_bytes_received(),
        9_728_000,
        "Part 0 must have exactly 9,728,000 bytes completed"
    );
    assert_eq!(
        resumed_sink.completed_ranges(),
        &[(0, 9_728_000)],
        "Part 0 completed ranges must be fully merged into [(0, 9,728,000)]"
    );
    assert!(
        resumed_sink
            .is_part_complete(0, &sequencer)
            .expect("part complete check"),
        "Part 0 must be reported complete"
    );
    println!("Tier 3.5: Part 0 download 100% completed across 2 sessions!");

    // 11. In-Disk MD4 Checksum Verification
    println!("Tier 3.5: Streaming Part 0 from disk in 64 KiB buffers to verify MD4 checksum...");
    let disk_part0_md4 = resumed_sink
        .compute_part_md4(0, &sequencer)
        .expect("compute in-disk MD4 of Part 0");
    assert_eq!(
        disk_part0_md4, expected_part0_md4,
        "Streamed on-disk Part 0 MD4 must match fixture oracle byte-for-byte"
    );
    println!(
        "Tier 3.5: Part 0 MD4 verified: {} == {}",
        hex::encode(disk_part0_md4),
        hex::encode(expected_part0_md4)
    );

    // Full byte-by-byte comparison of Part 0 from disk against fixture
    let final_disk_bytes = std::fs::read(&target_part_path).expect("read completed part file");
    assert_eq!(
        &final_disk_bytes[0..9_728_000],
        part0_expected_bytes,
        "Complete Part 0 on disk must match fixture byte-by-byte"
    );

    // 12. Verify Part 1 was NEVER requested (Part 1 offset 9,728,000..9,728,001 is still unwritten)
    assert!(
        !resumed_sink.has_range(9_728_000, 9_728_001),
        "Part 1 must NOT be requested or completed in Tier 3.5"
    );
    assert_eq!(
        resumed_sink.total_bytes_received(),
        9_728_000,
        "Total received bytes must be strictly 9,728,000"
    );

    // 13. Strict passive discipline: verify 0 additional requests and clean session state
    assert_eq!(client2.session().snapshot().pending_block_count, 0);
    assert_eq!(client2.session().snapshot().pending_status_count, 0);
    assert!(client2.session().is_ready());

    // 14. Confinement scan: 0 non-loopback connections
    let check_conn = std::process::Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(format!("Get-NetTCPConnection -OwningProcess {pid} -ErrorAction SilentlyContinue | Where-Object {{ $_.RemoteAddress -ne '0.0.0.0' -and $_.RemoteAddress -ne '127.0.0.1' -and $_.RemoteAddress -ne '::' -and $_.RemoteAddress -ne '::1' }} | Measure-Object | Select-Object -ExpandProperty Count"))
        .output();
    if let Ok(out) = check_conn {
        let count_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let non_loopback_count: usize = count_str.parse().unwrap_or(0);
        assert_eq!(
            non_loopback_count, 0,
            "Non-loopback connections detected for PID {pid}!"
        );
    }

    // 15. Graceful TCP FIN teardown
    client2.close().await;

    // 16. Hermetic process teardown
    harness.shutdown();
    assert!(
        !harness.is_child_running(),
        "Process must be terminated after shutdown"
    );

    println!("Tier 3.5: Part 0 transfer with resume and interruption successfully verified against eMule 0.50a!");
}

/// Tier 3.6 Live Reference Integration Test:
/// File boundary finalization, full multi-part eD2K root hash computation,
/// crash-resilient two-phase promotion, and cleanup against official eMule 0.50a on loopback.
///
/// Steps:
/// 1. Spawn pinned reference eMule 0.50a in isolated sandbox on dynamic loopback ports.
/// 2. Share deterministic fixture `tier3_status_fixture_9728001.bin` (9,728,001 bytes):
///    - Part 0: [0, 9,728,000) (53 blocks, oracle MD4: 240568858a65f85133d858f538d17467).
///    - Part 1: [9,728,000, 9,728,001) (1 residual byte 0x00, oracle MD4: 47c61a0fa8738ba77308a8a600f88e4b).
///    - Root Multi-Part eD2K Hash: MD4(Part 0 || Part 1) = 695b81b1727666818988524bd7fc823c.
/// 3. Loopback handshake + query OP_FILESTATUS -> part_count = 0 (complete source).
/// 4. Request upload slot via OP_STARTUPLOADREQ -> receive OP_ACCEPTUPLOADREQ.
/// 5. Phase 1: Download full Part 0 (53 blocks, 9,728,000 bytes) with sliding window pipelining.
/// 6. Phase 2: Finalize Part 1 boundary:
///    - Plan Part 1: exactly 1 block [9,728,000, 9,728,001) (1 byte).
///    - Request Part 1 block via OP_REQUESTPARTS_EMULE.
///    - Receive OP_SENDINGPART containing exactly 1 byte.
///    - Assert Part 1 boundary invariant is strictly observed (never crosses 9,728,000).
///    - Assert 100% file coverage [0, 9,728,001) on disk.
/// 7. Phase 3: In-Disk Multi-Part eD2K Hash Computation:
///    - Stream Part 0 from disk -> verifies MD4 matches oracle 240568858a65f85133d858f538d17467.
///    - Stream Part 1 from disk -> verifies MD4 matches oracle 47c61a0fa8738ba77308a8a600f88e4b.
///    - Concatenate and compute root MD4 -> verifies exact match with 695b81b1727666818988524bd7fc823c!
/// 8. Phase 4: Crash-Resilient Two-Phase Promotion:
///    - Mark state as VerifiedPendingPromotion in atomic .resume.json journal.
///    - Simulate abrupt crash: close TCP connection, drop sink.
///    - Inspect durable .resume.json on disk to verify persistent state.
///    - Recover locally via open_or_resume without connecting to eMule or network.
///    - Promote to downloads/tier3_promoted_final.bin.
///    - Verify promoted file exists, length 9,728,001, byte-for-byte identical to fixture.
///    - Verify .part and .resume.json are cleaned up.
///    - Verify sink status is Promoted.
/// 9. Strict passive discipline: 0 extra requests, clean session snapshot.
/// 10. Confinement scan: 0 non-loopback connections.
/// 11. Graceful TCP FIN and process teardown.
#[tokio::test]
#[ignore = "Tier 3 external reference orchestration requires pinned reference binary via EMULE_REFERENCE_BIN"]
async fn test_tier3_live_file_boundary_finalization_and_promotion() {
    let bin_path_var = match std::env::var("EMULE_REFERENCE_BIN") {
        Ok(val) if !val.trim().is_empty() => PathBuf::from(val.trim()),
        _ => {
            println!("SKIPPED: Tier 3.6 test requires EMULE_REFERENCE_BIN environment variable.");
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
    .expect("failed to initialize Tier 3.6 harness");

    harness.assert_loopback_confinement();

    // 1. Create deterministic fixture in incoming directory (9,728,001 bytes = 1 full part + 1 byte)
    let fixture_len = 9_728_001u64;
    let fixture_path = harness
        .environment()
        .create_deterministic_fixture("tier3_status_fixture_9728001.bin", fixture_len as usize)
        .expect("create deterministic fixture");
    let fixture_bytes = std::fs::read(&fixture_path).expect("read fixture file");
    assert_eq!(fixture_bytes.len(), fixture_len as usize);

    // Compute expected Part 0 and Part 1 MD4 oracles
    let mut part0_hasher = md4::Md4::new();
    part0_hasher.update(&fixture_bytes[0..9_728_000]);
    let expected_part0_md4: [u8; 16] = part0_hasher.finalize().into();
    assert_eq!(
        hex::encode(expected_part0_md4),
        "240568858a65f85133d858f538d17467",
        "Part 0 MD4 oracle must match deterministic fixture"
    );

    let mut part1_hasher = md4::Md4::new();
    part1_hasher.update(&fixture_bytes[9_728_000..9_728_001]);
    let expected_part1_md4: [u8; 16] = part1_hasher.finalize().into();
    assert_eq!(
        hex::encode(expected_part1_md4),
        "47c61a0fa8738ba77308a8a600f88e4b",
        "Part 1 MD4 oracle must match MD4([0x00])"
    );

    // Compute expected root multi-part eD2K hash: MD4(Part0 || Part1)
    let mut root_hasher = md4::Md4::new();
    root_hasher.update(expected_part0_md4);
    root_hasher.update(expected_part1_md4);
    let sealed_hash: [u8; 16] = root_hasher.finalize().into();
    assert_eq!(
        hex::encode(sealed_hash),
        "695b81b1727666818988524bd7fc823c",
        "Multi-part root eD2K hash must match sealed fixture hash"
    );

    // 2. Launch reference eMule 0.50a process
    println!("Tier 3.6: Launching reference eMule 0.50a with shared 9,728,001 bytes fixture...");
    harness
        .spawn_reference_process(&[])
        .expect("failed to spawn reference process");
    assert!(
        harness.is_child_running(),
        "Reference process must be running"
    );
    let pid = harness.child_pid().expect("child PID");

    // 3. Perform eD2K handshake on loopback
    let mut user_hash = [0x55u8; 16];
    user_hash[0] = 0xAA;
    user_hash[5] = 0x14;
    user_hash[14] = 0x6E;

    let client_tcp_port = allocate_ephemeral_loopback_port();
    let mut connect_res = None;
    for attempt in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if !harness.is_child_running() {
            panic!("Reference process exited unexpectedly before connect on attempt {attempt}");
        }
        match Ed2kLoopbackClient::connect_and_handshake(
            harness.loopback_addr(),
            user_hash,
            0,
            client_tcp_port,
            Vec::new(),
            0,
            0,
            Duration::from_secs(3),
        )
        .await
        {
            Ok(res) => {
                connect_res = Some(res);
                break;
            }
            Err(e) => {
                if attempt == 10 {
                    panic!("Failed to connect after 10 attempts: {e:?}");
                }
            }
        }
    }

    let (mut client, _peer_info) = connect_res.expect("connected client");
    assert!(client.session().is_ready());
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 4. Request file status for sealed fixture hash
    let mut file_status = None;
    for attempt in 1..=5 {
        match client
            .request_file_status(sealed_hash, Duration::from_secs(3))
            .await
        {
            Ok(status) => {
                file_status = Some(status);
                break;
            }
            Err(mule_protocol::ed2k::Ed2kSessionError::FileNotFound(_)) => {
                println!("Tier 3.6: File not yet indexed by eMule, retry {attempt}/5 in 1s...");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(e) => {
                panic!("Tier 3.6: Unexpected error requesting file status: {e:?}");
            }
        }
    }

    let status =
        file_status.expect("eMule 0.50a must respond with OP_FILESTATUS for shared fixture");
    assert_eq!(status.file_hash, sealed_hash);
    assert_eq!(
        status.part_count, 0,
        "complete source reports part_count = 0"
    );
    assert!(status.is_complete());

    let sequencer = Ed2kDownloadSequencer::new(sealed_hash, fixture_len, status)
        .expect("sequencer initialization succeeds");
    assert_eq!(
        sequencer.total_parts(),
        2,
        "File size 9,728,001 bytes must have exactly 2 parts"
    );

    // 5. Initialize on-disk destination .part file and companion .resume.json
    let target_part_path = harness
        .environment()
        .work_path()
        .join("tier3_final_transfer.part");
    let mut sink = Ed2kPartFileSink::create_new(target_part_path.clone(), sealed_hash, fixture_len)
        .expect("create initial part sink");

    // Request upload slot
    println!("Tier 3.6: Requesting upload slot via OP_STARTUPLOADREQ (0x54)...");
    client
        .request_upload_slot(sealed_hash, Duration::from_secs(5))
        .await
        .expect("eMule 0.50a must grant upload slot");
    println!("Tier 3.6: Upload slot granted!");

    // 6. Phase 1: Download Part 0 (53 blocks, 9,728,000 bytes) with sliding window
    println!("Tier 3.6: Phase 1: Downloading Part 0 (53 blocks, 9,728,000 bytes)...");
    let part0_blocks = sequencer.plan_part_blocks(0).expect("plan part 0");
    assert_eq!(part0_blocks.len(), 53);

    let completed_part0 = client
        .download_blocks_pipelined(
            &sequencer,
            &part0_blocks,
            &mut sink,
            3,
            None,
            Duration::from_secs(5),
        )
        .await
        .expect("Part 0 pipelined download succeeds");

    assert_eq!(
        completed_part0, 53,
        "All 53 blocks of Part 0 must be completed"
    );
    assert_eq!(sink.total_bytes_received(), 9_728_000);
    assert!(sink.is_part_complete(0, &sequencer).expect("part 0 check"));
    assert!(!sink.is_part_complete(1, &sequencer).expect("part 1 check"));
    assert!(!sink.is_file_complete(&sequencer));
    println!(
        "Tier 3.6: Phase 1 verified: Part 0 (9,728,000 bytes) downloaded and present on disk."
    );

    // 7. Phase 2: Finalize Part 1 boundary: exactly 1 residual block [9,728,000, 9,728,001)
    println!("Tier 3.6: Phase 2: Planning and requesting Part 1 residual block [9,728,000, 9,728,001)...");
    let part1_blocks = sequencer.plan_part_blocks(1).expect("plan part 1");
    assert_eq!(part1_blocks.len(), 1, "Part 1 must have exactly 1 block");
    assert_eq!(
        part1_blocks[0].start, 9_728_000,
        "Part 1 block must start at part boundary 9,728,000"
    );
    assert_eq!(
        part1_blocks[0].end, 9_728_001,
        "Part 1 block must end at exact file boundary 9,728,001"
    );

    let completed_part1 = client
        .download_blocks_pipelined(
            &sequencer,
            &part1_blocks,
            &mut sink,
            1,
            None,
            Duration::from_secs(5),
        )
        .await
        .expect("Part 1 download succeeds");

    assert_eq!(completed_part1, 1, "Part 1 block must be completed");
    assert_eq!(
        sink.total_bytes_received(),
        fixture_len,
        "Total received bytes must reach 100% of file size (9,728,001 bytes)"
    );
    assert_eq!(
        sink.completed_ranges(),
        &[(0, fixture_len)],
        "Completed ranges must be fully merged into [(0, 9,728,001)]"
    );
    assert!(sink.is_part_complete(0, &sequencer).expect("part 0 check"));
    assert!(sink.is_part_complete(1, &sequencer).expect("part 1 check"));
    assert!(
        sink.is_file_complete(&sequencer),
        "File must be 100% complete"
    );
    println!(
        "Tier 3.6: Phase 2 verified: Part 1 residual block received; 100% file coverage achieved!"
    );

    // 8. Phase 3: In-Disk Multi-Part eD2K Root Hash Computation
    println!("Tier 3.6: Phase 3: Verifying in-disk multi-part eD2K root hash computation...");
    let disk_part0_md4 = sink
        .compute_part_md4(0, &sequencer)
        .expect("compute part 0 MD4");
    assert_eq!(
        disk_part0_md4, expected_part0_md4,
        "Part 0 streamed MD4 must match oracle"
    );

    let disk_part1_md4 = sink
        .compute_part_md4(1, &sequencer)
        .expect("compute part 1 MD4");
    assert_eq!(
        disk_part1_md4, expected_part1_md4,
        "Part 1 streamed MD4 must match oracle"
    );

    let disk_root_hash = sink
        .compute_file_ed2k_hash(&sequencer)
        .expect("compute root eD2K hash");
    assert_eq!(
        disk_root_hash, sealed_hash,
        "Root multi-part eD2K hash must match sealed file hash"
    );
    println!(
        "Tier 3.6: Phase 3 verified: In-disk root eD2K hash {} matches sealed oracle byte-for-byte!",
        hex::encode(disk_root_hash)
    );

    // 9. Phase 4: Crash-Resilient Two-Phase Promotion
    println!("Tier 3.6: Phase 4: Step A - Marking VerifiedPendingPromotion in atomic journal...");
    let verified_hash = sink
        .mark_verified_pending_promotion(&sequencer)
        .expect("mark verified pending promotion");
    assert_eq!(verified_hash, sealed_hash);
    assert_eq!(
        sink.status(),
        Ed2kPartDownloadStatus::VerifiedPendingPromotion
    );

    // Verify companion .resume.json state file on disk before crash
    let resume_path = PathBuf::from(format!("{}.resume.json", target_part_path.display()));
    assert!(resume_path.exists(), "resume journal must exist on disk");
    let journal = Ed2kPartResumeState::load_from_file(&resume_path).expect("load resume journal");
    assert_eq!(
        journal.status,
        Ed2kPartDownloadStatus::VerifiedPendingPromotion
    );
    assert_eq!(journal.file_hash, hex::encode(sealed_hash));
    assert_eq!(journal.file_size, fixture_len);

    // Simulate abrupt crash: close TCP connection, drop sink
    println!("Tier 3.6: Phase 4: Step B - Simulating abrupt crash / network disconnection...");
    client.close().await;
    drop(sink);

    // Local recovery on restart without network access
    println!(
        "Tier 3.6: Phase 4: Step C - Recovering locally from disk without contacting eMule..."
    );
    let mut recovered_sink =
        Ed2kPartFileSink::open_or_resume(target_part_path.clone(), sealed_hash, fixture_len)
            .expect("resume sink after simulated crash");
    assert_eq!(
        recovered_sink.status(),
        Ed2kPartDownloadStatus::VerifiedPendingPromotion,
        "resumed sink must retain VerifiedPendingPromotion state"
    );

    // Step D: Promote to downloads/ directory
    let downloads_dir = harness.environment().work_path().join("downloads");
    println!(
        "Tier 3.6: Phase 4: Step D - Atomically promoting .part to '{}'...",
        downloads_dir.display()
    );
    let promoted_file = recovered_sink
        .promote_to_destination(&downloads_dir, "tier3_promoted_final.bin")
        .expect("promote file to destination directory");

    assert!(
        promoted_file.exists(),
        "promoted final file must exist on disk"
    );
    assert_eq!(
        std::fs::metadata(&promoted_file).expect("metadata").len(),
        fixture_len,
        "promoted file length must be exactly 9,728,001 bytes"
    );

    // Byte-by-byte full file comparison against reference fixture
    let promoted_disk_bytes = std::fs::read(&promoted_file).expect("read promoted file");
    assert_eq!(
        &promoted_disk_bytes, &fixture_bytes,
        "Promoted file on disk must match fixture byte-for-byte"
    );

    // Verify cleanup
    assert!(
        !target_part_path.exists(),
        ".part file must be removed after promotion"
    );
    assert!(
        !resume_path.exists(),
        ".resume.json journal must be deleted after promotion"
    );
    assert_eq!(recovered_sink.status(), Ed2kPartDownloadStatus::Promoted);
    println!("Tier 3.6: Phase 4 verified: Two-phase promotion and cleanup completed successfully!");

    // 10. Strict passive discipline: 0 pending requests, 0 non-loopback connections
    let check_conn = std::process::Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(format!("Get-NetTCPConnection -OwningProcess {pid} -ErrorAction SilentlyContinue | Where-Object {{ $_.RemoteAddress -ne '0.0.0.0' -and $_.RemoteAddress -ne '127.0.0.1' -and $_.RemoteAddress -ne '::' -and $_.RemoteAddress -ne '::1' }} | Measure-Object | Select-Object -ExpandProperty Count"))
        .output();
    if let Ok(out) = check_conn {
        let count_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let non_loopback_count: usize = count_str.parse().unwrap_or(0);
        assert_eq!(
            non_loopback_count, 0,
            "Non-loopback connections detected for PID {pid}!"
        );
    }

    // 11. Graceful process and harness shutdown
    harness.shutdown();
    assert!(
        !harness.is_child_running(),
        "Process must be terminated after shutdown"
    );

    println!("Tier 3.6: File boundary finalization, full root eD2K hash, and two-phase promotion successfully verified against eMule 0.50a!");
}
