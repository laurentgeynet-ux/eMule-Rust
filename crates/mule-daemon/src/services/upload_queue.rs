// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use crate::services::peer_session::log_addr_str;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use crate::services::DaemonEvent;

/// Maximum number of queued upload requests.
/// eMule 0.72a `QueueSize` default (100 × QueueSizePref 50).
pub const MAX_UPLOAD_QUEUE: usize = 5000;
/// Maximum number of concurrent active uploads.
/// eMule 0.72a `MAX_UP_CLIENTS_ALLOWED` (Opcodes.h) — plafond absolu de
/// slots upload, quels que soient les autres réglages.
pub const MAX_ACTIVE_UPLOADS: usize = 100;
/// eMule `AcceptNewClient` : les `max(MIN_UP_CLIENTS_ALLOWED, 4)` = 4
/// premiers slots sont toujours accordés — le plancher de débit moyen
/// (`min_upload_rate`) ne peut pas geler l'upload quand des clients
/// attendent. Au-delà, chaque nouveau slot exige que la moyenne par slot
/// reste au-dessus du plancher.
const GUARANTEED_UPLOAD_SLOTS: usize = 4;

/// eMule trickle threshold: an active slot whose measured rate stays below
/// 1 KiB/s past the grace period is demoted to a trickle slot — it keeps a
/// residual share of bandwidth but no longer occupies a full upload slot
/// (`CUploadQueue` keeps slow clients alive while freeing the slot).
const TRICKLE_RATE_BPS: u64 = 1024;
/// Grace period before a slow slot may be demoted (60 s).
const TRICKLE_GRACE_MS: i64 = 60_000;

/// eMule `UPLOAD_CLIENT_MAXDATARATE` (`Opcodes.h`) : débit cible maximal
/// par client upload — 25 KiB/s.
const UPLOAD_CLIENT_MAXDATARATE_BPS: u64 = 25 * 1024;
/// eMule `MAX_PURGEQUEUETIME` (`Opcodes.h`) : une entrée de file dont le
/// dernier re-ask date de plus d'1 h est purgée
/// (`CUploadQueue::FindBestClientInQueue`).
const MAX_PURGEQUEUE_MS: i64 = 60 * 60 * 1000;
/// eMule `SESSIONMAXTRANS` (`Opcodes.h`) = `PARTSIZE + 20*1024` : charge
/// utile maximale servie par session quand `FullChunkTransfers` est actif
/// (défaut eMule = `true`). Au-delà, le client est renvoyé en file si un
/// nouveau slot ne peut pas être ouvert (`!ForceNewClient()`).
const SESSION_MAX_TRANS_BYTES: u64 = 9_728_000 + 20 * 1024;
/// eMule `MIN_REQUESTTIME` (`Opcodes.h`) : un re-ask du même fichier avant
/// 10 min compte comme « bad request » ; `BADCLIENTBAN` (4) cumulés → ban.
const MIN_REQUEST_TIME_MS: i64 = 10 * 60 * 1000;
const BAD_CLIENT_BAN: u8 = 4;
/// eMule `AddClientToQueue` (`UploadQueue.cpp:586-596`) : au plus 3
/// clients depuis la même IP en file d'attente — file + slots actifs
/// combinés (la `TrackedClientsList` eMule couvre les deux côtés).
const MAX_SAME_IP_QUEUED: usize = 3;

/// Fenêtre d'échantillonnage du débit upload (~2 s, cadence du statsTick) :
/// `upload_rate_bps` est recalculé à chaque fenêtre échue — même approche
/// que les débits de download mesurés par tranches de 500 ms dans
/// `peer_session/transfer.rs`.
const RATE_WINDOW_MS: i64 = 2_000;
/// Premier échantillon rapide (500 ms) : une session qui vient d'ouvrir sa
/// fenêtre n'attend pas 2 s avant d'afficher un débit réel.
const RATE_FIRST_SAMPLE_MS: i64 = 500;

/// Sonde du débit upload effectif (`MaxSpeed` eMule) — typiquement une
/// closure lisant `upload_bucket.rate_per_sec()`.
type UploadSpeedProbe = Option<std::sync::Arc<dyn Fn() -> u64 + Send + Sync>>;

/// State of an upload slot or queue entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadSlotState {
    Waiting,
    Uploading,
    /// Slow client kept alive on residual bandwidth; does not count against
    /// `max_active` (eMule "trickle slot").
    Trickle,
    Completed,
    Rejected,
    Aborted,
}

/// Snapshot of an upload session / slot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UploadSessionSnapshot {
    /// Identity key used for dedup: `uh:<userhash>` when the peer announced
    /// a user hash in OP_HELLO, otherwise its `ip:port` address.
    pub client_key: String,
    pub client_address: String,
    pub file_hash: String,
    pub file_name: String,
    pub transferred_bytes: u64,
    pub upload_rate_bps: u64,
    pub state: UploadSlotState,
    pub queue_rank: Option<u32>,
    pub started_at_unix_ms: i64,
    /// Début de la fenêtre de mesure courante (ms Unix, 0 = pas de mesure).
    /// État interne du `UploadManager` — non sérialisé.
    #[serde(skip)]
    pub rate_window_start_ms: i64,
    /// Octets envoyés dans la fenêtre courante. Non sérialisé.
    #[serde(skip)]
    pub rate_window_bytes: u64,
    /// `true` dès qu'une fenêtre de mesure a été refermée — requis pour la
    /// rétrogradation `Trickle` (un débit jamais mesuré n'est pas « lent »).
    /// Non sérialisé.
    #[serde(skip)]
    pub rate_sampled: bool,
    /// Multiplicateur de crédits ×100 au moment de l'admission — conservé
    /// pour reconstruire l'entrée de file lors d'une rotation de session
    /// (eMule préserve le score via `credits`). Non sérialisé.
    #[serde(skip)]
    pub credit_multiplier_bps: u32,
    /// Poids de priorité du fichier servi (`GetFilePrioAsNumber`) — même
    /// usage que `credit_multiplier_bps`. Non sérialisé.
    #[serde(skip)]
    pub file_prio_num: u32,
    /// Début d'attente d'origine (eMule `credits->GetSecWaitStartTime`) :
    /// `enqueued_at` de l'entrée promue, ou l'heure du grant direct —
    /// préservé pour qu'un client renvoyé en file retrouve son score
    /// (rotation de session). Non sérialisé.
    #[serde(skip)]
    pub queued_since_ms: i64,
    /// `true` quand le fichier servi est en priorité upload « auto »
    /// (eMule `m_bAutoUpPriority`). Non sérialisé.
    #[serde(skip)]
    pub auto_prio: bool,
}

impl UploadSessionSnapshot {
    /// Débit effectif à `now_ms` : le dernier débit échantillonné tant que
    /// la fenêtre courante reste fraîche ; au-delà de `RATE_WINDOW_MS` sans
    /// nouvel envoi, les octets résiduels de la fenêtre sont rapportés au
    /// temps écoulé — le débit décroît vers 0 quand le pair ne demande plus
    /// rien (même convention que `decay_stale_rates` côté downloads).
    pub fn effective_rate_bps(&self, now_ms: i64) -> u64 {
        let elapsed = now_ms.saturating_sub(self.rate_window_start_ms);
        if self.rate_window_start_ms == 0 || elapsed < RATE_WINDOW_MS {
            self.upload_rate_bps
        } else {
            self.rate_window_bytes.saturating_mul(1000) / elapsed as u64
        }
    }
}

/// Résultat du lookup `OP_REASKFILEPING` dans la file d'attente —
/// eMule `GetWaitingClientByIP_UDP` + contrôle `GetUploadFileID`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UdpRankLookup {
    /// Client identifié, hash conforme : répondre `OP_REASKACK` + rang.
    Rank(u32),
    /// Plusieurs clients en file partagent cette IP sans correspondance
    /// de port UDP — eMule ne répond pas (force la connexion TCP).
    Ambiguous,
    /// Client identifié mais le hash demandé ≠ son fichier en attente —
    /// eMule journalise l'erreur et ne répond pas.
    FileMismatch,
    /// Aucun client en file à cette IP — eMule répond `OP_QUEUEFULL`
    /// seulement si la file approche la saturation.
    NoSender,
}

/// Entry waiting in the upload queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuedUpload {
    /// Identity key (`uh:<userhash>` or `ip:port`) used for dedup/promotion.
    pub client_key: String,
    pub client_address: String,
    pub file_hash: String,
    pub file_name: String,
    pub enqueued_at_unix_ms: i64,
    pub credit_multiplier_bps: u32, // Multiplier * 100 (e.g. 100 for 1.0x, 1000 for 10.0x)
    /// eMule `GetFilePrioAsNumber` of the requested shared file:
    /// veryhigh=18, high=9, normal=7, low=6, verylow=2.
    #[serde(default = "default_file_prio_num")]
    pub file_prio_num: u32,
    /// eMule `IS_IDBADGUY`/`IsBanned` semantics: score is always 0, the entry
    /// waits forever at the tail (never promoted, still listed).
    #[serde(default)]
    pub banned: bool,
    /// Horodatage du dernier re-ask du client (`SetLastUpRequest` eMule) —
    /// sert à la purge `MAX_PURGEQUEUETIME` (1 h).
    #[serde(default)]
    pub last_request_unix_ms: i64,
    /// Compteur « bad requests » eMule (`Requested_File_Struct::badrequests`)
    /// : re-ask du même fichier avant `MIN_REQUESTTIME` (10 min) → ++ ;
    /// `BAD_CLIENT_BAN` atteint → `banned`.
    #[serde(default)]
    pub bad_requests: u8,
    /// Signal de promotion : le dispatcher qui a reçu `OP_QUEUERANKING`
    /// attend dessus pour pousser `OP_ACCEPTUPLOADREQ` dès qu'un slot se
    /// libère (eMule `AddUpNextClient` écrit directement sur le socket en
    /// attente — sans ça, le client ne serait servi qu'à son prochain
    /// re-ask, ~30 min).
    #[serde(skip)]
    pub promote_notify: std::sync::Arc<tokio::sync::Notify>,
    /// Port UDP eMule annoncé dans le HELLO du client (`emule_udp_port`)
    /// — eMule `GetUDPPort()` : permet à `GetWaitingClientByIP_UDP` de
    /// distinguer deux clients derrière la même IP (NAT) lors d'un
    /// `OP_REASKFILEPING`.
    #[serde(default)]
    pub client_udp_port: Option<u16>,
    /// `true` quand le fichier demandé a `up_priority == "auto"` (eMule
    /// `m_bAutoUpPriority`) — `file_prio_num` est alors rafraîchi depuis
    /// `auto_up_prio` à chaque tri de la file, comme eMule lit
    /// `GetFilePrioAsNumber()` en direct dans `GetScore`.
    #[serde(default)]
    pub auto_prio: bool,
}

fn default_file_prio_num() -> u32 {
    7
}

