use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use mule_network_policy::{NetworkPolicyManager, ResourceBudgets};
use std::net::SocketAddr;

use crate::config::DaemonConfig;
use crate::services::downloads::types::DownloadSnapshot;
use crate::services::downloads::DownloadService;
use crate::services::peer_session::PeerSessionManager;
use crate::services::server_session::ServerSessionManager;
use crate::services::DaemonEvent;

#[derive(Clone)]
pub struct AppState {
    pub config: DaemonConfig,
    pub downloads: Arc<Mutex<DownloadService>>,
    pub network_policy: NetworkPolicyManager,
    pub server_session: ServerSessionManager,
    pub peer_session: PeerSessionManager,
    /// Source Exchange partagé : policy réseau, identité locale et journaux
    /// de throttle eMule (`SetLastSrcReqTime` / `GetLastAskedForSources` /
    /// `m_ClientSrcAnswered`) — injecté dans `peer_session` et
    /// `inbound_listener`.
    pub sx_env: crate::services::peer_session::source_exchange::SxEnv,
    pub inbound_listener: crate::services::inbound_listener::InboundListenerService,
    pub identity: Arc<Mutex<crate::services::IdentityService>>,
    /// Shared SUI anomaly counters (identity.status `sui` object).
    pub sui_stats: Arc<crate::services::identity::SuiStats>,
    pub kad: Arc<crate::services::KadService>,
    /// Client-to-client UDP service sharing the Kad UDP socket
    /// (`OP_REASKFILEPING`/`OP_REASKACK`/`OP_QUEUEFULL`/`OP_FILENOTFOUND`).
    pub client_udp: crate::services::ClientUdpService,
    /// Server UDP service (`OP_GLOBGETSOURCES2`/`OP_GLOBFOUNDSOURCES`),
    /// separate socket like eMule's `UDPSocket`.
    pub server_udp: crate::services::server_udp::ServerUdpService,
    /// USS (Upload Speed Sense) — dynamic upload limiter driven by
    /// measured latency to the connected server or a configured host.
    pub uss: crate::services::UssService,
    pub upnp: Arc<crate::services::UpnpService>,
    pub sequence: Arc<AtomicU64>,
    pub event_tx: broadcast::Sender<String>,
    pub instance_id: Uuid,
    pub session_id: Uuid,
    pub abort_handles: Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
    pub shutdown_notify: Arc<tokio::sync::Notify>,
    /// Effective ports (persisted settings override `DaemonConfig`):
    /// eD2K TCP listener and Kad UDP. Mirrors eMule Preferences → Connexion.
    ed2k_port_eff: Arc<std::sync::atomic::AtomicU16>,
    kad_port_eff: Arc<std::sync::atomic::AtomicU16>,
    /// Whether UDP (Kad + server UDP + client UDP) is enabled at all —
    /// eMule « UDP désactivé » checkbox.
    udp_enabled: Arc<AtomicBool>,
    /// Gate du quota journalier d'upload (`dailyUploadQuotaGb`, Emule-Sion) :
    /// `false` bloque les nouvelles demandes de slot. Piloté par
    /// `spawn_quota_watcher`.
    upload_quota_gate: Arc<AtomicBool>,
    /// Totaux cumulés depuis l'installation (persistés en settings DB,
    /// incrémentés du delta de session à chaque `statsTick`, remis à zéro
    /// par `stats.resetLifetimeTotals`).
    pub lifetime_downloaded_bytes: Arc<AtomicU64>,
    pub lifetime_uploaded_bytes: Arc<AtomicU64>,
    #[cfg(feature = "lab-tools")]
    pub active_lab_peer: Arc<Mutex<Option<(std::net::SocketAddr, tokio::task::AbortHandle)>>>,
}

