use mule_core::model::{Ed2kServer, PeerCredit, PeerSource, PeerSourceStatus, SharedFile};
use mule_core::Database;
use std::collections::HashMap;
use uuid::Uuid;

use super::types::ActiveDownload;
use crate::error::DaemonError;

pub fn upsert_servers(db: &mut Database, servers: &[Ed2kServer]) -> Result<usize, DaemonError> {
    db.upsert_servers(servers)
        .map_err(|e| DaemonError::Business(format!("DB error upserting servers: {e}")))
}

pub fn list_servers(db: &Database) -> Result<Vec<Ed2kServer>, DaemonError> {
    db.list_servers()
        .map_err(|e| DaemonError::Business(format!("DB error listing servers: {e}")))
}

pub fn set_server_enabled(
    db: &mut Database,
    address: &str,
    enabled: bool,
) -> Result<bool, DaemonError> {
    db.set_server_enabled(address, enabled)
        .map_err(|e| DaemonError::Business(format!("DB error updating server status: {e}")))
}

pub fn delete_server(db: &mut Database, address: &str) -> Result<bool, DaemonError> {
    db.delete_server(address)
        .map_err(|e| DaemonError::Business(format!("DB error deleting server: {e}")))
}

pub fn clear_servers(db: &mut Database) -> Result<usize, DaemonError> {
    db.clear_servers()
        .map_err(|e| DaemonError::Business(format!("DB error clearing servers: {e}")))
}

pub fn upsert_peer_sources(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    file_hash: &str,
    sources: &[PeerSource],
) -> Result<usize, DaemonError> {
    let res = db
        .upsert_peer_sources(file_hash, sources)
        .map_err(|e| DaemonError::Business(format!("DB error upserting peer sources: {e}")))?;
    let count = db
        .list_peer_sources(file_hash)
        .map(|s| s.len() as u32)
        .unwrap_or(0);
    let clean_hex = file_hash.trim().to_lowercase();
    for a in active.values_mut() {
        if a.download.ed2k_hash.to_hex().to_lowercase() == clean_hex {
            a.source_count = count;
        }
    }
    Ok(res)
}

pub fn list_peer_sources(db: &Database, file_hash: &str) -> Result<Vec<PeerSource>, DaemonError> {
    db.list_peer_sources(file_hash)
        .map_err(|e| DaemonError::Business(format!("DB error listing peer sources: {e}")))
}

pub fn update_peer_source_status(
    db: &mut Database,
    file_hash: &str,
    address: &str,
    status: PeerSourceStatus,
    error: Option<&str>,
    cooldown_until_unix_ms: Option<i64>,
) -> Result<bool, DaemonError> {
    db.update_peer_source_status(file_hash, address, status, error, cooldown_until_unix_ms)
        .map_err(|e| DaemonError::Business(format!("DB error updating peer source status: {e}")))
}

pub fn reset_peer_source_cooldown(
    db: &mut Database,
    file_hash: &str,
    address: &str,
) -> Result<bool, DaemonError> {
    db.reset_peer_source_cooldown(file_hash, address)
        .map_err(|e| DaemonError::Business(format!("DB error resetting peer source cooldown: {e}")))
}

pub fn delete_peer_source(
    db: &mut Database,
    file_hash: &str,
    address: &str,
) -> Result<bool, DaemonError> {
    db.delete_peer_source(file_hash, address)
        .map_err(|e| DaemonError::Business(format!("DB error deleting peer source: {e}")))
}

pub fn clear_peer_sources(db: &mut Database, file_hash: &str) -> Result<usize, DaemonError> {
    db.clear_peer_sources(file_hash)
        .map_err(|e| DaemonError::Business(format!("DB error clearing peer sources: {e}")))
}

pub fn list_shared_files(db: &Database) -> Result<Vec<SharedFile>, DaemonError> {
    db.list_shared_files()
        .map_err(|e| DaemonError::Business(format!("DB error listing shared files: {e}")))
}

pub fn add_shared_file(db: &mut Database, file: &SharedFile) -> Result<(), DaemonError> {
    db.upsert_shared_file(file)
        .map_err(|e| DaemonError::Business(format!("DB error adding shared file: {e}")))
}

pub fn remove_shared_file(db: &mut Database, file_hash: &str) -> Result<bool, DaemonError> {
    db.delete_shared_file(file_hash)
        .map_err(|e| DaemonError::Business(format!("DB error deleting shared file: {e}")))
}

pub fn get_shared_file(db: &Database, file_hash: &str) -> Result<Option<SharedFile>, DaemonError> {
    db.get_shared_file(file_hash)
        .map_err(|e| DaemonError::Business(format!("DB error getting shared file: {e}")))
}

pub fn get_peer_credit(
    db: &Database,
    pubkey_hash: &str,
) -> Result<Option<PeerCredit>, DaemonError> {
    db.get_peer_credit(pubkey_hash)
        .map_err(|e| DaemonError::Business(format!("DB error getting peer credit: {e}")))
}

pub fn record_credit_upload(
    db: &mut Database,
    pubkey_hash: &str,
    pubkey_hex: &str,
    bytes: u64,
) -> Result<(), DaemonError> {
    db.record_credit_upload(pubkey_hash, pubkey_hex, bytes)
        .map_err(|e| DaemonError::Business(format!("DB error recording credit upload: {e}")))
}

pub fn record_credit_download(
    db: &mut Database,
    pubkey_hash: &str,
    pubkey_hex: &str,
    bytes: u64,
) -> Result<(), DaemonError> {
    db.record_credit_download(pubkey_hash, pubkey_hex, bytes)
        .map_err(|e| DaemonError::Business(format!("DB error recording credit download: {e}")))
}

pub fn list_peer_credits(db: &Database, limit: usize) -> Result<Vec<PeerCredit>, DaemonError> {
    db.list_peer_credits(limit)
        .map_err(|e| DaemonError::Business(format!("DB error listing peer credits: {e}")))
}