/// Maps an upload priority string to the eMule `GetFilePrioAsNumber` weight
/// (`UploadClient.cpp`): veryhigh=18, high=9, normal=7, low=6, verylow=2.
/// `PR_POWER` (powershare) n'a pas de cas dédié dans eMule → branche
/// `default` = 7, comme `PR_NORMAL` (le powershare joue sur la publication,
/// pas sur le score de file).
/// `"auto"` (eMule `m_bAutoUpPriority`) retombe sur la priorité effective
/// initiale `PR_HIGH` = 9 — le recalcul dynamique
/// (`UpdateAutoUpPriority`) est porté par `UploadManager::auto_up_prio`.
pub fn file_prio_num(priority: &str) -> u32 {
    match priority.to_lowercase().as_str() {
        "veryhigh" => 18,
        "high" => 9,
        "low" => 6,
        "verylow" => 2,
        "auto" => 9,
        _ => 7,
    }
}

impl QueuedUpload {
    /// Computes the eMule waiting score:
    /// `wait_secs * credit_multiplier * (file_prio / 10)`, 0 when banned.
    /// Stored in centi-units (`*_bps` fields keep two decimals).
    pub fn effective_score(&self, now_ms: i64) -> u64 {
        if self.banned {
            return 0;
        }
        let wait_secs = ((now_ms.saturating_sub(self.enqueued_at_unix_ms)) / 1000).max(1) as u64;
        let mult = (self.credit_multiplier_bps as u64).max(100);
        wait_secs * mult * self.file_prio_num as u64 / 1000
    }

    /// eMule `CUpDownClient::GetCombinedFilePrioAndCredit` :
    /// `10 * GetFilePrioAsNumber() * GetScoreRatio()` — utilisé pour
    /// l'admission au-delà de la *soft* queue limit.
    pub fn combined_prio_credit(&self) -> u64 {
        10 * self.file_prio_num as u64 * self.credit_multiplier_bps as u64 / 100
    }
}

/// Fair bounded upload queue and active slot tracker.
/// Supports up to `MAX_ACTIVE_UPLOADS` concurrent upload slots
/// (eMule grants several simultaneous upload slots).
#[derive(Clone)]
pub struct UploadManager {
    queue: Arc<Mutex<VecDeque<QueuedUpload>>>,
    active_uploads: Arc<Mutex<std::collections::HashMap<String, UploadSessionSnapshot>>>,
    active_count: Arc<AtomicUsize>,
    max_active: Arc<AtomicUsize>,
    /// USS `AcceptNewClient` gate — when false, new clients stay queued
    /// while latency is above tolerance (`UssService::accept_flag`).
    uss_gate: Arc<std::sync::RwLock<Arc<AtomicBool>>>,
    /// Optional event channel (set by `AppState`) for `upload.uploadsUpdated`.
    event_tx: Arc<std::sync::RwLock<Option<broadcast::Sender<String>>>>,
    sequence: Arc<std::sync::RwLock<Option<Arc<AtomicU64>>>>,
    /// Throttle for progress-driven emissions (~500 ms).
    last_progress_emit_ms: Arc<AtomicI64>,
    /// Friend client keys (`uh:<userhash>`) — granted a dedicated slot even
    /// when `max_active` is reached (eMule friend slot).
    friends: Arc<std::sync::RwLock<std::collections::HashSet<String>>>,
    /// Maximum waiting-queue length (eMule `QueueSize` pref, default 500).
    max_queue: Arc<AtomicUsize>,
    /// Minimum upload bandwidth per slot in bytes/sec (eMule `MinUpload`):
    /// a new slot is only opened when existing slots still reach it.
    min_upload_rate_bps: Arc<std::sync::atomic::AtomicU64>,
    /// Sonde du débit upload configuré/effectif (`MaxSpeed` eMule) — lit
    /// `upload_bucket.rate_per_sec()` en direct : reflète la limite manuelle
    /// (`rate_limit_upload_bps`) comme la valeur dynamique pilotée par USS
    /// (`IsDynUpEnabled → lastCommonRouteFinder->GetUpload()`). 0 = illimité.
    upload_speed_probe: Arc<std::sync::RwLock<UploadSpeedProbe>>,
    /// Quota journalier d'upload (eMule-Rust `dailyUploadQuotaGb`) — quand
    /// `false`, aucune nouvelle demande de slot n'est acceptée.
    quota_gate: Arc<std::sync::RwLock<Arc<AtomicBool>>>,
    /// Total cumulatif des octets uploadés depuis le démarrage du daemon
    /// (compteur source du quota journalier).
    total_uploaded_bytes: Arc<AtomicU64>,
    /// Totaux uploadés par fichier (hash hex en minuscules) — hydratés
    /// depuis `share_upload_stats` au démarrage puis incrémentés à chaque
    /// bloc envoyé ; persistés par deltas dans la boucle `statsTick`.
    file_upload_totals: Arc<std::sync::RwLock<std::collections::HashMap<String, u64>>>,
    /// Début d'attente préservé pour un client renvoyé de son slot par la
    /// rotation de session (`CheckForTimeOver`) — appliqué comme
    /// `enqueued_at` s'il re-demande (eMule conserve le wait start dans
    /// `credits` ; le score accumulé n'est pas perdu).
    rotation_grace: Arc<Mutex<std::collections::HashMap<String, i64>>>,
    /// Instantané des hash actuellement partagés (minuscules) — rafraîchi
    /// par la boucle de tick via [`Self::set_shared_hashes`]. eMule purge
    /// de la file les clients dont le fichier demandé n'est plus partagé
    /// (`FindBestClientInQueue` → `!sharedfiles->GetFileByID`). `None` =
    /// sonde non câblée → la purge par partage est inactive (tests).
    shared_hashes: Arc<std::sync::RwLock<Option<std::collections::HashSet<String>>>>,
    /// Priorité effective recalculée des fichiers en `up_priority = "auto"`
    /// (hash en minuscules → `GetFilePrioAsNumber`) — registre des
    /// fichiers auto *et* cache du dernier `UpdateAutoUpPriority`.
    /// eMule `CKnownFile::UpdateAutoUpPriority` : plus de 20 uploaders
    /// → low (6), plus de 1 → normal (7), sinon → high (9).
    auto_up_prio: Arc<std::sync::RwLock<std::collections::HashMap<String, u32>>>,
}

