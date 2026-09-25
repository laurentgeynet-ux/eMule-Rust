// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! eMule `OP_CHANGE_CLIENT_ID` (`ListenSocket.cpp:590-616`) — a peer that
//! changed eD2K server announces its new hybrid ID and new server IP:
//! `<u32 newUserID><u32 newServerIP>` (trailing bytes tolerated).
//!
//! eMule applies `SetUserIDHybrid` + `SetServerIP`/`SetServerPort` on the
//! client object when the payload is meaningful:
//! - `newUserID` LowID and the announced server exists in the local
//!   serverlist → adopt the LowID + server coordinates;
//! - `newUserID` equal to the peer's real IP (a HighID) → adopt it, and
//!   update the server coordinates when the server is known;
//! - anything else → ignored ("unknown contents").
//!
//! Our per-connection model has no hybrid-ID field: the observable part
//! is the source's server coordinates (`PeerSource.server_ip` /
//! `server_port`), which feed `IsLocalServer` / `has_server_callback_option`
//! for LowID callback decisions — exactly what eMule uses them for.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::services::downloads::DownloadService;

use super::log_addr_str;

/// eMule `IsLowID` — a client ID below this bound is a server-issued
/// LowID, not a routable IP (`opcodes.h` `HIGHEST_LOWID_ED2K + 1`).
const LOWID_BOUND: u32 = 16_777_216;

