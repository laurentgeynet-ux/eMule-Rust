use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use mule_core::kad::{KBucketTable, KadContact, KadId};
use mule_filter::IpFilter;
use mule_format::kad::{NodesDatFile, MAX_NODES_DAT_SIZE};
use mule_network_policy::{classify_ip, IpCategory, NetworkPolicyManager, NetworkProfile};

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
            let mut table = self.routing_table.lock().await;
            for c in contacts {
                table.insert_or_update(c);
            }
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
            let kad_id = target_kad_id_hex
                .and_then(KadId::from_hex)
                .unwrap_or_else(KadId::random);

            let contact = KadContact {
                kad_id,
                ip: v4,
                udp_port,
                tcp_port: None,
                version: 2,
                verified: true,
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

        Ok(count)
    }

    /// Performs iterative Find Node lookup for a target Kad ID with concurrency alpha = 3.
    pub async fn find_node(&self, target_kad_id: &KadId) -> Result<Vec<KadContactEntryDto>, String> {
        let initial_closest = {
            let table = self.routing_table.lock().await;
            table.find_closest(target_kad_id, 3)
        };

        if initial_closest.is_empty() {
            return Ok(Vec::new());
        }

        for contact in initial_closest {
            let addr = SocketAddr::new(IpAddr::V4(contact.ip), contact.udp_port);
            if let Ok(neighbors) = self.udp.send_find_node(addr, target_kad_id).await {
                let mut table = self.routing_table.lock().await;
                for n in neighbors {
                    table.insert_or_update(n);
                }
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
                    verified: false,
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
                verified: false,
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
}
