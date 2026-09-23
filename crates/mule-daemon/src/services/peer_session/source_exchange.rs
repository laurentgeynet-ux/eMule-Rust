// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Gestion du Source Exchange (SX/SX2) alignée sur eMule 0.72a.
//!
//! Références : `ListenSocket.cpp:970-1011,1165-1251` (dispatch + throttle),
//! `PartFile.cpp:2368-2405` (`CanAddSource`), `PartFile.cpp:3602-3712`
//! (`CreateSrcInfoPacket`), `PartFile.cpp:3714-3890` (`AddClientSources`),
//! `DownloadClient.cpp:212-255` (`IsSourceRequestAllowed`),
//! `Opcodes.h:69-115` (`SOURCECLIENTREASKS`, `SOURCECLIENTREASKF`,
//! `MINCOMMONPENALTY`, `RARE_FILE`, `CONNECTION_LATENCY`).

use mule_protocol::ed2k::{
    Ed2kAnswerSources, Ed2kDecoder, Ed2kEncoder, Ed2kRequestSources, SxSource,
    SOURCE_EXCHANGE2_VERSION, OP_ANSWERSOURCES, OP_ANSWERSOURCES2, OP_REQUESTSOURCES,
    OP_REQUESTSOURCES2, PR_EMULE, PR_EMULE_COMPRESSED,
};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use mule_network_policy::{validate_connect_address, NetworkPolicyManager};

use crate::services::downloads::DownloadService;
use crate::services::obfuscation_stream::ObfuscatedStream;

/// eMule `SOURCECLIENTREASKS` (`Opcodes.h:69`) — 40 min entre deux requêtes
/// SX vers le même client.
pub const SOURCECLIENTREASKS: Duration = Duration::from_secs(40 * 60);

/// eMule `SOURCECLIENTREASKF` (`Opcodes.h:70`) — 5 min entre deux réponses
/// SX reçues pour le même fichier (fenêtre « sources fraîches »).
pub const SOURCECLIENTREASKF: Duration = Duration::from_secs(5 * 60);

/// eMule `MINCOMMONPENALTY` (`Opcodes.h:90`) — pénalité ×4 sur les timers
/// pour les fichiers non rares.
pub const MINCOMMONPENALTY: u32 = 4;

/// eMule `RARE_FILE` (`Opcodes.h:115`) — un fichier avec ≤ 50 sources est
/// « rare » et bénéficie de timers plus courts.
pub const RARE_FILE: usize = 50;

/// eMule `CONNECTION_LATENCY` (`Opcodes.h:120`) — marge de latence ajoutée
/// aux comparaisons de temps (22,05 s).
pub const CONNECTION_LATENCY: Duration = Duration::from_millis(22050);

/// Capacité filaire SX d'eMule : `CreateSrcInfoPacket` coupe après
/// `if (nCount > 500) break;` — 501 entrées maximum par réponse.
const SX_RESPONSE_MAX_SOURCES: usize = 501;

/// Cap de sources connues par fichier quand `.part.met` ne fixe pas
/// `FT_MAXSOURCES` — eMule `GetMaxSourcePerFileDefault() = 400`
/// (`Preferences.cpp:1976`).
const DEFAULT_MAX_SOURCES_PER_FILE: usize = 400;

/// État partagé du Source Exchange : policy réseau, identité locale et
/// journaux de throttle. Un seul exemplaire créé dans `AppState`, injecté
/// dans le `PeerSessionManager` et l'`InboundListenerService` — équivalent
/// des compteurs eMule répartis entre `CUpDownClient` et `CPartFile`.
#[derive(Clone)]
pub struct SxEnv {
    /// Policy réseau (classification IP + ipfilter) — équivalent
    /// `IsGoodIP` + `theApp.ipfilter->IsFiltered` d'eMule, en plus strict.
    pub network_policy: NetworkPolicyManager,
    /// Dernière réponse SX envoyée à chaque pair — eMule
    /// `CUpDownClient::m_dwLastSourceRequest` (côté serveur SX).
    pub answer_log: Arc<std::sync::Mutex<HashMap<IpAddr, Instant>>>,
    /// Dernière requête SX émise vers chaque pair — eMule
    /// `CUpDownClient::GetLastAskedForSources()` (côté client SX).
    pub request_log: Arc<std::sync::Mutex<HashMap<IpAddr, Instant>>>,
    /// Dernière réponse SX reçue par fichier — eMule
    /// `CPartFile::m_ClientSrcAnswered` (`GetLastAnsweredTime`).
    pub file_answer_log: Arc<std::sync::Mutex<HashMap<[u8; 16], Instant>>>,
    /// Notre user hash MD4 — les sources qui le portent sont nous-mêmes
    /// (eMule droppe `userhash == own` à l'ajout, `DownloadQueue.cpp:479`).
    pub own_user_hash: [u8; 16],
}

impl SxEnv {
    /// Crée un environnement SX vide.
    pub fn new(network_policy: NetworkPolicyManager, own_user_hash: [u8; 16]) -> Self {
        Self {
            network_policy,
            answer_log: Arc::new(std::sync::Mutex::new(HashMap::new())),
            request_log: Arc::new(std::sync::Mutex::new(HashMap::new())),
            file_answer_log: Arc::new(std::sync::Mutex::new(HashMap::new())),
            own_user_hash,
        }
    }
}

/// Identité locale eD2K pour les checks `CanAddSource` d'eMule
/// (`PartFile.cpp:2380-2403`) : détecter les sources qui désignent
/// nous-mêmes ou une cible injoignable.
#[derive(Clone, Copy, Debug, Default)]
pub struct SxLocalIds {
    /// `client_id` attribué par le serveur eD2K courant (0 = déconnecté).
    pub client_id: u32,
    /// IPv4 du serveur courant (format hybrid little-endian), 0 si inconnu.
    pub server_ip: u32,
    /// Port du serveur courant.
    pub server_port: u16,
    /// Notre port TCP d'écoute eD2K.
    pub tcp_port: u16,
    /// `true` quand notre `client_id` est un HighID (non `IsLowID`) —
    /// eMule `!theApp.IsFirewalled()`.
    pub is_high_id: bool,
}

/// Capacités SX annoncées par le pair distant + état de complétude.
#[derive(Clone, Copy, Debug, Default)]
pub struct PeerSxCaps {
    /// Version SX1 annoncée par le pair (`m_bySourceExchange1Ver`,
    /// CT_EMULE_MISCOPTIONS1 bits 12-15 ou tag `ET_SOURCEEXCHANGE`).
    /// `0` = le pair n'a pas annoncé SX1.
    pub sx1_version: u8,
    /// SX2 annoncé (`SupportsSourceExchange2`, MISCOPTIONS2 bit 10).
    pub sx2: bool,
    /// `m_bCompleteSource` : le pair détient le fichier complet — eMule ne
    /// demande pas de sources à une source complète.
    pub complete_source: bool,
}

