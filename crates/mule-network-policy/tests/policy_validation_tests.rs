use std::net::SocketAddr;
use std::time::Duration;
use mule_network_policy::{
    redact_socket_addr, AuditDecision, AuditEventType, ConnectionPurpose,
    NetworkPolicyManager, NetworkProfile, NetworkState, PolicyError, PolicyViolation,
    ResourceBudgets,
};

#[test]
fn test_local_lab_profile_confinement() {
    let manager = NetworkPolicyManager::new(
        NetworkProfile::LocalLab,
        ResourceBudgets::default(),
    );

    // Loopback IPv4 and IPv6 allowed
    let loopback_v4: SocketAddr = "127.0.0.1:4662".parse().unwrap();
    let permit = manager.request_tcp_connect(loopback_v4, ConnectionPurpose::Ed2kPeer);
    assert!(permit.is_ok());

    let loopback_v6: SocketAddr = "[::1]:4662".parse().unwrap();
    let permit_v6 = manager.request_tcp_connect(loopback_v6, ConnectionPurpose::Ed2kPeer);
    assert!(permit_v6.is_ok());

    // Non-loopback public rejected
    let public_ip: SocketAddr = "91.121.162.140:4662".parse().unwrap();
    let err = manager.request_tcp_connect(public_ip, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::NonLoopbackDisallowedInLab(_))));

    // Non-loopback LAN rejected
    let lan_ip: SocketAddr = "192.168.1.1:4662".parse().unwrap();
    let err = manager.request_tcp_connect(lan_ip, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::NonLoopbackDisallowedInLab(_))));
}

#[test]
fn test_public_preview_anti_ssrf_and_filtering() {
    let manager = NetworkPolicyManager::new(
        NetworkProfile::PublicEd2kPreview,
        ResourceBudgets::default(),
    );

    // Public routable IP allowed
    let public_ip: SocketAddr = "91.121.162.140:4662".parse().unwrap();
    assert!(manager.request_tcp_connect(public_ip, ConnectionPurpose::Ed2kPeer).is_ok());

    // Private RFC 1918 rejected
    let lan_10: SocketAddr = "10.0.0.1:4662".parse().unwrap();
    let err = manager.request_tcp_connect(lan_10, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::PrivateLanDisallowed(_))));

    let lan_172: SocketAddr = "172.20.0.1:4662".parse().unwrap();
    let err = manager.request_tcp_connect(lan_172, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::PrivateLanDisallowed(_))));

    let lan_192: SocketAddr = "192.168.1.100:4662".parse().unwrap();
    let err = manager.request_tcp_connect(lan_192, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::PrivateLanDisallowed(_))));

    // IPv4-mapped IPv6 private LAN rejected
    let mapped_lan: SocketAddr = "[::ffff:192.168.1.1]:4662".parse().unwrap();
    let err = manager.request_tcp_connect(mapped_lan, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::PrivateLanDisallowed(_))));

    // Loopback in public profile rejected
    let loopback: SocketAddr = "127.0.0.1:4662".parse().unwrap();
    let err = manager.request_tcp_connect(loopback, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::LoopbackDisallowedInPublicProfile(_))));

    // Link-local rejected
    let link_local: SocketAddr = "169.254.10.20:4662".parse().unwrap();
    let err = manager.request_tcp_connect(link_local, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::LinkLocalDisallowed(_))));

    // Multicast rejected
    let multicast: SocketAddr = "224.0.0.1:4662".parse().unwrap();
    let err = manager.request_tcp_connect(multicast, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::MulticastDisallowed(_))));

    // Broadcast rejected
    let broadcast: SocketAddr = "255.255.255.255:4662".parse().unwrap();
    let err = manager.request_tcp_connect(broadcast, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::BroadcastDisallowed(_))));

    // Carrier-Grade NAT rejected
    let cgnat: SocketAddr = "100.64.1.1:4662".parse().unwrap();
    let err = manager.request_tcp_connect(cgnat, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::CarrierGradeNatDisallowed(_))));

    // Port 0 rejected
    let port_zero: SocketAddr = "91.121.162.140:0".parse().unwrap();
    let err = manager.request_tcp_connect(port_zero, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::InvalidPort(_))));
}

#[test]
fn test_kad_udp_profile_enforcement() {
    let public_ip: SocketAddr = "91.121.162.140:4662".parse().unwrap();

    // In PublicEd2kPreview: KadNode is disallowed
    let manager = NetworkPolicyManager::new(
        NetworkProfile::PublicEd2kPreview,
        ResourceBudgets::default(),
    );
    let err = manager.request_tcp_connect(public_ip, ConnectionPurpose::KadNode).unwrap_err();
    assert!(matches!(err, PolicyError::KadNotAllowedInProfile(_)));

    // In PublicEd2kKadPreview: KadNode is allowed
    manager.set_profile(NetworkProfile::PublicEd2kKadPreview);
    assert_eq!(manager.profile(), NetworkProfile::PublicEd2kKadPreview);
    let permit = manager.request_tcp_connect(public_ip, ConnectionPurpose::KadNode);
    assert!(permit.is_ok());
}

