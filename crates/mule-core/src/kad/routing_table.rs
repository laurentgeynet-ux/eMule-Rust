use std::collections::VecDeque;
use super::contact::KadContact;
use super::kad_id::KadId;

/// Bucket capacity (standard Kademlia K = 20).
pub const K_BUCKET_SIZE: usize = 20;
/// Replacement cache capacity per bucket.
pub const REPLACEMENT_CACHE_SIZE: usize = 10;
/// Total number of K-buckets for 128-bit Kad ID.
pub const NUM_BUCKETS: usize = 128;

#[derive(Debug, Clone)]
pub struct KBucket {
    pub contacts: VecDeque<KadContact>,
    pub replacements: VecDeque<KadContact>,
}

impl Default for KBucket {
    fn default() -> Self {
        Self {
            contacts: VecDeque::with_capacity(K_BUCKET_SIZE),
            replacements: VecDeque::with_capacity(REPLACEMENT_CACHE_SIZE),
        }
    }
}

impl KBucket {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_full(&self) -> bool {
        self.contacts.len() >= K_BUCKET_SIZE
    }

    pub fn get(&self, kad_id: &KadId) -> Option<&KadContact> {
        self.contacts.iter().find(|c| &c.kad_id == kad_id)
    }

    pub fn insert_or_update(&mut self, contact: KadContact) -> Option<KadContact> {
        if let Some(pos) = self.contacts.iter().position(|c| c.kad_id == contact.kad_id) {
            // Already present: move to tail (most recently seen) and update
            let mut existing = self.contacts.remove(pos).unwrap();
            existing.ip = contact.ip;
            existing.udp_port = contact.udp_port;
            existing.tcp_port = contact.tcp_port;
            existing.version = contact.version;
            existing.last_seen_unix_ms = contact.last_seen_unix_ms;
            if contact.verified {
                existing.verified = true;
            }
            self.contacts.push_back(existing);
            None
        } else if self.contacts.len() < K_BUCKET_SIZE {
            // Not full: push to tail
            self.contacts.push_back(contact);
            None
        } else {
            // Full: add to replacement cache if not already present
            if let Some(pos) = self.replacements.iter().position(|c| c.kad_id == contact.kad_id) {
                self.replacements.remove(pos);
            } else if self.replacements.len() >= REPLACEMENT_CACHE_SIZE {
                self.replacements.pop_front();
            }
            self.replacements.push_back(contact.clone());
            // Return candidate to ping/evict (the head of the bucket)
            self.contacts.front().cloned()
        }
    }

    pub fn remove(&mut self, kad_id: &KadId) -> Option<KadContact> {
        if let Some(pos) = self.contacts.iter().position(|c| &c.kad_id == kad_id) {
            let removed = self.contacts.remove(pos);
            // Promote candidate from replacement cache
            if let Some(replacement) = self.replacements.pop_back() {
                self.contacts.push_back(replacement);
            }
            removed
        } else {
            if let Some(pos) = self.replacements.iter().position(|c| &c.kad_id == kad_id) {
                return self.replacements.remove(pos);
            }
            None
        }
    }
}

/// Kademlia Routing Table containing 128 buckets.
#[derive(Debug, Clone)]
pub struct KBucketTable {
    local_id: KadId,
    buckets: Vec<KBucket>,
}

impl KBucketTable {
    pub fn new(local_id: KadId) -> Self {
        let mut buckets = Vec::with_capacity(NUM_BUCKETS);
        for _ in 0..NUM_BUCKETS {
            buckets.push(KBucket::new());
        }
        Self { local_id, buckets }
    }

    pub fn local_id(&self) -> &KadId {
        &self.local_id
    }

    pub fn bucket_index_for(&self, kad_id: &KadId) -> Option<usize> {
        self.local_id.bucket_index_for(kad_id)
    }

    pub fn insert_or_update(&mut self, contact: KadContact) -> Option<KadContact> {
        if contact.kad_id == self.local_id {
            return None; // Do not insert self
        }
        if let Some(idx) = self.bucket_index_for(&contact.kad_id) {
            self.buckets[idx].insert_or_update(contact)
        } else {
            None
        }
    }

    pub fn remove(&mut self, kad_id: &KadId) -> Option<KadContact> {
        if let Some(idx) = self.bucket_index_for(kad_id) {
            self.buckets[idx].remove(kad_id)
        } else {
            None
        }
    }

    pub fn get(&self, kad_id: &KadId) -> Option<&KadContact> {
        if let Some(idx) = self.bucket_index_for(kad_id) {
            self.buckets[idx].get(kad_id)
        } else {
            None
        }
    }

    pub fn total_contacts(&self) -> usize {
        self.buckets.iter().map(|b| b.contacts.len()).sum()
    }

    pub fn total_replacements(&self) -> usize {
        self.buckets.iter().map(|b| b.replacements.len()).sum()
    }

    /// Finds up to `count` closest contacts to the given target key.
    pub fn find_closest(&self, target: &KadId, count: usize) -> Vec<KadContact> {
        let mut all_contacts = Vec::new();
        for b in &self.buckets {
            for c in &b.contacts {
                all_contacts.push(c.clone());
            }
        }

        all_contacts.sort_by(|a, b| {
            let dist_a = a.kad_id.xor_distance(target);
            let dist_b = b.kad_id.xor_distance(target);
            dist_a.cmp(&dist_b)
        });

        all_contacts.truncate(count);
        all_contacts
    }

    /// Returns a snapshot of non-empty buckets and their contact counts.
    pub fn bucket_distribution(&self) -> Vec<(usize, usize)> {
        self.buckets
            .iter()
            .enumerate()
            .filter(|(_, b)| !b.contacts.is_empty())
            .map(|(i, b)| (i, b.contacts.len()))
            .collect()
    }
}
