// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

use uuid::Uuid;

use crate::error::CoreError;
use crate::kad::{KadContact, KadIndexedSource};
use crate::model::{Download, DownloadStatus, Ed2kServer, PeerSource, PeerSourceStatus};
use crate::ranges::RangeSet;

/// Abstraction trait for download persistence operations.
pub trait DownloadRepository {
    fn insert_download(&mut self, download: &Download) -> Result<(), CoreError>;
    fn get_download(&self, id: &Uuid) -> Result<Option<Download>, CoreError>;
    fn get_all_downloads(&self) -> Result<Vec<Download>, CoreError>;
    fn update_status(&mut self, id: &Uuid, status: DownloadStatus) -> Result<(), CoreError>;
    fn delete_download(&mut self, id: &Uuid) -> Result<bool, CoreError>;
    fn save_received_ranges(&mut self, id: &Uuid, ranges: &RangeSet) -> Result<(), CoreError>;
    fn load_received_ranges(&self, id: &Uuid) -> Result<RangeSet, CoreError>;
}

/// Abstraction trait for peer sources persistence operations.
pub trait PeerSourceRepository {
    fn upsert_peer_sources(
        &mut self,
        file_hash_hex: &str,
        sources: &[PeerSource],
    ) -> Result<usize, CoreError>;
    fn list_peer_sources(&self, file_hash_hex: &str) -> Result<Vec<PeerSource>, CoreError>;
    fn update_peer_source_status(
        &mut self,
        file_hash_hex: &str,
        address: &str,
        status: PeerSourceStatus,
        error: Option<&str>,
        cooldown_until_unix_ms: Option<i64>,
    ) -> Result<bool, CoreError>;
    fn delete_peer_source(&mut self, file_hash_hex: &str, address: &str)
        -> Result<bool, CoreError>;
}

/// Abstraction trait for server list persistence.
pub trait ServerRepository {
    fn list_servers(&self) -> Result<Vec<Ed2kServer>, CoreError>;
    fn upsert_servers(&mut self, servers: &[Ed2kServer]) -> Result<usize, CoreError>;
    fn delete_server(&mut self, address: &str) -> Result<bool, CoreError>;
    fn clear_servers(&mut self) -> Result<usize, CoreError>;
}

/// Abstraction trait for Kademlia routing and indexing storage.
pub trait KadRepository {
    fn upsert_kad_contact(&mut self, contact: &KadContact) -> Result<(), CoreError>;
    fn list_kad_contacts(&self, limit: usize) -> Result<Vec<KadContact>, CoreError>;
    fn insert_kad_sources(&mut self, sources: &[KadIndexedSource]) -> Result<usize, CoreError>;
    fn get_kad_sources(
        &self,
        target_kad_id: &[u8; 16],
        limit: usize,
        now_ms: u64,
    ) -> Result<Vec<KadIndexedSource>, CoreError>;
}