#[test]
fn test_resource_budgets_exhaustion_and_drop() {
    let mut budgets = ResourceBudgets::default();
    budgets.max_active_peer_connections = 2;
    budgets.max_active_server_connections = 1;

    let manager = NetworkPolicyManager::new(NetworkProfile::LocalLab, budgets);
    let target1: SocketAddr = "127.0.0.1:4662".parse().unwrap();
    let target2: SocketAddr = "127.0.0.1:4663".parse().unwrap();
    let target3: SocketAddr = "127.0.0.1:4664".parse().unwrap();

    // Acquire 2 peer permits
    let permit1 = manager.request_tcp_connect(target1, ConnectionPurpose::Ed2kPeer).unwrap();
    let permit2 = manager.request_tcp_connect(target2, ConnectionPurpose::Ed2kPeer).unwrap();
    assert_eq!(manager.active_peer_connections_count(), 2);
    assert_eq!(manager.state(), NetworkState::Connected);

    // 3rd attempt exceeds budget
    let err = manager.request_tcp_connect(target3, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert_eq!(err, PolicyError::BudgetExhausted("max_active_peer_connections"));

    // Server connection budget is separate
    let server_permit = manager.request_tcp_connect(target1, ConnectionPurpose::Ed2kServer).unwrap();
    assert_eq!(manager.active_server_connections_count(), 1);

    // 2nd server attempt exceeds budget
    let err_server = manager.request_tcp_connect(target2, ConnectionPurpose::Ed2kServer).unwrap_err();
    assert_eq!(err_server, PolicyError::BudgetExhausted("max_active_server_connections"));

    // Drop permit1 -> allows new peer connection
    drop(permit1);
    assert_eq!(manager.active_peer_connections_count(), 1);

    let permit3 = manager.request_tcp_connect(target3, ConnectionPurpose::Ed2kPeer);
    assert!(permit3.is_ok());

    // Clean up
    drop(permit2);
    drop(permit3);
    drop(server_permit);
    assert_eq!(manager.active_peer_connections_count(), 0);
    assert_eq!(manager.active_server_connections_count(), 0);
    assert_eq!(manager.state(), NetworkState::PreviewActive);
}

#[tokio::test]
async fn test_kill_switch_atomic_cancellation_and_reenable() {
    let manager = NetworkPolicyManager::new(
        NetworkProfile::LocalLab,
        ResourceBudgets::default(),
    );

    let target: SocketAddr = "127.0.0.1:4662".parse().unwrap();
    let permit = manager.request_tcp_connect(target, ConnectionPurpose::Ed2kPeer).unwrap();
    assert!(!permit.is_cancelled());

    // Spawn a task waiting on permit cancellation
    let cancel_token = permit.cancellation_token();
    let task = tokio::spawn(async move {
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(5)) => false,
            _ = cancel_token.cancelled() => true,
        }
    });

    // Invoke Kill Switch
    manager.disconnect_all();
    assert_eq!(manager.state(), NetworkState::Disabled);
    assert!(permit.is_cancelled());

    let was_cancelled = task.await.unwrap();
    assert!(was_cancelled, "Active socket task should be woken up by cancellation token");

    // Any new request must fail immediately with NetworkDisabled
    let err = manager.request_tcp_connect(target, ConnectionPurpose::Ed2kPeer).unwrap_err();
    assert_eq!(err, PolicyError::NetworkDisabled);

    // Dropping permit simulates the socket task exiting and releasing its permit
    drop(permit);
    assert_eq!(manager.active_peer_connections_count(), 0);

    // Re-enable network
    manager.enable();
    assert_eq!(manager.state(), NetworkState::PreviewActive);

    // New request succeeds
    let new_permit = manager.request_tcp_connect(target, ConnectionPurpose::Ed2kPeer);
    assert!(new_permit.is_ok());
}

