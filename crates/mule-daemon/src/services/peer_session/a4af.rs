// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Socket-level A4AF — eMule `CUpDownClient::SwapToAnotherFile`/`DoSwap`
//! (`DownloadClient.cpp:1530`).
//!
//! When a session reports *No Needed Parts* or *File Not Found*, eMule does
//! not drop the TCP connection: `DoSwap` moves the client to another
//! `CPartFile`'s source list and the same socket re-issues
//! `OP_SETREQFILEID`/file-status for the new file. This module picks the
//! swap target and performs the bookkeeping around it; the session loop in
//! `connection.rs` reuses the live stream for the new request.

use std::sync::Arc;
use tokio::sync::Mutex;

use mule_core::model::{PeerFailureReason, PeerSourceStatus};

use crate::services::downloads::DownloadService;

/// Maximum consecutive swaps on one socket — eMule has no explicit bound
/// (re-ask timing prevents ping-pong); we cap to avoid an infinite chain of
/// NNP files for a pathological peer.
const MAX_A4AF_SWAPS: u32 = 4;

/// NNP re-ask cooldown — eMule `FILEREASKTIME * 2` = 58 min
/// (`DownloadClient.cpp:1888`, `PartFile.cpp:2276`).
const NNP_REASK_COOLDOWN_MS: i64 = 58 * 60 * 1000;

/// Maximum swaps per session (`MAX_A4AF_SWAPS`).
pub(crate) fn max_swaps() -> u32 {
    MAX_A4AF_SWAPS
}

/// Failure reasons that keep the socket alive and are eligible for a swap:
/// eMule calls `SwapToAnotherFile` on NNP (`ProcessFileStatus`,
/// `SendBlockRequests`) and FNF (`UDPReaskFNF`, `OP_FILEREQANSNOFILE` via
/// `ListenSocket.cpp:428`). Any other error means the connection is dead —
/// nothing to swap on.
pub(crate) fn is_swap_eligible(reason: &PeerFailureReason) -> bool {
    matches!(
        reason,
        PeerFailureReason::NoNeededParts | PeerFailureReason::FileNotFound
    )
}

/// Picks the file a session should swap to (eMule `SwapToAnotherFile`
/// candidate scan over `m_OtherRequests_list` then `m_OtherNoNeeded_list`,
/// with `SwapToRightFile` ordering reduced to: highest download priority,
/// then deterministic hash order).
///
/// A candidate file must be an active download (eMule `IsPartFile`,
/// `!IsStopped`, `PS_READY || PS_EMPTY`), must list `address` as a source
/// (eMule A4AFsrclist), and must not already exceed its source cap
/// (`cur_file->GetSourceCount() < GetMaxSources()`). Sources flagged
/// NNP/FNF *on the candidate file* are only chosen when no clean candidate
/// exists (eMule OtherNoNeeded tier).
pub(crate) async fn find_swap_target(
    downloads: &Arc<Mutex<DownloadService>>,
    address: &str,
    current_hash: &str,
    max_peers_per_file: usize,
    now_ms: i64,
) -> Option<(String, [u8; 16])> {
    struct Cand {
        hash: String,
        prio: u8,
        nnp: bool,
    }

    let mut cands: Vec<Cand> = {
        let dl = downloads.lock().await;
        let mut v = Vec::new();
        for d in dl.list_downloads() {
            if d.ed2k_hash.is_empty()
                || d.ed2k_hash.eq_ignore_ascii_case(current_hash)
                || !matches!(
                    d.status.as_str(),
                    "queued" | "waiting_for_sources" | "downloading"
                )
            {
                continue;
            }
            let cap = d.max_sources.map(|m| m as usize).unwrap_or(max_peers_per_file);
            if d.source_count as usize >= cap {
                continue;
            }
            let Ok(srcs) = dl.list_peer_sources(&d.ed2k_hash) else {
                continue;
            };
            let Some(src) = srcs.iter().find(|s| s.address == address) else {
                continue;
            };
            let nnp_flag = src
                .last_error
                .as_deref()
                .map(|e| {
                    let e = e.to_lowercase();
                    e.contains("no needed parts") || e.contains("file not found")
                })
                .unwrap_or(false);
            // Cooldown applies to every status — an NNP-flagged source
            // (`Allowed` + 58 min cooldown) must not be re-picked within the
            // same socket chain (eMule `DontSwapTo`/`GetTimeUntilReask`).
            if src.cooldown_until_unix_ms.is_some_and(|c| c > now_ms) {
                continue;
            }
            let viable = match src.status {
                PeerSourceStatus::Candidate
                | PeerSourceStatus::Allowed
                | PeerSourceStatus::Tried
                | PeerSourceStatus::Connected
                | PeerSourceStatus::Failed => true,
                PeerSourceStatus::Rejected | PeerSourceStatus::Filtered => false,
            };
            if !viable {
                continue;
            }
            let prio = match d.priority.as_str() {
                "high" => 2u8,
                "normal" => 1,
                _ => 0,
            };
            v.push(Cand {
                hash: d.ed2k_hash.clone(),
                prio,
                nnp: nnp_flag,
            });
        }
        v
    };

    // eMule: OtherRequests (clean sources) before OtherNoNeeded (NNP-flagged
    // on that file), `RightFileHasHigherPrio` = download priority first.
    cands.sort_by(|a, b| {
        (a.nnp as u8)
            .cmp(&(b.nnp as u8))
            .then(b.prio.cmp(&a.prio))
            .then(a.hash.cmp(&b.hash))
    });
    let cand = cands.into_iter().next()?;
    let mut hash = [0u8; 16];
    let raw = hex::decode(&cand.hash).ok()?;
    if raw.len() != 16 {
        return None;
    }
    hash.copy_from_slice(&raw);
    Some((cand.hash, hash))
}