/// Applies one inbound `OP_CHANGE_CLIENT_ID` payload for the connection
/// at `remote_str` ("ip:port"), on behalf of every file hash the peer is
/// engaged with. No-op on malformed payloads and for the cases eMule
/// ignores (unknown server, unknown contents).
pub(crate) async fn apply_change_client_id(
    payload: &[u8],
    remote_str: &str,
    file_hashes: &[String],
    downloads: &Arc<Mutex<DownloadService>>,
) {
    if payload.len() < 8 {
        return;
    }
    let new_user_id = u32::from_le_bytes(payload[..4].try_into().unwrap_or_default());
    let new_server_ip = u32::from_le_bytes(payload[4..8].try_into().unwrap_or_default());

    let is_lowid = new_user_id != 0 && new_user_id < LOWID_BOUND;
    // HighID case: the announced ID equals the peer's real IP on the
    // wire — eMule compares `nNewUserID` (little-endian read) to
    // `client->GetIP()` stored in network order.
    let peer_ip_wire: Option<u32> =
        remote_str
            .parse::<SocketAddr>()
            .ok()
            .and_then(|a| match a.ip() {
                IpAddr::V4(v4) => Some(u32::from_be_bytes(v4.octets())),
                IpAddr::V6(_) => None,
            });
    let high_id_case = !is_lowid && peer_ip_wire == Some(new_user_id);
    if !is_lowid && !high_id_case {
        tracing::debug!(
            "Peer {} sent OP_CHANGE_CLIENT_ID with unknown contents (newUserID={new_user_id:#010x})",
            log_addr_str(remote_str),
        );
        return;
    }

    // eMule `serverlist->GetServerByIP(nNewServerIP)` — the update only
    // happens when the announced server is one we already know.
    let server_port = {
        let dl_svc = downloads.lock().await;
        dl_svc
            .list_servers()
            .unwrap_or_default()
            .into_iter()
            .find_map(|s| {
                let ip_u32 =
                    s.ip.parse::<std::net::Ipv4Addr>()
                        .ok()
                        .map(|v4| u32::from_le_bytes(v4.octets()))?;
                (ip_u32 == new_server_ip).then_some(s.port)
            })
    };
    let Some(server_port) = server_port else {
        return;
    };

    // Update the source rows this connection maps to: the `ip:port` row
    // always, plus the `lowid:<new_id>` row when the announced ID is a
    // LowID (the claim ties that lowid identity to its new server —
    // `IsLocalServer` can then match it for callback decisions).
    let mut dl_svc = downloads.lock().await;
    for file_hash in file_hashes {
        let _ = dl_svc.update_peer_source_server(file_hash, remote_str, new_server_ip, server_port);
        if is_lowid {
            let _ = dl_svc.update_peer_source_server(
                file_hash,
                &format!("lowid:{new_user_id}"),
                new_server_ip,
                server_port,
            );
        }
    }
    tracing::debug!(
        "Peer {} changed eD2K identity: newUserID={new_user_id:#010x} on known server {new_server_ip:#010x}:{server_port} ({} file scope(s) updated)",
        log_addr_str(remote_str),
        file_hashes.len(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use mule_core::model::{
        Ed2kServer, PeerSource, PeerSourceOrigin, PeerSourceStatus, ServerFilterStatus,
    };

    fn test_service(dir: &tempfile::TempDir) -> DownloadService {
        DownloadService::new(&dir.path().join("mule.db"), dir.path().join("tmp"))
            .expect("DownloadService")
    }

    fn server(ip: &str, port: u16) -> Ed2kServer {
        Ed2kServer {
            address: format!("{ip}:{port}"),
            ip: ip.to_string(),
            port,
            name: "test".to_string(),
            description: String::new(),
            ping: 0,
            preference: 0,
            failed_count: 0,
            dyn_ip: None,
            version: None,
            filter_status: ServerFilterStatus::Allowed,
            last_seen_unix_ms: None,
            created_at_unix_ms: 0,
            updated_at_unix_ms: 0,
        }
    }

    fn source(file_hash: &str, addr: &str) -> PeerSource {
        PeerSource {
            file_hash: file_hash.to_string(),
            address: addr.to_string(),
            ip: 0,
            port: 0,
            origin: PeerSourceOrigin::SourceExchange,
            discovered_at_unix_ms: 0,
            last_seen_unix_ms: 0,
            status: PeerSourceStatus::Candidate,
            fail_count: 0,
            last_error: None,
            cooldown_until_unix_ms: None,
            user_hash: None,
            crypt_options: 0,
            buddy_id: None,
            software: None,
            availability: None,
            server_ip: 0,
            server_port: 0,
            udp_port: 0,
        }
    }

    fn payload(new_id: u32, server_ip: u32) -> Vec<u8> {
        let mut p = Vec::with_capacity(8);
        p.extend_from_slice(&new_id.to_le_bytes());
        p.extend_from_slice(&server_ip.to_le_bytes());
        p
    }

    fn server_fields(svc: &DownloadService, file_hash: &str, addr: &str) -> (u32, u16) {
        svc.list_peer_sources(file_hash)
            .unwrap()
            .into_iter()
            .find(|s| s.address == addr)
            .map(|s| (s.server_ip, s.server_port))
            .unwrap_or((0, 0))
    }

    /// eMule `ListenSocket.cpp:598-605` : LowID + serveur connu → les
    /// coordonnées du nouveau serveur s'appliquent sur la ligne `lowid:<id>`
    /// (et la ligne `ip:port` de la connexion).
    #[tokio::test]
    async fn lowid_on_known_server_updates_source_server_fields() {
        let dir = tempfile::tempdir().unwrap();
        let svc = Arc::new(Mutex::new(test_service(&dir)));
        {
            let mut dl = svc.lock().await;
            dl.set_lowid_source_acceptance(true);
            // Serveur connu : 1.2.3.4:4661 (server_ip wire = LE octets).
            dl.upsert_servers(&[server("1.2.3.4", 4661)]).unwrap();
            let hash = "aa".repeat(16);
            dl.upsert_peer_sources(
                &hash,
                &[source(&hash, "lowid:42"), source(&hash, "9.9.9.9:4662")],
            )
            .unwrap();
        }
        let hash = "aa".repeat(16);

        // newUserID=42 (LowID), newServerIP=1.2.3.4 en u32 LE wire.
        let p = payload(42, u32::from_le_bytes([1, 2, 3, 4]));
        apply_change_client_id(&p, "9.9.9.9:4662", std::slice::from_ref(&hash), &svc).await;

        let dl = svc.lock().await;
        let wire_ip = u32::from_le_bytes([1, 2, 3, 4]);
        assert_eq!(server_fields(&dl, &hash, "lowid:42"), (wire_ip, 4661));
        assert_eq!(server_fields(&dl, &hash, "9.9.9.9:4662"), (wire_ip, 4661));
    }

    /// LowID annoncé sur un serveur INCONNU de la serverlist → eMule ne
    /// touche rien (`pNewServer == NULL` → pas de SetUserIDHybrid).
    #[tokio::test]
    async fn lowid_on_unknown_server_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let svc = Arc::new(Mutex::new(test_service(&dir)));
        {
            let mut dl = svc.lock().await;
            dl.set_lowid_source_acceptance(true);
            let hash = "bb".repeat(16);
            dl.upsert_peer_sources(&hash, &[source(&hash, "lowid:42")])
                .unwrap();
        }
        let hash = "bb".repeat(16);
        let p = payload(42, u32::from_le_bytes([8, 8, 8, 8]));
        apply_change_client_id(&p, "9.9.9.9:4662", std::slice::from_ref(&hash), &svc).await;
        let dl = svc.lock().await;
        assert_eq!(server_fields(&dl, &hash, "lowid:42"), (0, 0));
    }

    /// HighID : `nNewUserID` == IP réelle du pair (wire/BE) + serveur
    /// connu → les coordonnées s'appliquent sur la ligne `ip:port`
    /// uniquement (aucune ligne `lowid:` n'existe pour un HighID).
    #[tokio::test]
    async fn highid_matching_peer_ip_updates_ip_row_only() {
        let dir = tempfile::tempdir().unwrap();
        let svc = Arc::new(Mutex::new(test_service(&dir)));
        {
            let mut dl = svc.lock().await;
            dl.upsert_servers(&[server("1.2.3.4", 4661)]).unwrap();
            let hash = "cc".repeat(16);
            dl.upsert_peer_sources(&hash, &[source(&hash, "9.9.9.9:4662")])
                .unwrap();
        }
        let hash = "cc".repeat(16);
        // newUserID = IP du pair 9.9.9.9 en ordre wire (BE).
        let p = payload(
            u32::from_be_bytes([9, 9, 9, 9]),
            u32::from_le_bytes([1, 2, 3, 4]),
        );
        apply_change_client_id(&p, "9.9.9.9:4662", std::slice::from_ref(&hash), &svc).await;
        let dl = svc.lock().await;
        assert_eq!(
            server_fields(&dl, &hash, "9.9.9.9:4662"),
            (u32::from_le_bytes([1, 2, 3, 4]), 4661)
        );
    }

    /// Contenu inconnu (HighID qui ne correspond pas à l'IP du pair) et
    /// payload tronqué → ignorés comme eMule.
    #[tokio::test]
    async fn unknown_contents_and_short_payload_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let svc = Arc::new(Mutex::new(test_service(&dir)));
        {
            let mut dl = svc.lock().await;
            dl.upsert_servers(&[server("1.2.3.4", 4661)]).unwrap();
            let hash = "dd".repeat(16);
            dl.upsert_peer_sources(&hash, &[source(&hash, "9.9.9.9:4662")])
                .unwrap();
        }
        let hash = "dd".repeat(16);
        // HighID ≠ IP du pair → "unknown contents" eMule.
        let p = payload(
            u32::from_be_bytes([7, 7, 7, 7]),
            u32::from_le_bytes([1, 2, 3, 4]),
        );
        apply_change_client_id(&p, "9.9.9.9:4662", std::slice::from_ref(&hash), &svc).await;
        // Payload tronqué → ignoré.
        apply_change_client_id(
            &[1, 2, 3],
            "9.9.9.9:4662",
            std::slice::from_ref(&hash),
            &svc,
        )
        .await;
        let dl = svc.lock().await;
        assert_eq!(server_fields(&dl, &hash, "9.9.9.9:4662"), (0, 0));
    }
}
