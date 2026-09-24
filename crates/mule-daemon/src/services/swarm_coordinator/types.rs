// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Swarm coordinator constants and internal state structure.

use crate::state::AppState;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Interval between coordinator evaluation ticks.
pub const COORDINATOR_TICK_INTERVAL: Duration = Duration::from_secs(3);

/// eMule `SERVERREASKTIME` (Opcodes.h) : délai minimal entre deux requêtes
/// TCP de sources (OP_GETSOURCES) pour un même fichier.
pub const SERVER_REASK_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// eMule `UDPSERVERREASKTIME` (Opcodes.h) : délai minimal entre deux
/// requêtes UDP de sources (OP_GLOBGETSOURCES) pour un même fichier.
/// eMule cadence ce canal via un compteur global (`m_lastudpsearchtime`)
/// en itérant tous les fichiers ; la cadence par fichier est équivalente.
pub const UDP_SERVER_REASK_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// eMule `KADEMLIAREASKTIME` (Opcodes.h) : base du backoff de recherche de
/// sources Kad. L'échéance réelle est `now + KADEMLIA_REASK_INTERVAL *
/// total_searches` (`m_TotalSearchesKad`, plafonné à `KAD_MAX_TOTAL_SEARCHES`).
pub const KADEMLIA_REASK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// eMule `KADEMLIAASKTIME` (Opcodes.h) : espacement minimal entre deux
/// démarrages de recherche de sources Kad, tous fichiers confondus
/// (`CDownloadQueue::DoKademliaFileRequest`).
pub const KADEMLIA_ASK_INTERVAL: Duration = Duration::from_secs(1);

/// eMule `KADEMLIATOTALFILE` (Opcodes.h) : nombre maximal de recherches de
/// sources Kad simultanées (`CKademlia::GetTotalFile()`).
pub const KAD_MAX_FILE_SEARCHES: usize = 5;

/// eMule plafond de `m_TotalSearchesKad` (PartFile.cpp) : le backoff Kad
/// cesse de croître après 7 recherches (≈ 7 h entre réinterrogations).
pub const KAD_MAX_TOTAL_SEARCHES: u32 = 7;

/// eMule `MAX_SOURCES_FILE_SOFT` (Opcodes.h) : borne absolue du seuil de
/// réinterrogation TCP (`CPartFile::GetMaxSourcePerFileSoft`).
pub const MAX_SOURCES_FILE_SOFT: usize = 750;

/// eMule `MAX_SOURCES_FILE_UDP` (Opcodes.h) : borne absolue du seuil de
/// réinterrogation UDP/Kad (`CPartFile::GetMaxSourcePerFileUDP`).
pub const MAX_SOURCES_FILE_UDP: usize = 50;

/// Maximum number of new peer connections to initiate per file per tick
/// (anti-burst par fichier). Le pacing global reste assuré par
/// `ResourceBudgets.rate_limit_per_min` (240/min = eMule
/// `MaxConnectionsPerFiveSeconds` 20 ≈ 4/s) appliqué à chaque
/// `request_tcp_connect` — 12/tick à 3 s ≈ 4/s, donc un fichier seul peut
/// consommer tout le rythme eMule sans le dépasser.
pub const MAX_CONNECT_PER_TICK: usize = 12;

/// Interval between periodic system heartbeats (60 seconds).
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);

/// Minimum delay between two automatic server (re)connect attempts.
pub const SERVER_RECONNECT_INTERVAL: Duration = Duration::from_secs(60);

/// Per-address backoff before retrying a server that failed to connect.
pub const SERVER_FAIL_COOLDOWN: Duration = Duration::from_secs(300);

/// Minimum interval between excess-source purge passes per file
/// (eMule `CPartFile::Process` slow timers run every ~30-60s).
pub const SOURCE_PURGE_INTERVAL: Duration = Duration::from_secs(60);

/// Full A4AF rebalance cadence (eMule `ProcessA4AFClients` ~8 min).
/// Event-driven NNP swaps still run on every tick.
pub const A4AF_FULL_INTERVAL: Duration = Duration::from_secs(8 * 60);

/// eMule `CPartFile::GetMaxSourcePerFileSoft` : seuil de réinterrogation
/// TCP des sources — 90 % du plafond du fichier, borné à
/// `MAX_SOURCES_FILE_SOFT`.
pub(crate) fn max_source_per_file_soft(max_sources: usize) -> usize {
    (max_sources * 9 / 10).min(MAX_SOURCES_FILE_SOFT)
}

/// eMule `CPartFile::GetMaxSourcePerFileUDP` : seuil de réinterrogation
/// UDP/Kad — 75 % du plafond du fichier, borné à `MAX_SOURCES_FILE_UDP`.
pub(crate) fn max_source_per_file_udp(max_sources: usize) -> usize {
    (max_sources * 3 / 4).min(MAX_SOURCES_FILE_UDP)
}

/// Plafonds de connexions pairs lus à chaud par le coordinateur.
///
/// eMule n'a pas de quota par fichier : chaque fichier connecte ses
/// sources jusqu'à `max_peers_per_file` (`maxsourceperfile`, ou
/// `FT_MAXSOURCES` quand le `.part.met` le précise), la limite globale
/// `max_global_peers` jouant le rôle de `maxconnections`
/// (`CListenSocket::TooManySockets`). `min_peers_per_file` ne sert qu'au
/// rebalance A4AF (`ProcessA4AFClients`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwarmLimits {
    /// Plafond global de connexions pairs actives (eMule `maxconnections`).
    pub max_global_peers: usize,
    /// Plafond de sources connectées par fichier (eMule `maxsourceperfile`).
    pub max_peers_per_file: usize,
    /// Seuil de famine déclenchant un swap A4AF (extension eMule-Rust).
    pub min_peers_per_file: usize,
}

