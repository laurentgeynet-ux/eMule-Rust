use serde_json::{json, Value};
use tempfile::tempdir;

use mule_daemon::rpc::methods::handle_request;
use mule_daemon::rpc::types::JsonRpcRequest;
use mule_daemon::{AppState, DaemonConfig, DownloadService, WorkDirMode};
use mule_network_policy::NetworkProfile;

fn setup_test_state(profile: NetworkProfile) -> (AppState, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join("temp");
    let token = "wan_test_token_456".to_string();

    let config = DaemonConfig::new(dir.path().to_path_buf(), &token)
        .with_workdir_mode(WorkDirMode::Portable)
        .with_network_profile(profile)
        .with_upnp_enabled(false); // disable UPnP auto-probe in test to avoid network delay
    let downloads = DownloadService::in_memory(temp_dir).unwrap();
    let state = AppState::new(config, downloads);

    (state, dir)
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
async fn test_upnp_status_and_rpc() {
    let (state, _dir) = setup_test_state(NetworkProfile::PublicEd2kKadPreview);
    let mut authenticated = true;

    // 1. upnp.status initially disabled
    let req = make_req(1, "upnp.status", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["enabled"], false);
    assert_eq!(res["tcpPort"], 39735);
    assert_eq!(res["udpPort"], 39737);

    // 2. upnp.setEnabled true
    let req = make_req(2, "upnp.setEnabled", Some(json!({ "enabled": true })));
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["enabled"], true);

    // 3. upnp.unmapPorts
    let req = make_req(3, "upnp.unmapPorts", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
}

#[tokio::test]
async fn test_inbound_listener_0_0_0_0_in_preview_mode() {
    let (state, _dir) = setup_test_state(NetworkProfile::PublicEd2kKadPreview);
    let mut authenticated = true;

    // Start inbound listener on ephemeral port (port 0)
    let req = make_req(
        10,
        "listener.start",
        Some(json!({ "port": 0, "bindIp": "0.0.0.0" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_none(),
        "Expected success, got: {:?}",
        resp.error
    );
    let res = resp.result.unwrap();
    assert_eq!(res["started"], true);
    let listen_addr = res["listenAddress"].as_str().unwrap();
    assert!(listen_addr.starts_with("0.0.0.0:"));

    // Check listener status
    let req = make_req(11, "listener.status", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
    let res = resp.result.unwrap();
    assert_eq!(res["isListening"], true);

    // Stop listener
    let req = make_req(12, "listener.stop", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
}

#[tokio::test]
async fn test_inbound_listener_confined_to_loopback_in_local_lab() {
    let (state, _dir) = setup_test_state(NetworkProfile::LocalLab);
    let mut authenticated = true;

    // In LocalLab, explicit binding to 0.0.0.0 must be rejected by network policy
    let req = make_req(
        20,
        "listener.start",
        Some(json!({ "port": 0, "bindIp": "0.0.0.0" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_some(),
        "Expected rejection of 0.0.0.0 in LocalLab"
    );

    // Default binding without bindIp succeeds on loopback 127.0.0.1
    let req_default = make_req(21, "listener.start", Some(json!({ "port": 0 })));
    let resp_default = handle_request(&state, req_default, &mut authenticated)
        .await
        .unwrap();
    assert!(resp_default.error.is_none());
    let res = resp_default.result.unwrap();
    assert_eq!(res["started"], true);
    let listen_addr = res["listenAddress"].as_str().unwrap();
    assert!(listen_addr.starts_with("127.0.0.1:"));

    let _ = handle_request(
        &state,
        make_req(22, "listener.stop", None),
        &mut authenticated,
    )
    .await;
}

#[tokio::test]
async fn test_kad_udp_start_on_0_0_0_0_in_preview_mode() {
    let (state, _dir) = setup_test_state(NetworkProfile::PublicEd2kKadPreview);
    let mut authenticated = true;

    // Start Kad UDP on 0.0.0.0 with ephemeral port 0
    let req = make_req(
        30,
        "kad.startUdp",
        Some(json!({ "bindAddress": "0.0.0.0:0" })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_none(),
        "Expected success, got: {:?}",
        resp.error
    );
    let res = resp.result.unwrap();
    let bound = res["boundAddress"].as_str().unwrap();
    assert!(bound.starts_with("0.0.0.0:"));

    // Stop Kad UDP
    let req = make_req(31, "kad.stopUdp", None);
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(resp.error.is_none());
}

#[tokio::test]
async fn test_upnp_privileged_ports_rejected() {
    let (state, _dir) = setup_test_state(NetworkProfile::PublicEd2kKadPreview);
    let mut authenticated = true;

    // Port 80 (< 1024) must be rejected
    let req = make_req(
        40,
        "upnp.mapPorts",
        Some(json!({ "tcpPort": 80, "udpPort": 39737 })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_some(),
        "Privileged TCP port 80 must be rejected"
    );

    // Port 53 (< 1024) must be rejected
    let req = make_req(
        41,
        "upnp.mapPorts",
        Some(json!({ "tcpPort": 39735, "udpPort": 53 })),
    );
    let resp = handle_request(&state, req, &mut authenticated)
        .await
        .unwrap();
    assert!(
        resp.error.is_some(),
        "Privileged UDP port 53 must be rejected"
    );
}

#[test]
fn test_upnp_wan_ip_and_gateway_validation() {
    use mule_daemon::services::upnp::{is_cgnat_ip, is_valid_gateway_addr, is_valid_wan_ip};
    use std::net::{IpAddr, SocketAddr};

    // Valid public routable WAN IPs
    assert!(is_valid_wan_ip("8.8.8.8".parse().unwrap()));
    assert!(
        is_valid_wan_ip("198.51.100.1".parse().unwrap())
            || is_valid_wan_ip("93.184.216.34".parse().unwrap())
    );

    // Invalid WAN IPs (private, loopback, CGNAT, link-local)
    assert!(!is_valid_wan_ip("192.168.1.1".parse::<IpAddr>().unwrap()));
    assert!(!is_valid_wan_ip("10.0.0.1".parse::<IpAddr>().unwrap()));
    assert!(!is_valid_wan_ip("172.16.0.1".parse::<IpAddr>().unwrap()));
    assert!(!is_valid_wan_ip("127.0.0.1".parse::<IpAddr>().unwrap()));
    assert!(!is_valid_wan_ip("100.64.0.1".parse::<IpAddr>().unwrap())); // CGNAT — not publicly routable
    assert!(!is_valid_wan_ip("169.254.1.1".parse::<IpAddr>().unwrap())); // Link-local
    assert!(!is_valid_wan_ip("0.0.0.0".parse::<IpAddr>().unwrap()));

    // Valid gateway addresses (private LAN or loopback for testing)
    assert!(is_valid_gateway_addr(
        "192.168.1.1:1900".parse::<SocketAddr>().unwrap()
    ));
    assert!(is_valid_gateway_addr(
        "10.0.0.1:1900".parse::<SocketAddr>().unwrap()
    ));
    assert!(is_valid_gateway_addr(
        "127.0.0.1:1900".parse::<SocketAddr>().unwrap()
    ));

    // Invalid gateway addresses (public IPs trying to act as local gateway)
    assert!(!is_valid_gateway_addr(
        "8.8.8.8:1900".parse::<SocketAddr>().unwrap()
    ));

    // CGNAT range (RFC 6598 — 100.64.0.0/10): boundaries and interior
    // is_cgnat_ip must return true for the entire 100.64.0.0–100.127.255.255 range.
    assert!(is_cgnat_ip("100.64.0.0".parse::<IpAddr>().unwrap())); // first address
    assert!(is_cgnat_ip("100.64.0.1".parse::<IpAddr>().unwrap()));
    assert!(is_cgnat_ip("100.100.10.5".parse::<IpAddr>().unwrap()));
    assert!(is_cgnat_ip("100.127.255.255".parse::<IpAddr>().unwrap())); // last address
                                                                        // Just outside the /10 block
    assert!(!is_cgnat_ip("100.128.0.0".parse::<IpAddr>().unwrap()));
    assert!(!is_cgnat_ip("100.63.255.255".parse::<IpAddr>().unwrap()));
    // Unrelated addresses are not CGNAT
    assert!(!is_cgnat_ip("8.8.8.8".parse::<IpAddr>().unwrap()));
    assert!(!is_cgnat_ip("192.168.1.1".parse::<IpAddr>().unwrap()));
    // IPv6 is never CGNAT
    assert!(!is_cgnat_ip("::1".parse::<IpAddr>().unwrap()));
}

#[test]
fn test_upnp_mapping_ownership_inspection() {
    use igd_next::PortMappingProtocol;
    use mule_daemon::services::upnp::{inspect_mapping_entry, MappingOwnership};
    use std::net::{Ipv4Addr, SocketAddrV4};

    let our_endpoint = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 31), 39735);

    // 1. Exact match: identical protocol, port, local client IP, internal port and Emule-Sion prefix
    assert_eq!(
        inspect_mapping_entry(
            PortMappingProtocol::TCP,
            39735,
            "192.168.1.31",
            39735,
            "Emule-Sion eD2K TCP",
            PortMappingProtocol::TCP,
            39735,
            our_endpoint,
            "Emule-Sion",
        ),
        Some(MappingOwnership::ExactMatch)
    );

    // 2. Stale lease: Emule-Sion prefix but old client IP (e.g. DHCP lease change)
    assert_eq!(
        inspect_mapping_entry(
            PortMappingProtocol::TCP,
            39735,
            "192.168.1.50",
            39735,
            "Emule-Sion eD2K TCP",
            PortMappingProtocol::TCP,
            39735,
            our_endpoint,
            "Emule-Sion",
        ),
        Some(MappingOwnership::StaleLease)
    );

    // 3. Stale lease: Emule-Sion prefix but changed internal port
    assert_eq!(
        inspect_mapping_entry(
            PortMappingProtocol::TCP,
            39735,
            "192.168.1.31",
            4662,
            "Emule-Sion eD2K TCP",
            PortMappingProtocol::TCP,
            39735,
            our_endpoint,
            "Emule-Sion",
        ),
        Some(MappingOwnership::StaleLease)
    );

    // 4. Foreign conflict: same port/protocol, but different description (another software / manually created rule)
    assert_eq!(
        inspect_mapping_entry(
            PortMappingProtocol::TCP,
            39735,
            "192.168.1.31",
            39735,
            "Web Server / Torrent",
            PortMappingProtocol::TCP,
            39735,
            our_endpoint,
            "Emule-Sion",
        ),
        Some(MappingOwnership::ForeignConflict)
    );

    // 5. Foreign conflict: same port/protocol, empty description
    assert_eq!(
        inspect_mapping_entry(
            PortMappingProtocol::TCP,
            39735,
            "192.168.1.100",
            39735,
            "",
            PortMappingProtocol::TCP,
            39735,
            our_endpoint,
            "Emule-Sion",
        ),
        Some(MappingOwnership::ForeignConflict)
    );

    // 6. Protocol mismatch: entry is UDP while checking TCP -> ignored (None)
    assert_eq!(
        inspect_mapping_entry(
            PortMappingProtocol::UDP,
            39735,
            "192.168.1.31",
            39735,
            "Emule-Sion Kad UDP",
            PortMappingProtocol::TCP,
            39735,
            our_endpoint,
            "Emule-Sion",
        ),
        None
    );

    // 7. External port mismatch -> ignored (None)
    assert_eq!(
        inspect_mapping_entry(
            PortMappingProtocol::TCP,
            8080,
            "192.168.1.31",
            8080,
            "Emule-Sion eD2K TCP",
            PortMappingProtocol::TCP,
            39735,
            our_endpoint,
            "Emule-Sion",
        ),
        None
    );
}