/// eMule `CUpDownClient::IsSourceRequestAllowed` (`DownloadClient.cpp:212-255`)
/// — version mono-fichier : nos sessions sont liées à un seul fichier
/// (`partfile == m_reqfile`, `sourceExchangeCheck = false`), les clauses
/// A4AF disparaissent donc.
///
/// - `source_count` : `GetSourceCount()` — nombre total de sources connues.
/// - `valid_source_count` : `GetValidSourcesCount()` — sources dans un état
///   viable (non filtrées/rejetées).
/// - `max_source_soft` : `GetMaxSourcePerFileSoft()` — cap souple du fichier
///   (`max_sources * 9/10`, défaut 360).
pub fn is_source_request_allowed(
    env: &SxEnv,
    peer_ip: IpAddr,
    file_hash: &[u8; 16],
    source_count: usize,
    valid_source_count: usize,
    max_source_soft: usize,
    caps: &PeerSxCaps,
) -> bool {
    // `ExtProtocolAvailable() && (SupportsSourceExchange2() || sx1 > 1)`
    if !(caps.sx2 || caps.sx1_version > 1) {
        return false;
    }
    // `m_reqfile->GetMaxSourcePerFileSoft() > uSources`
    if source_count >= max_source_soft {
        return false;
    }

    let last_req = env
        .request_log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&peer_ip)
        .copied();
    let never_asked = last_req.is_none();
    let client_elapsed = last_req
        .map(|t| t.elapsed().saturating_add(CONNECTION_LATENCY))
        .unwrap_or(Duration::MAX);
    let file_elapsed = env
        .file_answer_log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(file_hash)
        .map(|t| t.elapsed().saturating_add(CONNECTION_LATENCY))
        .unwrap_or(Duration::MAX);

    // Source non complète, fichier très rare (≤ RARE_FILE/5).
    (!caps.complete_source
        && (never_asked || client_elapsed > SOURCECLIENTREASKS)
        && source_count <= RARE_FILE / 5)
        // Source non complète, fichier rare (≤ RARE_FILE, ou
        // ≤ RARE_FILE/2 + sources valides quand on a déjà des valides).
        || (!caps.complete_source
            && (never_asked || client_elapsed > SOURCECLIENTREASKS)
            && (source_count <= RARE_FILE
                || source_count <= RARE_FILE / 2 + valid_source_count)
            && file_elapsed > SOURCECLIENTREASKF)
        // Fichier non rare : client > 160 min et fichier > 20 min.
        || ((never_asked || client_elapsed > SOURCECLIENTREASKS * MINCOMMONPENALTY)
            && file_elapsed > SOURCECLIENTREASKF * MINCOMMONPENALTY)
}

/// eMule `SetLastAskedForSources()` + `CPartFile::SetLastAnsweredTimeTimeout()`
/// (`PartFile.h:267`) — enregistre l'émission d'une requête SX et recule le
/// timer fichier pour ne pas bloquer le re-ask suivant (`m_ClientSrcAnswered
/// = now + 2*CONNECTION_LATENCY - SOURCECLIENTREASKS`).
pub fn mark_sx_request_sent(env: &SxEnv, peer_ip: IpAddr, file_hash: [u8; 16]) {
    env.request_log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(peer_ip, Instant::now());
    let back = SOURCECLIENTREASKS.saturating_sub(2 * CONNECTION_LATENCY);
    let pseudo = Instant::now()
        .checked_sub(back)
        .unwrap_or_else(Instant::now);
    env.file_answer_log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(file_hash, pseudo);
}

/// eMule `CPartFile::SetLastAnsweredTime()` — enregistre la réception d'une
/// réponse SX valide pour ce fichier (utilisé par `IsSourceRequestAllowed`).
pub fn mark_sx_answer_received(env: &SxEnv, file_hash: [u8; 16]) {
    env.file_answer_log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(file_hash, Instant::now());
}

/// Version SX1 annoncée dans un `OP_EMULEINFO` — valeur du tag
/// `ET_SOURCEEXCHANGE` (eMule `ProcessMuleInfoPacket` lit la valeur comme
/// `m_bySourceExchange1Ver`).
pub fn emule_info_sx_version(info: &mule_protocol::ed2k::Ed2kEmuleInfo) -> u8 {
    use mule_protocol::ed2k::tags::{TagName, TagValue, TAG_SPECIAL_EMULE_EXT_SOURCEEXCHANGE};
    info.tags
        .iter()
        .find_map(|t| {
            if !matches!(t.name, TagName::Id(TAG_SPECIAL_EMULE_EXT_SOURCEEXCHANGE) | TagName::Id(0x04)) {
                return None;
            }
            match t.value {
                TagValue::UInt32(v) => u8::try_from(v).ok(),
                TagValue::UInt16(v) => u8::try_from(v).ok(),
                TagValue::UInt8(v) => Some(v),
                _ => None,
            }
        })
        .unwrap_or(0)
}

/// Construit les capacités SX d'un pair depuis son `Ed2kPeerInfo` (HELLO)
/// et son `OP_EMULEINFO` éventuel — eMule `GetSourceExchange1Version()`
/// (misc1 bits 12-15) avec repli sur la valeur du tag `ET_SOURCEEXCHANGE`.
pub fn peer_sx_caps(
    info: &mule_protocol::ed2k::Ed2kPeerInfo,
    emule_info: Option<&mule_protocol::ed2k::Ed2kEmuleInfo>,
) -> PeerSxCaps {
    PeerSxCaps {
        sx1_version: info
            .source_exchange1_version()
            .max(emule_info.map(emule_info_sx_version).unwrap_or(0)),
        sx2: info.supports_source_exchange2(),
        complete_source: false,
    }
}

/// Nombre de sources « valides » au sens eMule `GetValidSourcesCount()` —
/// sources dans un état exploitable (pas filtrées, rejetées ou en échec).
fn valid_source_count(sources: &[mule_core::model::PeerSource]) -> usize {
    sources
        .iter()
        .filter(|s| {
            !matches!(
                s.status,
                mule_core::model::PeerSourceStatus::Failed
                    | mule_core::model::PeerSourceStatus::Rejected
                    | mule_core::model::PeerSourceStatus::Filtered
            )
        })
        .count()
}

/// Cap souple eMule `GetMaxSourcePerFileSoft()` — `max_sources * 9/10`
/// (`PartFile.h:281`). Sans override par fichier (FT_MAXSOURCES), le
/// réglage global `max_peers_per_download` fait foi — comme eMule qui
/// retombe sur `thePrefs.maxsourceperfile`.
fn max_source_soft(
    snap: Option<&crate::services::downloads::types::DownloadSnapshot>,
    dl_svc: &crate::services::downloads::DownloadService,
) -> usize {
    let global = dl_svc
        .get_setting("max_peers_per_download")
        .ok()
        .flatten()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_SOURCES_PER_FILE);
    snap.and_then(|d| d.max_sources)
        .map(|m| (m as usize) * 9 / 10)
        .unwrap_or(global * 9 / 10)
}

