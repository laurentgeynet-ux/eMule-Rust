// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! USS (Upload Speed Sense) — dynamic upload limiter based on measured
//! network latency, adapted from eMule's `LastCommonRouteFinder`.
//!
//! Like eMule, the probe target is the "last common host": ICMP routes are
//! traced toward a sample of remote peers (server, upload clients,
//! download sources) and the deepest hop shared by all routes — typically
//! the ISP aggregation router — is pinged. That measures congestion on the
//! *local* uplink specifically. On Windows this uses `IcmpSendEcho`
//! (unprivileged); without a common host or on other platforms it falls
//! back to TCP-connect RTT against the connected eD2K server or a
//! configured `host:port`. The control law itself is the same:
//!
//! ```text
//! soll_ping = ms_mode ? tolerance_ms : initial_ping * ((tolerance-100)/100)
//! normalized_ping = median(last N pings) - initial_ping
//! hping = soll_ping - normalized_ping
//! hping < 0 -> upload += hping * 1024 * 10 / (goingDownDivider * initial_ping)
//! hping > 0 -> upload += hping * 1024 * 10 / (goingUpDivider * initial_ping)
//! clamp [min_upload, max_upload]
//! ```
//!
//! When latency is above tolerance, `accept_new_client` goes false — the
//! upload queue then keeps new clients waiting (eMule `AcceptNewClient`).

use crate::services::peer_session::log_peer_ip;
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::task::AbortHandle;

use mule_network_policy::cancellation::CancellationToken;
use mule_network_policy::PolicyRateLimiters;

use crate::services::downloads::DownloadService;
use crate::services::server_session::{ServerConnectionStatus, ServerSessionManager};
use crate::services::upload_queue::UploadManager;
use crate::services::uss_icmp;

const USS_SETTINGS_KEY: &str = "uss_config";
/// eMule `USSEnabled` default: off.
const DEFAULT_ENABLED: bool = false;
/// eMule `USSPingTolerance` default: 500 → `(500-100)/100 = 4.0` × initial ping.
const DEFAULT_TOLERANCE_PERCENT: u32 = 500;
/// eMule `USSPingToleranceMilliseconds` default: 200 ms.
const DEFAULT_TOLERANCE_MS: u32 = 200;
/// eMule `USSGoingUpDivider` / `USSGoingDownDivider` defaults.
const DEFAULT_DIVIDER: u32 = 1000;
/// eMule `USSNumberOfPings` default: 1.
const DEFAULT_NUM_PINGS: usize = 1;
/// Hard-coded `uLowestInitialPingAllowed` in `UploadQueue.cpp` (20 ms).
const LOWEST_INITIAL_PING_MS: u32 = 20;
/// Probes used to establish the baseline RTT (eMule `initialPingCounter = 10`).
const BASELINE_PROBES: usize = 10;
/// Probe timeout.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
/// ICMP probe timeout (last-common-host ping).
const ICMP_PROBE_TIMEOUT_MS: u32 = 2000;
/// Give up the baseline/control loop after this many consecutive probe
/// failures (eMule restarts after 60).
const MAX_CONSECUTIVE_FAILURES: u32 = 60;
/// Resolved ping target is cached this long — route discovery runs at
/// most once per TTL (eMule re-evaluates its host set periodically).
const TARGET_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(15 * 60);
/// Max traceroute destinations sampled (eMule keeps a small host set).
const MAX_TRACE_DESTINATIONS: usize = 8;

/// Probe target — `Icmp` is the discovered last-common-host (eMule LCRF),
/// `Tcp` is the connect-RTT fallback (explicit `pingHost` or connected
/// eD2K server).
#[derive(Debug, Clone, PartialEq, Eq)]
enum PingTarget {
    Icmp(std::net::Ipv4Addr),
    Tcp(SocketAddr),
}

