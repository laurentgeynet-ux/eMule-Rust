pub mod nodes_dat;
pub mod search_keywords;
pub mod search_sources;
pub mod types;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;

use mule_core::kad::{KBucketTable, KadContact, KadId};
use mule_core::model::PeerSource;
use mule_filter::IpFilter;
use mule_network_policy::{classify_ip, CancellationToken, IpCategory, NetworkPolicyManager, NetworkProfile};

use crate::services::downloads::DownloadService;
use crate::services::kad_udp::KadUdpService;

pub use types::{KadContactEntryDto, KadStatusSnapshot, NodesDatInspectReport};

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

                        tracing::info!(
                            "[KAD_ROUTING] Kad routing table status: {} verified contacts in memory",
                            total_contacts
                        );

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
        nodes_dat::inspect_nodes_dat(raw_bytes, &self.policy_mgr, &self.ip_filter)
    }

    pub async fn import_nodes_dat(&self, raw_bytes: &[u8]) -> Result<usize, String> {
        let imported_contacts = nodes_dat::parse_valid_nodes_contacts(raw_bytes, &self.policy_mgr, &self.ip_filter)?;

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
        search_sources::search_sources_iterative(
            file_hash_hex,
            file_size,
            &self.routing_table,
            &self.downloads,
            &self.policy_mgr,
            &self.ip_filter,
            &self.udp,
            &self.in_flight_searches,
        ).await
    }

    /// Publishes this node as a source for a given file hash to the closest Kad contacts.
    pub async fn publish_source(
        &self,
        file_hash_hex: &str,
        file_size: u64,
        tcp_port: u16,
    ) -> Result<usize, String> {
        let target_kad_id = KadId::from_hash_hex(file_hash_hex)
            .ok_or_else(|| format!("Invalid target file hash hex: {file_hash_hex}"))?;

        let closest_contacts = {
            let table = self.routing_table.lock().await;
            table.find_closest(&target_kad_id, 20)
        };

        if closest_contacts.is_empty() {
            return Ok(0);
        }

        let mut success_count = 0;
        let total_queried = closest_contacts.len();
        for contact in closest_contacts {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            if let Ok(status) = self.udp.send_publish_source(addr, &target_kad_id.0, file_size, tcp_port).await {
                if status == 0 {
                    success_count += 1;
                }
            }
        }

        tracing::info!(
            "[KAD_PUBLISH] Published source for file {} ({} bytes, port {}) to {}/{} closest Kad nodes",
            file_hash_hex,
            file_size,
            tcp_port,
            success_count,
            total_queried
        );

        Ok(success_count)
    }

    /// Searches Kad DHT for keyword entries matching a search term.
    pub async fn search_keywords(
        &self,
        term: &str,
        min_size: Option<u64>,
        max_size: Option<u64>,
    ) -> Result<Vec<serde_json::Value>, String> {
        search_keywords::search_keywords_iterative(
            term,
            min_size,
            max_size,
            &self.routing_table,
            &self.policy_mgr,
            &self.ip_filter,
            &self.udp,
        ).await
    }

    /// Publishes keyword entries for a file to the closest Kad contacts.
    pub async fn publish_keyword(
        &self,
        file_hash_hex: &str,
        file_name: &str,
        file_size: u64,
    ) -> Result<usize, String> {
        search_keywords::publish_keyword(
            file_hash_hex,
            file_name,
            file_size,
            &self.routing_table,
            &self.udp,
        ).await
    }
}