/// Évalue `IsSourceRequestAllowed` puis émet `OP_REQUESTSOURCES(2)` vers le
/// pair — eMule envoie la requête SX à chaque `SendFileRequest` permis
/// (~reask cycle) ; `mark_sx_request_sent` n'est appelé qu'en cas d'écriture
/// réussie (`SetLastAskedForSources` + `SetLastAnsweredTimeTimeout`).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn maybe_send_sx_request(
    stream: &mut ObfuscatedStream<TcpStream>,
    encoder: &Ed2kEncoder,
    target_addr: SocketAddr,
    file_hash_str: &str,
    dl_hash_bytes: [u8; 16],
    downloads: &Arc<Mutex<DownloadService>>,
    env: &SxEnv,
    caps: &PeerSxCaps,
) {
    let (source_count, valid_count, soft) = {
        let dl_svc = downloads.lock().await;
        let sources = dl_svc.list_peer_sources(file_hash_str).unwrap_or_default();
        (
            sources.len(),
            valid_source_count(&sources),
            max_source_soft(dl_svc.find_by_ed2k_hash(file_hash_str).as_ref(), &dl_svc),
        )
    };
    if !is_source_request_allowed(
        env,
        target_addr.ip(),
        &dl_hash_bytes,
        source_count,
        valid_count,
        soft,
        caps,
    ) {
        return;
    }
    let sx_req = if caps.sx2 {
        Ed2kRequestSources::new_v2(dl_hash_bytes)
    } else {
        Ed2kRequestSources::new_v1(dl_hash_bytes)
    };
    match encoder.encode_request_sources(&sx_req) {
        Ok(pkt) => {
            let mut out = Vec::with_capacity(6 + pkt.payload.len());
            out.push(pkt.protocol);
            out.extend_from_slice(&(pkt.frame_len() as u32).to_le_bytes());
            out.push(pkt.opcode);
            out.extend_from_slice(&pkt.payload);
            use tokio::io::AsyncWriteExt;
            match stream.write_all(&out).await {
                Ok(()) => {
                    mark_sx_request_sent(env, target_addr.ip(), dl_hash_bytes);
                    tracing::info!(
                        "[SOURCE_EXCHANGE] Sent {} (version {}) to peer {target_addr} for file {file_hash_str}",
                        if caps.sx2 { "OP_REQUESTSOURCES2" } else { "OP_REQUESTSOURCES" },
                        sx_req.version
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to send SX request to {target_addr}: {e}");
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to encode SX request for {target_addr}: {e}");
        }
    }
}

/// `true` si l'IP/port du source passe le filtre réseau (équivalent
/// `IsGoodIP` + `ipfilter->IsFiltered` — plus strict qu'eMule : rejette
/// LAN/loopback/réservé en profil public, autorise le loopback en LocalLab).
fn source_address_allowed(env: &SxEnv, ip_addr: Ipv4Addr, port: u16) -> bool {
    let sock = SocketAddr::new(IpAddr::V4(ip_addr), port);
    if validate_connect_address(&sock, env.network_policy.profile()).is_err() {
        return false;
    }
    env.network_policy
        .ip_filter()
        .check_ip(IpAddr::V4(ip_addr))
        .is_none()
}

/// Traite une réponse SX (`OP_ANSWERSOURCES`/`OP_ANSWERSOURCES2`) — eMule
/// `CPartFile::AddClientSources` + `CanAddSource` (`PartFile.cpp:3714-3890,
/// 2368-2405`).
///
/// Filtrage à l'ingestion, miroir d'eMule (avec notre policy plus stricte) :
/// - hash du fichier attendu ;
/// - `user_hash == own` → drop ;
/// - HighID : `IsGoodIP` (policy), `ipfilter`, self-check `client_id`+port ;
/// - LowID : conservé comme `lowid:<id>` (callback serveur), sauf si nous
///   sommes nous-mêmes LowID (`IsFirewalled`) ou si c'est notre propre ID ;
/// - arrêt à `GetMaxSources()` (`max_sources` ou 400 par défaut).
pub async fn handle_sx_answer(
    packet: &mule_protocol::framing::RawPacket,
    dl_hash_bytes: [u8; 16],
    file_hash_str: &str,
    target_addr: SocketAddr,
    decoder: &Ed2kDecoder,
    downloads: &Arc<Mutex<DownloadService>>,
    env: &SxEnv,
    local: &SxLocalIds,
) {
    if !((packet.protocol == PR_EMULE || packet.protocol == PR_EMULE_COMPRESSED)
        && (packet.opcode == OP_ANSWERSOURCES || packet.opcode == OP_ANSWERSOURCES2))
    {
        return;
    }

    let ans = match decoder.decode_answer_sources(packet.protocol, packet.opcode, &packet.payload) {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!("Peer {target_addr}: failed to decode SX answer: {e}");
            return;
        }
    };

    if ans.file_hash != dl_hash_bytes {
        tracing::debug!(
            "[SOURCE_EXCHANGE] Ignored SX answer from {target_addr}: file {} is not the requested download {file_hash_str}",
            hex::encode(ans.file_hash)
        );
        return;
    }

    // eMule `SetLastAnsweredTime` : une réponse valide recule le re-ask.
    mark_sx_answer_received(env, dl_hash_bytes);

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // Cap d'ingestion : `GetSourceCount() >= GetMaxSources()` stoppe la
    // boucle (sources déjà connues + nouvelles acceptées).
    let (existing_count, max_sources) = {
        let dl_svc = downloads.lock().await;
        let max = dl_svc
            .find_by_ed2k_hash(file_hash_str)
            .and_then(|d| d.max_sources)
            .map(|m| m as usize)
            .or_else(|| {
                // eMule `GetMaxSources()` : sans override par fichier, le
                // réglage global `max_peers_per_download` fait foi.
                dl_svc
                    .get_setting("max_peers_per_download")
                    .ok()
                    .flatten()
                    .and_then(|s| s.trim().parse::<usize>().ok())
            })
            .unwrap_or(DEFAULT_MAX_SOURCES_PER_FILE);
        (
            dl_svc
                .list_peer_sources(file_hash_str)
                .map(|v| v.len())
                .unwrap_or(0),
            max,
        )
    };

    let mut discovered_sources = Vec::new();
    let mut dropped_lowid_firewalled = 0usize;
    let mut dropped_filtered = 0usize;
    let mut dropped_self = 0usize;

    for src in ans.sources {
        if existing_count + discovered_sources.len() >= max_sources {
            break;
        }
        // Source qui porte notre user hash : c'est nous.
        if src.user_hash == Some(env.own_user_hash) {
            dropped_self += 1;
            continue;
        }

        let addr_str = if src.is_low_id() {
            // eMule `CanAddSource` : LowID inutilisable quand on est
            // firewalled (impossible de recevoir son callback).
            if !local.is_high_id {
                dropped_lowid_firewalled += 1;
                continue;
            }
            // Self-check LowID : notre propre client_id sur notre serveur.
            if local.client_id != 0
                && src.ip == local.client_id
                && src.server_ip == local.server_ip
                && src.server_port == local.server_port
            {
                dropped_self += 1;
                continue;
            }
            format!("lowid:{}", src.ip)
        } else {
            if src.port == 0 {
                dropped_filtered += 1;
                continue;
            }
            let ip_addr = Ipv4Addr::from(src.ip_bytes);
            // `IsGoodIP` + `ipfilter->IsFiltered` (policy réseau du projet).
            if !source_address_allowed(env, ip_addr, src.port) {
                dropped_filtered += 1;
                continue;
            }
            // Self-check HighID : notre client_id + notre port
            // (`client_id` HighID = notre IP en network order).
            if local.client_id >= 16_777_216
                && u32::from_be_bytes(src.ip_bytes) == local.client_id
                && src.port == local.tcp_port
            {
                dropped_self += 1;
                continue;
            }
            format!("{ip_addr}:{}", src.port)
        };

        let hex_user_hash = src.user_hash.map(hex::encode);
        discovered_sources.push(mule_core::model::PeerSource {
            file_hash: file_hash_str.to_string(),
            address: addr_str,
            ip: src.ip,
            port: src.port,
            origin: mule_core::model::PeerSourceOrigin::SourceExchange,
            discovered_at_unix_ms: now_ms,
            last_seen_unix_ms: now_ms,
            status: mule_core::model::PeerSourceStatus::Candidate,
            fail_count: 0,
            last_error: None,
            cooldown_until_unix_ms: None,
            user_hash: hex_user_hash,
            crypt_options: src.crypt_options,
            buddy_id: None,
            software: None,
            availability: None,
        });
    }

    if discovered_sources.is_empty() {
        tracing::debug!(
            "[SOURCE_EXCHANGE] SX answer from {target_addr} for file {file_hash_str} contained no usable source (filtered={dropped_filtered}, lowid_firewalled={dropped_lowid_firewalled}, self={dropped_self})"
        );
    } else {
        let mut dl_svc = downloads.lock().await;
        let inserted = dl_svc
            .upsert_peer_sources(file_hash_str, &discovered_sources)
            .unwrap_or(0);
        let total_swarm = dl_svc
            .find_by_ed2k_hash(file_hash_str)
            .map(|d| d.source_count)
            .unwrap_or(0);
        tracing::info!(
            "[SOURCE_EXCHANGE] Received {} sources via SX from peer {} for file {} -> {} new unique sources persisted (total swarm: {} sources, dropped: filtered={} lowid_fw={} self={})",
            discovered_sources.len(),
            target_addr,
            file_hash_str,
            inserted,
            total_swarm,
            dropped_filtered,
            dropped_lowid_firewalled,
            dropped_self
        );
    }
}

/// Sélection des sources à servir — eMule `CreateSrcInfoPacket`
/// (`PartFile.cpp:3652-3698`) :
/// - `!HasLowID() && IsValidSource()` : uniquement des sources vérifiées
///   (états `Connected`/`Allowed` — les `Candidate` jamais contactées ne
///   sont pas re-propagées) ;
/// - `srcstatus` connu : masque de disponibilité présent, non vide, et dont
///   la longueur correspond au nombre de parts du fichier ;
/// - ≥ 1 part utile au demandeur quand son part-status est connu
///   (`reqstatus`), sinon ≥ 1 part tout court ;
/// - cap `nCount > 500` → 501 entrées maximum.
fn select_sx_response_sources(
    known_sources: &[mule_core::model::PeerSource],
    file_part_count: usize,
    requester_mask: Option<&[u8]>,
    env: &SxEnv,
) -> Vec<SxSource> {
    let mut out = Vec::new();
    for src in known_sources {
        // `!cur_src->HasLowID()` — les LowID ne sont jamais servis en SX.
        if src.ip < 16_777_216 || src.port == 0 {
            continue;
        }
        // `IsValidSource()` — sources vérifiées seulement.
        if !matches!(
            src.status,
            mule_core::model::PeerSourceStatus::Connected
                | mule_core::model::PeerSourceStatus::Allowed
        ) {
            continue;
        }
        let ip_addr = Ipv4Addr::from(src.ip.to_le_bytes());
        if !source_address_allowed(env, ip_addr, src.port) {
            continue;
        }
        // `srcstatus && cur_src->GetPartCount() == GetPartCount()` :
        // le masque doit être connu et compatible avec le fichier.
        let Some(mask) = src.availability.as_deref() else {
            continue;
        };
        if file_part_count > 0 && mask.len() != file_part_count.div_ceil(8) {
            continue;
        }
        // `srcstatus[i] && (!reqstatus || !reqstatus[i])` : la source doit
        // couvrir ≥ 1 part que le demandeur n'a pas (ou ≥ 1 part si son
        // part-status est inconnu).
        let useful = (0..mask.len() * 8).any(|i| {
            mask[i / 8] & (1 << (i % 8)) != 0
                && requester_mask.is_none_or(|req| {
                    i / 8 >= req.len() || req[i / 8] & (1 << (i % 8)) == 0
                })
        });
        if !useful {
            continue;
        }
        out.push(SxSource {
            ip_bytes: src.ip.to_le_bytes(),
            ip: src.ip,
            port: src.port,
            // ServerIP/ServerPort non persistés dans notre modèle — 0 est
            // licite sur le fil (eMule les envoie quand connus).
            server_ip: 0,
            server_port: 0,
            user_hash: src.user_hash_bytes(),
            crypt_options: src.crypt_options,
        });
        // eMule : `if (nCount > 500) break;`
        if out.len() >= SX_RESPONSE_MAX_SOURCES {
            break;
        }
    }
    out
}

/// Traite une requête SX entrante (`OP_REQUESTSOURCES`/`OP_REQUESTSOURCES2`)
/// — eMule `ListenSocket.cpp:1165-1228` + `CreateSrcInfoPacket`.
///
/// - Gate `byRequestedVersion > 0 || sx1_version > 1` ;
/// - le fichier doit être connu (partagé ou en téléchargement) ;
/// - throttle `SetLastSrcReqTime` : jamais répondu → OK, part-file rare
///   → > 40 min, sinon > 160 min ;
/// - version négociée : `min(requested, 4)` en SX2, sinon SX1 à la version
///   annoncée par le pair ; pas de réponse si `nCount == 0` ;
/// - `requester_mask` : part-status du demandeur quand connu (ext-info
///   `OP_REQFILENAME`), utilisé pour ne servir que des sources utiles.
#[allow(clippy::too_many_arguments)]
pub async fn handle_sx_request(
    packet: &mule_protocol::framing::RawPacket,
    dl_hash_bytes: Option<[u8; 16]>,
    target_addr: SocketAddr,
    decoder: &Ed2kDecoder,
    encoder: &Ed2kEncoder,
    downloads: &Arc<Mutex<DownloadService>>,
    stream: &mut ObfuscatedStream<TcpStream>,
    env: &SxEnv,
    caps: &PeerSxCaps,
    requester_mask: Option<&[u8]>,
) {
    if packet.protocol != PR_EMULE
        || (packet.opcode != OP_REQUESTSOURCES && packet.opcode != OP_REQUESTSOURCES2)
    {
        return;
    }
    match decoder.decode_request_sources(packet.protocol, packet.opcode, &packet.payload) {
        Ok(req) => {
            respond_to_sx_request(
                &req,
                packet.opcode == OP_REQUESTSOURCES2,
                dl_hash_bytes,
                target_addr,
                encoder,
                downloads,
                stream,
                env,
                caps,
                requester_mask,
            )
            .await;
        }
        Err(e) => {
            tracing::warn!("Peer {target_addr}: failed to decode SX request: {e}");
        }
    }
}

/// Répond à une requête SX décodée — partagé entre le chemin standalone et
/// les sous-opcodes `OP_MULTIPACKET*` (eMule envoie la réponse SX en paquet
/// séparé, `ListenSocket.cpp:969`).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn respond_to_sx_request(
    req: &Ed2kRequestSources,
    is_sx2_req: bool,
    dl_hash_bytes: Option<[u8; 16]>,
    target_addr: SocketAddr,
    encoder: &Ed2kEncoder,
    downloads: &Arc<Mutex<DownloadService>>,
    stream: &mut ObfuscatedStream<TcpStream>,
    env: &SxEnv,
    caps: &PeerSxCaps,
    requester_mask: Option<&[u8]>,
) {
    // eMule `ListenSocket.cpp:986` : `byRequestedVersion > 0 ||
    // client->GetSourceExchange1Version() > 1`.
    let by_requested_version = if is_sx2_req { req.version } else { 0 };
    if by_requested_version == 0 && caps.sx1_version <= 1 {
        return;
    }
    // Session mono-fichier : ne répondre que pour le fichier de la session.
    if dl_hash_bytes.is_some() && dl_hash_bytes != Some(req.file_hash) {
        return;
    }

    let file_hash_str = hex::encode(req.file_hash);
    // eMule : sharedfiles d'abord, puis downloadqueue (`ListenSocket.cpp:1192`).
    let (is_part_file, file_part_count, known_sources) = {
        let dl_svc = downloads.lock().await;
        let shared = dl_svc.get_shared_file(&file_hash_str).unwrap_or(None);
        let dl_snap = dl_svc.find_by_ed2k_hash(&file_hash_str);
        let known = dl_snap.is_some() || shared.as_ref().is_some_and(|s| s.enabled);
        if !known {
            // Fichier inconnu : eMule appelle `CheckFailedFileIdReqs` —
            // ici, silence.
            return;
        }
        let is_part = dl_snap
            .as_ref()
            .map(|d| !matches!(d.status.to_lowercase().as_str(), "completed" | "complete"))
            .unwrap_or(false);
        let part_size = mule_protocol::ed2k::ED2K_PART_SIZE as u64;
        let parts = dl_snap
            .as_ref()
            .map(|d| d.total_bytes.div_ceil(part_size) as usize)
            .or_else(|| shared.as_ref().map(|s| s.file_size.div_ceil(part_size) as usize))
            .unwrap_or(0);
        (
            is_part,
            parts,
            dl_svc.list_peer_sources(&file_hash_str).unwrap_or_default(),
        )
    };

    // Throttle eMule (`ListenSocket.cpp:1204-1215`) : jamais répondu → OK ;
    // part-file rare (≤ RARE_FILE sources) → > SOURCECLIENTREASKS ;
    // sinon → > SOURCECLIENTREASKS × MINCOMMONPENALTY.
    let peer_ip = target_addr.ip();
    let last_answer = env
        .answer_log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&peer_ip)
        .copied();
    let never_asked = last_answer.is_none();
    let elapsed = last_answer
        .map(|t| t.elapsed().saturating_add(CONNECTION_LATENCY))
        .unwrap_or(Duration::MAX);
    let allowed = never_asked
        || elapsed > SOURCECLIENTREASKS * MINCOMMONPENALTY
        || (is_part_file && known_sources.len() <= RARE_FILE && elapsed > SOURCECLIENTREASKS);
    if !allowed {
        tracing::debug!(
            "[SOURCE_EXCHANGE] SX request from {target_addr} for {file_hash_str} rate-limited (last answer {:?} ago)",
            last_answer.map(|t| t.elapsed())
        );
        return;
    }
    // `client->SetLastSrcReqTime()`
    env.answer_log
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(peer_ip, Instant::now());

    let resp_sources =
        select_sx_response_sources(&known_sources, file_part_count, requester_mask, env);
    if resp_sources.is_empty() {
        // eMule `if (!nCount) return 0;` — pas de réponse sans source valide.
        tracing::debug!(
            "[SOURCE_EXCHANGE] SX request from {target_addr} for {file_hash_str}: no valid source to offer"
        );
        return;
    }

    // Négociation eMule (`PartFile.cpp:3629-3646`) :
    // `SupportsSourceExchange2() && byRequestedVersion > 0` → SX2 à
    // `min(requested, 4)` ; sinon SX1 à `GetSourceExchange1Version()`.
    let (version, is_sx2_packet) = if caps.sx2 && by_requested_version > 0 {
        (by_requested_version.min(SOURCE_EXCHANGE2_VERSION), true)
    } else {
        (caps.sx1_version, false)
    };
    if version == 0 {
        return;
    }

    let ans = Ed2kAnswerSources::new_v(version, req.file_hash, resp_sources);
    match encoder.encode_answer_sources(&ans, is_sx2_packet, false) {
        Ok(ans_pkt) => {
            let mut resp_wire = Vec::with_capacity(6 + ans_pkt.payload.len());
            resp_wire.push(ans_pkt.protocol);
            resp_wire.extend_from_slice(&(ans_pkt.frame_len() as u32).to_le_bytes());
            resp_wire.push(ans_pkt.opcode);
            resp_wire.extend_from_slice(&ans_pkt.payload);
            use tokio::io::AsyncWriteExt;
            if let Err(e) = stream.write_all(&resp_wire).await {
                tracing::warn!("Failed to send SX answer to {target_addr}: {e}");
            } else {
                tracing::info!(
                    "[SOURCE_EXCHANGE] Sent SX response ({} version {}, {} sources) to peer {} for file {}",
                    if is_sx2_packet { "SX2" } else { "SX1" },
                    version,
                    ans.sources.len(),
                    target_addr,
                    file_hash_str
                );
            }
        }
        Err(e) => {
            tracing::warn!("Failed to encode SX answer for {target_addr}: {e}");
        }
    }
}

