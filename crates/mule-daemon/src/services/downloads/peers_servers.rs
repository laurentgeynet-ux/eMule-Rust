// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use mule_core::model::{Ed2kServer, PeerCredit, PeerSource, PeerSourceStatus, SharedFile};
use mule_core::Database;
use std::collections::HashMap;
use uuid::Uuid;

use super::{db_err, types::ActiveDownload};
use crate::error::DaemonError;

pub fn upsert_servers(db: &mut Database, servers: &[Ed2kServer]) -> Result<usize, DaemonError> {
    db.upsert_servers(servers)
        .map_err(db_err("upserting servers"))
}

pub fn list_servers(db: &Database) -> Result<Vec<Ed2kServer>, DaemonError> {
    db.list_servers()
        .map_err(db_err("listing servers"))
}

pub fn set_server_enabled(
    db: &mut Database,
    address: &str,
    enabled: bool,
) -> Result<bool, DaemonError> {
    db.set_server_enabled(address, enabled)
        .map_err(db_err("updating server status"))
}

pub fn delete_server(db: &mut Database, address: &str) -> Result<bool, DaemonError> {
    db.delete_server(address)
        .map_err(db_err("deleting server"))
}

pub fn clear_servers(db: &mut Database) -> Result<usize, DaemonError> {
    db.clear_servers()
        .map_err(db_err("clearing servers"))
}

pub fn upsert_peer_sources(
    db: &mut Database,
    active: &mut HashMap<Uuid, ActiveDownload>,
    file_hash: &str,
    sources: &[PeerSource],
) -> Result<usize, DaemonError> {
    let res = db
        .upsert_peer_sources(file_hash, sources)
        .map_err(db_err("upserting peer sources"))?;
    let count = db
        .list_peer_sources(file_hash)
        .map(|s| s.iter().filter(|x| x.status.is_srclist_visible()).count() as u32)
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
        .map_err(db_err("listing peer sources"))
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
        .map_err(db_err("updating peer source status"))
}

pub fn update_peer_source_details(
    db: &mut Database,
    file_hash: &str,
    address: &str,
    software: Option<&str>,
    availability: Option<&[u8]>,
) -> Result<bool, DaemonError> {
    db.update_peer_source_details(file_hash, address, software, availability)
        .map_err(db_err("updating peer source details"))
}

pub fn update_peer_source_server(
    db: &mut Database,
    file_hash: &str,
    address: &str,
    server_ip: u32,
    server_port: u16,
) -> Result<bool, DaemonError> {
    db.update_peer_source_server(file_hash, address, server_ip, server_port)
        .map_err(db_err("updating peer source server"))
}

pub fn reset_peer_source_cooldown(
    db: &mut Database,
    file_hash: &str,
    address: &str,
) -> Result<bool, DaemonError> {
    db.reset_peer_source_cooldown(file_hash, address)
        .map_err(db_err("resetting peer source cooldown"))
}

pub fn delete_peer_source(
    db: &mut Database,
    file_hash: &str,
    address: &str,
) -> Result<bool, DaemonError> {
    db.delete_peer_source(file_hash, address)
        .map_err(db_err("deleting peer source"))
}

pub fn clear_peer_sources(db: &mut Database, file_hash: &str) -> Result<usize, DaemonError> {
    db.clear_peer_sources(file_hash)
        .map_err(db_err("clearing peer sources"))
}

pub fn list_shared_files(db: &Database) -> Result<Vec<SharedFile>, DaemonError> {
    db.list_shared_files()
        .map_err(db_err("listing shared files"))
}

pub fn add_shared_file(db: &mut Database, file: &SharedFile) -> Result<(), DaemonError> {
    db.upsert_shared_file(file)
        .map_err(db_err("adding shared file"))
}

/// eMule `UAPPref` (`GetNewAutoUp`, défaut `true`) : les nouveaux fichiers
/// partagés démarrent en priorité upload « auto » — `UpdateAutoUpPriority`
/// recalcule l'effectif selon le nombre de clients en upload.
pub fn auto_up_priority_enabled(db: &Database) -> bool {
    db.get_setting("auto_priority_uploads")
        .ok()
        .flatten()
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "on"))
        .unwrap_or(true)
}

pub fn remove_shared_file(db: &mut Database, file_hash: &str) -> Result<bool, DaemonError> {
    db.delete_shared_file(file_hash)
        .map_err(db_err("deleting shared file"))
}

pub fn get_shared_file(db: &Database, file_hash: &str) -> Result<Option<SharedFile>, DaemonError> {
    db.get_shared_file(file_hash)
        .map_err(db_err("getting shared file"))
}

/// Ajoute `delta` octets au total uploadé persisté du fichier partagé.
pub fn add_share_uploaded_bytes(
    db: &mut Database,
    file_hash: &str,
    delta: u64,
) -> Result<(), DaemonError> {
    db.add_share_uploaded_bytes(file_hash, delta)
        .map_err(db_err("adding share uploaded bytes"))
}

/// Totaux uploadés persistés par hash de fichier (clés en minuscules).
pub fn list_share_uploaded_bytes(
    db: &Database,
) -> Result<std::collections::HashMap<String, u64>, DaemonError> {
    db.list_share_uploaded_bytes()
        .map_err(db_err("listing share upload stats"))
}

pub fn get_peer_credit(
    db: &Database,
    pubkey_hash: &str,
) -> Result<Option<PeerCredit>, DaemonError> {
    db.get_peer_credit(pubkey_hash)
        .map_err(db_err("getting peer credit"))
}

pub fn record_credit_upload(
    db: &mut Database,
    pubkey_hash: &str,
    pubkey_hex: &str,
    bytes: u64,
) -> Result<(), DaemonError> {
    db.record_credit_upload(pubkey_hash, pubkey_hex, bytes)
        .map_err(db_err("recording credit upload"))
}

pub fn record_credit_download(
    db: &mut Database,
    pubkey_hash: &str,
    pubkey_hex: &str,
    bytes: u64,
) -> Result<(), DaemonError> {
    db.record_credit_download(pubkey_hash, pubkey_hex, bytes)
        .map_err(db_err("recording credit download"))
}

pub fn list_peer_credits(db: &Database, limit: usize) -> Result<Vec<PeerCredit>, DaemonError> {
    db.list_peer_credits(limit)
        .map_err(db_err("listing peer credits"))
}