#[test]
fn test_redacted_audit_log() {
    let manager = NetworkPolicyManager::new(
        NetworkProfile::PublicEd2kPreview,
        ResourceBudgets::default(),
    );

    let public_ip: SocketAddr = "91.121.162.140:4662".parse().unwrap();
    let private_ip: SocketAddr = "192.168.1.50:4662".parse().unwrap();

    let _ = manager.request_tcp_connect(public_ip, ConnectionPurpose::Ed2kPeer);
    let _ = manager.request_tcp_connect(private_ip, ConnectionPurpose::Ed2kPeer);

    let entries = manager.audit_entries(10);
    assert!(entries.len() >= 2);

    // Check redaction
    assert_eq!(redact_socket_addr(&public_ip), "91.121.*.*:4662");
    assert_eq!(redact_socket_addr(&private_ip), "192.168.*.*:4662");

    let loopback: SocketAddr = "127.0.0.1:4662".parse().unwrap();
    assert_eq!(redact_socket_addr(&loopback), "127.0.0.1:4662");

    // Verify recent log entry fields
    let last = &entries[0]; // most recent
    assert_eq!(last.decision, AuditDecision::Denied);
    assert_eq!(last.event_type, AuditEventType::ConnectAttempt);
    assert_eq!(last.redacted_target, Some("192.168.*.*:4662".to_string()));
}

#[test]
fn test_listener_policy_and_budget() {
    let mut budgets = ResourceBudgets::default();
    budgets.max_active_listeners = 1;

    let manager = NetworkPolicyManager::new(NetworkProfile::LocalLab, budgets);

    // Loopback bind allowed
    let loopback_bind: SocketAddr = "127.0.0.1:4662".parse().unwrap();
    let permit = manager.request_tcp_listen(loopback_bind, ConnectionPurpose::Listener);
    assert!(permit.is_ok());
    let permit = permit.unwrap();
    assert_eq!(manager.active_listeners_count(), 1);

    // Exceeding listener budget
    let loopback_bind2: SocketAddr = "127.0.0.1:4663".parse().unwrap();
    let err = manager.request_tcp_listen(loopback_bind2, ConnectionPurpose::Listener).unwrap_err();
    assert_eq!(err, PolicyError::BudgetExhausted("max_active_listeners"));

    // Drop permit releases budget
    drop(permit);
    assert_eq!(manager.active_listeners_count(), 0);

    // Non-loopback bind rejected in Lab profile
    let lan_bind: SocketAddr = "192.168.1.1:4662".parse().unwrap();
    let err = manager.request_tcp_listen(lan_bind, ConnectionPurpose::Listener).unwrap_err();
    assert!(matches!(err, PolicyError::AddressViolation(PolicyViolation::ListenOnNonLoopbackDisallowed(_))));
}

#[test]
fn test_budget_mutation_and_validation() {
    let manager = NetworkPolicyManager::new(
        NetworkProfile::LocalLab,
        ResourceBudgets::default(),
    );

    // Valid budget update
    let mut new_budgets = ResourceBudgets::default();
    new_budgets.max_active_peer_connections = 50;
    assert!(manager.set_budgets(new_budgets.clone()).is_ok());
    assert_eq!(manager.budgets().max_active_peer_connections, 50);

    // Invalid budget update (zero max conns)
    let mut invalid_budgets = ResourceBudgets::default();
    invalid_budgets.max_active_peer_connections = 0;
    let err = manager.set_budgets(invalid_budgets).unwrap_err();
    assert!(matches!(err, PolicyError::InvalidBudgets(_)));

    // Invalid budget update (max_total_buffer_bytes < max_frame_size_bytes)
    let mut invalid_buf = ResourceBudgets::default();
    invalid_buf.max_total_buffer_bytes = 100;
    invalid_buf.max_frame_size_bytes = 1000;
    let err = manager.set_budgets(invalid_buf).unwrap_err();
    assert!(matches!(err, PolicyError::InvalidBudgets(_)));
}

#[test]
fn test_ipv6_redaction_and_profile_parsing() {
    let ipv6_pub: SocketAddr = "[2001:db8:85a3::8a2e:370:7334]:4662".parse().unwrap();
    let redacted = redact_socket_addr(&ipv6_pub);
    assert_eq!(redacted, "[2001:db8:*:*:*:*:*:*]:4662");

    let ipv6_loopback: SocketAddr = "[::1]:4662".parse().unwrap();
    assert_eq!(redact_socket_addr(&ipv6_loopback), "[::1]:4662");

    // Profile from_str and display
    assert_eq!("local-lab".parse::<NetworkProfile>().unwrap(), NetworkProfile::LocalLab);
    assert_eq!("public-ed2k-preview".parse::<NetworkProfile>().unwrap(), NetworkProfile::PublicEd2kPreview);
    assert_eq!("public-ed2k-kad-preview".parse::<NetworkProfile>().unwrap(), NetworkProfile::PublicEd2kKadPreview);
    assert!("invalid-profile".parse::<NetworkProfile>().is_err());
    assert_eq!(NetworkProfile::LocalLab.to_string(), "local-lab");
}