/// État d'un fichier consommé par le rebalance A4AF
/// (eMule `ProcessA4AFClients`) : poids de priorité et pairs actifs.
pub(crate) struct A4afFileState {
    pub file_hash: String,
    /// Poids eMule : Low=1, Normal=2, High=4, Release=8.
    pub priority_weight: u32,
    /// Nombre de sessions pair actives du fichier.
    pub current_active_peers: usize,
}

impl A4afFileState {
    pub(crate) fn new(
        file_hash: String,
        priority_str: &str,
        source_count: usize,
        current_active_peers: usize,
    ) -> Self {
        let priority_weight = match priority_str.to_lowercase().as_str() {
            "low" => 1,
            "high" => 4,
            "release" => 8,
            // eMule `PR_AUTO` — `CPartFile::UpdateAutoDownPriority` : le
            // poids effectif dépend du nombre de sources connues
            // (`GetSourceCount`) : plus de 100 → low, plus de 20 → normal,
            // sinon → high. Recalculé à chaque tick du coordinateur comme
            // eMule le fait dans `CPartFile::Process`.
            "auto" => {
                if source_count > 100 {
                    1
                } else if source_count > 20 {
                    2
                } else {
                    4
                }
            }
            _ => 2, // "normal" ou non précisé
        };
        Self {
            file_hash,
            priority_weight,
            current_active_peers,
        }
    }
}

/// État de recherche de sources Kad d'un fichier — équivalent eMule
/// `m_LastSearchTimeKad` + `m_TotalSearchesKad` (PartFile.h).
#[derive(Debug, Clone, Copy)]
pub(crate) struct KadSourceSearchState {
    /// Échéance absolue avant laquelle aucune nouvelle recherche Kad n'est
    /// lancée pour ce fichier (`now + KADEMLIA_REASK_INTERVAL *
    /// total_searches` au moment du départ).
    pub next_reask: Instant,
    /// Nombre de recherches Kad déjà lancées pour ce fichier
    /// (eMule `m_TotalSearchesKad`, ≤ `KAD_MAX_TOTAL_SEARCHES`).
    pub total_searches: u32,
}

/// Swarm coordinator orchestrating peer connections and background downloads.
pub struct SwarmCoordinator {
    pub(crate) state: AppState,
    /// Dernière requête TCP de sources par fichier (eMule `m_LastSearchTime`).
    pub(crate) last_source_query_tcp: HashMap<String, Instant>,
    /// Dernière requête UDP de sources par fichier (eMule cycle
    /// `m_lastudpsearchtime` ramené à une cadence par fichier).
    pub(crate) last_source_query_udp: HashMap<String, Instant>,
    /// État de recherche Kad par fichier (eMule `m_LastSearchTimeKad` +
    /// `m_TotalSearchesKad`).
    pub(crate) kad_source_search: HashMap<String, KadSourceSearchState>,
    /// Horodatage du dernier démarrage de recherche Kad, tous fichiers
    /// confondus (eMule `m_lastkademliafilerequest` — `KADEMLIAASKTIME`).
    pub(crate) last_kad_file_request: Instant,
    pub(crate) last_heartbeat: Instant,
    /// Last connect attempt per server address (failover backoff).
    pub(crate) server_attempts: HashMap<String, Instant>,
    /// Last time an automatic server connect was attempted.
    pub(crate) last_server_reconnect: Option<Instant>,
    /// Server address that was connected before the last drop (excluded
    /// from the immediate failover pick).
    pub(crate) last_connected_server: Option<String>,
    /// Last excess-source purge pass per file hash (eMule 80% rule).
    pub(crate) last_source_purge: HashMap<String, Instant>,
    /// Last full A4AF rebalance pass (~8 min eMule cadence).
    pub(crate) last_a4af_full: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// eMule `CPartFile::UpdateAutoDownPriority` : la priorité « auto »
    /// dépend du nombre de sources connues — ≤20 → high (4), 21..=100
    /// → normal (2), >100 → low (1).
    #[test]
    fn a4af_auto_priority_weight_thresholds() {
        for (sources, expected) in [
            (0usize, 4u32),
            (1, 4),
            (20, 4),
            (21, 2),
            (50, 2),
            (100, 2),
            (101, 1),
            (500, 1),
        ] {
            let s = A4afFileState::new("h".to_string(), "auto", sources, 0);
            assert_eq!(
                s.priority_weight, expected,
                "auto priority with {sources} sources"
            );
        }
    }

    /// Une priorité manuelle ignore toujours le compte de sources.
    #[test]
    fn a4af_manual_priority_ignores_source_count() {
        for (prio, expected) in [("low", 1u32), ("normal", 2), ("high", 4), ("release", 8)] {
            for sources in [0usize, 50, 500] {
                let s = A4afFileState::new("h".to_string(), prio, sources, 0);
                assert_eq!(s.priority_weight, expected, "{prio}@{sources}");
            }
        }
    }
}
