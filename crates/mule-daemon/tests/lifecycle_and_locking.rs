use mule_daemon::{DaemonError, DaemonSession, DirectoryLock};
use std::net::TcpListener;
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn test_exclusive_directory_lock() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    // 1. First lock acquisition succeeds
    let lock1 = DirectoryLock::acquire(path).expect("First lock should succeed");
    assert!(lock1.path().exists());

    // 2. Second lock acquisition on same directory must fail with AlreadyRunning
    let lock2_res = DirectoryLock::acquire(path);
    match lock2_res {
        Err(DaemonError::AlreadyRunning(p)) => {
            assert_eq!(p, path);
        }
        other => panic!("Expected AlreadyRunning error, got {other:?}"),
    }

    // 3. Dropping the first lock allows acquiring a new one
    drop(lock1);

    let lock3 = DirectoryLock::acquire(path).expect("Subsequent lock after drop should succeed");
    drop(lock3);
}

#[test]
fn test_atomic_session_file_and_token() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    // Token management
    assert_eq!(DaemonSession::read_token(path), None);
    DaemonSession::write_token(path, "my_secure_token_456").unwrap();
    assert_eq!(
        DaemonSession::read_token(path),
        Some("my_secure_token_456".to_string())
    );

    // Session file management
    let instance_id = Uuid::new_v4();
    let session = DaemonSession::new(1, instance_id, "ws://127.0.0.1:4660/rpc");
    session
        .write_atomic(path)
        .expect("Atomic session write should succeed");

    let loaded = DaemonSession::read(path).expect("Reading session should succeed");
    assert_eq!(loaded.protocol_version, 1);
    assert_eq!(loaded.instance_id, instance_id);
    assert_eq!(loaded.endpoint, "ws://127.0.0.1:4660/rpc");
    assert_eq!(loaded.pid, std::process::id());
    assert!(loaded.started_at_unix_ms > 0);

    // Cleanup
    DaemonSession::remove(path);
    assert!(!path.join("session.json").exists());
}

#[test]
fn test_stale_or_corrupt_session_file_handled() {
    let dir = tempdir().unwrap();
    let path = dir.path();

    // Write junk into session.json
    std::fs::write(path.join("session.json"), b"{ invalid json ...").unwrap();

    let read_res = DaemonSession::read(path);
    assert!(read_res.is_err());

    // Overwriting atomically fixes it cleanly
    let session = DaemonSession::new(1, Uuid::new_v4(), "ws://127.0.0.1:5000/rpc");
    session
        .write_atomic(path)
        .expect("Overwriting corrupt file should succeed");

    let loaded = DaemonSession::read(path).expect("Reading recovered session should succeed");
    assert_eq!(loaded.endpoint, "ws://127.0.0.1:5000/rpc");
}

#[tokio::test]
async fn test_ephemeral_port_fallback_behavior() {
    // Occupy a specific port
    let occupier = TcpListener::bind("127.0.0.1:0").unwrap();
    let busy_port = occupier.local_addr().unwrap().port();

    // Attempting to bind another listener on busy_port fails with AddrInUse
    let bind_res = tokio::net::TcpListener::bind(format!("127.0.0.1:{busy_port}")).await;
    assert!(bind_res.is_err());
    assert_eq!(bind_res.unwrap_err().kind(), std::io::ErrorKind::AddrInUse);

    // Fallback on 127.0.0.1:0 succeeds
    let fallback = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    assert_ne!(fallback.local_addr().unwrap().port(), busy_port);
}
