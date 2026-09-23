// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

use super::types::ServerSessionSnapshot;
use crate::services::DaemonEvent;
use mule_protocol::ed2k::{FoundSources, FoundSourcesObfu, SearchResult};
use serde_json::json;

pub fn emit_event(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    snapshot: &ServerSessionSnapshot,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    DaemonEvent::ServerSessionUpdated {
        sequence: seq,
        session: snapshot.clone(),
    }
    .send(event_tx);
}

pub fn emit_search_results(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    sr: &SearchResult,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let files: Vec<serde_json::Value> = sr
        .files
        .iter()
        .map(|f| {
            json!({
                "fileHash": hex::encode(f.file_hash),
                "fileName": f.name().unwrap_or_else(|| "Unknown".to_string()),
                "fileSize": f.size().unwrap_or(0),
                "sourceCount": f.source_count().unwrap_or(1),
            })
        })
        .collect();

    DaemonEvent::Custom {
        method: "server.searchResult".to_string(),
        payload: json!({
            "sequence": seq,
            "files": files,
        }),
    }
    .send(event_tx);
}

pub fn emit_found_sources(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    fs: &FoundSources,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    emit_found_sources_payload(
        event_tx,
        seq,
        &fs.file_hash,
        fs.sources.iter().map(source_to_json).collect(),
    );
}

pub fn emit_found_sources_obfu(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    fs: &FoundSourcesObfu,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    emit_found_sources_payload(
        event_tx,
        seq,
        &fs.file_hash,
        fs.sources.iter().map(source_to_json).collect(),
    );
}

fn source_to_json<S: FoundSourceLike>(s: &S) -> serde_json::Value {
    json!({
        "address": s.to_address_string(),
        "ip": s.ip_u32(),
        "port": s.port_u16(),
    })
}

/// Common surface of `FoundSourcePeer` / `FoundSourcePeerObfu` for the
/// `server.foundSources` payload.
trait FoundSourceLike {
    fn to_address_string(&self) -> String;
    fn ip_u32(&self) -> u32;
    fn port_u16(&self) -> u16;
}

impl FoundSourceLike for mule_protocol::ed2k::FoundSourcePeer {
    fn to_address_string(&self) -> String {
        self.to_address_string()
    }
    fn ip_u32(&self) -> u32 {
        self.ip
    }
    fn port_u16(&self) -> u16 {
        self.port
    }
}

impl FoundSourceLike for mule_protocol::ed2k::FoundSourcePeerObfu {
    fn to_address_string(&self) -> String {
        self.to_address_string()
    }
    fn ip_u32(&self) -> u32 {
        self.ip
    }
    fn port_u16(&self) -> u16 {
        self.port
    }
}

fn emit_found_sources_payload(
    event_tx: &broadcast::Sender<String>,
    seq: u64,
    file_hash: &[u8; 16],
    sources: Vec<serde_json::Value>,
) {
    DaemonEvent::Custom {
        method: "server.foundSources".to_string(),
        payload: json!({
            "sequence": seq,
            "fileHash": hex::encode(file_hash),
            "sources": sources,
        }),
    }
    .send(event_tx);
}