impl AppState {
    pub fn new(config: DaemonConfig, mut downloads: DownloadService) -> Self {
        let (event_tx, _) = broadcast::channel(1024);
        let (max_per_dl, max_global) = {
            let max_g = downloads
                .get_setting("max_global_download_peers")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(config.max_global_download_peers);
            let max_f = downloads
                .get_setting("max_peers_per_download")
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(config.max_peers_per_download);
            (max_f, max_g)
        };

        let mut budgets = ResourceBudgets::default();
        if budgets.max_active_peer_connections < max_global {
            budgets.max_active_peer_connections =
                max_global.min(mule_network_policy::MAX_PEER_CONNECTIONS_CAP);
        }
        // Persisted Connexion overrides (eMule Preferences) applied at boot.
        let get_usize = |key: &str| {
            downloads
                .get_setting(key)
                .ok()
                .flatten()
                .and_then(|s| s.parse::<usize>().ok())
        };
        if let Some(v) = get_usize("max_connections") {
            budgets.max_active_peer_connections =
                v.clamp(1, mule_network_policy::MAX_PEER_CONNECTIONS_CAP);
        }
        if let Some(v) = get_usize("max_conn_per_min") {
            budgets.rate_limit_per_min = (v as u32).clamp(1, 1200);
        }
        let bool_setting = |key: &str| -> Option<bool> {
            downloads
                .get_setting(key)
                .ok()
                .flatten()
                .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "on"))
        };
        let obf_enabled =
            bool_setting("obfuscation_enabled").unwrap_or(config.obfuscation_enabled);
        let obf_required =
            bool_setting("obfuscation_required").unwrap_or(config.obfuscation_required);
        let ed2k_port_eff = get_usize("ed2k_port")
            .map(|v| v.clamp(1, 65535) as u16)
            .unwrap_or(config.ed2k_port);
        let kad_port_eff = get_usize("kad_port")
            .map(|v| v.clamp(1, 65535) as u16)
            .unwrap_or(config.kad_port);
        let udp_enabled = bool_setting("udp_enabled").unwrap_or(true);
        // Mode paranoid (Emule-Sion) : slots d'upload réservés aux pairs
        // obfusqués avec signature SUI vérifiée.
        let paranoid_mode = bool_setting("paranoid_mode").unwrap_or(false);
        let max_upload_queue = get_usize("max_upload_queue");
        let min_upload_kbs = get_usize("min_upload_kbs");
        let upnp_enabled_boot = bool_setting("upnp_enabled").unwrap_or(config.upnp_enabled);

        let network_policy = NetworkPolicyManager::new(config.network_profile, budgets);
        // Valeurs clampées à réécrire dans le store — différées car les
        // closures ci-dessus empruntent `downloads` immuablement.
        let mut clamped_limits: Vec<(&'static str, u64)> = Vec::new();
        // Persisted rate limits (eMule MaxUpload/MaxDownload, stored via
        // `limits.set`) applied at boot.
        {
            let get_u64 = |key: &str| {
                downloads
                    .get_setting(key)
                    .ok()
                    .flatten()
                    .and_then(|s| s.parse::<u64>().ok())
            };
            let mut rl = mule_filter::RateLimitsConfig::default();
            let mut has_override = false;
            if let Some(v) = get_u64("rate_limit_download_bps") {
                rl.download_bytes_per_sec = v;
                has_override = true;
            }
            if let Some(v) = get_u64("rate_limit_upload_bps") {
                rl.upload_bytes_per_sec = v;
                has_override = true;
            }
            if let Some(v) = get_u64("rate_limit_conn_per_min") {
                rl.connections_per_minute = v as u32;
                has_override = true;
            }
            if let Some(v) = get_u64("rate_limit_server_req_per_sec") {
                rl.server_requests_per_sec = v as u32;
                has_override = true;
            }
            if let Some(v) = get_u64("rate_limit_kad_pkt_per_sec") {
                rl.kad_packets_per_sec = v as u32;
                has_override = true;
            }
            // eMule LoadPreferences : une limite persistée ne peut pas
            // excéder la capacité de ligne déclarée — download ≤ capacité
            // × 9/10, upload ≤ capacité × 4/5 (0 = illimité, jamais
            // clampé). Même règle que `handle_limits_set`.
            let cap_dl = get_u64("capacity_download_kbs").unwrap_or(0);
            let cap_ul = get_u64("capacity_upload_kbs").unwrap_or(0);
            if rl.download_bytes_per_sec > 0 && cap_dl > 0 {
                let cap = cap_dl.saturating_mul(1024).saturating_mul(9) / 10;
                let clamped = rl.download_bytes_per_sec.min(cap);
                if clamped != rl.download_bytes_per_sec {
                    clamped_limits.push(("rate_limit_download_bps", clamped));
                    rl.download_bytes_per_sec = clamped;
                }
            }
            if rl.upload_bytes_per_sec > 0 && cap_ul > 0 {
                let cap = cap_ul.saturating_mul(1024).saturating_mul(4) / 5;
                let clamped = rl.upload_bytes_per_sec.min(cap);
                if clamped != rl.upload_bytes_per_sec {
                    clamped_limits.push(("rate_limit_upload_bps", clamped));
                    rl.upload_bytes_per_sec = clamped;
                }
            }
            // eMule `GetMaxDownloadInBytesPerSec` : ratio upload/download
            // — le bucket reçoit l'effectif ; le setting persisté garde la
            // valeur *configurée* (le ratio n'est jamais réécrit).
            let eff_dl = mule_network_policy::emule_ud_ratio(
                rl.download_bytes_per_sec,
                rl.upload_bytes_per_sec,
            );
            if eff_dl != rl.download_bytes_per_sec {
                rl.download_bytes_per_sec = eff_dl;
                has_override = true;
            }
            if has_override {
                network_policy.rate_limiters().apply_config(&rl);
            }
        }
        // Persisted IP filter (eMule keeps ipfilter.dat loaded across
        // restarts): reload the file and restore the enabled flag.
        {
            if let Some(path) = downloads
                .get_setting("ipfilter_path")
                .ok()
                .flatten()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
            {
                match network_policy.ip_filter().load_from_file(std::path::Path::new(&path)) {
                    Ok(report) => {
                        tracing::info!("[IPFILTER] Rechargé depuis {path} ({report:?})")
                    }
                    Err(e) => {
                        tracing::warn!("[IPFILTER] Rechargement impossible ({path}): {e}")
                    }
                }
            }
            if bool_setting("ipfilter_enabled").unwrap_or(false) {
                network_policy.ip_filter().set_enabled(true);
            }
        }
        // Réécriture des limites clampées au boot : la persistance reflète
        // la valeur effectivement appliquée (cohérence avec `limits.set`,
        // qui persiste la valeur clampée).
        for (key, v) in clamped_limits {
            if let Err(e) = downloads.set_setting(key, &v.to_string()) {
                tracing::warn!("[LIMITS] Échec persistance {key} clampé: {e}");
            }
        }
        let sequence = Arc::new(AtomicU64::new(1));
        let server_session = ServerSessionManager::new(event_tx.clone(), Arc::clone(&sequence));
        let peer_session = PeerSessionManager::new(event_tx.clone(), Arc::clone(&sequence))
            .with_limits(max_per_dl, max_global);
        // Totaux « depuis l'installation » persistés en settings — lus une
        // fois au boot puis incrémentés des deltas de session par la
        // boucle statsTick (remis à zéro via `stats.resetLifetimeTotals`).
        let u64_setting = |key: &str| {
            downloads
                .get_setting(key)
                .ok()
                .flatten()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        };
        let lifetime_downloaded_bytes =
            Arc::new(AtomicU64::new(u64_setting("lifetime_downloaded_bytes")));
        let lifetime_uploaded_bytes =
            Arc::new(AtomicU64::new(u64_setting("lifetime_uploaded_bytes")));
        downloads.set_event_channel(event_tx.clone(), Arc::clone(&sequence));
        let dl_arc = Arc::new(Mutex::new(downloads));
        server_session.set_downloads(Arc::clone(&dl_arc));
        let identity_service = crate::services::IdentityService::load_or_generate(&config.data_dir);
        let user_hash = identity_service.user_hash();
        // Environnement Source Exchange partagé entre toutes les sessions
        // (eMule garde ces compteurs dans CUpDownClient/CPartFile).
        let sx_env = crate::services::peer_session::source_exchange::SxEnv::new(
            network_policy.clone(),
            user_hash,
        );
        let sui_pubkey_der = if identity_service.has_key() {
            Some(identity_service.public_key_bytes().to_vec())
        } else {
            None
        };
        let identity = Arc::new(Mutex::new(identity_service));
        let sui_stats = Arc::new(crate::services::identity::SuiStats::default());
        let mut inbound_listener = crate::services::inbound_listener::InboundListenerService::new(
            Arc::new(network_policy.clone()),
            Arc::clone(&dl_arc),
            user_hash,
            sui_pubkey_der,
            obf_enabled,
            obf_required,
            config.obfuscation_padding_max,
            Arc::clone(&sui_stats),
        );
        inbound_listener.set_identity(Arc::clone(&identity));
        inbound_listener.set_sx_env(sx_env.clone());
        inbound_listener.set_paranoid_mode(paranoid_mode);
        let upload_quota_gate = Arc::new(AtomicBool::new(true));
        inbound_listener
            .upload_manager()
            .set_quota_gate(Arc::clone(&upload_quota_gate));
        inbound_listener.set_peer_session(peer_session.clone());
        inbound_listener.set_server_session(server_session.clone());
        // Sens inverse : OP_CALLBACKREQUESTED relayé par le serveur (un
        // pair HighID veut nous joindre alors que nous sommes LowID)
        // déclenche un connect-back sortant servi par l'inbound listener.
        server_session.set_inbound_listener(inbound_listener.clone());
        // eMule IsServerCryptLayerConnection : tentative d'obfuscation du
        // handshake serveur selon les préférences effectives (persistées).
        server_session
            .set_obfuscation_preference(obf_enabled || obf_required, config.obfuscation_padding_max);
        let kad_id = mule_core::kad::KadId::random();
        let kad = Arc::new(crate::services::KadService::new(
            kad_id,
            Arc::clone(&dl_arc),
            network_policy.clone(),
            network_policy.ip_filter(),
        ));
        kad.set_ed2k_tcp_port(ed2k_port_eff);
        kad.set_fw_check_identity(user_hash);
        // Nudges `kad.statusChanged` quand la table de routage évolue en
        // tâche de fond — le badge UI se met à jour sans ouvrir la section.
        kad.set_event_channel(event_tx.clone(), Arc::clone(&sequence));
        // Buddy relay role: TCP port the buddy client connects to, and the
        // GetMyConnectOptions-style flags byte sent in KADEMLIA_FINDBUDDY_RES
        // (bit0/1 = obfuscation supported+requested, bit2 = required).
        let connect_opts: u8 = if obf_enabled { 0x03 } else { 0 }
            | if obf_required { 0x04 } else { 0 };
        kad.udp().set_buddy_tcp_info(ed2k_port_eff, connect_opts);
        // The eD2K server-assigned HighID feeds Kad with our public IPv4
        // (PUBLISH_SOURCE advertised address).
        server_session.set_kad(Arc::clone(&kad));
        inbound_listener.set_kad_udp(Arc::clone(kad.udp()));
        inbound_listener.set_kad(Arc::clone(&kad));
        inbound_listener.set_kad_udp_port(kad_port_eff);
        let client_udp = crate::services::ClientUdpService::new();
        client_udp.set_downloads(Arc::clone(&dl_arc));
        client_udp.set_upload_manager(inbound_listener.upload_manager().clone());
        client_udp.set_peer_session(peer_session.clone());
        client_udp.set_buddy_registry(kad.udp().buddy_registry());
        inbound_listener.set_client_udp(client_udp.clone());
        let server_udp = crate::services::server_udp::ServerUdpService::new(network_policy.clone());
        server_udp.set_downloads(Arc::clone(&dl_arc));
        server_udp.set_event_channel(event_tx.clone(), sequence.clone());
        let uss = crate::services::UssService::new();
        uss.set_server_session(server_session.clone());
        uss.set_upload_manager(inbound_listener.upload_manager().clone());
        uss.set_downloads(Arc::clone(&dl_arc));
        inbound_listener
            .upload_manager()
            .set_uss_gate(uss.accept_flag());
        if let Some(q) = max_upload_queue {
            inbound_listener.upload_manager().set_max_queue(q);
        }
        if let Some(min_kbs) = min_upload_kbs {
            inbound_listener
                .upload_manager()
                .set_min_upload_rate((min_kbs as u64).saturating_mul(1024));
        }
        inbound_listener
            .upload_manager()
            .set_event_channel(event_tx.clone(), Arc::clone(&sequence));
        // Friend slots: load persisted friends into the upload manager
        // (eMule friends.met → dedicated slots for `uh:<userhash>` keys).
        {
            if let Ok(dl) = dl_arc.try_lock() {
                let friends = dl.list_friends().unwrap_or_default();
                inbound_listener.upload_manager().set_friends(
                    friends
                        .iter()
                        .map(|f| format!("uh:{}", f.user_hash))
                        .collect(),
                );
                // Totaux uploadés par fichier persistés (`share_upload_stats`)
                // — l'UploadManager les incrémente ensuite en mémoire.
                let totals = dl.list_share_uploaded_bytes().unwrap_or_default();
                inbound_listener
                    .upload_manager()
                    .set_file_upload_totals(totals);
            }
        }
        let kad_udp = Arc::clone(kad.udp());
        let client_udp_for_kad = client_udp.clone();
        let kad_udp_dl = Arc::clone(&dl_arc);
        tokio::spawn(async move {
            kad_udp.set_client_udp(client_udp_for_kad).await;
            kad_udp.set_downloads(kad_udp_dl).await;
        });

        // Periodic aggregate stats tick (event.statsTick, topic `stats`).
        // Skipped entirely while no WebSocket client is subscribed.
        {
            let stats_downloads = Arc::clone(&dl_arc);
            let stats_uploads = inbound_listener.upload_manager().clone();
            let stats_peers = peer_session.clone();
            let stats_server = server_session.clone();
            let stats_kad = Arc::clone(&kad);
            let stats_tx = event_tx.clone();
            let stats_seq = Arc::clone(&sequence);
            let stats_lifetime_dl = Arc::clone(&lifetime_downloaded_bytes);
            let stats_lifetime_ul = Arc::clone(&lifetime_uploaded_bytes);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                let mut last_session_dl = 0u64;
                let mut last_session_ul = 0u64;
                // Totaux par fichier déjà persistés — le delta écrit à
                // chaque tick ne couvre que les octets envoyés depuis.
                let mut last_file_totals = stats_uploads.per_file_uploaded_bytes();
                loop {
                    interval.tick().await;
                    if stats_tx.receiver_count() == 0 {
                        continue;
                    }
                    let session_uploaded_bytes = stats_uploads.total_uploaded_bytes();
                    let (downloads, session_downloaded_bytes) = {
                        let mut dl = stats_downloads.lock().await;
                        // Cumul « depuis l'installation » : on ajoute le delta
                        // de session écoulé depuis le dernier tick (seuls les
                        // deltas positifs comptent — les évictions de parts
                        // corrompues ne retirent rien au total) puis on
                        // persiste, sinon un crash perdrait le delta courant.
                        let session_dl = dl.session_downloaded_bytes();
                        let d_dl = session_dl.saturating_sub(last_session_dl);
                        let d_ul = session_uploaded_bytes.saturating_sub(last_session_ul);
                        if d_dl > 0 || d_ul > 0 {
                            let lt_dl =
                                stats_lifetime_dl.fetch_add(d_dl, Ordering::SeqCst) + d_dl;
                            let lt_ul =
                                stats_lifetime_ul.fetch_add(d_ul, Ordering::SeqCst) + d_ul;
                            let _ = dl.set_setting(
                                "lifetime_downloaded_bytes",
                                &lt_dl.to_string(),
                            );
                            let _ = dl.set_setting(
                                "lifetime_uploaded_bytes",
                                &lt_ul.to_string(),
                            );
                        }
                        last_session_dl = session_dl;
                        last_session_ul = session_uploaded_bytes;
                        // Persiste les deltas d'upload par fichier dans
                        // `share_upload_stats` (même cadence que les cumuls
                        // lifetime — un crash ne perd qu'un tick).
                        let file_totals = stats_uploads.per_file_uploaded_bytes();
                        for (hash, total) in &file_totals {
                            let prev =
                                last_file_totals.get(hash.as_str()).copied().unwrap_or(0);
                            let d = total.saturating_sub(prev);
                            if d > 0 {
                                let _ = dl.add_share_uploaded_bytes(hash, d);
                            }
                        }
                        last_file_totals = file_totals;
                        (dl.list_downloads(), session_dl)
                    };
                    let download_bps: u64 = downloads.iter().map(|d| d.download_rate_bps).sum();
                    let downloads_active = downloads
                        .iter()
                        .filter(|d| d.status == "downloading")
                        .count();
                    let upload_bps: u64 = stats_uploads
                        .active_snapshots()
                        .await
                        .iter()
                        .map(|u| u.upload_rate_bps)
                        .sum();
                    let peers_connected = stats_peers.swarm_snapshot(None).await.active_peer_count;
                    let server_connected = matches!(
                        stats_server.snapshot().await.status,
                        crate::services::server_session::ServerConnectionStatus::Connected
                    );
                    let kad_status = stats_kad.get_status().await;
                    DaemonEvent::StatsTick {
                        sequence: stats_seq.fetch_add(1, Ordering::SeqCst),
                        stats: crate::services::events::StatsTickSnapshot {
                            download_bps,
                            upload_bps,
                            downloads_active,
                            peers_connected,
                            server_connected,
                            kad_contacts: kad_status.total_contacts,
                            kad_udp_bound: kad_status.udp_bind_address.is_some(),
                            session_downloaded_bytes,
                            session_uploaded_bytes,
                            lifetime_downloaded_bytes:
                                stats_lifetime_dl.load(Ordering::SeqCst),
                            lifetime_uploaded_bytes:
                                stats_lifetime_ul.load(Ordering::SeqCst),
                        },
                    }
                    .send(&stats_tx);
                    // Poussée périodique des snapshots d'upload sur la même
                    // cadence : rafraîchit les débits mesurés par fenêtre et
                    // les fait décroître vers 0 quand un client cesse de
                    // demander des blocs (sinon l'UI resterait figée sur la
                    // dernière valeur entre deux `update_progress`).
                    if stats_uploads.active_len() > 0
                        || stats_uploads.waiting_len().await > 0
                    {
                        stats_uploads.emit_uploads_updated().await;
                    }
                }
            });
        }
        peer_session.set_kad(Arc::clone(&kad));
        peer_session.set_rate_limiters(network_policy.rate_limiters());
        peer_session.set_identity(Arc::clone(&identity));
        peer_session.set_sui_stats(Arc::clone(&sui_stats));
        let upnp = Arc::new(crate::services::UpnpService::new(
            upnp_enabled_boot,
            ed2k_port_eff,
            kad_port_eff,
        ));
        Self {
            config,
            downloads: dl_arc,
            network_policy,
            server_session,
            peer_session,
            sx_env,
            inbound_listener,
            identity,
            sui_stats,
            kad,
            client_udp,
            server_udp,
            uss,
            upnp,
            sequence,
            event_tx,
            instance_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            abort_handles: Arc::new(Mutex::new(Vec::new())),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
            ed2k_port_eff: Arc::new(std::sync::atomic::AtomicU16::new(ed2k_port_eff)),
            kad_port_eff: Arc::new(std::sync::atomic::AtomicU16::new(kad_port_eff)),
            udp_enabled: Arc::new(AtomicBool::new(udp_enabled)),
            upload_quota_gate,
            lifetime_downloaded_bytes,
            lifetime_uploaded_bytes,
            #[cfg(feature = "lab-tools")]
            active_lab_peer: Arc::new(Mutex::new(None)),
        }
    }

    /// Registers a background task handle so it can be cleanly aborted on shutdown.
    /// Effective obfuscation (protocol encryption) policy: persisted
    /// `obfuscation_enabled`/`obfuscation_required` settings override the
    /// startup `DaemonConfig` values (eMule Preferences → Security).
    pub async fn effective_obfuscation(&self) -> (bool, bool) {
        let dl = self.downloads.lock().await;
        let get = |key: &str, default: bool| {
            dl.get_setting(key)
                .ok()
                .flatten()
                .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "on"))
                .unwrap_or(default)
        };
        let enabled = get("obfuscation_enabled", self.config.obfuscation_enabled);
        let required = get("obfuscation_required", self.config.obfuscation_required);
        (enabled, required)
    }

    /// Effective eD2K TCP port (persisted `ed2k_port` or startup config).
    pub fn ed2k_port(&self) -> u16 {
        self.ed2k_port_eff.load(Ordering::Relaxed)
    }

    /// Effective Kad UDP port (persisted `kad_port` or startup config).
    pub fn kad_port(&self) -> u16 {
        self.kad_port_eff.load(Ordering::Relaxed)
    }

    /// Whether UDP services (Kad UDP, server UDP, client UDP) are enabled.
    pub fn udp_enabled(&self) -> bool {
        self.udp_enabled.load(Ordering::Relaxed)
    }

    /// Updates the effective ports / UDP flag (called by `settings.set`).
    pub fn set_effective_ports(&self, ed2k: u16, kad: u16) {
        self.ed2k_port_eff.store(ed2k, Ordering::Relaxed);
        self.kad_port_eff.store(kad, Ordering::Relaxed);
    }

    pub fn set_udp_enabled(&self, enabled: bool) {
        self.udp_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Effective nickname announced in eD2K hellos / server login
    /// (eMule `nick` pref). Default: "eMule-Rust".
    pub async fn current_nickname(&self) -> String {
        self.downloads
            .lock()
            .await
            .get_setting("nickname")
            .ok()
            .flatten()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "eMule-Rust".to_string())
    }

    pub async fn register_task(&self, handle: tokio::task::AbortHandle) {
        let mut handles = self.abort_handles.lock().await;
        handles.push(handle);
    }

    /// Aborts all active background tasks associated with this daemon state.
    pub async fn abort_all_tasks(&self) {
        let mut handles = self.abort_handles.lock().await;
        for handle in handles.drain(..) {
            handle.abort();
        }
        #[cfg(feature = "lab-tools")]
        {
            let mut peer = self.active_lab_peer.lock().await;
            if let Some((_, h)) = peer.take() {
                h.abort();
            }
        }
    }

    /// Emits a sequenced download update event to all active WebSocket subscribers.
    pub fn broadcast_download_update(&self, mut snapshot: DownloadSnapshot) {
        // Attach the swarm's in-flight block ranges (eMule "pending" yellow
        // segments) — the downloads service can't see peer sessions itself.
        snapshot.pending_ranges =
            self.peer_session.try_pending_ranges(&snapshot.ed2k_hash);
        self.emit_event(|sequence| DaemonEvent::DownloadUpdated {
            sequence,
            download: snapshot,
        });
    }

    /// Emits any sequenced daemon event. The single entry point for
    /// handlers/services that don't own the sequence counter.
    pub fn emit_event(&self, make: impl FnOnce(u64) -> DaemonEvent) {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        make(seq).send(&self.event_tx);
    }

    /// Reads a persisted boolean setting (defaults to `default` when absent).
    pub async fn setting_bool(&self, key: &str, default: bool) -> bool {
        self.downloads
            .lock()
            .await
            .get_setting(key)
            .ok()
            .flatten()
            .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "on"))
            .unwrap_or(default)
    }

    /// Auto-connect / auto-reconnect watcher (eMule `Autoconnect`,
    /// `Reconnect`, `AutoDeadServer`). Every 15 s: if no server session is
    /// active and a target is available, reconnects through the normal RPC
    /// path (network policy, kill switch and disabled-server checks apply).
    /// `server_autoremove_dead` deletes a target after `DeadServerRetry`
    /// consecutive failures (eMule 0.72a default = 1). Call once after
    /// construction.
    pub fn spawn_server_watcher(&self) {
        // `AppState` fields are all shared `Arc`s — a clone still points at
        // the same services/session managers as the RPC state.
        let state = Arc::new(self.clone());
        let handle = tokio::spawn(async move {
            let mut failures: u32 = 0;
            let mut autoconnect_done = false;
            let mut serverlist_updated = false;
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(15));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                // eMule `AutoUpdateServerList`: refresh the server list once
                // per session from a configured local `.met` file
                // (`server_autoupdate_path` setting — no remote fetch; the
                // daemon deliberately has no outbound HTTP client).
                if !serverlist_updated {
                    serverlist_updated = true;
                    if state.setting_bool("server_autoupdate_list", false).await {
                        let path = {
                            let dl = state.downloads.lock().await;
                            dl.get_setting("server_autoupdate_path")
                                .ok()
                                .flatten()
                                .unwrap_or_default()
                        };
                        if !path.trim().is_empty() {
                            let params =
                                serde_json::json!({ "filePath": path.trim() });
                            match crate::rpc::handlers::servers::handle_servers_import_met(
                                &state,
                                Some(params),
                            )
                            .await
                            {
                                Ok(v) => tracing::info!("[SERVERLIST] auto-update: {v}"),
                                Err(e) => {
                                    tracing::warn!("[SERVERLIST] auto-update failed: {e}")
                                }
                            }
                        }
                    }
                }
                if state.server_session.is_active().await {
                    failures = 0;
                    continue;
                }
                // Kill switch: stay silent until the network is re-enabled.
                if state.network_policy.state() == mule_network_policy::NetworkState::Disabled {
                    continue;
                }
                let reconnect = state.setting_bool("server_reconnect", true).await;
                let target = if reconnect {
                    state.server_session.last_target()
                } else {
                    None
                };
                let target = match target {
                    Some(t) => Some(t),
                    None if !autoconnect_done
                        && state.setting_bool("server_autoconnect", false).await =>
                    {
                        autoconnect_done = true;
                        let servers = {
                            let dl = state.downloads.lock().await;
                            dl.list_servers().unwrap_or_default()
                        };
                        servers
                            .iter()
                            .filter(|s| {
                                s.filter_status
                                    != mule_core::model::ServerFilterStatus::Disabled
                            })
                            .max_by_key(|s| s.preference)
                            .or_else(|| servers.first())
                            .and_then(|s| s.address.parse::<SocketAddr>().ok())
                            .map(|a| crate::services::server_session::ServerTarget {
                                addr: a,
                                name: None,
                            })
                    }
                    None => continue,
                };
                let Some(t) = target else { continue };
                let addr = t.addr;
                let params = serde_json::json!({ "address": addr.to_string() });
                match crate::rpc::handlers::servers::handle_servers_connect(&state, Some(params))
                    .await
                {
                    Ok(_) => {
                        failures = 0;
                    }
                    Err(e) => {
                        failures += 1;
                        tracing::debug!("[SERVER_RECONNECT] attempt {failures} failed: {e}");
                        // eMule `DeadServerRetry` default = 1.
                        if failures >= 1
                            && state.setting_bool("server_autoremove_dead", true).await
                        {
                            let removed = {
                                let mut dl = state.downloads.lock().await;
                                dl.delete_server(&addr.to_string()).unwrap_or(false)
                            };
                            if removed {
                                tracing::info!(
                                    "[SERVER_RECONNECT] auto-removed dead server {addr} after {failures} failures"
                                );
                            }
                            state.server_session.clear_last_target();
                            failures = 0;
                        }
                    }
                }
            }
        });
        let abort = handle.abort_handle();
        self.abort_handles
            .try_lock()
            .map(|mut h| h.push(abort))
            .ok();
    }

    /// Régénère l'identité cryptographique locale (clé RSA SUI, user_hash
    /// eD2K, KadID) et la propage à chaud à tous les consommateurs :
    /// listener entrant (hellos + challenges SUI), identité firewall Kad,
    /// table de routage Kad. Les sessions déjà établies conservent leur
    /// identité d'origine ; les nouvelles utilisent la nouvelle.
    /// Appelée par `identity.rotate` et par la rotation périodique
    /// `identityRotationHours`. Les crédits associés à l'ancienne clé ne
    /// s'appliquent plus (comportement voulu — nouvelle identité).
    pub async fn rotate_identity(
        &self,
    ) -> Result<crate::services::identity::IdentityStatus, String> {
        let new_id = crate::services::IdentityService::rotate_key(&self.config.data_dir)
            .map_err(|e| format!("Failed to rotate identity key: {e}"))?;
        let user_hash = new_id.user_hash();
        let pubkey = new_id
            .has_key()
            .then(|| new_id.public_key_bytes().to_vec());
        {
            let mut guard = self.identity.lock().await;
            *guard = new_id;
        }
        self.inbound_listener.set_identity_material(user_hash, pubkey);
        self.kad.set_fw_check_identity(user_hash);
        let new_kad_id = self.kad.rotate_local_id().await;
        tracing::info!(
            "[IDENTITY] Rotation effectuée — nouveau user_hash {} / KadID {}",
            hex::encode(user_hash),
            new_kad_id
        );
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if let Err(e) = self
            .downloads
            .lock()
            .await
            .set_setting("identity_last_rotation_unix", &now.to_string())
        {
            tracing::warn!("[IDENTITY] Échec persistance last_rotation: {e}");
        }
        Ok(self.identity.lock().await.status())
    }

    /// Rotation périodique de l'identité (`identityRotationHours`, défaut 0 =
    /// désactivé). Vérifie toutes les 60 s si l'intervalle est écoulé depuis
    /// `identity_last_rotation_unix` (persisté — survit aux redémarrages).
    /// Call once after construction.
    pub fn spawn_identity_rotator(&self) {
        let state = Arc::new(self.clone());
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let (hours, last) = {
                    let dl = state.downloads.lock().await;
                    let h = dl
                        .get_setting("identity_rotation_hours")
                        .ok()
                        .flatten()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        .unwrap_or(0);
                    let l = dl
                        .get_setting("identity_last_rotation_unix")
                        .ok()
                        .flatten()
                        .and_then(|s| s.trim().parse::<i64>().ok())
                        .unwrap_or(0);
                    (h, l)
                };
                if hours == 0 {
                    continue;
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                if last <= 0 {
                    // Première activation : le démarrage compte comme point
                    // de référence (évite une rotation immédiate au boot).
                    let _ = state
                        .downloads
                        .lock()
                        .await
                        .set_setting("identity_last_rotation_unix", &now.to_string());
                    continue;
                }
                if now.saturating_sub(last) < (hours * 3600) as i64 {
                    continue;
                }
                match state.rotate_identity().await {
                    Ok(_) => tracing::info!(
                        "[IDENTITY] Rotation périodique effectuée (intervalle {} h)",
                        hours
                    ),
                    Err(e) => tracing::warn!("[IDENTITY] Rotation périodique échouée: {e}"),
                }
            }
        });
        let abort = handle.abort_handle();
        self.abort_handles
            .try_lock()
            .map(|mut h| h.push(abort))
            .ok();
    }

    /// Quota journalier d'upload (`dailyUploadQuotaGb`, défaut 0 = illimité).
    /// Toutes les 30 s : cumule le compteur `UploadManager` dans le compteur
    /// persisté `upload_quota_used_bytes`, remis à zéro à chaque nouveau jour
    /// UTC (`upload_quota_day` = numéro de jour Unix). Quota atteint → le gate
    /// coupe les nouvelles demandes de slot ; quota levé/remis → gate rouvert.
    /// Call once after construction.
    pub fn spawn_quota_watcher(&self) {
        let state = Arc::new(self.clone());
        let gate = Arc::clone(&self.upload_quota_gate);
        let uploads = self.inbound_listener.upload_manager().clone();
        let handle = tokio::spawn(async move {
            let mut last_total = uploads.total_uploaded_bytes();
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(30));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let total = uploads.total_uploaded_bytes();
                let delta = total.saturating_sub(last_total);
                last_total = total;
                let now_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                let today = now_secs / 86_400;
                let mut dl = state.downloads.lock().await;
                let get_u64 = |key: &str| {
                    dl.get_setting(key)
                        .ok()
                        .flatten()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                };
                let quota_gb = get_u64("daily_upload_quota_gb").unwrap_or(0);
                let stored_day = get_u64("upload_quota_day").unwrap_or(0) as i64;
                let mut used = if stored_day == today {
                    get_u64("upload_quota_used_bytes").unwrap_or(0)
                } else {
                    0
                };
                if delta > 0 || stored_day != today {
                    used = used.saturating_add(delta);
                    if let Err(e) = dl.set_setting("upload_quota_day", &today.to_string()) {
                        tracing::warn!("[QUOTA] Échec persistance upload_quota_day: {e}");
                    }
                    if let Err(e) =
                        dl.set_setting("upload_quota_used_bytes", &used.to_string())
                    {
                        tracing::warn!("[QUOTA] Échec persistance upload_quota_used_bytes: {e}");
                    }
                }
                let exhausted =
                    quota_gb > 0 && used >= quota_gb.saturating_mul(1 << 30);
                let prev = gate.swap(!exhausted, Ordering::SeqCst);
                if prev && exhausted {
                    tracing::warn!(
                        "[QUOTA] Quota journalier d'upload atteint ({} Gio) — nouveaux slots refusés",
                        quota_gb
                    );
                } else if !prev && !exhausted {
                    tracing::info!("[QUOTA] Quota d'upload réouvert (compteur remis à zéro ou quota relevé)");
                }
            }
        });
        let abort = handle.abort_handle();
        self.abort_handles
            .try_lock()
            .map(|mut h| h.push(abort))
            .ok();
    }
}