impl Default for UploadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UploadManager {
    /// Creates a new `UploadManager`.
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_UPLOAD_QUEUE))),
            active_uploads: Arc::new(Mutex::new(std::collections::HashMap::new())),
            active_count: Arc::new(AtomicUsize::new(0)),
            max_active: Arc::new(AtomicUsize::new(MAX_ACTIVE_UPLOADS)),
            uss_gate: Arc::new(std::sync::RwLock::new(Arc::new(AtomicBool::new(true)))),
            event_tx: Arc::new(std::sync::RwLock::new(None)),
            sequence: Arc::new(std::sync::RwLock::new(None)),
            last_progress_emit_ms: Arc::new(AtomicI64::new(0)),
            friends: Arc::new(std::sync::RwLock::new(std::collections::HashSet::new())),
            max_queue: Arc::new(AtomicUsize::new(MAX_UPLOAD_QUEUE)),
            min_upload_rate_bps: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            upload_speed_probe: Arc::new(std::sync::RwLock::new(None)),
            quota_gate: Arc::new(std::sync::RwLock::new(Arc::new(AtomicBool::new(true)))),
            total_uploaded_bytes: Arc::new(AtomicU64::new(0)),
            file_upload_totals: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            rotation_grace: Arc::new(Mutex::new(std::collections::HashMap::new())),
            shared_hashes: Arc::new(std::sync::RwLock::new(None)),
            auto_up_prio: Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// Remplace l'instantané des hash partagés — appelé périodiquement par
    /// la boucle de tick (eMule lit `sharedfiles` en direct dans
    /// `FindBestClientInQueue`). Passer `Some(set)` active la purge des
    /// entrées de file dont le fichier n'est plus partagé.
    pub fn set_shared_hashes(&self, hashes: std::collections::HashSet<String>) {
        *self.shared_hashes.write().unwrap() = Some(hashes);
    }

    /// `true` quand le hash est absent de l'instantané des partages —
    /// `true` aussi quand la sonde n'est pas câblée (pas de purge dans ce
    /// cas, eMule `sharedfiles` étant la source de vérité).
    fn hash_still_shared(&self, file_hash: &str) -> bool {
        self.shared_hashes
            .read()
            .unwrap()
            .as_ref()
            .is_none_or(|set| set.contains(file_hash))
    }

    /// Hydrate les totaux uploadés par fichier persistés en SQLite
    /// (`share_upload_stats`) — appelé une fois au démarrage.
    pub fn set_file_upload_totals(&self, totals: std::collections::HashMap<String, u64>) {
        *self.file_upload_totals.write().unwrap() = totals;
    }

    /// Copie des totaux uploadés par fichier (hash en minuscules).
    pub fn per_file_uploaded_bytes(&self) -> std::collections::HashMap<String, u64> {
        self.file_upload_totals.read().unwrap().clone()
    }

    /// Débit effectif courant agrégé par fichier (somme des slots actifs
    /// sur le même hash, clés en minuscules) — eMule affiche ce débit dans
    /// la colonne « Upload speed » de la liste des fichiers partagés.
    pub async fn per_file_rates_bps(&self) -> std::collections::HashMap<String, u64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let active = self.active_uploads.lock().await;
        let mut rates = std::collections::HashMap::new();
        for s in active.values() {
            *rates.entry(s.file_hash.to_lowercase()).or_insert(0u64) += s.effective_rate_bps(now);
        }
        rates
    }

    /// Sets the minimum per-slot upload rate (eMule `MinUpload`, B/s).
    /// `0` disables the floor.
    pub fn set_min_upload_rate(&self, bps: u64) {
        self.min_upload_rate_bps
            .store(bps, std::sync::atomic::Ordering::Relaxed);
    }

    /// Installe la sonde du débit upload effectif (`MaxSpeed` eMule) — appelée
    /// par `AppState` avec une closure lisant
    /// `upload_bucket.rate_per_sec()`.
    pub fn set_upload_speed_probe(&self, probe: impl Fn() -> u64 + Send + Sync + 'static) {
        *self.upload_speed_probe.write().unwrap() = Some(std::sync::Arc::new(probe));
    }

    /// Débit upload configuré/effectif en octets/s (0 = illimité ou sonde
    /// absente) — eMule `MaxSpeed` dans `AcceptNewClient`.
    fn max_upload_speed_bps(&self) -> u64 {
        self.upload_speed_probe
            .read()
            .unwrap()
            .as_ref()
            .map(|p| p())
            .unwrap_or(0)
    }

    /// Sets the maximum waiting-queue length (eMule `QueueSize` pref).
    pub fn set_max_queue(&self, max: usize) {
        self.max_queue
            .store(max.clamp(10, 5000), std::sync::atomic::Ordering::Relaxed);
    }

    /// Installs the friend client-key set (`uh:<userhash>` entries).
    pub fn set_friends(&self, friends: std::collections::HashSet<String>) {
        *self.friends.write().unwrap() = friends;
    }

    /// Adds/removes a friend client key at runtime (RPC-driven).
    pub fn set_friend(&self, client_key: &str, is_friend: bool) {
        let mut f = self.friends.write().unwrap();
        if is_friend {
            f.insert(client_key.to_string());
        } else {
            f.remove(client_key);
        }
    }

    /// True when `client_key` belongs to a configured friend.
    fn is_friend(&self, client_key: &str) -> bool {
        self.friends.read().unwrap().contains(client_key)
    }

    /// Installs the broadcast channel used to emit `upload.uploadsUpdated`.
    pub fn set_event_channel(&self, tx: broadcast::Sender<String>, seq: Arc<AtomicU64>) {
        *self.event_tx.write().unwrap() = Some(tx);
        *self.sequence.write().unwrap() = Some(seq);
    }

    /// Emits the full uploads snapshot (active slots + queue + aggregate
    /// rate). No-op when no event channel is installed. `pub` pour la
    /// poussée périodique depuis la boucle `statsTick` (`state.rs`) qui
    /// rafraîchit les débits mesurés entre deux envois de blocs.
    pub async fn emit_uploads_updated(&self) {
        let (tx, seq) = {
            let tx = self.event_tx.read().unwrap().clone();
            let seq = self.sequence.read().unwrap().clone();
            (tx, seq)
        };
        let (Some(tx), Some(seq)) = (tx, seq) else {
            return;
        };
        let active = self.active_snapshots().await;
        let queue = self.list_queue().await;
        let rate = active.iter().map(|s| s.upload_rate_bps).sum();
        DaemonEvent::UploadsUpdated {
            sequence: seq.fetch_add(1, Ordering::SeqCst),
            active,
            queue,
            upload_rate_bps: rate,
        }
        .send(&tx);
    }

    /// Emits `uploadsUpdated` from the hot progress path, throttled to at
    /// most once every 500 ms.
    async fn emit_uploads_updated_throttled(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let last = self.last_progress_emit_ms.load(Ordering::SeqCst);
        if now - last < 500 {
            return;
        }
        if self
            .last_progress_emit_ms
            .compare_exchange(last, now, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.emit_uploads_updated().await;
        }
    }

    /// Installs the USS accept-new-client flag
    /// (`LastCommonRouteFinder::AcceptNewClient` semantics).
    pub fn set_uss_gate(&self, flag: Arc<AtomicBool>) {
        *self.uss_gate.write().unwrap() = flag;
    }

    /// Installe le gate du quota journalier (`dailyUploadQuotaGb`) — `false`
    /// bloque toute nouvelle demande de slot.
    pub fn set_quota_gate(&self, flag: Arc<AtomicBool>) {
        *self.quota_gate.write().unwrap() = flag;
    }

    /// Octets uploadés cumulés depuis le démarrage du daemon.
    pub fn total_uploaded_bytes(&self) -> u64 {
        self.total_uploaded_bytes.load(Ordering::SeqCst)
    }

    /// Overrides the maximum number of concurrent active upload slots.
    pub fn set_max_active(&self, max: usize) {
        self.max_active.store(max.max(1), Ordering::SeqCst);
    }

    /// Number of currently active upload slots.
    pub fn active_len(&self) -> usize {
        self.active_count.load(Ordering::SeqCst)
    }

    /// Requests an upload slot for an inbound client.
    ///
    /// - If a slot is available, immediately grants `UploadSlotState::Uploading`.
    /// - If all slots are occupied, enqueues the client in the
    ///   credit-weighted queue and returns `UploadSlotState::Waiting` + rank.
    /// - Returns an error if queue capacity is exceeded.
    ///
    /// `client_key` is the identity used for dedup/release — `uh:<userhash>`
    /// when the peer announced a user hash, otherwise `client_address`.
    /// `client_address` stays the display/UDP-lookup address (`ip:port`).
    pub async fn request_upload_slot(
        &self,
        client_key: &str,
        client_address: &str,
        file_hash: &str,
        file_name: &str,
        credit_multiplier: f64,
        file_prio: &str,
        banned: bool,
        client_udp_port: Option<u16>,
    ) -> Result<(UploadSlotState, Option<u32>), String> {
        let mut active = self.active_uploads.lock().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let auto_prio = file_prio.eq_ignore_ascii_case("auto");

        // Client already holds a slot — eMule `IsDownloading()` : un client
        // qui télécharge déjà et demande un autre fichier conserve son slot
        // (`OP_ACCEPTUPLOADREQ` + `SetUploadFileID`), il ne rejoint jamais
        // la file.
        if active.contains_key(client_key) {
            let mut switched_from = None;
            if let Some(cur) = active.get_mut(client_key) {
                if cur.file_hash != file_hash {
                    switched_from = Some(cur.file_hash.clone());
                    cur.file_hash = file_hash.to_string();
                    cur.file_name = file_name.to_string();
                    cur.auto_prio = auto_prio;
                }
            }
            if let Some(old_hash) = switched_from {
                let new_num = self.resolve_file_prio_num(&active, file_hash, file_prio);
                if let Some(cur) = active.get_mut(client_key) {
                    cur.file_prio_num = new_num;
                }
                // Le fichier précédent perd un uploader, le nouveau en
                // gagne un — `UpdateAutoUpPriority` des deux côtés.
                self.update_auto_up_prio(&active, &old_hash);
                self.update_auto_up_prio(&active, file_hash);
            }
            return Ok((UploadSlotState::Uploading, None));
        }

        // Quota journalier atteint (`dailyUploadQuotaGb`) : aucune nouvelle
        // demande n'est acceptée jusqu'à la remise à zéro du compteur.
        if !self.quota_gate.read().unwrap().load(Ordering::SeqCst) {
            return Err("Daily upload quota reached".to_string());
        }

        // Grant a slot immediately while capacity remains and USS accepts
        // new clients (acceptNewClient=false when latency is too high).
        // Trickle slots keep residual bandwidth but do not occupy a full slot.
        if self.can_open_slot(&active, now) {
            let snap = UploadSessionSnapshot {
                client_key: client_key.to_string(),
                client_address: client_address.to_string(),
                file_hash: file_hash.to_string(),
                file_name: file_name.to_string(),
                transferred_bytes: 0,
                upload_rate_bps: 0,
                state: UploadSlotState::Uploading,
                queue_rank: None,
                started_at_unix_ms: now,
                rate_window_start_ms: 0,
                rate_window_bytes: 0,
                rate_sampled: false,
                credit_multiplier_bps: (credit_multiplier.clamp(1.0, 10.0) * 100.0) as u32,
                file_prio_num: self.resolve_file_prio_num(&active, file_hash, file_prio),
                queued_since_ms: now,
                auto_prio,
            };
            active.insert(client_key.to_string(), snap);
            // `AddUploadingClient` → `UpdateAutoUpPriority` : le nombre
            // d'uploaders du fichier vient de monter.
            self.update_auto_up_prio(&active, file_hash);
            self.active_count
                .store(Self::uploading_count(&active), Ordering::SeqCst);
            tracing::info!(
                "[UPLOAD_SLOT_GRANTED] Client {} granted active upload slot for file '{}' (hash: {}) ({}/{} slots used)",
                log_addr_str(client_address), file_name, file_hash, Self::uploading_count(&active), self.max_active.load(Ordering::SeqCst)
            );
            drop(active);
            self.emit_uploads_updated().await;
            return Ok((UploadSlotState::Uploading, None));
        }

        // eMule friend slot: a friend bypasses the full queue and receives a
        // dedicated extra slot beyond `max_active`.
        if self.is_friend(client_key) {
            let snap = UploadSessionSnapshot {
                client_key: client_key.to_string(),
                client_address: client_address.to_string(),
                file_hash: file_hash.to_string(),
                file_name: file_name.to_string(),
                transferred_bytes: 0,
                upload_rate_bps: 0,
                state: UploadSlotState::Uploading,
                queue_rank: None,
                started_at_unix_ms: now,
                rate_window_start_ms: 0,
                rate_window_bytes: 0,
                rate_sampled: false,
                credit_multiplier_bps: (credit_multiplier.clamp(1.0, 10.0) * 100.0) as u32,
                file_prio_num: self.resolve_file_prio_num(&active, file_hash, file_prio),
                queued_since_ms: now,
                auto_prio,
            };
            active.insert(client_key.to_string(), snap);
            self.update_auto_up_prio(&active, file_hash);
            tracing::info!(
                "[UPLOAD_FRIEND_SLOT] Friend {} granted dedicated upload slot for file '{}' ({} full slots)",
                log_addr_str(client_address), file_name, Self::uploading_count(&active)
            );
            drop(active);
            self.emit_uploads_updated().await;
            return Ok((UploadSlotState::Uploading, None));
        }

        // Un client renvoyé de son slot par la rotation de session
        // retrouve son wait start d'origine (eMule `credits` le conserve) :
        // son score accumulé n'est pas perdu — il revient près du sommet.
        let rotation_grace = self.rotation_grace.lock().await.remove(client_key);

        // All active slots occupied: enqueue if space available
        let mut q = self.queue.lock().await;
        let mult_bps = (credit_multiplier.clamp(1.0, 10.0) * 100.0) as u32;

        // Priorité repassée en manuel : le fichier sort du registre auto
        // et les entrées qui le demandent reprennent le poids explicite.
        if !auto_prio {
            self.clear_auto_file(&mut q, file_hash, file_prio_num(file_prio));
        }

        // eMule : un seul objet client = une seule entrée de file, quel que
        // soit le fichier demandé. Le re-ask rafraîchit `last_request`,
        // met à jour le fichier demandé et renvoie le rang courant —
        // et nourrit le compteur anti-abus (`MIN_REQUESTTIME`/`BADCLIENTBAN`).
        if let Some(pos) = q.iter().position(|entry| entry.client_key == client_key) {
            let entry = &mut q[pos];
            if entry.file_hash == file_hash
                && now.saturating_sub(entry.last_request_unix_ms) < MIN_REQUEST_TIME_MS
            {
                entry.bad_requests = entry.bad_requests.saturating_add(1);
                if entry.bad_requests >= BAD_CLIENT_BAN {
                    entry.banned = true;
                }
            } else {
                entry.bad_requests = entry.bad_requests.saturating_sub(1);
            }
            entry.last_request_unix_ms = now;
            entry.file_hash = file_hash.to_string();
            entry.file_name = file_name.to_string();
            entry.file_prio_num = self.resolve_file_prio_num(&active, file_hash, file_prio);
            entry.auto_prio = auto_prio;
            entry.credit_multiplier_bps = mult_bps;
            entry.banned |= banned;
            // Le score évolue avec le temps — recalculer l'ordre avant de
            // rapporter le rang (eMule évalue `GetScore` à la demande).
            self.sort_queue_by_score(&mut q, now);
            let rank = q
                .iter()
                .position(|e| e.client_key == client_key)
                .map(|p| (p + 1) as u32)
                .unwrap_or(q.len() as u32);
            drop(q);
            drop(active);
            self.emit_uploads_updated_throttled().await;
            return Ok((UploadSlotState::Waiting, Some(rank)));
        }

        // eMule `AddClientToQueue` — la limite de file est *souple* :
        // `hard = soft + max(soft, 800)/4`. Entre soft et hard, seuls les
        // clients à slot ami ou dont `combined(prio × crédits)` dépasse la
        // moyenne de la file sont admis.
        let soft_limit = self.max_queue.load(std::sync::atomic::Ordering::Relaxed);
        let hard_limit = soft_limit + soft_limit.max(800) / 4;
        if q.len() >= hard_limit {
            return Err(format!("Upload queue full ({hard_limit} max)"));
        }
        if q.len() >= soft_limit && !self.is_friend(client_key) {
            let avg: u64 = q.iter().map(|e| e.combined_prio_credit()).sum::<u64>()
                / q.len().max(1) as u64;
            let mine = 10
                * self.resolve_file_prio_num(&active, file_hash, file_prio) as u64
                * mult_bps as u64
                / 100;
            if mine < avg {
                return Err("Upload queue soft limit reached".to_string());
            }
        }

        // eMule `AddClientToQueue` : refuse l'entrée quand 3 clients de
        // la même IP sont déjà en file ou actifs (`cSameIP >= 3` +
        // `GetClientsFromIP >= 3`). L'IP est extraite de `client_address`
        // via `rsplit_once(':')` pour supporter les adresses IPv6
        // bracketées.
        let req_ip = client_address.rsplit_once(':').map(|(h, _)| h).unwrap_or("");
        if !req_ip.is_empty() {
            let same_ip = q
                .iter()
                .filter(|e| e.client_address.rsplit_once(':').is_some_and(|(h, _)| h == req_ip))
                .count()
                + active
                    .values()
                    .filter(|s| s.client_address.rsplit_once(':').is_some_and(|(h, _)| h == req_ip))
                    .count();
            if same_ip >= MAX_SAME_IP_QUEUED {
                return Err("Too many clients from the same IP".to_string());
            }
        }

        let enqueued_at = rotation_grace.unwrap_or(now);

        q.push_back(QueuedUpload {
            client_key: client_key.to_string(),
            client_address: client_address.to_string(),
            file_hash: file_hash.to_string(),
            file_name: file_name.to_string(),
            enqueued_at_unix_ms: enqueued_at,
            credit_multiplier_bps: mult_bps,
            file_prio_num: self.resolve_file_prio_num(&active, file_hash, file_prio),
            banned,
            last_request_unix_ms: now,
            bad_requests: 0,
            promote_notify: std::sync::Arc::new(tokio::sync::Notify::new()),
            client_udp_port,
            auto_prio,
        });

        // Sort queue by effective score descending (eMule ranks clients by
        // live `GetScore`, pas par ordre d'arrivée).
        self.sort_queue_by_score(&mut q, now);

        let rank = q
            .iter()
            .position(|entry| entry.client_key == client_key)
            .map(|pos| (pos + 1) as u32)
            .unwrap_or(q.len() as u32);

        tracing::info!(
            "[UPLOAD_QUEUED] Client {} queued for file '{}' (hash: {}) with rank #{} (credits mult: {:.2}x, queue size: {})",
            log_addr_str(client_address), file_name, file_hash, rank, credit_multiplier, q.len()
        );
        drop(q);
        drop(active);
        self.emit_uploads_updated().await;

        Ok((UploadSlotState::Waiting, Some(rank)))
    }

    /// Number of slots currently occupying a full upload slot — trickle
    /// slots are excluded (eMule counts only full-speed uploads).
    fn uploading_count(active: &std::collections::HashMap<String, UploadSessionSnapshot>) -> usize {
        active
            .values()
            .filter(|s| s.state == UploadSlotState::Uploading)
            .count()
    }

    /// eMule `CKnownFile::UpdateAutoUpPriority` : priorité effective d'un
    /// fichier « auto » selon le nombre de clients qui l'uploadent
    /// (`GetQueuedCount` — sessions Uploading + Trickle) : plus de 20
    /// → low (6), plus de 1 → normal (7), sinon → high (9).
    fn compute_auto_up_prio(
        active: &std::collections::HashMap<String, UploadSessionSnapshot>,
        lower_hash: &str,
    ) -> u32 {
        let count = active
            .values()
            .filter(|s| {
                s.file_hash.eq_ignore_ascii_case(lower_hash)
                    && matches!(s.state, UploadSlotState::Uploading | UploadSlotState::Trickle)
            })
            .count();
        if count > 20 {
            6
        } else if count > 1 {
            7
        } else {
            9
        }
    }

    /// Priorité effective d'un fichier « auto » : la première demande
    /// enregistre le fichier dans `auto_up_prio` avec la valeur calculée
    /// sur l'état courant (eMule démarre à `PR_HIGH` puis recalcule à
    /// chaque ajout/retrait d'uploader).
    fn auto_up_prio_for(
        &self,
        active: &std::collections::HashMap<String, UploadSessionSnapshot>,
        file_hash: &str,
    ) -> u32 {
        let key = file_hash.to_lowercase();
        if let Some(&v) = self.auto_up_prio.read().unwrap().get(&key) {
            return v;
        }
        let v = Self::compute_auto_up_prio(active, &key);
        self.auto_up_prio.write().unwrap().insert(key, v);
        v
    }

    /// Recalcule la priorité effective d'un fichier auto après une mutation
    /// des sessions actives — no-op si le fichier n'est pas en mode auto
    /// (absent du registre `auto_up_prio`).
    fn update_auto_up_prio(
        &self,
        active: &std::collections::HashMap<String, UploadSessionSnapshot>,
        file_hash: &str,
    ) {
        let key = file_hash.to_lowercase();
        if !self.auto_up_prio.read().unwrap().contains_key(&key) {
            return;
        }
        let v = Self::compute_auto_up_prio(active, &key);
        self.auto_up_prio.write().unwrap().insert(key, v);
    }

    /// Le fichier repasse en priorité manuelle : sort du registre auto et
    /// réécrit `file_prio_num`/`auto_prio` des entrées de file concernées
    /// (la priorité fichier est une propriété du fichier partagé, commune
    /// à tous les clients qui le demandent).
    fn clear_auto_file(&self, q: &mut VecDeque<QueuedUpload>, file_hash: &str, manual_num: u32) {
        let key = file_hash.to_lowercase();
        if self.auto_up_prio.write().unwrap().remove(&key).is_none()
            && !q.iter().any(|e| e.auto_prio && e.file_hash.eq_ignore_ascii_case(file_hash))
        {
            return;
        }
        for e in q.iter_mut() {
            if e.auto_prio && e.file_hash.eq_ignore_ascii_case(file_hash) {
                e.auto_prio = false;
                e.file_prio_num = manual_num;
            }
        }
    }

    /// `GetFilePrioAsNumber` effectif : `"auto"` lit la valeur recalculée
    /// (`auto_up_prio`), les autres priorités passent par le mapping
    /// statique eMule.
    fn resolve_file_prio_num(
        &self,
        active: &std::collections::HashMap<String, UploadSessionSnapshot>,
        file_hash: &str,
        prio_str: &str,
    ) -> u32 {
        if prio_str.eq_ignore_ascii_case("auto") {
            self.auto_up_prio_for(active, file_hash)
        } else {
            file_prio_num(prio_str)
        }
    }

    /// Trie la file par `effective_score` décroissant — eMule compare les
    /// scores **à la demande** (`GetQueueRank`, `FindBestClientInQueue`) ;
    /// le tri matérialise le même ordre pour `pop_front`/rang affiché.
    /// Les entrées « auto » rafraîchissent d'abord `file_prio_num` depuis
    /// `auto_up_prio`, comme eMule lit `GetFilePrioAsNumber()` en direct.
    fn sort_queue_by_score(&self, q: &mut VecDeque<QueuedUpload>, now_ms: i64) {
        {
            let auto_map = self.auto_up_prio.read().unwrap();
            for e in q.iter_mut() {
                if e.auto_prio {
                    if let Some(&v) = auto_map.get(&e.file_hash.to_lowercase()) {
                        e.file_prio_num = v;
                    }
                }
            }
        }
        let mut entries: Vec<QueuedUpload> = q.drain(..).collect();
        entries.sort_by_key(|a| std::cmp::Reverse(a.effective_score(now_ms)));
        q.extend(entries);
    }

    /// eMule `CUploadQueue::GetTargetClientDataRate` : ≤3 slots → 3 KiB/s,
    /// sinon `slots × 1 KiB/s` plafonné à `UPLOAD_CLIENT_MAXDATARATE`
    /// (25 KiB/s). La variante `min_datarate` retourne ×3/4.
    fn target_client_data_rate_bps(open_slots: usize, min_datarate: bool) -> u64 {
        let r = if open_slots <= 3 {
            3 * 1024
        } else {
            (open_slots as u64 * 1024).min(UPLOAD_CLIENT_MAXDATARATE_BPS)
        };
        if min_datarate {
            r * 3 / 4
        } else {
            r
        }
    }

    /// eMule `CUploadQueue::AcceptNewClient` : ouvre un slot si
    /// - le veto USS est levé (`lastCommonRouteFinder->AcceptNewClient`),
    /// - `uploading < max(MIN_UP_CLIENTS_ALLOWED, 4)` → toujours,
    /// - `uploading >= MAX_UP_CLIENTS_ALLOWED` → jamais,
    /// - `uploading < datarate / minTargetRate` : le débit mesuré nourrit
    ///   encore chaque client au plancher,
    /// - `uploading < MaxSpeed / targetRate` : la capacité configurée
    ///   (ou mesurée par USS) permet un slot de plus au débit cible.
    ///
    /// Partagé par `request_upload` (nouveau client), `promote_waiting`
    /// (promotion) et `process_tick` (rotation) pour éviter la divergence.
    fn can_open_slot(
        &self,
        active: &std::collections::HashMap<String, UploadSessionSnapshot>,
        now: i64,
    ) -> bool {
        if !self.uss_gate.read().unwrap().load(Ordering::SeqCst) {
            return false;
        }
        let uploading = Self::uploading_count(active);
        // `MAX_UP_CLIENTS_ALLOWED` d'abord : eMule teste le garanti avant
        // le plafond, mais y le plafond (100) dépasse toujours le garanti
        // (4) — ici `max_active` est réglable et peut passer sous 4.
        if uploading >= self.max_active.load(Ordering::SeqCst) {
            return false;
        }
        if uploading < GUARANTEED_UPLOAD_SLOTS {
            return true;
        }
        let target = Self::target_client_data_rate_bps(uploading, false).max(1);
        let min_target = Self::target_client_data_rate_bps(uploading, true).max(1);
        let total_bps: u64 = active
            .values()
            .filter(|s| s.state == UploadSlotState::Uploading)
            .map(|s| s.effective_rate_bps(now))
            .sum();
        // `curUploadSlots >= datarate / minTarget` → refus : chaque client
        // existant doit conserver au moins le débit plancher.
        if uploading as u64 >= total_bps / min_target {
            return false;
        }
        // `curUploadSlots >= MaxSpeed*1024 / target` → refus : la capacité
        // upload configurée ne permet pas un slot supplémentaire au débit
        // cible. 0 = illimité → seul le terme datarate borne la croissance
        // (eMule ajoute un plafond « graph rate » interne non modélisé ici).
        let max_speed = self.max_upload_speed_bps();
        if max_speed > 0 && uploading as u64 >= max_speed / target {
            return false;
        }
        // Plancher optionnel `min_upload_kbs` (eMule `MinUpload` nourrit USS,
        // pas `AcceptNewClient` — conservé comme garde utilisateur
        // supplémentaire, inactif par défaut).
        let min_rate = self.min_upload_rate_bps.load(Ordering::Relaxed);
        if min_rate > 0 && total_bps / (uploading.max(1) as u64) < min_rate {
            return false;
        }
        true
    }

    /// Promotes waiting clients while full slots remain. Caller must hold
    /// the `active_uploads` lock and pass it in.
    async fn promote_waiting(
        &self,
        active: &mut std::collections::HashMap<String, UploadSessionSnapshot>,
    ) {
        let mut q = self.queue.lock().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        // Purge des entrées mortes — eMule `FindBestClientInQueue` retire
        // les clients dont le dernier re-ask date de plus de
        // `MAX_PURGEQUEUETIME` (1 h) ou dont le fichier demandé n'est
        // plus partagé (`!sharedfiles->GetFileByID`).
        q.retain(|e| {
            now.saturating_sub(e.last_request_unix_ms) <= MAX_PURGEQUEUE_MS
                && self.hash_still_shared(&e.file_hash)
        });
        // Le score évolue avec l'attente : re-trier avant de choisir le
        // meilleur (eMule lit `GetScore` en direct).
        self.sort_queue_by_score(&mut q, now);
        self.promote_locked(&mut q, active, now);
        self.active_count
            .store(Self::uploading_count(active), Ordering::SeqCst);
    }

    /// Corps de `promote_waiting` sur une file déjà verrouillée — partagé
    /// avec `process_tick` qui promeut après chaque éviction de session.
    /// Retourne `true` si au moins un client a été promu.
    fn promote_locked(
        &self,
        q: &mut VecDeque<QueuedUpload>,
        active: &mut std::collections::HashMap<String, UploadSessionSnapshot>,
        now: i64,
    ) -> bool {
        let mut promoted = false;
        loop {
            // Même éligibilité qu'`AcceptNewClient` : débit cible par
            // client, capacité `MaxSpeed` et veto USS s'appliquent aussi
            // à la promotion.
            if !self.can_open_slot(active, now) {
                break;
            }
            // Un client banni (score 0, re-asks abusifs ou SUI rejeté)
            // n'est jamais promu — eMule `GetScore` = 0 l'empêche de
            // gagner `FindBestClientInQueue`. La file est triée par score
            // décroissant : un banni en tête signifie qu'il ne reste que
            // des bannis.
            if q.front().is_some_and(|e| e.banned) {
                break;
            }
            let Some(next) = q.pop_front() else {
                break;
            };
            if active.contains_key(&next.client_key) {
                continue;
            }

            let snap = UploadSessionSnapshot {
                client_key: next.client_key.clone(),
                client_address: next.client_address.clone(),
                file_hash: next.file_hash.clone(),
                file_name: next.file_name.clone(),
                transferred_bytes: 0,
                upload_rate_bps: 0,
                state: UploadSlotState::Uploading,
                queue_rank: None,
                started_at_unix_ms: now,
                rate_window_start_ms: 0,
                rate_window_bytes: 0,
                rate_sampled: false,
                credit_multiplier_bps: next.credit_multiplier_bps,
                file_prio_num: next.file_prio_num,
                // Le wait start survit à la promotion — eMule le conserve
                // dans `credits` ; il resservira si le client est renvoyé
                // en file par la rotation de session.
                queued_since_ms: next.enqueued_at_unix_ms,
                auto_prio: next.auto_prio,
            };
            let promoted_hash = snap.file_hash.clone();
            tracing::info!(
                "[UPLOAD_PROMOTED] Client {} promoted from queue to active upload slot for file '{}' (hash: {})",
                log_addr_str(&snap.client_address), snap.file_name, snap.file_hash
            );
            active.insert(next.client_key, snap);
            // `AddUploadingClient` → `UpdateAutoUpPriority`.
            self.update_auto_up_prio(active, &promoted_hash);
            // eMule `AddUpNextClient` pousse OP_ACCEPTUPLOADREQ sur le
            // socket en attente — le dispatcher réveillé l'émet.
            next.promote_notify.notify_one();
            promoted = true;
        }
        promoted
    }

    /// Updates active upload progress and measures the slot's real rate.
    ///
    /// Le débit est mesuré en interne par fenêtres (`RATE_WINDOW_MS`,
    /// premier échantillon à `RATE_FIRST_SAMPLE_MS`) à partir des octets
    /// réellement envoyés — l'appelant ne fournit que `bytes_sent`, jamais
    /// un débit estimé. `effective_rate_bps` décroît vers 0 quand le pair
    /// cesse de demander des blocs.
    ///
    /// Slot focusing (eMule semantics): an active slot whose measured rate
    /// stays under `TRICKLE_RATE_BPS` past `TRICKLE_GRACE_MS` is demoted to
    /// `Trickle` — it keeps residual bandwidth but frees its slot so the
    /// best queued client is promoted. This concentrates bandwidth on fast
    /// clients.
    pub async fn update_progress(&self, client_key: &str, bytes_sent: u64) {
        let mut demoted = false;
        {
            let mut active = self.active_uploads.lock().await;
            if let Some(cur) = active.get_mut(client_key) {
                cur.transferred_bytes = cur.transferred_bytes.saturating_add(bytes_sent);
                self.total_uploaded_bytes
                    .fetch_add(bytes_sent, Ordering::SeqCst);
                *self
                    .file_upload_totals
                    .write()
                    .unwrap()
                    .entry(cur.file_hash.to_lowercase())
                    .or_default() += bytes_sent;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                if cur.rate_window_start_ms == 0 {
                    cur.rate_window_start_ms = now;
                }
                cur.rate_window_bytes = cur.rate_window_bytes.saturating_add(bytes_sent);
                let elapsed = now.saturating_sub(cur.rate_window_start_ms);
                // Referme la fenêtre échue — ou dès 500 ms pour le premier
                // échantillon — et mesure le débit réellement desservi.
                if elapsed >= RATE_WINDOW_MS
                    || (!cur.rate_sampled && elapsed >= RATE_FIRST_SAMPLE_MS)
                {
                    cur.upload_rate_bps =
                        cur.rate_window_bytes.saturating_mul(1000) / elapsed as u64;
                    cur.rate_window_start_ms = now;
                    cur.rate_window_bytes = 0;
                    cur.rate_sampled = true;
                }
                if cur.state == UploadSlotState::Uploading
                    && cur.rate_sampled
                    && cur.upload_rate_bps < TRICKLE_RATE_BPS
                    && now.saturating_sub(cur.started_at_unix_ms) > TRICKLE_GRACE_MS
                {
                    cur.state = UploadSlotState::Trickle;
                    demoted = true;
                    tracing::info!(
                        "[UPLOAD_TRICKLE] Client {} demoted to trickle slot (rate {} B/s < {} B/s) — slot freed for queue",
                        cur.client_address, cur.upload_rate_bps, TRICKLE_RATE_BPS
                    );
                }
            }
            if demoted {
                self.promote_waiting(&mut active).await;
            }
        }
        if demoted {
            self.emit_uploads_updated().await;
        } else {
            self.emit_uploads_updated_throttled().await;
        }
    }

    /// Releases the client's active upload slot and promotes waiting clients
    /// until all slots are filled again. Accepts either the `client_key`
    /// (`uh:<userhash>` or `ip:port`) or a bare `ip:port` address — the
    /// latter resolves through the stored `client_address` so RPC callers
    /// that only know the address keep working.
    pub async fn release_slot(&self, client_key: &str) {
        let mut active = self.active_uploads.lock().await;
        let resolved_key = if active.contains_key(client_key) {
            client_key.to_string()
        } else {
            active
                .iter()
                .find(|(_, s)| s.client_address == client_key)
                .map(|(k, _)| k.clone())
                .unwrap_or_else(|| client_key.to_string())
        };
        if let Some(cur) = active.remove(&resolved_key) {
            tracing::info!(
                "[UPLOAD_RELEASED] Active upload slot released by client {} for file '{}' (transferred: {} bytes)",
                resolved_key, cur.file_name, cur.transferred_bytes
            );
            // `RemoveUploadingClient` → `UpdateAutoUpPriority` : le fichier
            // vient de perdre un uploader.
            self.update_auto_up_prio(&active, &cur.file_hash);
        }

        // Also remove from waiting queue if queued
        {
            let mut q = self.queue.lock().await;
            q.retain(|entry| {
                entry.client_key != resolved_key && entry.client_address != client_key
            });
        }
        // Et le wait start préservé par la rotation — le client ne reviendra
        // pas le réclamer.
        self.rotation_grace.lock().await.remove(&resolved_key);

        // Promote from queue while free slots remain
        self.promote_waiting(&mut active).await;
        drop(active);
        self.emit_uploads_updated().await;
    }

    /// `OP_END_OF_DOWNLOAD` (`0x49`) — eMule `ListenSocket.cpp:537` : le
    /// downloader signale qu'il a terminé le fichier. Si `file_hash`
    /// correspond au fichier actuellement uploadé ou demandé par ce client,
    /// celui-ci est retiré du slot actif / de la file (`RemoveFromUploadQueue`).
    /// Retourne `true` si le client a été retiré ; `false` quand le hash ne
    /// correspond à aucun fichier servi pour ce client (l'appelant applique
    /// alors `CheckFailedFileIdReqs`).
    pub async fn end_of_download(&self, client_key: &str, file_hash: &str) -> bool {
        let hash_matches = {
            let active = self.active_uploads.lock().await;
            let in_active = active.values().any(|s| {
                (s.client_key == client_key || s.client_address == client_key)
                    && s.file_hash.eq_ignore_ascii_case(file_hash)
            });
            let q = self.queue.lock().await;
            let in_queue = q.iter().any(|e| {
                (e.client_key == client_key || e.client_address == client_key)
                    && e.file_hash.eq_ignore_ascii_case(file_hash)
            });
            in_active || in_queue
        };
        if hash_matches {
            self.release_slot(client_key).await;
        }
        hash_matches
    }

    /// Handle de promotion de l'entrée en file de `client_key` — le
    /// dispatcher l'attend pour pousser `OP_ACCEPTUPLOADREQ` dès que le
    /// client est promu (eMule `AddUpNextClient` écrit sur le socket).
    pub async fn promotion_notify(
        &self,
        client_key: &str,
    ) -> Option<std::sync::Arc<tokio::sync::Notify>> {
        self.queue
            .lock()
            .await
            .iter()
            .find(|e| e.client_key == client_key)
            .map(|e| std::sync::Arc::clone(&e.promote_notify))
    }

    /// Tick périodique — équivalent de `CUploadQueue::Process()` appelé par
    /// `UploadTimer` eMule :
    /// 1. purge des entrées de file > `MAX_PURGEQUEUETIME` (1 h),
    /// 2. rotation des sessions — `CheckForTimeOver` avec
    ///    `FullChunkTransfers` (défaut eMule) : un client ayant reçu plus de
    ///    `SESSIONMAXTRANS` est renvoyé en file quand un nouveau slot ne
    ///    peut pas être ouvert (`!ForceNewClient()`), son temps d'upload +
    ///    bonus 15 min comptant comme attente (`GetScore(isdownloading)`),
    /// 3. promotion des meilleurs clients en attente tant que les slots le
    ///    permettent (`ForceNewClient`/`AcceptNewClient`).
    ///
    /// Doit tourner en permanence, même sans client UI connecté.
    pub async fn process_tick(&self) {
        let mut active = self.active_uploads.lock().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let mut evicted: Vec<(String, i64)> = Vec::new();
        let mut changed = false;
        {
            let mut q = self.queue.lock().await;
            let before = q.len();
            q.retain(|e| {
                now.saturating_sub(e.last_request_unix_ms) <= MAX_PURGEQUEUE_MS
                    && self.hash_still_shared(&e.file_hash)
            });
            changed |= q.len() != before;
            // Rotation : tant que la file est disputée (pas de nouveau slot
            // ouvrable), les sessions au-delà de SESSIONMAXTRANS cèdent leur
            // slot — eMule `CheckForTimeOver`. Le slot libéré est rempli
            // immédiatement (eMule promeut après chaque retrait) avant
            // d'évaluer la session suivante.
            loop {
                if q.is_empty() || self.can_open_slot(&active, now) {
                    break;
                }
                let victim = active
                    .values()
                    .find(|s| {
                        s.state == UploadSlotState::Uploading
                            && s.transferred_bytes >= SESSION_MAX_TRANS_BYTES
                            && !self.is_friend(&s.client_key)
                    })
                    .map(|s| s.client_key.clone());
                let Some(victim) = victim else {
                    break;
                };
                if let Some(cur) = active.remove(&victim) {
                    tracing::info!(
                        "[UPLOAD_ROTATED] Client {} upload slot released after {} bytes session ('{}') — re-queues on next request",
                        log_addr_str(&cur.client_address), cur.transferred_bytes, cur.file_name
                    );
                    // eMule : le client n'est PAS re-enfilé ici — il revient
                    // en file à son prochain re-ask.
                    self.update_auto_up_prio(&active, &cur.file_hash);
                    evicted.push((cur.client_key, cur.queued_since_ms));
                }
                self.sort_queue_by_score(&mut q, now);
                changed |= self.promote_locked(&mut q, &mut active, now);
            }
            self.sort_queue_by_score(&mut q, now);
            changed |= self.promote_locked(&mut q, &mut active, now);
            drop(q);
        }
        // Wait start d'origine (conservé dans `credits` chez eMule) gardé
        // pour que le score accumulé survive à la rotation.
        if !evicted.is_empty() {
            let mut grace = self.rotation_grace.lock().await;
            for (key, since) in evicted {
                grace.insert(key, since);
            }
            drop(grace);
            changed = true;
        }
        self.active_count
            .store(Self::uploading_count(&active), Ordering::SeqCst);
        drop(active);
        if changed {
            self.emit_uploads_updated().await;
        }
    }

    /// Clears queue and active slots on kill switch or network reset.
    pub async fn abort_all(&self) {
        let mut active = self.active_uploads.lock().await;
        active.clear();
        self.active_count.store(0, Ordering::SeqCst);
        let mut q = self.queue.lock().await;
        q.clear();
        drop(q);
        self.rotation_grace.lock().await.clear();
        drop(active);
        self.emit_uploads_updated().await;
    }

    /// Returns the snapshot of the first active upload (if any).
    /// `upload_rate_bps` y est le débit effectif à l'instant de la lecture.
    pub async fn active_snapshot(&self) -> Option<UploadSessionSnapshot> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let active = self.active_uploads.lock().await;
        active.values().next().cloned().map(|mut s| {
            s.upload_rate_bps = s.effective_rate_bps(now);
            s
        })
    }

    /// Returns snapshots of all active upload slots.
    /// `upload_rate_bps` y est le débit effectif à l'instant de la lecture
    /// (décroît vers 0 quand un pair ne demande plus de blocs).
    pub async fn active_snapshots(&self) -> Vec<UploadSessionSnapshot> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let active = self.active_uploads.lock().await;
        active
            .values()
            .cloned()
            .map(|mut s| {
                s.upload_rate_bps = s.effective_rate_bps(now);
                s
            })
            .collect()
    }

    /// Returns the 1-based queue position of a waiting client matched by
    /// IP address (`CUploadQueue::GetWaitingClientByIP_UDP` semantics:
    /// UDP pings carry only the sender IP).
    pub async fn waiting_rank_by_ip(&self, ip: &std::net::IpAddr) -> Option<u32> {
        let ip_str = ip.to_string();
        let q = self.queue.lock().await;
        q.iter()
            .position(|entry| {
                entry
                    .client_address
                    .rsplit_once(':')
                    .map(|(host, _)| host == ip_str)
                    .unwrap_or(false)
            })
            .map(|pos| (pos + 1) as u32)
    }

    /// eMule `GetWaitingClientByIP_UDP(ip, udp_port, bIgnorePortOnUniqueIP
    /// = true, &bSenderMultipleIpUnknown)` + contrôle `md4equ(reqfilehash,
    /// sender->GetUploadFileID())` de `ClientUDPSocket.cpp` :
    /// - un client en file avec la même IP **et** le même port UDP annoncé
    ///   est identifié directement ;
    /// - sinon, s'il n'existe qu'**un seul** client en file à cette IP, il
    ///   est retenu (tolérance au port, `bIgnorePortOnUniqueIP`) ;
    /// - plusieurs clients à la même IP sans correspondance de port →
    ///   [`UdpRankLookup::Ambiguous`] (eMule ne répond pas : il force une
    ///   connexion TCP pour lever l'ambiguïté NAT) ;
    /// - le hash demandé doit correspondre au fichier du client trouvé,
    ///   sinon [`UdpRankLookup::FileMismatch`] (eMule ne répond pas).
    pub async fn waiting_rank_for_udp(
        &self,
        ip: &std::net::IpAddr,
        udp_port: u16,
        file_hash: &str,
    ) -> UdpRankLookup {
        let ip_str = ip.to_string();
        let clean_hash = file_hash.trim().to_lowercase();
        let q = self.queue.lock().await;
        let same_ip = |e: &&QueuedUpload| {
            e.client_address
                .rsplit_once(':')
                .map(|(host, _)| host == ip_str)
                .unwrap_or(false)
        };
        // 1. Correspondance exacte IP + port UDP (désambiguïsation NAT).
        if let Some(pos) = q.iter().position(|e| {
            same_ip(&e) && e.client_udp_port.is_some_and(|p| p == udp_port)
        }) {
            let entry = &q[pos];
            return if entry.file_hash.eq_ignore_ascii_case(&clean_hash) {
                UdpRankLookup::Rank((pos + 1) as u32)
            } else {
                UdpRankLookup::FileMismatch
            };
        }
        // 2. Tolérance « unique IP » : un seul client en file à cette IP.
        let matches: Vec<usize> = q
            .iter()
            .enumerate()
            .filter(|(_, e)| same_ip(e))
            .map(|(pos, _)| pos)
            .collect();
        match matches.as_slice() {
            [] => UdpRankLookup::NoSender,
            [pos] => {
                let entry = &q[*pos];
                if entry.file_hash.eq_ignore_ascii_case(&clean_hash) {
                    UdpRankLookup::Rank((*pos + 1) as u32)
                } else {
                    UdpRankLookup::FileMismatch
                }
            }
            _ => UdpRankLookup::Ambiguous,
        }
    }

    /// eMule `SetLastUpRequest` — un `OP_REASKFILEPING` accepté d'un
    /// client en file rafraîchit son horodatage de re-ask (la purge
    /// `MAX_PURGEQUEUETIME` 1 h ne doit pas frapper un client vivant qui
    /// n'a plus de socket TCP ouverte de re-ask).
    pub async fn note_udp_reask(&self, ip: &std::net::IpAddr) {
        let ip_str = ip.to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let mut q = self.queue.lock().await;
        for entry in q.iter_mut() {
            if entry
                .client_address
                .rsplit_once(':')
                .map(|(host, _)| host == ip_str)
                .unwrap_or(false)
            {
                entry.last_request_unix_ms = now;
            }
        }
    }

    /// Number of clients currently waiting in the upload queue.
    pub async fn waiting_len(&self) -> usize {
        self.queue.lock().await.len()
    }

    /// True when the waiting queue is within 50 entries of capacity —
    /// eMule answers `OP_QUEUEFULL` to UDP pings from unknown clients in
    /// that state (`GetWaitingUserCount() + 50 > GetQueueSize()`).
    pub async fn is_nearly_full(&self) -> bool {
        self.waiting_len().await + 50 > self.max_queue.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Returns list of all queued upload snapshots.
    pub async fn list_queue(&self) -> Vec<UploadSessionSnapshot> {
        let q = self.queue.lock().await;
        q.iter()
            .enumerate()
            .map(|(idx, item)| UploadSessionSnapshot {
                client_key: item.client_key.clone(),
                client_address: item.client_address.clone(),
                file_hash: item.file_hash.clone(),
                file_name: item.file_name.clone(),
                transferred_bytes: 0,
                upload_rate_bps: 0,
                state: UploadSlotState::Waiting,
                queue_rank: Some((idx + 1) as u32),
                started_at_unix_ms: item.enqueued_at_unix_ms,
                rate_window_start_ms: 0,
                rate_window_bytes: 0,
                rate_sampled: false,
                credit_multiplier_bps: item.credit_multiplier_bps,
                file_prio_num: item.file_prio_num,
                queued_since_ms: item.enqueued_at_unix_ms,
                auto_prio: item.auto_prio,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn multiple_active_slots_up_to_limit() {
        let mgr = UploadManager::new();
        mgr.set_max_active(3);
        for i in 0..3 {
            let (state, _) = mgr
                .request_upload_slot(
                    &format!("c{i}"),
                    &format!("c{i}"),
                    "h",
                    "f",
                    1.0,
                    "normal",
                    false,
                    None,
                )
                .await
                .unwrap();
            assert_eq!(state, UploadSlotState::Uploading);
        }
        // 4th client queues
        let (state, rank) = mgr
            .request_upload_slot("c3", "c3", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        assert_eq!(rank, Some(1));
        assert_eq!(mgr.active_snapshots().await.len(), 3);
    }

    #[tokio::test]
    async fn release_promotes_until_slots_full() {
        let mgr = UploadManager::new();
        mgr.set_max_active(2);
        mgr.request_upload_slot("a", "a", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("b", "b", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("c", "c", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("d", "d", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();

        mgr.release_slot("a").await;
        let actives = mgr.active_snapshots().await;
        assert_eq!(actives.len(), 2);
        assert!(actives.iter().any(|s| s.client_address == "c"));
        assert_eq!(mgr.list_queue().await.len(), 1);

        mgr.release_slot("b").await;
        assert!(mgr
            .active_snapshots()
            .await
            .iter()
            .any(|s| s.client_address == "d"));
        assert!(mgr.list_queue().await.is_empty());
    }

    #[tokio::test]
    async fn userhash_key_dedups_across_ips_and_release_by_address_works() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        // Same userhash reconnecting from a different IP is the same client.
        mgr.request_upload_slot("uh:AA", "1.1.1.1:4662", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        let (state, _) = mgr
            .request_upload_slot("uh:AA", "2.2.2.2:4662", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);
        // release_slot by bare ip:port resolves via client_address.
        mgr.release_slot("1.1.1.1:4662").await;
        assert_eq!(mgr.active_len(), 0);
    }

    #[tokio::test]
    async fn slow_slot_demoted_to_trickle_and_promotes_queue() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        // c1 takes the only slot.
        let (state, _) = mgr
            .request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);
        // c2 waits.
        let (state, _) = mgr
            .request_upload_slot("c2", "c2", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        // c1 stays slow past the grace period → trickle → c2 promoted.
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        {
            let mut active = mgr.active_uploads.lock().await;
            let s = active.get_mut("c1").unwrap();
            s.started_at_unix_ms -= TRICKLE_GRACE_MS + 1;
            // Fenêtre de mesure amorcée il y a 1,5 s : le premier
            // échantillon (≥ 500 ms) mesure ~341 B/s < TRICKLE_RATE_BPS.
            s.rate_window_start_ms = now_ms - 1_500;
        }
        mgr.update_progress("c1", 512).await;
        let actives = mgr.active_snapshots().await;
        assert_eq!(
            actives.iter().find(|s| s.client_key == "c1").unwrap().state,
            UploadSlotState::Trickle
        );
        assert_eq!(
            actives.iter().find(|s| s.client_key == "c2").unwrap().state,
            UploadSlotState::Uploading
        );
        assert_eq!(mgr.active_len(), 1, "trickle does not occupy a full slot");
    }

    #[tokio::test]
    async fn per_file_stats_aggregate_sessions_and_persist_totals() {
        let mgr = UploadManager::new();
        // Hydratation au boot : le fichier a déjà 1000 B uploadés.
        mgr.set_file_upload_totals(std::collections::HashMap::from([(
            "haa".to_string(),
            1000u64,
        )]));
        // Deux slots actifs sur le même hash (casse mixte) + un autre fichier.
        mgr.request_upload_slot("c1", "c1", "HAA", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("c2", "c2", "haa", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("c3", "c3", "hbb", "g", 1.0, "normal", false, None)
            .await
            .unwrap();
        // Fenêtre de mesure amorcée il y a 1 s → premier échantillon au
        // prochain update_progress (elapsed ≥ RATE_FIRST_SAMPLE_MS).
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        {
            let mut active = mgr.active_uploads.lock().await;
            for s in active.values_mut() {
                s.rate_window_start_ms = now - 1_000;
            }
        }
        mgr.update_progress("c1", 1500).await;
        mgr.update_progress("c2", 1500).await;
        mgr.update_progress("c3", 1500).await;

        let totals = mgr.per_file_uploaded_bytes();
        assert_eq!(totals["haa"], 4000, "hydraté 1000 + 1500 + 1500");
        assert_eq!(totals["hbb"], 1500);
        assert!(!totals.contains_key("hcc"));

        let rates = mgr.per_file_rates_bps().await;
        // ~1500 B/s par slot → ~3000 B/s agrégé pour haa, ~1500 pour hbb.
        let haa = rates["haa"];
        let hbb = rates["hbb"];
        assert!(haa > hbb, "deux slots agrégés ({haa}) > un slot ({hbb})");
        assert!(haa > 0 && hbb > 0);
        assert!(!rates.contains_key("hcc"));

        // Slots libérés : débit retombe, total persisté conservé.
        mgr.release_slot("c1").await;
        mgr.release_slot("c2").await;
        assert_eq!(
            mgr.per_file_rates_bps()
                .await
                .get("haa")
                .copied()
                .unwrap_or(0),
            0
        );
        assert_eq!(mgr.per_file_uploaded_bytes()["haa"], 4000);
    }

    #[tokio::test]
    async fn fast_slot_within_grace_is_not_demoted() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        // Slow rate but still within grace period → stays Uploading.
        mgr.update_progress("c1", 512).await;
        let s = &mgr.active_snapshots().await[0];
        assert_eq!(s.state, UploadSlotState::Uploading);
    }

    #[tokio::test]
    async fn uss_gate_keeps_new_clients_queued() {
        let mgr = UploadManager::new();
        let flag = Arc::new(AtomicBool::new(false));
        mgr.set_uss_gate(flag.clone());
        // Gate closed: free slots exist but the client must queue.
        let (state, rank) = mgr
            .request_upload_slot("a", "a", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        assert_eq!(rank, Some(1));
        assert_eq!(mgr.active_len(), 0);
        // Gate open: next request is granted immediately.
        flag.store(true, Ordering::SeqCst);
        let (state, _) = mgr
            .request_upload_slot("b", "b", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);
    }

    #[tokio::test]
    async fn events_emitted_on_grant_queue_release() {
        let mgr = UploadManager::new();
        let (tx, mut rx) = broadcast::channel(16);
        mgr.set_event_channel(tx, Arc::new(AtomicU64::new(1)));

        mgr.set_max_active(1);
        mgr.request_upload_slot("a", "a", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(ev.contains("upload.uploadsUpdated"));
        assert!(ev.contains("\"uploadRateBps\""));

        mgr.request_upload_slot("b", "b", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(ev.contains("upload.uploadsUpdated"));
        assert!(ev.contains("queue"));

        mgr.release_slot("a").await;
        let ev = rx.recv().await.unwrap();
        assert!(ev.contains("upload.uploadsUpdated"));
    }

    #[tokio::test]
    async fn progress_tracked_per_client() {
        let mgr = UploadManager::new();
        mgr.request_upload_slot("a", "a", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("b", "b", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.update_progress("a", 100).await;
        mgr.update_progress("b", 200).await;
        let actives = mgr.active_snapshots().await;
        assert_eq!(
            actives
                .iter()
                .find(|s| s.client_address == "a")
                .unwrap()
                .transferred_bytes,
            100
        );
        assert_eq!(
            actives
                .iter()
                .find(|s| s.client_address == "b")
                .unwrap()
                .transferred_bytes,
            200
        );
    }

    #[tokio::test]
    async fn rate_is_measured_per_window_and_decays_on_stall() {
        let mgr = UploadManager::new();
        mgr.request_upload_slot("a", "a", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        let now_ms = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64
        };
        // Fenêtre amorcée il y a 1 s : 200 KiB envoyés → premier échantillon
        // (≥ 500 ms) ≈ 200 KiB/s — le débit mesuré reflète les octets
        // réellement servis, pas la limite configurée.
        {
            let mut active = mgr.active_uploads.lock().await;
            active.get_mut("a").unwrap().rate_window_start_ms = now_ms() - 1_000;
        }
        mgr.update_progress("a", 200 * 1024).await;
        let snaps = mgr.active_snapshots().await;
        let rate = snaps[0].upload_rate_bps;
        assert!(
            (150 * 1024..=250 * 1024).contains(&rate),
            "measured rate should be ~200 KiB/s, got {rate}"
        );
        // Pair inactif depuis > 2 s : le débit effectif retombe à 0 au lieu
        // de rester figé sur la dernière valeur.
        assert_eq!(snaps[0].effective_rate_bps(now_ms() + 10_000), 0);
    }

    /// eMule `AcceptNewClient` : `curUploadSlots >= MaxSpeed*1024/targetRate`
    /// → refus. Le nombre de slots suit la capacité upload configurée,
    /// pas un plafond fixe de 100.
    #[tokio::test]
    async fn accept_new_client_caps_slots_by_configured_upload_speed() {
        let mgr = UploadManager::new();
        let speed = Arc::new(AtomicU64::new(12 * 1024));
        let speed_probe = Arc::clone(&speed);
        mgr.set_upload_speed_probe(move || speed_probe.load(Ordering::Relaxed));

        // Les 4 slots garantis passent toujours (max(MIN_UP_CLIENTS,4)).
        for i in 0..4 {
            let (state, _) = mgr
                .request_upload_slot(&format!("c{i}"), &format!("c{i}"), "h", "f", 1.0, "normal", false, None)
                .await
                .unwrap();
            assert_eq!(state, UploadSlotState::Uploading);
        }
        // 5e : 12 KiB/s / 4 KiB/s cible = 3 < 4 slots → refus → file.
        let (state, _) = mgr
            .request_upload_slot("c5", "c5", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);

        // Débit mesuré réel (40 KiB/s) : le terme `datarate/minTarget`
        // passerait (40/3 ≈ 13 > 4) mais `MaxSpeed/target` refuse encore.
        {
            let mut active = mgr.active_uploads.lock().await;
            for s in active.values_mut() {
                s.upload_rate_bps = 10 * 1024;
            }
        }
        let (state, _) = mgr
            .request_upload_slot("c6", "c6", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);

        // Limite relevée à 200 KiB/s → 200/4 = 50 > 4 → slot ouvert.
        speed.store(200 * 1024, Ordering::Relaxed);
        let (state, _) = mgr
            .request_upload_slot("c7", "c7", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);
    }

    /// eMule `IsDownloading` → `OP_ACCEPTUPLOADREQ` + `SetUploadFileID` :
    /// un client qui détient un slot conserve ce slot quand il demande un
    /// autre fichier — jamais de deuxième entrée en file.
    #[tokio::test]
    async fn downloading_client_other_file_keeps_slot() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("c1", "c1", "h1", "f1", 1.0, "normal", false, None)
            .await
            .unwrap();
        let (state, _) = mgr
            .request_upload_slot("c1", "c1", "h2", "f2", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);
        let snaps = mgr.active_snapshots().await;
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].file_hash, "h2");
        assert_eq!(mgr.waiting_len().await, 0);
    }

    /// eMule : un objet client = une entrée de file. Le re-ask pour un
    /// autre fichier met à jour l'entrée existante au lieu d'en créer une.
    #[tokio::test]
    async fn queued_client_single_entry_and_updates_on_reask() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("c2", "c2", "h1", "f1", 1.0, "normal", false, None)
            .await
            .unwrap();
        let (state, _) = mgr
            .request_upload_slot("c2", "c2", "h2", "f2", 1.0, "high", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        assert_eq!(mgr.waiting_len().await, 1);
        let q = mgr.queue.lock().await;
        assert_eq!(q[0].file_hash, "h2");
        assert_eq!(q[0].file_prio_num, 9);
    }

    /// eMule `CheckForTimeOver` (`FullChunkTransfers`, défaut) : une session
    /// au-delà de `SESSIONMAXTRANS` est renvoyée en file quand aucun slot ne
    /// peut s'ouvrir — le wait start (`credits`) survit à la rotation.
    #[tokio::test]
    async fn process_tick_rotates_full_session_and_preserves_wait() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("c2", "c2", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        {
            let mut active = mgr.active_uploads.lock().await;
            let s = active.get_mut("c1").unwrap();
            s.transferred_bytes = SESSION_MAX_TRANS_BYTES;
            s.queued_since_ms = now - 60_000; // wait start d'origine
        }
        mgr.process_tick().await;
        // c1 évincé → c2 promu. c1 ne réintègre la file qu'à son prochain
        // re-ask (comportement eMule : RemoveFromUploadQueue → US_NONE).
        let snaps = mgr.active_snapshots().await;
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].client_key, "c2");
        assert_eq!(mgr.waiting_len().await, 0);

        // Re-ask de c1 : wait start restauré → enqueued_at antérieur à now,
        // le score accumulé n'est pas perdu.
        let (state, rank) = mgr
            .request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        assert_eq!(rank, Some(1));
        let q = mgr.queue.lock().await;
        assert_eq!(
            q[0].enqueued_at_unix_ms,
            now - 60_000,
            "wait start d'origine restauré après rotation"
        );
    }

    /// La promotion signale le dispatcher (`notify`) qui pousse
    /// `OP_ACCEPTUPLOADREQ` — le client en attente ne dépend pas de son
    /// prochain re-ask.
    #[tokio::test]
    async fn promotion_notify_fires_when_slot_frees() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("c2", "c2", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        let notify = mgr.promotion_notify("c2").await.unwrap();
        mgr.release_slot("c1").await;
        tokio::time::timeout(std::time::Duration::from_millis(200), notify.notified())
            .await
            .expect("la promotion doit signaler le dispatcher");
        assert_eq!(mgr.active_snapshots().await[0].client_key, "c2");
    }

    /// eMule `MAX_PURGEQUEUETIME` : une entrée sans re-ask depuis plus d'1 h
    /// est purgée de la file.
    #[tokio::test]
    async fn stale_queue_entries_purged_after_one_hour() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("c2", "c2", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        {
            let mut q = mgr.queue.lock().await;
            q[0].last_request_unix_ms -= MAX_PURGEQUEUE_MS + 1;
        }
        mgr.process_tick().await;
        assert_eq!(mgr.waiting_len().await, 0);
    }

    /// eMule soft/hard limit : au-delà de `QueueSize`, seuls les clients
    /// dont `combined(prio × crédits)` dépasse la moyenne de la file (ou
    /// les amis) sont admis ; `hard = soft + max(soft,800)/4` reste un
    /// refus sec.
    #[tokio::test]
    async fn soft_limit_admits_only_above_average_combined_score() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.set_max_queue(10); // soft=10, hard=210
        mgr.request_upload_slot("c0", "c0", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        // Remplit la soft limit avec des clients très scorés
        // (veryhigh=18 × 10.0 crédits → combined 1800).
        for i in 0..10 {
            mgr.request_upload_slot(&format!("q{i}"), &format!("q{i}"), "h", "f", 10.0, "veryhigh", false, None)
                .await
                .unwrap();
        }
        assert_eq!(mgr.waiting_len().await, 10);
        // Combined faible (normal ×1.0 = 70 < 1800) → refus.
        assert!(mgr
            .request_upload_slot("low1", "low1", "h", "f", 1.0, "normal", false, None)
            .await
            .is_err());
        // Combined égal à la moyenne → admis.
        let (state, _) = mgr
            .request_upload_slot("hi1", "hi1", "h", "f", 10.0, "veryhigh", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
    }

    /// eMule `MIN_REQUESTTIME`/`BADCLIENTBAN` : 4 re-asks du même fichier en
    /// moins de 10 min → client banni (score 0, jamais promu).
    #[tokio::test]
    async fn aggressive_reasks_mark_client_banned() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("c1", "c1", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        mgr.request_upload_slot("c2", "c2", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        for _ in 0..3 {
            mgr.request_upload_slot("c2", "c2", "h", "f", 1.0, "normal", false, None)
                .await
                .unwrap();
        }
        assert!(!mgr.queue.lock().await[0].banned);
        mgr.request_upload_slot("c2", "c2", "h", "f", 1.0, "normal", false, None)
            .await
            .unwrap();
        let q = mgr.queue.lock().await;
        assert!(q[0].banned);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        assert_eq!(q[0].effective_score(now), 0);
    }

    /// Mapping `GetFilePrioAsNumber` eMule : veryhigh=18, high=9, normal=7,
    /// low=6, verylow=2 ; powershare tombe sur la branche default (7) ;
    /// « auto » = priorité effective initiale `PR_HIGH` (9).
    #[test]
    fn file_prio_num_matches_emule_get_file_prio_as_number() {
        assert_eq!(file_prio_num("veryhigh"), 18);
        assert_eq!(file_prio_num("high"), 9);
        assert_eq!(file_prio_num("normal"), 7);
        assert_eq!(file_prio_num("low"), 6);
        assert_eq!(file_prio_num("verylow"), 2);
        assert_eq!(file_prio_num("powershare"), 7);
        assert_eq!(file_prio_num("auto"), 9);
        assert_eq!(file_prio_num("unknown"), 7);
    }

    /// eMule `CKnownFile::UpdateAutoUpPriority` : la priorité effective
    /// d'un fichier « auto » suit le nombre de clients qui l'uploadent —
    /// plus de 20 → low (6), plus de 1 → normal (7), sinon → high (9).
    #[tokio::test]
    async fn auto_up_priority_tracks_uploader_count() {
        let mgr = UploadManager::new();
        let (state, _) = mgr
            .request_upload_slot("c1", "c1", "haa", "f", 1.0, "auto", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Uploading);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let fake = |key: &str| UploadSessionSnapshot {
            client_key: key.to_string(),
            client_address: key.to_string(),
            file_hash: "haa".to_string(),
            file_name: "f".to_string(),
            transferred_bytes: 0,
            upload_rate_bps: 0,
            state: UploadSlotState::Uploading,
            queue_rank: None,
            started_at_unix_ms: now,
            rate_window_start_ms: 0,
            rate_window_bytes: 0,
            rate_sampled: false,
            credit_multiplier_bps: 100,
            file_prio_num: 9,
            queued_since_ms: now,
            auto_prio: true,
        };

        let mut active = mgr.active_uploads.lock().await;
        // c1 actif → +20 fakes = 21 uploaders du même fichier.
        for i in 0..20 {
            let k = format!("f{i}");
            active.insert(k.clone(), fake(&k));
        }
        mgr.update_auto_up_prio(&active, "haa");
        assert_eq!(
            mgr.auto_up_prio.read().unwrap()["haa"], 6,
            ">20 uploaders → low"
        );

        active.remove("f0");
        mgr.update_auto_up_prio(&active, "haa");
        assert_eq!(
            mgr.auto_up_prio.read().unwrap()["haa"], 7,
            "20 uploaders → normal"
        );

        for i in 1..19 {
            active.remove(&format!("f{i}"));
        }
        mgr.update_auto_up_prio(&active, "haa");
        assert_eq!(
            mgr.auto_up_prio.read().unwrap()["haa"], 7,
            "2 uploaders → encore normal"
        );

        active.remove("f19");
        mgr.update_auto_up_prio(&active, "haa");
        assert_eq!(
            mgr.auto_up_prio.read().unwrap()["haa"], 9,
            "≤1 uploader → high"
        );
    }

    /// Une entrée de file « auto » reflète le dernier
    /// `UpdateAutoUpPriority` à chaque re-ask/tri ; un re-ask avec une
    /// priorité manuelle sort le fichier du mode auto.
    #[tokio::test]
    async fn auto_prio_queue_entry_refresh_and_manual_override() {
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        mgr.request_upload_slot("busy", "busy", "hzz", "z", 1.0, "normal", false, None)
            .await
            .unwrap();
        let (state, _) = mgr
            .request_upload_slot("q1", "q1", "haa", "f", 1.0, "auto", false, None)
            .await
            .unwrap();
        assert_eq!(state, UploadSlotState::Waiting);
        {
            let q = mgr.queue.lock().await;
            assert!(q[0].auto_prio);
            assert_eq!(q[0].file_prio_num, 9, "auto initial → high");
        }

        // 21 uploaders actifs sur haa → effective low au prochain re-ask.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        {
            let mut active = mgr.active_uploads.lock().await;
            for i in 0..21 {
                let k = format!("f{i}");
                active.insert(
                    k.clone(),
                    UploadSessionSnapshot {
                        client_key: k.clone(),
                        client_address: k,
                        file_hash: "haa".to_string(),
                        file_name: "f".to_string(),
                        transferred_bytes: 0,
                        upload_rate_bps: 0,
                        state: UploadSlotState::Uploading,
                        queue_rank: None,
                        started_at_unix_ms: now,
                        rate_window_start_ms: 0,
                        rate_window_bytes: 0,
                        rate_sampled: false,
                        credit_multiplier_bps: 100,
                        file_prio_num: 9,
                        queued_since_ms: now,
                        auto_prio: true,
                    },
                );
            }
            mgr.update_auto_up_prio(&active, "haa");
        }
        mgr.request_upload_slot("q1", "q1", "haa", "f", 1.0, "auto", false, None)
            .await
            .unwrap();
        {
            let q = mgr.queue.lock().await;
            assert!(q[0].auto_prio);
            assert_eq!(q[0].file_prio_num, 6, "auto recalculée → low");
        }

        // Bascule manuelle : sortie du registre auto + poids explicite.
        mgr.request_upload_slot("q1", "q1", "haa", "f", 1.0, "high", false, None)
            .await
            .unwrap();
        {
            let q = mgr.queue.lock().await;
            assert!(!q[0].auto_prio);
            assert_eq!(q[0].file_prio_num, 9);
        }
        assert!(!mgr.auto_up_prio.read().unwrap().contains_key("haa"));
    }

    #[tokio::test]
    async fn udp_rank_lookup_matches_emule() {
        use std::net::{IpAddr, Ipv4Addr};
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));

        // Occupe l'unique slot actif pour que les clients suivants
        // rejoignent la file.
        mgr.request_upload_slot("uh:F", "f", "hf", "f", 1.0, "normal", false, None)
            .await
            .unwrap();

        // Un client en file pour "haa" avec port UDP annoncé 4672.
        mgr.request_upload_slot(
            "uh:AAAA",
            "10.0.0.5:5000",
            "haa",
            "f",
            1.0,
            "normal",
            false,
            Some(4672),
        )
        .await
        .unwrap();

        // Hash conforme + port conforme → rang 1.
        assert_eq!(
            mgr.waiting_rank_for_udp(&ip, 4672, "haa").await,
            UdpRankLookup::Rank(1)
        );
        // Port inconnu mais IP unique → tolérance `bIgnorePortOnUniqueIP`.
        assert_eq!(
            mgr.waiting_rank_for_udp(&ip, 9999, "haa").await,
            UdpRankLookup::Rank(1)
        );
        // Hash différent → silence eMule (`reqfile does not match`).
        assert_eq!(
            mgr.waiting_rank_for_udp(&ip, 4672, "hbb").await,
            UdpRankLookup::FileMismatch
        );
        // IP inconnue → NoSender (→ QUEUEFULL seulement si quasi-pleine).
        assert_eq!(
            mgr.waiting_rank_for_udp(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 6)), 4672, "haa")
                .await,
            UdpRankLookup::NoSender
        );
    }

    #[tokio::test]
    async fn udp_rank_lookup_disambiguates_same_ip_by_port() {
        use std::net::{IpAddr, Ipv4Addr};
        let mgr = UploadManager::new();
        mgr.set_max_active(1);
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9));

        mgr.request_upload_slot("uh:F", "f", "hf", "f", 1.0, "normal", false, None)
            .await
            .unwrap();

        // Deux clients derrière la même IP (NAT), ports UDP distincts.
        mgr.request_upload_slot(
            "uh:A",
            "10.0.0.9:5000",
            "haa",
            "f",
            1.0,
            "normal",
            false,
            Some(4672),
        )
        .await
        .unwrap();
        mgr.request_upload_slot(
            "uh:B",
            "10.0.0.9:5001",
            "hbb",
            "g",
            1.0,
            "normal",
            false,
            Some(4673),
        )
        .await
        .unwrap();

        // Le port UDP désambiguïse → le bon client et le bon fichier.
        assert_eq!(
            mgr.waiting_rank_for_udp(&ip, 4673, "hbb").await,
            UdpRankLookup::Rank(2)
        );
        // Sans correspondance de port → Ambiguous (eMule force le TCP).
        assert_eq!(
            mgr.waiting_rank_for_udp(&ip, 1111, "haa").await,
            UdpRankLookup::Ambiguous
        );
        // Port exact mais fichier différent → FileMismatch (silence).
        assert_eq!(
            mgr.waiting_rank_for_udp(&ip, 4672, "hcc").await,
            UdpRankLookup::FileMismatch
        );
    }
}
