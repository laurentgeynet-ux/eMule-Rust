use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

use mule_protocol::ed2k::{FoundSources, FoundSourcesObfu, SearchResult};
use serde_json::json;
use super::types::ServerSessionSnapshot;

pub fn emit_event(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    snapshot: &ServerSessionSnapshot,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let notif = json!({
        "jsonrpc": "2.0",
        "method": "server.sessionUpdated",
        "params": {
            "sequence": seq,
            "session": snapshot,
        }
    });
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
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

    let notif = json!({
        "jsonrpc": "2.0",
        "method": "server.searchResult",
        "params": {
            "sequence": seq,
            "files": files,
        }
    });
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}

pub fn emit_found_sources(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    fs: &FoundSources,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let sources: Vec<serde_json::Value> = fs
        .sources
        .iter()
        .map(|s| {
            json!({
                "address": s.to_address_string(),
                "ip": s.ip,
                "port": s.port,
            })
        })
        .collect();

    let notif = json!({
        "jsonrpc": "2.0",
        "method": "server.foundSources",
        "params": {
            "sequence": seq,
            "fileHash": hex::encode(fs.file_hash),
            "sources": sources,
        }
    });
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}

pub fn emit_found_sources_obfu(
    event_tx: &broadcast::Sender<String>,
    sequence: &Arc<AtomicU64>,
    fs: &FoundSourcesObfu,
) {
    let seq = sequence.fetch_add(1, Ordering::SeqCst);
    let sources: Vec<serde_json::Value> = fs
        .sources
        .iter()
        .map(|s| {
            json!({
                "address": s.to_address_string(),
                "ip": s.ip,
                "port": s.port,
            })
        })
        .collect();

    let notif = json!({
        "jsonrpc": "2.0",
        "method": "server.foundSources",
        "params": {
            "sequence": seq,
            "fileHash": hex::encode(fs.file_hash),
            "sources": sources,
        }
    });
    if let Ok(json) = serde_json::to_string(&notif) {
        let _ = event_tx.send(json);
    }
}