impl std::fmt::Display for PingTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PingTarget::Icmp(ip) => write!(f, "icmp:{ip}"),
            PingTarget::Tcp(addr) => write!(f, "tcp:{addr}"),
        }
    }
}
/// Upload speed never goes below this floor when USS drives it (1 KiB/s,
/// eMule `m_minUpload` lower bound).
const MIN_UPLOAD_BPS: u64 = 1024;

/// USS configuration, persisted as JSON in the settings table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UssConfig {
    pub enabled: bool,
    /// Optional `host:port` probe target; falls back to the connected server.
    pub ping_host: Option<String>,
    /// Lower bound of the dynamic limit (bytes/sec).
    pub min_upload_bps: u64,
    /// Upper bound of the dynamic limit (bytes/sec); 0 = uncapped
    /// (eMule `UNLIMITED` — the limit then floats freely).
    pub max_upload_bps: u64,
    /// true → `soll_ping = ping_tolerance_ms`; false → percent mode.
    pub use_ms_tolerance: bool,
    /// Raw eMule `USSPingTolerance` value; factor = (v-100)/100.
    pub ping_tolerance_percent: u32,
    /// Target ping ceiling in ms (ms-tolerance mode).
    pub ping_tolerance_ms: u32,
    pub going_up_divider: u32,
    pub going_down_divider: u32,
    /// Median window size (`USSNumberOfPings`).
    pub number_of_pings: usize,
}

impl Default for UssConfig {
    fn default() -> Self {
        Self {
            enabled: DEFAULT_ENABLED,
            ping_host: None,
            min_upload_bps: 10 * 1024,
            max_upload_bps: 0,
            use_ms_tolerance: false,
            ping_tolerance_percent: DEFAULT_TOLERANCE_PERCENT,
            ping_tolerance_ms: DEFAULT_TOLERANCE_MS,
            going_up_divider: DEFAULT_DIVIDER,
            going_down_divider: DEFAULT_DIVIDER,
            number_of_pings: DEFAULT_NUM_PINGS,
        }
    }
}

/// Runtime snapshot exposed via RPC.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UssSnapshot {
    pub enabled: bool,
    /// `disabled` | `waiting` (no probe target) | `preparing` (baseline) |
    /// `active` | `error`.
    pub state: &'static str,
    /// Current dynamic upload limit in bytes/sec (0 when USS is off).
    pub current_limit_bps: u64,
    /// Median latency of the last probes (ms).
    pub latency_ms: u32,
    /// Baseline (lowest observed) latency (ms).
    pub lowest_ping_ms: u32,
    /// Whether new upload clients are currently accepted.
    pub accept_new_client: bool,
    /// Probe target actually in use.
    pub ping_target: Option<String>,
}

struct UssInner {
    config: UssConfig,
    state: &'static str,
    current_limit_bps: u64,
    latency_ms: u32,
    lowest_ping_ms: u32,
    ping_target: Option<String>,
    /// Upload limit configured manually via `limits.set` — restored when
    /// USS is disabled.
    manual_limit_bps: u64,
    /// Download limit *configurée* (`rate_limit_download_bps`) — le bucket
    /// download est replafonné à `emule_ud_ratio(dl_cfg, upload)` à chaque
    /// ajustement dynamique (eMule `GetMaxDownloadInBytesPerSec` dynamique).
    download_limit_bps: u64,
}

/// USS controller. Drives `upload_bucket` when enabled.
#[derive(Clone)]
pub struct UssService {
    inner: Arc<RwLock<UssInner>>,
    accept_new_client: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    task: Arc<Mutex<Option<AbortHandle>>>,
    server_session: Arc<RwLock<Option<ServerSessionManager>>>,
    upload_manager: Arc<RwLock<Option<UploadManager>>>,
    downloads: Arc<RwLock<Option<Arc<tokio::sync::Mutex<DownloadService>>>>>,
}

impl Default for UssService {
    fn default() -> Self {
        Self::new()
    }
}