/// Records the swap bookkeeping before the session re-asks another file on
/// the same socket (eMule `DoSwap`: the client leaves the old file's source
/// list — deadlist for FNF, NNP cooldown — and joins the new file's list).
pub(crate) async fn apply_swap(
    downloads: &Arc<Mutex<DownloadService>>,
    address: &str,
    from_hash: &str,
    to_hash: &str,
    reason: &PeerFailureReason,
    now_ms: i64,
) {
    let mut dl = downloads.lock().await;
    match reason {
        PeerFailureReason::NoNeededParts => {
            let _ = dl.update_peer_source_status(
                from_hash,
                address,
                PeerSourceStatus::Allowed,
                Some("no needed parts (A4AF swap)"),
                Some(now_ms + NNP_REASK_COOLDOWN_MS),
            );
        }
        PeerFailureReason::FileNotFound => {
            let _ = dl.update_peer_source_status(
                from_hash,
                address,
                PeerSourceStatus::Rejected,
                Some("file not found (A4AF swap)"),
                None,
            );
        }
        _ => {}
    }
    // The source is now actively served for `to_hash` on the live socket:
    // mark it connected and clear any leftover cooldown so the coordinator
    // counts it as a live source (eMule `SwapTo->srclist.AddTail(this)`).
    let _ = dl.update_peer_source_status(
        to_hash,
        address,
        PeerSourceStatus::Connected,
        None,
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use mule_core::model::{PeerSource, PeerSourceOrigin};

    #[test]
    fn swap_eligibility() {
        assert!(is_swap_eligible(&PeerFailureReason::NoNeededParts));
        assert!(is_swap_eligible(&PeerFailureReason::FileNotFound));
        assert!(!is_swap_eligible(&PeerFailureReason::TimeoutQueue));
        assert!(!is_swap_eligible(&PeerFailureReason::ConnectRefused));
        assert!(!is_swap_eligible(&PeerFailureReason::CorruptedData));
    }

    fn peer_source(file_hash: &str, addr: &str) -> PeerSource {
        PeerSource {
            file_hash: file_hash.to_string(),
            address: addr.to_string(),
            ip: 0,
            port: 0,
            origin: PeerSourceOrigin::Server,
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
        }
    }

    fn hash_hex(byte: u8) -> String {
        hex::encode([byte; 16])
    }

    fn svc_with_two_files(
        hash_a: &str,
        hash_b: &str,
        tmp: &tempfile::TempDir,
    ) -> DownloadService {
        let mut svc = DownloadService::in_memory(tmp.path().join("temp")).unwrap();
        for (name, h) in [("file_a.bin", hash_a), ("file_b.bin", hash_b)] {
            let link = format!("ed2k://|file|{name}|10000000|{h}|/");
            svc.add_ed2k_link(
                &link,
                tmp.path(),
                mule_core::storage::part_file::AllocationPolicy::None,
                None,
            )
            .unwrap();
        }
        svc
    }

    #[tokio::test]
    async fn swap_target_picks_other_file_source() {
        let tmp = tempfile::tempdir().unwrap();
        let (ha, hb) = (hash_hex(0x11), hash_hex(0x22));
        let mut svc = svc_with_two_files(&ha, &hb, &tmp);
        let addr = "1.2.3.4:4662";
        svc.upsert_peer_sources(&hb, &[peer_source(&hb, addr)]).unwrap();
        let svc = Arc::new(Mutex::new(svc));

        let got = find_swap_target(&svc, addr, &ha, 100, 0).await;
        assert_eq!(got.map(|(h, _)| h), Some(hb.clone()));
        // Not offered for the file it already serves.
        assert!(find_swap_target(&svc, addr, &hb, 100, 0).await.is_none());
        // Unknown address -> nothing.
        assert!(find_swap_target(&svc, "9.9.9.9:1", &ha, 100, 0).await.is_none());
    }

    #[tokio::test]
    async fn swap_target_rejects_rejected_and_cooled_sources() {
        let tmp = tempfile::tempdir().unwrap();
        let (ha, hb) = (hash_hex(0x33), hash_hex(0x44));
        let mut svc = svc_with_two_files(&ha, &hb, &tmp);
        let addr = "5.6.7.8:4662";
        let mut src = peer_source(&hb, addr);
        src.status = PeerSourceStatus::Rejected;
        svc.upsert_peer_sources(&hb, &[src]).unwrap();
        let svc = Arc::new(Mutex::new(svc));
        assert!(find_swap_target(&svc, addr, &ha, 100, 0).await.is_none());

        // Failed with a live cooldown is skipped; expired cooldown is viable.
        {
            let mut dl = svc.lock().await;
            dl.update_peer_source_status(
                &hb,
                addr,
                PeerSourceStatus::Failed,
                Some("dial failed"),
                Some(1000),
            )
            .unwrap();
        }
        assert!(find_swap_target(&svc, addr, &ha, 100, 500).await.is_none());
        assert_eq!(
            find_swap_target(&svc, addr, &ha, 100, 2000).await.map(|(h, _)| h),
            Some(hb)
        );
    }

    #[tokio::test]
    async fn swap_target_prefers_clean_over_nnp() {
        let tmp = tempfile::tempdir().unwrap();
        let (ha, hb, hc) = (hash_hex(0x55), hash_hex(0x66), hash_hex(0x77));
        let mut svc = svc_with_two_files(&ha, &hb, &tmp);
        // Third download whose source record is NNP-flagged.
        let link_c = format!("ed2k://|file|file_c.bin|10000000|{hc}|/");
        svc.add_ed2k_link(
            &link_c,
            tmp.path(),
            mule_core::storage::part_file::AllocationPolicy::None,
            None,
        )
        .unwrap();
        let addr = "8.8.8.8:4662";
        let mut nnp_src = peer_source(&hb, addr);
        nnp_src.status = PeerSourceStatus::Allowed;
        nnp_src.last_error = Some("no needed parts".to_string());
        svc.upsert_peer_sources(&hb, &[nnp_src]).unwrap();
        svc.upsert_peer_sources(&hc, &[peer_source(&hc, addr)]).unwrap();
        let svc = Arc::new(Mutex::new(svc));
        // Clean candidate (hc) wins over the NNP-flagged one (hb).
        assert_eq!(
            find_swap_target(&svc, addr, &ha, 100, 0).await.map(|(h, _)| h),
            Some(hc)
        );
    }

    #[tokio::test]
    async fn apply_swap_writes_deadlist_and_live_states() {
        let tmp = tempfile::tempdir().unwrap();
        let (ha, hb) = (hash_hex(0x88), hash_hex(0x99));
        let mut svc = svc_with_two_files(&ha, &hb, &tmp);
        let addr = "7.7.7.7:4662";
        svc.upsert_peer_sources(&ha, &[peer_source(&ha, addr)]).unwrap();
        svc.upsert_peer_sources(&hb, &[peer_source(&hb, addr)]).unwrap();
        let svc = Arc::new(Mutex::new(svc));

        apply_swap(
            &svc,
            addr,
            &ha,
            &hb,
            &PeerFailureReason::NoNeededParts,
            1000,
        )
        .await;
        let dl = svc.lock().await;
        let a = dl
            .list_peer_sources(&ha)
            .unwrap()
            .into_iter()
            .find(|s| s.address == addr)
            .unwrap();
        assert_eq!(a.status, PeerSourceStatus::Allowed);
        assert_eq!(a.cooldown_until_unix_ms, Some(1000 + NNP_REASK_COOLDOWN_MS));
        let b = dl
            .list_peer_sources(&hb)
            .unwrap()
            .into_iter()
            .find(|s| s.address == addr)
            .unwrap();
        assert_eq!(b.status, PeerSourceStatus::Connected);
        drop(dl);

        apply_swap(&svc, addr, &ha, &hb, &PeerFailureReason::FileNotFound, 2000)
            .await;
        let dl = svc.lock().await;
        let a = dl
            .list_peer_sources(&ha)
            .unwrap()
            .into_iter()
            .find(|s| s.address == addr)
            .unwrap();
        assert_eq!(a.status, PeerSourceStatus::Rejected);
    }
}
