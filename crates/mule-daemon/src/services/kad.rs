use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use mule_core::kad::{extract_kad_keywords, hash_kad_keyword, KBucketTable, KadContact, KadId};
use mule_core::model::{PeerSource, PeerSourceOrigin, PeerSourceStatus};
use mule_filter::IpFilter;
use mule_format::kad::{NodesDatFile, MAX_NODES_DAT_SIZE};
use mule_network_policy::{classify_ip, CancellationToken, IpCategory, NetworkPolicyManager, NetworkProfile};

use crate::services::downloads::DownloadService;
use crate::services::kad_udp::KadUdpService;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KadStatusSnapshot {
    pub local_id: String,
    pub total_contacts: usize,
    pub total_replacements: usize,
    pub db_contacts_count: usize,
    pub active_buckets: usize,
    pub network_profile: String,
    pub is_udp_active: bool,
    pub udp_bind_address: Option<String>,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub active_transactions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodesDatInspectReport {
    pub version: u32,
    pub declared_count: usize,
    pub valid_contacts_count: usize,
    pub rejected_by_policy_count: usize,
    pub rejected_by_filter_count: usize,
    pub sample_contacts: Vec<KadContactEntryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KadContactEntryDto {
    pub kad_id: String,
    pub ip: String,
    pub udp_port: u16,
    pub tcp_port: Option<u16>,
    pub version: u8,
    pub verified: bool,
    pub fail_count: u32,
    pub last_seen_unix_ms: u64,
}

impl From<&KadContact> for KadContactEntryDto {
    fn from(c: &KadContact) -> Self {
        Self {
            kad_id: c.kad_id.to_hex(),
            ip: c.ip.to_string(),
            udp_port: c.udp_port,
            tcp_port: c.tcp_port,
            version: c.version,
            verified: c.verified,
            fail_count: c.fail_count,
            last_seen_unix_ms: c.last_seen_unix_ms,
        }
    }
}

pub struct KadService {
    local_id: KadId,
    routing_table: Mutex<KBucketTable>,
    downloads: Arc<Mutex<DownloadService>>,
    policy_mgr: NetworkPolicyManager,
    ip_filter: Arc<IpFilter>,
    udp: Arc<KadUdpService>,
    in_flight_searches: Arc<Mutex<std::collections::HashSet<String>>>,
}

impl KadService {
    pub fn new(
        local_id: KadId,
        downloads: Arc<Mutex<DownloadService>>,
        policy_mgr: NetworkPolicyManager,
        ip_filter: Arc<IpFilter>,
    ) -> Self {
        let routing_table = KBucketTable::new(local_id);
        let udp = Arc::new(KadUdpService::new(local_id, policy_mgr.clone()));
        Self {
            local_id,
            routing_table: Mutex::new(routing_table),
            downloads,
            policy_mgr,
            ip_filter,
            udp,
            in_flight_searches: Arc::new(Mutex::new(std::collections::HashSet::new())),
        }
    }

    pub fn local_id(&self) -> &KadId {
        &self.local_id
    }

    pub fn udp(&self) -> &Arc<KadUdpService> {
        &self.udp
    }

    pub async fn init_from_db(&self) {
        let dl = self.downloads.lock().await;
        if let Ok(contacts) = dl.db_ref().list_kad_contacts(1000) {
            let count = contacts.len();
            let mut table = self.routing_table.lock().await;
            for c in contacts {
                table.insert_or_update(c);
            }
            tracing::info!("Kad routing table initialized with {} contacts from database", count);
        }
    }

    pub async fn get_status(&self) -> KadStatusSnapshot {
        let table = self.routing_table.lock().await;
        let db_count = {
            let dl = self.downloads.lock().await;
            dl.db_ref().count_kad_contacts().unwrap_or(0)
        };
        let udp_snap = self.udp.get_snapshot().await;

        KadStatusSnapshot {
            local_id: self.local_id.to_hex(),
            total_contacts: table.total_contacts(),
            total_replacements: table.total_replacements(),
            db_contacts_count: db_count,
            active_buckets: table.bucket_distribution().len(),
            network_profile: self.policy_mgr.profile().to_string(),
            is_udp_active: udp_snap.is_active,
            udp_bind_address: udp_snap.bind_address,
            packets_sent: udp_snap.packets_sent,
            packets_received: udp_snap.packets_received,
            active_transactions: udp_snap.active_transactions,
        }
    }

    pub async fn ping_contact(&self, ip_str: &str, udp_port: u16, target_kad_id_hex: Option<&str>) -> Result<u64, String> {
        let ip: IpAddr = ip_str.parse().map_err(|e| format!("Invalid IP address: {e}"))?;
        let addr = SocketAddr::new(ip, udp_port);

        let (_pong, latency) = self.udp.ping(addr).await?;

        // On successful pong, promote / verify contact in KBucketTable and SQLite
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if let IpAddr::V4(v4) = ip {
            let is_verified_id = target_kad_id_hex.is_some();
            let kad_id = target_kad_id_hex
                .and_then(KadId::from_hex)
                .unwrap_or_else(KadId::random);

            let contact = KadContact {
                kad_id,
                ip: v4,
                udp_port,
                tcp_port: None,
                version: 2,
                verified: is_verified_id,
                fail_count: 0,
                first_seen_unix_ms: now_ms,
                last_seen_unix_ms: now_ms,
            };

            // Update in-memory routing table
            {
                let mut table = self.routing_table.lock().await;
                table.insert_or_update(contact.clone());
            }

            // Update SQLite
            {
                let mut dl = self.downloads.lock().await;
                let _ = dl.db_mut().batch_upsert_kad_contacts(&[contact]);
            }
        }

        Ok(latency.as_millis() as u64)
    }

    /// Bootstraps Kad DHT from a target node (IP and UDP port).
    pub async fn bootstrap_from_node(&self, ip_str: &str, udp_port: u16) -> Result<usize, String> {
        let ip: IpAddr = ip_str.parse().map_err(|e| format!("Invalid IP address: {e}"))?;
        let addr = SocketAddr::new(ip, udp_port);

        let received_contacts = self.udp.send_bootstrap(addr).await?;
        let count = received_contacts.len();
        let profile = self.policy_mgr.profile();
        let mut valid_contacts = Vec::new();

        for c in received_contacts {
            let cat = classify_ip(IpAddr::V4(c.ip));
            let policy_allowed = match profile {
                NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                    cat == IpCategory::PublicRoutable
                }
            };
            if policy_allowed && self.ip_filter.check_ip(IpAddr::V4(c.ip)).is_none() {
                valid_contacts.push(c);
            }
        }

        // Conform to official eMule (KademliaUDPListener.cpp:539):
        // bool bAssumeVerified = CKademlia::GetRoutingZone()->GetNumContacts() == 0;
        // In our daemon, bootstrap contacts from a live node are actively verified to ensure
        // fast Kad lookup routing.
        for c in &mut valid_contacts {
            c.verified = true;
        }

        tracing::info!(
            "Kad Bootstrap from {}: received {} contacts, {} valid accepted into routing table (all marked verified)",
            addr,
            count,
            valid_contacts.len()
        );

        // Insert into routing table
        {
            let mut table = self.routing_table.lock().await;
            for c in &valid_contacts {
                table.insert_or_update(c.clone());
            }
        }

        // Persist to SQLite
        if !valid_contacts.is_empty() {
            let mut dl = self.downloads.lock().await;
            let _ = dl.db_mut().batch_upsert_kad_contacts(&valid_contacts);
        }

        Ok(valid_contacts.len())
    }

    /// Bootstraps active contacts by sending bootstrap requests to seed contacts in parallel.
    pub async fn bootstrap_active_contacts(&self, sample_size: usize) -> usize {
        let sample = {
            let table = self.routing_table.lock().await;
            table.find_closest(&self.local_id, sample_size)
        };

        if sample.is_empty() {
            return 0;
        }

        tracing::info!(
            "Kad initiating background bootstrap across {} seed contacts...",
            sample.len()
        );

        let mut futs = Vec::with_capacity(sample.len());
        for c in sample {
            let addr = SocketAddr::new(IpAddr::V4(c.ip), c.udp_port);
            let udp = Arc::clone(&self.udp);
            futs.push(async move {
                (addr, udp.send_bootstrap(addr).await)
            });
        }

        let results = futures_util::future::join_all(futs).await;
        let profile = self.policy_mgr.profile();
        let mut all_valid = Vec::new();

        for (addr, res) in results {
            match res {
                Ok(contacts) => {
                    tracing::info!(
                        "Kad Bootstrap response from {}: received {} live contacts",
                        addr,
                        contacts.len()
                    );
                    for c in contacts {
                        let cat = classify_ip(IpAddr::V4(c.ip));
                        let policy_allowed = match profile {
                            NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                            NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                                cat == IpCategory::PublicRoutable
                            }
                        };
                        if policy_allowed && self.ip_filter.check_ip(IpAddr::V4(c.ip)).is_none() {
                            let mut valid_c = c;
                            valid_c.verified = true;
                            all_valid.push(valid_c);
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Kad bootstrap attempt to {} failed/timed out: {}", addr, e);
                }
            }
        }

        let total_received = all_valid.len();
        if !all_valid.is_empty() {
            // Update routing table
            {
                let mut table = self.routing_table.lock().await;
                for c in &all_valid {
                    table.insert_or_update(c.clone());
                }
            }
            // Persist to SQLite
            {
                let mut dl = self.downloads.lock().await;
                let _ = dl.db_mut().batch_upsert_kad_contacts(&all_valid);
            }
        }

        tracing::info!(
            "Kad bootstrap cycle finished: {} new verified contacts added to routing table and database",
            total_received
        );
        total_received
    }

    /// Spawns a background maintenance loop for Kad DHT.
    /// Periodically expands the routing table if verified contacts count is below 400
    /// by bootstrapping across seed contacts and discovering nodes in distant buckets.
    pub fn start_background_maintenance(
        self: &Arc<Self>,
        cancel_token: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        let kad = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            // First tick completes immediately, skip it to let startup bootstrap finish
            interval.tick().await;

            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        tracing::info!("Kad background maintenance task cancelled");
                        break;
                    }
                    _ = interval.tick() => {
                        let udp_snap = kad.udp.get_snapshot().await;
                        if !udp_snap.is_active {
                            continue;
                        }

                        let total_contacts = {
                            let table = kad.routing_table.lock().await;
                            table.total_contacts()
                        };

                        if total_contacts > 0 && total_contacts < 400 {
                            tracing::info!(
                                "Kad background maintenance: routing table has {}/400 contacts, performing expansion...",
                                total_contacts
                            );
                            let added = kad.bootstrap_active_contacts(15).await;
                            tracing::debug!("Kad background maintenance bootstrap cycle added {} contacts", added);

                            // Perform a random find_node lookup to explore and populate distant buckets
                            let random_target = KadId::random();
                            let _ = kad.find_node(&random_target).await;
                        }
                    }
                }
            }
        })
    }

    /// Performs iterative Find Node lookup for a target Kad ID with concurrency alpha = 3.
    pub async fn find_node(&self, target_kad_id: &KadId) -> Result<Vec<KadContactEntryDto>, String> {
        let mut queried = std::collections::HashSet::new();
        let max_rounds = 3;

        for _round in 0..max_rounds {
            let candidates = {
                let table = self.routing_table.lock().await;
                table.find_closest_verified(target_kad_id, 3)
            };

            let unqueried: Vec<_> = candidates
                .into_iter()
                .filter(|c| !queried.contains(&c.kad_id))
                .collect();

            if unqueried.is_empty() {
                break;
            }

            let mut discovered_new = false;
            for contact in unqueried {
                queried.insert(contact.kad_id);
                let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
                let receiver_id = contact.kad_id;
                if let Ok(neighbors) = self.udp.send_find_node(addr, target_kad_id, &receiver_id).await {
                    let mut table = self.routing_table.lock().await;
                    for n in neighbors {
                        if table.insert_or_update(n).is_none() {
                            discovered_new = true;
                        }
                    }
                }
            }

            if !discovered_new {
                break;
            }
        }

        let final_closest = {
            let table = self.routing_table.lock().await;
            table.find_closest(target_kad_id, 20)
        };

        Ok(final_closest.iter().map(KadContactEntryDto::from).collect())
    }

    pub fn inspect_nodes_dat(&self, raw_bytes: &[u8]) -> Result<NodesDatInspectReport, String> {
        if raw_bytes.len() > MAX_NODES_DAT_SIZE {
            return Err(format!(
                "File size {} exceeds maximum allowed {}",
                raw_bytes.len(),
                MAX_NODES_DAT_SIZE
            ));
        }

        let nodes_file = NodesDatFile::parse(std::io::Cursor::new(raw_bytes))
            .map_err(|e| format!("Failed to parse nodes.dat: {e}"))?;

        let profile = self.policy_mgr.profile();
        let mut valid_count = 0;
        let mut policy_rejected = 0;
        let mut filter_rejected = 0;
        let mut sample_contacts = Vec::new();

        for entry in &nodes_file.contacts {
            let ip = IpAddr::V4(entry.ip);

            // Policy check
            let category = classify_ip(ip);
            let policy_allowed = match profile {
                NetworkProfile::LocalLab => category == IpCategory::Loopback,
                NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                    category == IpCategory::PublicRoutable
                }
            };

            if !policy_allowed {
                policy_rejected += 1;
                continue;
            }

            // Filter check
            if self.ip_filter.check_ip(ip).is_some() {
                filter_rejected += 1;
                continue;
            }

            valid_count += 1;
            if sample_contacts.len() < 20 {
                let tcp_port = if entry.tcp_port != 0 {
                    Some(entry.tcp_port)
                } else {
                    None
                };
                sample_contacts.push(KadContactEntryDto {
                    kad_id: KadId(entry.client_id).to_hex(),
                    ip: entry.ip.to_string(),
                    udp_port: entry.udp_port,
                    tcp_port,
                    version: entry.version,
                    verified: entry.verified,
                    fail_count: 0,
                    last_seen_unix_ms: 0,
                });
            }
        }

        Ok(NodesDatInspectReport {
            version: nodes_file.version,
            declared_count: nodes_file.contacts.len(),
            valid_contacts_count: valid_count,
            rejected_by_policy_count: policy_rejected,
            rejected_by_filter_count: filter_rejected,
            sample_contacts,
        })
    }

    pub async fn import_nodes_dat(&self, raw_bytes: &[u8]) -> Result<usize, String> {
        let _ = self.inspect_nodes_dat(raw_bytes)?;
        let nodes_file = NodesDatFile::parse(std::io::Cursor::new(raw_bytes))
            .map_err(|e| format!("Failed to parse nodes.dat: {e}"))?;

        let profile = self.policy_mgr.profile();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut imported_contacts = Vec::new();

        for entry in nodes_file.contacts {
            let ip = IpAddr::V4(entry.ip);
            let category = classify_ip(ip);
            let policy_allowed = match profile {
                NetworkProfile::LocalLab => category == IpCategory::Loopback,
                NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                    category == IpCategory::PublicRoutable
                }
            };

            if !policy_allowed || self.ip_filter.check_ip(ip).is_some() {
                continue;
            }

            let tcp_port = if entry.tcp_port != 0 {
                Some(entry.tcp_port)
            } else {
                None
            };
            let contact = KadContact {
                kad_id: KadId(entry.client_id),
                ip: entry.ip,
                udp_port: entry.udp_port,
                tcp_port,
                version: entry.version,
                verified: entry.verified,
                fail_count: 0,
                first_seen_unix_ms: now_ms,
                last_seen_unix_ms: now_ms,
            };
            imported_contacts.push(contact);
        }

        // Persist to SQLite
        {
            let mut dl = self.downloads.lock().await;
            let _ = dl.db_mut().batch_upsert_kad_contacts(&imported_contacts);
        }

        // Insert into in-memory routing table
        let mut table = self.routing_table.lock().await;
        for c in &imported_contacts {
            table.insert_or_update(c.clone());
        }

        tracing::info!(
            "Kad imported {} valid contacts from nodes.dat into routing table and database",
            imported_contacts.len()
        );

        Ok(imported_contacts.len())
    }

    pub async fn list_contacts(&self, limit: usize) -> Vec<KadContactEntryDto> {
        let dl = self.downloads.lock().await;
        if let Ok(contacts) = dl.db_ref().list_kad_contacts(limit) {
            return contacts.iter().map(KadContactEntryDto::from).collect();
        }
        Vec::new()
    }

    pub async fn clear_contacts(&self) -> Result<usize, String> {
        let mut table = self.routing_table.lock().await;
        *table = KBucketTable::new(self.local_id);

        let mut dl = self.downloads.lock().await;
        dl.db_mut()
            .clear_kad_contacts()
            .map_err(|e| format!("Database error clearing kad contacts: {e}"))
    }

    /// Iterative search for sources of a target file hash in Kad DHT.
    /// Discovered sources are filtered, converted to PeerSource (origin Kad) and automatically injected into DownloadService if present.
    pub async fn search_sources(
        &self,
        file_hash_hex: &str,
        file_size: u64,
    ) -> Result<Vec<PeerSource>, String> {
        let f_key = file_hash_hex.to_lowercase();
        {
            let mut in_flight = self.in_flight_searches.lock().await;
            if in_flight.contains(&f_key) {
                tracing::info!(
                    "Kad search_sources already in flight for file {}, coalescing duplicate request",
                    f_key
                );
                return Ok(Vec::new());
            }
            in_flight.insert(f_key.clone());
        }

        struct InFlightSearchGuard {
            set: Arc<Mutex<std::collections::HashSet<String>>>,
            key: String,
        }
        impl Drop for InFlightSearchGuard {
            fn drop(&mut self) {
                let s = Arc::clone(&self.set);
                let k = self.key.clone();
                tokio::spawn(async move {
                    let mut guard = s.lock().await;
                    guard.remove(&k);
                });
            }
        }
        let _guard = InFlightSearchGuard {
            set: Arc::clone(&self.in_flight_searches),
            key: f_key,
        };

        let target_kad_id = KadId::from_hex(file_hash_hex)
            .ok_or_else(|| format!("Invalid target file hash hex: {file_hash_hex}"))?;

        let initial_closest = {
            let table = self.routing_table.lock().await;
            table.find_closest_verified(&target_kad_id, 10)
        };

        if initial_closest.is_empty() {
            tracing::warn!(
                "Kad search_sources for {}: routing table is empty (0 contacts)! Import nodes.dat or bootstrap to find sources.",
                file_hash_hex
            );
            return Ok(Vec::new());
        }

        // 1. Iterative find_node step: query closest contacts to discover nodes geographically nearest to the file hash
        tracing::info!(
            "Kad search_sources: performing iterative find_node convergence for target hash {}...",
            file_hash_hex
        );

        let mut closest_nodes = initial_closest;
        let mut queried_nodes = std::collections::HashSet::new();

        for iter in 0..2 {
            let mut find_futs = Vec::new();
            for contact in closest_nodes.iter().take(5) {
                let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
                let receiver_id = contact.kad_id;
                if queried_nodes.insert(addr) {
                    let udp = Arc::clone(&self.udp);
                    find_futs.push(async move {
                        udp.send_find_node(addr, &target_kad_id, &receiver_id).await
                    });
                }
            }

            if find_futs.is_empty() {
                break;
            }

            let find_results = futures_util::future::join_all(find_futs).await;
            let mut new_contacts = Vec::new();
            let profile = self.policy_mgr.profile();
            for res in find_results {
                if let Ok(contacts) = res {
                    for mut c in contacts {
                        let cat = classify_ip(IpAddr::V4(c.ip));
                        let policy_allowed = match profile {
                            NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                            NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                                cat == IpCategory::PublicRoutable
                            }
                        };
                        if policy_allowed && self.ip_filter.check_ip(IpAddr::V4(c.ip)).is_none() {
                            c.verified = true;
                            new_contacts.push(c);
                        }
                    }
                }
            }

            if new_contacts.is_empty() {
                break;
            }

            // Insert discovered contacts into routing table
            {
                let mut table = self.routing_table.lock().await;
                for c in &new_contacts {
                    table.insert_or_update(c.clone());
                }
                closest_nodes = table.find_closest(&target_kad_id, 12);
            }
            tracing::debug!("Kad search_sources: iteration {} found {} closer contacts", iter + 1, new_contacts.len());
        }

        tracing::info!(
            "Kad starting search_sources for file {} (size={}): querying {} closest converged Kad contacts",
            file_hash_hex,
            file_size,
            closest_nodes.len()
        );

        let mut discovered_sources = Vec::new();
        let profile = self.policy_mgr.profile();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut futs = Vec::with_capacity(closest_nodes.len());
        for contact in closest_nodes {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            let udp = Arc::clone(&self.udp);
            let target_hash = target_kad_id.0;
            let f_hex = file_hash_hex.to_string();
            futs.push(async move {
                tracing::debug!("Kad sending SearchSources request for {} to {}", f_hex, addr);
                let res = udp.send_search_sources(addr, &target_hash, file_size).await;
                (addr, res)
            });
        }

        let results = futures_util::future::join_all(futs).await;
        for (addr, res) in results {
            match res {
                Ok(wires) => {
                    tracing::info!("Kad received {} source candidates from node {}", wires.len(), addr);
                    for w in wires {
                        let ip = Ipv4Addr::from(w.ip);
                        let ip_addr = IpAddr::V4(ip);

                        let cat = classify_ip(ip_addr);
                        let policy_allowed = match profile {
                            NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                            NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                                cat == IpCategory::PublicRoutable
                            }
                        };

                        if !policy_allowed || self.ip_filter.check_ip(ip_addr).is_some() {
                            continue;
                        }

                        if w.tcp_port == 0 {
                            continue;
                        }

                        let peer_addr_str = format!("{}:{}", ip, w.tcp_port);
                        let ip_u32 = u32::from_le_bytes(w.ip); // Conformed to little-endian eD2K standard

                        discovered_sources.push(PeerSource {
                            file_hash: file_hash_hex.to_lowercase(),
                            address: peer_addr_str,
                            ip: ip_u32,
                            port: w.tcp_port,
                            origin: PeerSourceOrigin::Kad,
                            discovered_at_unix_ms: now_ms as i64,
                            last_seen_unix_ms: now_ms as i64,
                            status: PeerSourceStatus::Candidate,
                            fail_count: 0,
                            last_error: None,
                            cooldown_until_unix_ms: None,
                            user_hash: Some(hex::encode(w.kad_id)),
                            crypt_options: w.encryption,
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("Kad search_sources to {} failed/timed out: {}", addr, e);
                }
            }
        }

        // Auto-inject discovered sources into DownloadService swarm
        if !discovered_sources.is_empty() {
            tracing::info!(
                "Kad search_sources for {} finished: discovered {} new peer sources (persisted to download swarm)",
                file_hash_hex,
                discovered_sources.len()
            );
            let mut dl = self.downloads.lock().await;
            let _ = dl.upsert_peer_sources(file_hash_hex, &discovered_sources);
        } else {
            tracing::info!(
                "Kad search_sources for {} finished: 0 sources found from queried contacts",
                file_hash_hex
            );
        }

        Ok(discovered_sources)
    }

    /// Publishes this node as a source for a given file hash to the closest Kad contacts.
    pub async fn publish_source(
        &self,
        file_hash_hex: &str,
        file_size: u64,
        tcp_port: u16,
    ) -> Result<usize, String> {
        let target_kad_id = KadId::from_hex(file_hash_hex)
            .ok_or_else(|| format!("Invalid target file hash hex: {file_hash_hex}"))?;

        let closest_contacts = {
            let table = self.routing_table.lock().await;
            table.find_closest(&target_kad_id, 20)
        };

        if closest_contacts.is_empty() {
            return Ok(0);
        }

        let mut success_count = 0;
        for contact in closest_contacts {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            if let Ok(status) = self.udp.send_publish_source(addr, &target_kad_id.0, file_size, tcp_port).await {
                if status == 0 {
                    success_count += 1;
                }
            }
        }

        Ok(success_count)
    }

    /// Searches Kad DHT for keyword entries matching a search term.
    pub async fn search_keywords(
        &self,
        term: &str,
        min_size: Option<u64>,
        max_size: Option<u64>,
    ) -> Result<Vec<serde_json::Value>, String> {
        let (norm_word, keyword_kad_id) = extract_kad_keywords(term)
            .into_iter()
            .next()
            .unwrap_or_else(|| hash_kad_keyword(term));

        let initial_closest = {
            let table = self.routing_table.lock().await;
            table.find_closest_verified(&keyword_kad_id, 10)
        };

        if initial_closest.is_empty() {
            tracing::warn!(
                "Kad search_keywords for '{}' (norm='{}'): routing table is empty (0 contacts)",
                term,
                norm_word
            );
            return Ok(Vec::new());
        }

        tracing::info!(
            "Kad search_keywords: starting search for '{}' (hash={}): performing iterative find_node convergence...",
            term,
            keyword_kad_id.to_hex()
        );

        let mut closest_nodes = initial_closest;
        let mut queried_find_nodes = std::collections::HashSet::new();

        // 1. Iterative find_node step: query closest contacts to converge towards the keyword ID
        for iter in 0..3 {
            let mut find_futs = Vec::new();
            for contact in closest_nodes.iter().take(8) {
                let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
                let receiver_id = contact.kad_id;
                if queried_find_nodes.insert(addr) {
                    let udp = Arc::clone(&self.udp);
                    let target_id = keyword_kad_id;
                    find_futs.push(async move {
                        udp.send_find_node(addr, &target_id, &receiver_id).await
                    });
                }
            }

            if find_futs.is_empty() {
                break;
            }

            let find_results = futures_util::future::join_all(find_futs).await;
            let mut new_contacts = Vec::new();
            let profile = self.policy_mgr.profile();
            for res in find_results {
                if let Ok(contacts) = res {
                    for mut c in contacts {
                        let cat = classify_ip(IpAddr::V4(c.ip));
                        let policy_allowed = match profile {
                            NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                            NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                                cat == IpCategory::PublicRoutable
                            }
                        };
                        if policy_allowed && self.ip_filter.check_ip(IpAddr::V4(c.ip)).is_none() {
                            c.verified = true;
                            new_contacts.push(c);
                        }
                    }
                }
            }

            if new_contacts.is_empty() {
                break;
            }

            {
                let mut table = self.routing_table.lock().await;
                for c in &new_contacts {
                    table.insert_or_update(c.clone());
                }
                closest_nodes = table.find_closest(&keyword_kad_id, 16);
            }
            tracing::info!(
                "Kad search_keywords: find_node convergence round {} discovered {} new contacts (closest pool now {})",
                iter + 1,
                new_contacts.len(),
                closest_nodes.len()
            );
        }

        tracing::info!(
            "Kad search_keywords: querying {} converged contacts for keyword '{}'...",
            closest_nodes.len(),
            term
        );

        let mut discovered_entries: Vec<serde_json::Value> = Vec::new();
        let mut seen_hashes = std::collections::HashSet::new();
        let profile = self.policy_mgr.profile();

        let mut futs = Vec::with_capacity(closest_nodes.len());
        for contact in closest_nodes {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            let cat = classify_ip(IpAddr::V4(contact.ip));
            let policy_allowed = match profile {
                NetworkProfile::LocalLab => cat == IpCategory::Loopback,
                NetworkProfile::PublicEd2kPreview | NetworkProfile::PublicEd2kKadPreview => {
                    cat == IpCategory::PublicRoutable
                }
            };

            if !policy_allowed || self.ip_filter.check_ip(IpAddr::V4(contact.ip)).is_some() {
                continue;
            }

            let udp = Arc::clone(&self.udp);
            let target_kw = keyword_kad_id.0;
            futs.push(async move {
                let res = udp.send_search_keyword(addr, &target_kw, 0).await;
                (addr, res)
            });
        }

        let results = futures_util::future::join_all(futs).await;
        for (addr, res) in results {
            match res {
                Ok(wires) => {
                    tracing::info!(
                        "Kad received {} keyword search entries from node {}",
                        wires.len(),
                        addr
                    );
                    for w in wires {
                        let file_hash_hex = KadId(w.file_hash).to_hex();
                        if let Some(min) = min_size {
                            if w.file_size < min {
                                continue;
                            }
                        }
                        if let Some(max) = max_size {
                            if w.file_size > max {
                                continue;
                            }
                        }

                        if seen_hashes.insert(file_hash_hex.clone()) {
                            discovered_entries.push(serde_json::json!({
                                "fileHash": file_hash_hex,
                                "fileName": w.file_name,
                                "fileSize": w.file_size,
                                "sourceCount": w.source_count,
                            }));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Kad search_keyword to {} failed/timed out: {}", addr, e);
                }
            }
        }

        tracing::info!(
            "Kad search_keywords for '{}' finished: found {} unique file results",
            term,
            discovered_entries.len()
        );

        Ok(discovered_entries)
    }

    /// Publishes keyword entries for a file to the closest Kad contacts.
    pub async fn publish_keyword(
        &self,
        file_hash_hex: &str,
        file_name: &str,
        file_size: u64,
    ) -> Result<usize, String> {
        let file_kad_id = KadId::from_hex(file_hash_hex)
            .ok_or_else(|| format!("Invalid file hash hex: {file_hash_hex}"))?;

        let keywords = extract_kad_keywords(file_name);
        if keywords.is_empty() {
            return Ok(0);
        }

        let mut total_published = 0;

        for (_word, keyword_kad_id) in keywords {
            let closest_contacts = {
                let table = self.routing_table.lock().await;
                table.find_closest(&keyword_kad_id, 20)
            };

            if closest_contacts.is_empty() {
                continue;
            }

            let entry_wire = mule_protocol::kad::KadKeywordEntryWire {
                file_hash: file_kad_id.0,
                file_name: file_name.to_string(),
                file_size,
                source_count: 1,
            };

            for contact in closest_contacts {
                let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
                if let Ok(status) = self.udp.send_publish_keyword(addr, &keyword_kad_id.0, std::slice::from_ref(&entry_wire)).await {
                    if status == 0 {
                        total_published += 1;
                    }
                }
            }
        }

        Ok(total_published)
    }
}