impl UssService {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(UssInner {
                config: UssConfig::default(),
                state: "disabled",
                current_limit_bps: 0,
                latency_ms: 0,
                lowest_ping_ms: 0,
                ping_target: None,
                manual_limit_bps: 0,
                download_limit_bps: 0,
            })),
            accept_new_client: Arc::new(AtomicBool::new(true)),
            running: Arc::new(AtomicBool::new(false)),
            task: Arc::new(Mutex::new(None)),
            server_session: Arc::new(RwLock::new(None)),
            upload_manager: Arc::new(RwLock::new(None)),
            downloads: Arc::new(RwLock::new(None)),
        }
    }

    pub fn set_server_session(&self, mgr: ServerSessionManager) {
        *self.server_session.write().unwrap() = Some(mgr);
    }

    pub fn set_upload_manager(&self, mgr: UploadManager) {
        *self.upload_manager.write().unwrap() = Some(mgr);
    }

    pub fn set_downloads(&self, dl: Arc<tokio::sync::Mutex<DownloadService>>) {
        *self.downloads.write().unwrap() = Some(dl);
        // Persisted config is loaded lazily by the control loop's first
        // iteration (avoids blocking in `AppState::new`).
    }

    /// Shared flag consulted by `UploadManager` before granting slots
    /// (eMule `LastCommonRouteFinder::AcceptNewClient`).
    pub fn accept_flag(&self) -> Arc<AtomicBool> {
        self.accept_new_client.clone()
    }

    pub fn config(&self) -> UssConfig {
        self.inner.read().unwrap().config.clone()
    }

    pub fn snapshot(&self) -> UssSnapshot {
        let inner = self.inner.read().unwrap();
        UssSnapshot {
            enabled: inner.config.enabled,
            state: inner.state,
            current_limit_bps: inner.current_limit_bps,
            latency_ms: inner.latency_ms,
            lowest_ping_ms: inner.lowest_ping_ms,
            accept_new_client: self.accept_new_client.load(Ordering::Relaxed),
            ping_target: inner.ping_target.clone(),
        }
    }

    /// Applies a new config (partial update), persists it, and nudges the
    /// control loop by restarting the task.
    pub async fn update_config<F: FnOnce(&mut UssConfig)>(&self, apply: F) {
        let (json, dl) = {
            let mut inner = self.inner.write().unwrap();
            apply(&mut inner.config);
            if !inner.config.enabled {
                inner.state = "disabled";
                inner.current_limit_bps = 0;
                self.accept_new_client.store(true, Ordering::Relaxed);
            }
            let dl = self.downloads.read().unwrap().clone();
            (serde_json::to_string(&inner.config).ok(), dl)
        };
        if let (Some(json), Some(dl)) = (json, dl) {
            let _ = dl.lock().await.set_setting(USS_SETTINGS_KEY, &json);
        }
    }

    /// Records the manual upload limit configured via `limits.set` so USS
    /// can restore it when disabled.
    pub fn note_manual_limit(&self, bps: u64) {
        self.inner.write().unwrap().manual_limit_bps = bps;
    }

    /// Records the *configured* download limit (`limits.set`, capacity
    /// clamp, boot) so the control loop can re-cap the download bucket
    /// with the eMule upload/download ratio on every dynamic update.
    pub fn note_manual_download_limit(&self, bps: u64) {
        self.inner.write().unwrap().download_limit_bps = bps;
    }

    /// Starts the background control loop. Idempotent.
    pub fn start(&self, limiters: Arc<PolicyRateLimiters>, cancel: CancellationToken) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let svc = self.clone();
        let running = Arc::clone(&self.running);
        let handle = tokio::spawn(async move {
            svc.run(limiters, cancel).await;
            // Le kill switch annule la boucle via le token ; on libère le
            // drapeau pour que `network.enable` puisse relancer le service.
            running.store(false, Ordering::SeqCst);
        });
        if let Ok(mut t) = self.task.try_lock() {
            *t = Some(handle.abort_handle());
        }
    }

    async fn run(&self, limiters: Arc<PolicyRateLimiters>, cancel: CancellationToken) {
        let upload_bucket = &limiters.upload_bucket;
        let download_bucket = &limiters.download_bucket;
        let mut delays: VecDeque<u32> = VecDeque::new();
        let mut initial_ping_ms: u32 = 0;
        let mut upload_bps: u64 = 0;
        let mut consecutive_failures: u32 = 0;
        let mut baseline_done = false;
        let loop_start = Instant::now();
        // Cible de ping résolue + horodatage — la découverte du dernier
        // routeur commun (traceroutes ICMP) est coûteuse : on la rejoue au
        // plus toutes les TARGET_CACHE_TTL, ou après re-baseline.
        let mut cached_target: Option<(PingTarget, Instant)> = None;

        // Load persisted config once.
        let dl_opt = { self.downloads.read().unwrap().clone() };
        if let Some(dl) = dl_opt {
            if let Ok(Some(json)) = dl.lock().await.get_setting(USS_SETTINGS_KEY) {
                if let Ok(cfg) = serde_json::from_str::<UssConfig>(&json) {
                    self.inner.write().unwrap().config = cfg;
                }
            }
            // Limite download configurée persistée (ratio eMule dynamique).
            if let Ok(Some(s)) = dl.lock().await.get_setting("rate_limit_download_bps") {
                if let Ok(v) = s.trim().parse::<u64>() {
                    self.inner.write().unwrap().download_limit_bps = v;
                }
            }
        }

        /// Replafonne le bucket download au ratio eMule
        /// (`GetMaxDownloadInBytesPerSec`) pour la limite upload courante.
        macro_rules! apply_ud_ratio {
            ($ul_eff:expr) => {{
                let dl_cfg = self.inner.read().unwrap().download_limit_bps;
                let eff = mule_network_policy::emule_ud_ratio(dl_cfg, $ul_eff);
                download_bucket.set_rate(eff, eff.max(65536));
            }};
        }

        loop {
            if cancel.is_cancelled() {
                return;
            }
            let cfg = self.config();
            if !cfg.enabled {
                // Restore manual limit once per transition.
                {
                    let mut inner = self.inner.write().unwrap();
                    if inner.state != "disabled" || inner.current_limit_bps != 0 {
                        let manual = inner.manual_limit_bps;
                        inner.state = "disabled";
                        inner.current_limit_bps = 0;
                        upload_bucket.set_rate(manual, manual.max(65536));
                        apply_ud_ratio!(manual);
                    }
                }
                baseline_done = false;
                delays.clear();
                initial_ping_ms = 0;
                self.accept_new_client.store(true, Ordering::Relaxed);
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
                }
                continue;
            }

            // Resolve probe target (cached — route discovery is expensive).
            let stale = cached_target
                .as_ref()
                .is_none_or(|(_, at)| at.elapsed() >= TARGET_CACHE_TTL);
            if stale {
                cached_target = self.resolve_target(&cfg).await.map(|t| (t, Instant::now()));
            }
            let Some((target, _)) = cached_target.clone() else {
                self.set_state("waiting");
                baseline_done = false;
                tokio::select! {
                    _ = cancel.cancelled() => return,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
                }
                continue;
            };
            self.inner.write().unwrap().ping_target = Some(target.to_string());

            // --- Baseline: lowest of up to BASELINE_PROBES probes ---
            if !baseline_done {
                self.set_state("preparing");
                let mut best = u32::MAX;
                let mut successes = 0usize;
                for _ in 0..BASELINE_PROBES {
                    if cancel.is_cancelled() || !self.config().enabled {
                        break;
                    }
                    if let Some(ms) = probe_target(&target).await {
                        successes += 1;
                        if ms > 0 && ms < best {
                            best = ms.max(LOWEST_INITIAL_PING_MS);
                        }
                    }
                    tokio::select! {
                        _ = cancel.cancelled() => return,
                        _ = tokio::time::sleep(std::time::Duration::from_millis(200)) => {}
                    }
                }
                if !self.config().enabled {
                    continue;
                }
                if successes == 0 {
                    // Target unreachable — wait before retrying.
                    self.set_state("waiting");
                    tokio::select! {
                        _ = cancel.cancelled() => return,
                        _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {}
                    }
                    continue;
                }
                initial_ping_ms = if best == u32::MAX {
                    LOWEST_INITIAL_PING_MS
                } else {
                    best
                };
                // eMule: upload = max(curUpload, minUpload), clamped to max.
                let cur_upload = self.current_upload_rate().await;
                let max = cfg.max_upload_bps;
                upload_bps = cur_upload.max(cfg.min_upload_bps.max(MIN_UPLOAD_BPS));
                if max > 0 {
                    upload_bps = upload_bps.min(max);
                }
                upload_bucket.set_rate(upload_bps, upload_bps.max(65536));
                apply_ud_ratio!(upload_bps);
                delays.clear();
                consecutive_failures = 0;
                baseline_done = true;
                self.set_state("active");
                self.inner.write().unwrap().lowest_ping_ms = initial_ping_ms;
            }

            // --- Control loop ---
            // eMule: ticksBetweenPings = 100*64/upload*1000 ms, clamped
            // [125ms, 1s] — probes use ~1% of upload bandwidth.
            let interval_ms = 6_400_000u64
                .checked_div(upload_bps)
                .map_or(1000, |v| v.clamp(125, 1000));
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(std::time::Duration::from_millis(interval_ms)) => {}
            }
            if cancel.is_cancelled() {
                return;
            }
            let cfg = self.config();
            if !cfg.enabled {
                continue;
            }

            // soll_ping: ms mode uses the fixed tolerance; percent mode
            // uses initial_ping * (tolerance-100)/100 (UploadQueue.cpp:921).
            let soll_ping: i64 = if cfg.use_ms_tolerance {
                cfg.ping_tolerance_ms as i64
            } else {
                let factor = (cfg.ping_tolerance_percent.max(101) - 100) as f64 / 100.0;
                (initial_ping_ms as f64 * factor) as i64
            };

            let raw_ping: i64 = match probe_target(&target).await {
                Some(ms) => {
                    consecutive_failures = 0;
                    let ms = ms.max(1);
                    // Track new lowest ping (eMule lowers initial_ping).
                    if ms < initial_ping_ms && initial_ping_ms > LOWEST_INITIAL_PING_MS {
                        initial_ping_ms = ms.max(LOWEST_INITIAL_PING_MS);
                        self.inner.write().unwrap().lowest_ping_ms = initial_ping_ms;
                    }
                    ms as i64
                }
                None => {
                    consecutive_failures += 1;
                    // eMule: failing ping counts as 3*(soll+initial) → forces
                    // a decrease.
                    (soll_ping + initial_ping_ms as i64) * 3
                }
            };

            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                // Re-baseline: topology/target may have changed — force
                // target re-resolution (route discovery) too.
                baseline_done = false;
                cached_target = None;
                delays.clear();
                continue;
            }

            delays.push_back(raw_ping.max(0) as u32);
            while delays.len() > cfg.number_of_pings.max(1) {
                delays.pop_front();
            }
            let ping_average = median(&delays);

            // eMule ramps up/down dividers for the first ~60s after start.
            let elapsed = loop_start.elapsed().as_secs();
            let (mut up_div, mut down_div) =
                (cfg.going_up_divider.max(1), cfg.going_down_divider.max(1));
            if elapsed < 20 {
                up_div = 1;
                down_div = 1;
            } else {
                let mul = if elapsed < 30 {
                    1
                } else if elapsed < 40 {
                    2
                } else if elapsed < 60 {
                    3
                } else {
                    4
                };
                down_div = (down_div * mul / 4).max(1);
                up_div = (up_div * mul / 4).max(1);
            }

            let normalized_ping = ping_average as i64 - initial_ping_ms as i64;
            let hping = soll_ping - normalized_ping;

            let cur_upload = self.current_upload_rate().await;
            if hping < 0 {
                // Latency above tolerance → lower the rate, refuse new clients.
                self.accept_new_client.store(false, Ordering::Relaxed);
                let diff = hping * 1024 * 10 / down_div as i64 / initial_ping_ms.max(1) as i64;
                upload_bps = if upload_bps as i64 + diff > 0 {
                    (upload_bps as i64 + diff) as u64
                } else {
                    0
                };
            } else if hping > 0 {
                self.accept_new_client.store(true, Ordering::Relaxed);
                // eMule only raises while actual throughput tracks the limit
                // (curUpload + 30KB > upload).
                if cur_upload + 30 * 1024 > upload_bps {
                    let diff = hping * 1024 * 10 / (up_div as i64 * initial_ping_ms.max(1) as i64);
                    upload_bps = upload_bps.saturating_add(diff.max(0) as u64);
                }
            }

            upload_bps = upload_bps.max(cfg.min_upload_bps.max(MIN_UPLOAD_BPS));
            if cfg.max_upload_bps > 0 {
                upload_bps = upload_bps.min(cfg.max_upload_bps);
            }
            upload_bucket.set_rate(upload_bps, upload_bps.max(65536));
            apply_ud_ratio!(upload_bps);

            {
                let mut inner = self.inner.write().unwrap();
                inner.current_limit_bps = upload_bps;
                inner.latency_ms = ping_average;
            }
        }
    }

    /// Current achieved upload throughput (sum of active slot rates) —
    /// eMule uses `uploadqueue->GetDatarate()` as `curUpload`.
    async fn current_upload_rate(&self) -> u64 {
        let mgr = self.upload_manager.read().unwrap().clone();
        match mgr {
            Some(mgr) => mgr
                .active_snapshots()
                .await
                .iter()
                .map(|s| s.upload_rate_bps)
                .sum(),
            None => 0,
        }
    }

    async fn resolve_target(&self, cfg: &UssConfig) -> Option<PingTarget> {
        // Explicit override wins (`uss.pingHost` in `limits.set`).
        if let Some(host) = &cfg.ping_host {
            if let Ok(addr) = host.parse::<SocketAddr>() {
                return Some(PingTarget::Tcp(addr));
            }
        }
        // eMule `LastCommonRouteFinder`: trace ICMP routes toward a sample
        // of remote hosts and ping the deepest shared hop — the ISP-side
        // router where local uplink congestion actually shows up.
        if let Some(ip) = self.discover_last_common_host().await {
            tracing::info!(
                "USS: last common host discovered, pinging {peer}",
                peer = log_peer_ip(ip)
            );
            return Some(PingTarget::Icmp(ip));
        }
        // Fallback: TCP-connect RTT to the connected eD2K server.
        let mgr = self.server_session.read().unwrap().clone()?;
        let snap = mgr.snapshot().await;
        if snap.status != ServerConnectionStatus::Connected {
            return None;
        }
        snap.server_address
            .and_then(|s| s.parse().ok())
            .map(PingTarget::Tcp)
    }

    /// Traces ICMP routes toward known remote peers (connected server,
    /// upload clients, download sources) and returns the deepest hop
    /// common to all routes. `None` when ICMP is unavailable or fewer
    /// than two distinct destinations are known (a single route has no
    /// meaningful "common" hop).
    async fn discover_last_common_host(&self) -> Option<std::net::Ipv4Addr> {
        if !uss_icmp::icmp_available() {
            return None;
        }
        let dests = self.collect_destinations().await;
        if dests.len() < 2 {
            return None;
        }
        let mut routes = Vec::new();
        for dest in dests {
            match uss_icmp::trace_route(dest).await {
                Some(hops) => routes.push(hops),
                None => tracing::debug!("USS: no ICMP route to {peer}", peer = log_peer_ip(dest)),
            }
        }
        let host = uss_icmp::last_common_host(&routes);
        if host.is_none() && !routes.is_empty() {
            tracing::debug!("USS: {} routes traced but no common hop", routes.len());
        }
        host
    }

    /// Distinct public IPv4 destinations for route discovery: connected
    /// eD2K server, active upload clients and download sources (eMule
    /// traces toward peers it is connected to).
    async fn collect_destinations(&self) -> Vec<std::net::Ipv4Addr> {
        let mut ips: Vec<std::net::Ipv4Addr> = Vec::new();

        let server = { self.server_session.read().unwrap().clone() };
        if let Some(mgr) = server {
            let snap = mgr.snapshot().await;
            if snap.status == ServerConnectionStatus::Connected {
                if let Some(Ok(SocketAddr::V4(a))) =
                    snap.server_address.map(|s| s.parse::<SocketAddr>())
                {
                    ips.push(*a.ip());
                }
            }
        }
        let uploader = { self.upload_manager.read().unwrap().clone() };
        if let Some(mgr) = uploader {
            for s in mgr.active_snapshots().await {
                if let Ok(SocketAddr::V4(a)) = s.client_address.parse::<SocketAddr>() {
                    ips.push(*a.ip());
                }
            }
        }
        let downloads = { self.downloads.read().unwrap().clone() };
        if let Some(dl) = downloads {
            let dl = dl.lock().await;
            for d in dl.list_downloads() {
                if let Some(sources) = dl.get_sources(&d.id) {
                    for addr in sources.keys() {
                        if let SocketAddr::V4(a) = addr {
                            ips.push(*a.ip());
                        }
                    }
                }
            }
        }

        // Keep only public routable IPs — LAN/loopback hops would pollute
        // the common-hop detection (the home gateway is trivially shared).
        ips.retain(|ip| {
            mule_network_policy::classify_ip(std::net::IpAddr::V4(*ip))
                == mule_network_policy::IpCategory::PublicRoutable
        });
        ips.sort_unstable();
        ips.dedup();
        ips.truncate(MAX_TRACE_DESTINATIONS);
        ips
    }

    fn set_state(&self, state: &'static str) {
        self.inner.write().unwrap().state = state;
    }
}