/// Traite un `OP_MULTIPACKET*` reçu pendant une session download outbound —
/// le pair peut y encapsuler sa requête SX (eMule `ListenSocket.cpp:970-1011`,
/// sous-opcodes sans hash : celui de l'enveloppe). Seuls les sous-opcodes SX
/// sont dispatchés ; `dl_hash_bytes` borne la réponse au fichier de la
/// session (eMule `m_reqfile`). Retourne `true` si le paquet était un
/// multipacket (consommé, même en cas d'erreur de décodage).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_sx_multipacket(
    packet: &mule_protocol::framing::RawPacket,
    dl_hash_bytes: [u8; 16],
    requester_mask: Option<&[u8]>,
    target_addr: SocketAddr,
    encoder: &Ed2kEncoder,
    downloads: &Arc<Mutex<DownloadService>>,
    stream: &mut ObfuscatedStream<TcpStream>,
    env: &SxEnv,
    caps: &PeerSxCaps,
    ext_req_version: u8,
) -> bool {
    use mule_protocol::ed2k::{
        Ed2kMultiPacketSubRequest, OP_MULTIPACKET, OP_MULTIPACKET_EXT, OP_MULTIPACKET_EXT2,
    };
    if packet.protocol != PR_EMULE
        || !matches!(
            packet.opcode,
            OP_MULTIPACKET | OP_MULTIPACKET_EXT | OP_MULTIPACKET_EXT2
        )
    {
        return false;
    }
    let mp = match mule_protocol::ed2k::decoder::decode_multipacket_request(
        packet.protocol,
        packet.opcode,
        &packet.payload,
        0,
        &mule_protocol::ed2k::Ed2kLimits::default(),
        ext_req_version,
    ) {
        Ok(mp) => mp,
        Err(e) => {
            tracing::debug!("Peer {target_addr}: malformed multipacket ignored: {e}");
            return true;
        }
    };
    let file_hash = mp.file.file_hash();
    if file_hash == [0u8; 16] {
        return true;
    }
    // L'ext-info `OP_REQFILENAME` (part-status du demandeur) sert de
    // `reqstatus` quand le sous-paquet SX le suit dans l'enveloppe.
    let mut ext_mask: Option<Vec<u8>> = None;
    for sub in &mp.sub_requests {
        match sub {
            Ed2kMultiPacketSubRequest::FileName {
                ext_info: Some(ext),
            } => {
                ext_mask = Some(ext.part_status.to_vec());
            }
            Ed2kMultiPacketSubRequest::RequestSources => {
                let req = Ed2kRequestSources::new_v1(file_hash);
                let mask = ext_mask.as_deref().or(requester_mask);
                respond_to_sx_request(
                    &req,
                    false,
                    Some(dl_hash_bytes),
                    target_addr,
                    encoder,
                    downloads,
                    stream,
                    env,
                    caps,
                    mask,
                )
                .await;
            }
            Ed2kMultiPacketSubRequest::RequestSources2 { version, options } => {
                let req = Ed2kRequestSources {
                    file_hash,
                    version: *version,
                    options: *options,
                };
                let mask = ext_mask.as_deref().or(requester_mask);
                respond_to_sx_request(
                    &req,
                    true,
                    Some(dl_hash_bytes),
                    target_addr,
                    encoder,
                    downloads,
                    stream,
                    env,
                    caps,
                    mask,
                )
                .await;
            }
            _ => {}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use mule_core::model::{
        PeerSource, PeerSourceOrigin, PeerSourceStatus, SharedFile,
    };
    use mule_protocol::ed2k::tags::{
        Ed2kTag, TagName, TagValue, TAG_SPECIAL_EMULE_EXT_SOURCEEXCHANGE,
    };
    use mule_protocol::ed2k::Ed2kEmuleInfo;
    use mule_protocol::framing::{PacketCodec, RawPacket};
    use mule_network_policy::{NetworkProfile, ResourceBudgets};
    use tokio::io::AsyncReadExt;

    fn test_env() -> SxEnv {
        SxEnv::new(
            NetworkPolicyManager::new(
                NetworkProfile::PublicEd2kKadPreview,
                ResourceBudgets::default(),
            ),
            [0xAA; 16],
        )
    }

    fn ip4(octets: [u8; 4]) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(octets))
    }

    fn peer_source(
        ip_octets: [u8; 4],
        port: u16,
        status: PeerSourceStatus,
        availability: Option<Vec<u8>>,
    ) -> PeerSource {
        let ip = u32::from_le_bytes(ip_octets);
        PeerSource {
            file_hash: "00".repeat(16),
            address: format!("{}.{}.{}.{}:{port}", ip_octets[0], ip_octets[1], ip_octets[2], ip_octets[3]),
            ip,
            port,
            origin: PeerSourceOrigin::SourceExchange,
            discovered_at_unix_ms: 0,
            last_seen_unix_ms: 0,
            status,
            fail_count: 0,
            last_error: None,
            cooldown_until_unix_ms: None,
            user_hash: Some(hex::encode([0x33; 16])),
            crypt_options: 1,
            buddy_id: None,
            software: None,
            availability,
        }
    }

    // ---- `IsSourceRequestAllowed` (DownloadClient.cpp:212-255) ----

    #[test]
    fn source_request_allowed_requires_sx_capability() {
        let env = test_env();
        let ip = ip4([85, 25, 13, 5]);
        let fh = [0x01; 16];
        // Pair sans aucune capacité SX → jamais de requête.
        assert!(!is_source_request_allowed(
            &env,
            ip,
            &fh,
            0,
            0,
            360,
            &PeerSxCaps::default()
        ));
    }

    #[test]
    fn source_request_allowed_rare_file_first_ask_then_client_cooldown() {
        let env = test_env();
        let ip = ip4([85, 25, 13, 5]);
        let fh = [0x01; 16];
        let caps = PeerSxCaps {
            sx1_version: 0,
            sx2: true,
            complete_source: false,
        };
        // Fichier rare, jamais interrogé → autorisé.
        assert!(is_source_request_allowed(
            &env, ip, &fh, 3, 1, 360, &caps
        ));
        // Requête émise → cooldown SOURCECLIENTREASKS côté client.
        mark_sx_request_sent(&env, ip, fh);
        assert!(!is_source_request_allowed(
            &env, ip, &fh, 3, 1, 360, &caps
        ));
        // Cooldown expiré (> SOURCECLIENTREASKS) → autorisé à nouveau.
        env.request_log.lock().unwrap().insert(
            ip,
            Instant::now()
                .checked_sub(Duration::from_secs(41 * 60))
                .unwrap_or_else(Instant::now),
        );
        assert!(is_source_request_allowed(
            &env, ip, &fh, 3, 1, 360, &caps
        ));
    }

    #[test]
    fn source_request_allowed_soft_cap_and_complete_source() {
        let env = test_env();
        let ip = ip4([85, 25, 13, 5]);
        let fh = [0x01; 16];
        let caps = PeerSxCaps {
            sx1_version: 0,
            sx2: true,
            complete_source: false,
        };
        // `GetMaxSourcePerFileSoft` atteint → pas de requête.
        assert!(!is_source_request_allowed(
            &env, ip, &fh, 360, 300, 360, &caps
        ));
        // Source complète : les clauses « rare » exigent
        // `!m_bCompleteSource` — il reste la clause non-rare (160 min),
        // qui échoue sans historique… sauf `never_asked` qui la rend
        // vraie : eMule demande une première fois même à une source
        // complète sur un fichier non rare. On vérifie le gate rare :
        // complete_source → clause rare impossible.
        let complete = PeerSxCaps {
            complete_source: true,
            ..caps
        };
        // Clause rare bloquée ; clause commune `never_asked` encore
        // possible — eMule l'autorise réellement (client jamais sollicité).
        // On s'assure surtout que les clauses rares ne passent pas :
        // fichier rare (3 sources) + complete → seule la clause commune
        // peut répondre → elle est vraie (never_asked). Documenté : eMule
        // demande effectivement aux sources complètes via la clause 3.
        assert!(is_source_request_allowed(&env, ip, &fh, 3, 1, 360, &complete));
    }

    // ---- `select_sx_response_sources` (CreateSrcInfoPacket) ----

    #[test]
    fn select_sources_filters_like_emule() {
        let env = test_env();
        let good = peer_source(
            [85, 25, 13, 7],
            4662,
            PeerSourceStatus::Connected,
            Some(vec![0x01]),
        );
        let allowed = peer_source(
            [85, 25, 13, 12],
            4662,
            PeerSourceStatus::Allowed,
            Some(vec![0x01]),
        );
        let lowid = {
            let mut s = peer_source([1, 0, 0, 0], 0, PeerSourceStatus::Connected, Some(vec![0x01]));
            s.address = "lowid:1".into();
            s.ip = 1;
            s
        };
        let candidate =
            peer_source([85, 25, 13, 8], 4662, PeerSourceStatus::Candidate, Some(vec![0x01]));
        let nomask = peer_source([85, 25, 13, 9], 4662, PeerSourceStatus::Connected, None);
        let lan =
            peer_source([10, 0, 0, 5], 4662, PeerSourceStatus::Connected, Some(vec![0x01]));
        let wrong_mask = peer_source(
            [85, 25, 13, 10],
            4662,
            PeerSourceStatus::Connected,
            Some(vec![0xFF, 0xFF]),
        );
        let no_part = peer_source(
            [85, 25, 13, 11],
            4662,
            PeerSourceStatus::Connected,
            Some(vec![0x00]),
        );
        let out = select_sx_response_sources(
            &[good, allowed, lowid, candidate, nomask, lan, wrong_mask, no_part],
            1,
            None,
            &env,
        );
        let ips: Vec<[u8; 4]> = out.iter().map(|s| s.ip_bytes).collect();
        assert_eq!(ips, vec![[85, 25, 13, 7], [85, 25, 13, 12]]);
    }

    #[test]
    fn select_sources_requires_part_useful_to_requester() {
        let env = test_env();
        let good = peer_source(
            [85, 25, 13, 7],
            4662,
            PeerSourceStatus::Connected,
            Some(vec![0x01]),
        );
        // Le demandeur détient déjà la seule part offerte → rien à servir.
        assert!(
            select_sx_response_sources(std::slice::from_ref(&good), 1, Some(&[0x01]), &env)
                .is_empty()
        );
        // Il ne l'a pas → la source est servie.
        assert_eq!(
            select_sx_response_sources(&[good], 1, Some(&[0x00]), &env).len(),
            1
        );
    }

    #[test]
    fn select_sources_caps_at_emule_500_limit() {
        let env = test_env();
        let mut sources = Vec::new();
        for i in 0..600u32 {
            let oct = [85, 25, (i / 250) as u8 + 10, (i % 250) as u8 + 1];
            sources.push(peer_source(
                oct,
                4662,
                PeerSourceStatus::Connected,
                Some(vec![0x01]),
            ));
        }
        let out = select_sx_response_sources(&sources, 1, None, &env);
        // eMule `if (nCount > 500) break;` → 501 entrées max.
        assert_eq!(out.len(), SX_RESPONSE_MAX_SOURCES);
    }

    // ---- `emule_info_sx_version` ----

    #[test]
    fn emule_info_sx_version_reads_et_sourceexchange() {
        let info = Ed2kEmuleInfo {
            client_version: 0x3C,
            protocol_version: 1,
            tags: vec![
                Ed2kTag {
                    name: TagName::Id(TAG_SPECIAL_EMULE_EXT_SOURCEEXCHANGE),
                    value: TagValue::UInt8(3),
                },
                Ed2kTag {
                    name: TagName::Id(0x99),
                    value: TagValue::UInt32(1),
                },
            ],
            unknown_tags: Vec::new(),
        };
        assert_eq!(emule_info_sx_version(&info), 3);
        let bare = Ed2kEmuleInfo {
            client_version: 0x3C,
            protocol_version: 1,
            tags: Vec::new(),
            unknown_tags: Vec::new(),
        };
        assert_eq!(emule_info_sx_version(&bare), 0);
    }

    // ---- Ingestion `handle_sx_answer` (AddClientSources + CanAddSource) ----

    async fn memory_downloads() -> (tempfile::TempDir, Arc<Mutex<DownloadService>>) {
        let tmp = tempfile::tempdir().unwrap();
        let dl = DownloadService::in_memory(tmp.path().to_path_buf()).unwrap();
        (tmp, Arc::new(Mutex::new(dl)))
    }

    fn sx_src(ip_octets: [u8; 4], port: u16, user_hash: Option<[u8; 16]>) -> SxSource {
        SxSource {
            ip_bytes: ip_octets,
            ip: u32::from_le_bytes(ip_octets),
            port,
            server_ip: 0,
            server_port: 0,
            user_hash,
            crypt_options: 0,
        }
    }

    #[tokio::test]
    async fn sx_answer_persists_only_valid_sources() {
        let (_tmp, downloads) = memory_downloads().await;
        let env = test_env();
        let file_hash = [0x11; 16];
        let file_hash_str = hex::encode(file_hash);
        let local = SxLocalIds {
            client_id: u32::from_be_bytes([85, 25, 13, 50]),
            server_ip: 0x0102_0304,
            server_port: 4661,
            tcp_port: 4662,
            is_high_id: true,
        };

        let sources = vec![
            // Valide : IP publique HighID.
            sx_src([85, 25, 13, 7], 4662, Some([0x01; 16])),
            // Notre user hash → drop.
            sx_src([85, 25, 13, 8], 4662, Some(env.own_user_hash)),
            // Port 0 → drop.
            sx_src([85, 25, 13, 9], 0, Some([0x02; 16])),
            // LAN → drop (policy réseau).
            sx_src([10, 0, 0, 7], 4662, Some([0x03; 16])),
            // LowID → conservé `lowid:` quand on est HighID.
            SxSource {
                server_ip: 0x0506_0708,
                server_port: 4661,
                ..sx_src([88, 0, 0, 0], 0, Some([0x04; 16]))
            },
            // LowID == notre client_id sur notre serveur → drop.
            SxSource {
                server_ip: 0x0102_0304,
                server_port: 4661,
                ..sx_src(local.client_id.to_le_bytes(), 0, Some([0x05; 16]))
            },
            // HighID == notre client_id + notre port → drop.
            sx_src([85, 25, 13, 50], 4662, Some([0x06; 16])),
        ];
        let ans = Ed2kAnswerSources::new_v(4, file_hash, sources);
        let encoder = Ed2kEncoder::default();
        let pkt = encoder.encode_answer_sources(&ans, true, false).unwrap();
        let target: SocketAddr = "85.25.13.1:5000".parse().unwrap();
        handle_sx_answer(
            &pkt,
            file_hash,
            &file_hash_str,
            target,
            &Ed2kDecoder::default(),
            &downloads,
            &env,
            &local,
        )
        .await;

        let kept = downloads
            .lock()
            .await
            .list_peer_sources(&file_hash_str)
            .unwrap();
        let mut addrs: Vec<String> = kept.iter().map(|s| s.address.clone()).collect();
        addrs.sort();
        assert_eq!(addrs, vec!["85.25.13.7:4662".to_string(), "lowid:88".to_string()]);
        assert!(kept.iter().all(|s| s.status == PeerSourceStatus::Candidate));
    }

    #[tokio::test]
    async fn sx_answer_drops_lowid_when_firewalled() {
        let (_tmp, downloads) = memory_downloads().await;
        let env = test_env();
        let file_hash = [0x22; 16];
        let file_hash_str = hex::encode(file_hash);
        // Nous-mêmes LowID → impossible de recevoir le callback.
        let local = SxLocalIds {
            is_high_id: false,
            ..SxLocalIds::default()
        };
        let sources = vec![
            SxSource {
                server_ip: 1,
                server_port: 4661,
                ..sx_src([77, 0, 0, 0], 0, None)
            },
            sx_src([85, 25, 13, 7], 4662, None),
        ];
        let ans = Ed2kAnswerSources::new_v(4, file_hash, sources);
        let pkt = Ed2kEncoder::default()
            .encode_answer_sources(&ans, true, false)
            .unwrap();
        let target: SocketAddr = "85.25.13.1:5000".parse().unwrap();
        handle_sx_answer(
            &pkt,
            file_hash,
            &file_hash_str,
            target,
            &Ed2kDecoder::default(),
            &downloads,
            &env,
            &local,
        )
        .await;
        let kept = downloads
            .lock()
            .await
            .list_peer_sources(&file_hash_str)
            .unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].address, "85.25.13.7:4662");
    }

    #[tokio::test]
    async fn sx_answer_ignores_other_file_hash() {
        let (_tmp, downloads) = memory_downloads().await;
        let env = test_env();
        let file_hash = [0x33; 16];
        let other = [0x44; 16];
        let file_hash_str = hex::encode(file_hash);
        let ans = Ed2kAnswerSources::new_v(4, other, vec![sx_src([85, 25, 13, 7], 4662, None)]);
        let pkt = Ed2kEncoder::default()
            .encode_answer_sources(&ans, true, false)
            .unwrap();
        handle_sx_answer(
            &pkt,
            file_hash,
            &file_hash_str,
            "85.25.13.1:5000".parse().unwrap(),
            &Ed2kDecoder::default(),
            &downloads,
            &env,
            &SxLocalIds::default(),
        )
        .await;
        assert!(downloads
            .lock()
            .await
            .list_peer_sources(&file_hash_str)
            .unwrap()
            .is_empty());
        assert!(downloads
            .lock()
            .await
            .list_peer_sources(&hex::encode(other))
            .unwrap()
            .is_empty());
    }

    // ---- Service `respond_to_sx_request` via loopback TCP ----

    async fn socket_pair() -> (ObfuscatedStream<TcpStream>, TcpStream) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let client = TcpStream::connect(addr).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        (ObfuscatedStream::plaintext(server), client)
    }

    async fn read_wire(client: &mut TcpStream) -> RawPacket {
        let codec = PacketCodec::new();
        let mut buf = BytesMut::new();
        let mut tmp = [0u8; 8192];
        loop {
            let n = tokio::time::timeout(Duration::from_secs(3), client.read(&mut tmp))
                .await
                .expect("timeout waiting for SX answer")
                .expect("read error");
            assert!(n > 0, "connection closed before SX answer");
            buf.extend_from_slice(&tmp[..n]);
            if let Ok(Some(pkt)) = codec.decode(&mut buf) {
                return pkt;
            }
        }
    }

    async fn downloads_with_shared(
        file_hash: [u8; 16],
    ) -> (tempfile::TempDir, Arc<Mutex<DownloadService>>, String) {
        let (tmp, downloads) = memory_downloads().await;
        let hex_hash = hex::encode(file_hash);
        {
            let mut dl = downloads.lock().await;
            dl.add_shared_file(&SharedFile::new(
                hex_hash.clone(),
                "file.bin",
                9_728_000,
                tmp.path().join("file.bin").to_string_lossy().to_string(),
            ))
            .unwrap();
            let useful = peer_source(
                [85, 25, 13, 7],
                4662,
                PeerSourceStatus::Connected,
                Some(vec![0x01]),
            );
            let useless = peer_source(
                [85, 25, 13, 8],
                4662,
                PeerSourceStatus::Connected,
                Some(vec![0x00]),
            );
            dl.upsert_peer_sources(&hex_hash, &[useful, useless]).unwrap();
        }
        (tmp, downloads, hex_hash)
    }

    #[tokio::test]
    async fn respond_sx2_serves_useful_sources_at_version4() {
        let file_hash = [0x42; 16];
        let (_tmp, downloads, _hex) = downloads_with_shared(file_hash).await;
        let env = test_env();
        let caps = PeerSxCaps {
            sx1_version: 3,
            sx2: true,
            complete_source: false,
        };
        let (mut our_end, mut peer_end) = socket_pair().await;
        let peer_addr: SocketAddr = "85.25.13.99:4662".parse().unwrap();
        let req = Ed2kRequestSources::new_v2(file_hash);
        respond_to_sx_request(
            &req,
            true,
            None,
            peer_addr,
            &Ed2kEncoder::default(),
            &downloads,
            &mut our_end,
            &env,
            &caps,
            None,
        )
        .await;
        let pkt = read_wire(&mut peer_end).await;
        assert_eq!(pkt.opcode, OP_ANSWERSOURCES2);
        let ans = Ed2kDecoder::default()
            .decode_answer_sources(pkt.protocol, pkt.opcode, &pkt.payload)
            .unwrap();
        assert_eq!(ans.version, SOURCE_EXCHANGE2_VERSION);
        assert_eq!(ans.file_hash, file_hash);
        // Seule la source avec une part utile est servie.
        assert_eq!(ans.sources.len(), 1);
        assert_eq!(ans.sources[0].ip_bytes, [85, 25, 13, 7]);
        assert_eq!(ans.sources[0].user_hash, Some([0x33; 16]));
    }

    #[tokio::test]
    async fn respond_sx1_request_falls_back_to_sx1_packet() {
        let file_hash = [0x43; 16];
        let (_tmp, downloads, _hex) = downloads_with_shared(file_hash).await;
        let env = test_env();
        let caps = PeerSxCaps {
            sx1_version: 3,
            sx2: true,
            complete_source: false,
        };
        let (mut our_end, mut peer_end) = socket_pair().await;
        let req = Ed2kRequestSources::new_v1(file_hash);
        respond_to_sx_request(
            &req,
            false,
            None,
            "85.25.13.99:4662".parse().unwrap(),
            &Ed2kEncoder::default(),
            &downloads,
            &mut our_end,
            &env,
            &caps,
            None,
        )
        .await;
        let pkt = read_wire(&mut peer_end).await;
        // eMule `byRequestedVersion == 0` → OP_ANSWERSOURCES SX1.
        assert_eq!(pkt.opcode, OP_ANSWERSOURCES);
        let ans = Ed2kDecoder::default()
            .decode_answer_sources(pkt.protocol, pkt.opcode, &pkt.payload)
            .unwrap();
        assert_eq!(ans.version, 3);
        assert_eq!(ans.sources.len(), 1);
    }

    #[tokio::test]
    async fn respond_rate_limits_second_request() {
        let file_hash = [0x44; 16];
        let (_tmp, downloads, _hex) = downloads_with_shared(file_hash).await;
        let env = test_env();
        let caps = PeerSxCaps {
            sx1_version: 3,
            sx2: true,
            complete_source: false,
        };
        let (mut our_end, mut peer_end) = socket_pair().await;
        let peer_addr: SocketAddr = "85.25.13.99:4662".parse().unwrap();
        let req = Ed2kRequestSources::new_v2(file_hash);
        let enc = Ed2kEncoder::default();
        for _ in 0..2 {
            respond_to_sx_request(
                &req,
                true,
                None,
                peer_addr,
                &enc,
                &downloads,
                &mut our_end,
                &env,
                &caps,
                None,
            )
            .await;
        }
        let _ = read_wire(&mut peer_end).await;
        // La 2e requête tombe sous le throttle SOURCECLIENTREASKS.
        let mut tmp = [0u8; 512];
        let res = tokio::time::timeout(
            Duration::from_millis(300),
            peer_end.read(&mut tmp),
        )
        .await;
        assert!(res.is_err(), "second SX request should have been rate-limited");
    }

    #[tokio::test]
    async fn respond_silent_for_unknown_file() {
        let file_hash = [0x45; 16];
        let (_tmp, downloads, _hex) = downloads_with_shared(file_hash).await;
        let env = test_env();
        let caps = PeerSxCaps {
            sx1_version: 3,
            sx2: true,
            complete_source: false,
        };
        let (mut our_end, mut peer_end) = socket_pair().await;
        // Hash inconnu : eMule ne répond pas (`ListenSocket.cpp:1192-1199`).
        let req = Ed2kRequestSources::new_v2([0x99; 16]);
        respond_to_sx_request(
            &req,
            true,
            None,
            "85.25.13.99:4662".parse().unwrap(),
            &Ed2kEncoder::default(),
            &downloads,
            &mut our_end,
            &env,
            &caps,
            None,
        )
        .await;
        let mut tmp = [0u8; 512];
        let res = tokio::time::timeout(
            Duration::from_millis(300),
            peer_end.read(&mut tmp),
        )
        .await;
        assert!(res.is_err(), "unknown file must not produce an SX answer");
        // Et `answer_log` ne doit pas avoir été marqué (throttle non consommé).
        assert!(env
            .answer_log
            .lock()
            .unwrap()
            .get(&ip4([85, 25, 13, 99]))
            .is_none());
    }

    #[tokio::test]
    async fn respond_silent_when_no_source_to_offer() {
        let file_hash = [0x46; 16];
        let (_tmp, downloads, hex_hash) = downloads_with_shared(file_hash).await;
        // Aucune source valide : retirer celles insérées par le helper.
        downloads
            .lock()
            .await
            .clear_peer_sources(&hex_hash)
            .unwrap();
        let env = test_env();
        let caps = PeerSxCaps {
            sx1_version: 3,
            sx2: true,
            complete_source: false,
        };
        let (mut our_end, mut peer_end) = socket_pair().await;
        let req = Ed2kRequestSources::new_v2(file_hash);
        respond_to_sx_request(
            &req,
            true,
            None,
            "85.25.13.99:4662".parse().unwrap(),
            &Ed2kEncoder::default(),
            &downloads,
            &mut our_end,
            &env,
            &caps,
            None,
        )
        .await;
        let mut tmp = [0u8; 512];
        let res = tokio::time::timeout(
            Duration::from_millis(300),
            peer_end.read(&mut tmp),
        )
        .await;
        assert!(res.is_err(), "empty answer must not be sent (eMule !nCount)");
    }
}