/// Measures RTT to `target` in milliseconds — ICMP echo for the
/// discovered last-common-host (eMule behaviour), TCP connect for the
/// fallback targets.
async fn probe_target(target: &PingTarget) -> Option<u32> {
    match target {
        PingTarget::Icmp(ip) => uss_icmp::ping(*ip, ICMP_PROBE_TIMEOUT_MS).await,
        PingTarget::Tcp(addr) => probe_tcp_rtt(*addr).await,
    }
}

/// Measures TCP connect RTT to `addr` in milliseconds.
async fn probe_tcp_rtt(addr: SocketAddr) -> Option<u32> {
    let start = Instant::now();
    match tokio::time::timeout(PROBE_TIMEOUT, tokio::net::TcpStream::connect(addr)).await {
        Ok(Ok(_stream)) => Some(start.elapsed().as_millis() as u32),
        _ => None,
    }
}

/// Median of the delay window (`LastCommonRouteFinder::Median`).
fn median(delays: &VecDeque<u32>) -> u32 {
    if delays.is_empty() {
        return 0;
    }
    let mut v: Vec<u32> = delays.iter().copied().collect();
    v.sort_unstable();
    v[v.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_window() {
        let mut d = VecDeque::new();
        assert_eq!(median(&d), 0);
        d.extend([10, 30, 20]);
        assert_eq!(median(&d), 20);
        let d2: VecDeque<u32> = [10, 20, 30, 40].into();
        assert_eq!(median(&d2), 30);
    }

    #[test]
    fn default_config_matches_emule() {
        let c = UssConfig::default();
        assert!(!c.enabled);
        assert_eq!(c.ping_tolerance_percent, 500);
        assert_eq!(c.ping_tolerance_ms, 200);
        assert_eq!(c.going_up_divider, 1000);
        assert_eq!(c.going_down_divider, 1000);
        assert_eq!(c.number_of_pings, 1);
    }

    #[tokio::test]
    async fn snapshot_disabled_by_default() {
        let svc = UssService::new();
        let snap = svc.snapshot();
        assert_eq!(snap.state, "disabled");
        assert!(snap.accept_new_client);
        assert!(svc.accept_flag().load(Ordering::Relaxed));
    }
}
