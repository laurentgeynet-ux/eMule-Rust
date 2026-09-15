use std::net::Ipv4Addr;
use mule_core::kad::{KadContact, KadId, KBucketTable, K_BUCKET_SIZE};
use mule_core::persistence::Database;

#[test]
fn test_kad_id_xor_metric_and_distance() {
    let id_a = KadId([0u8; 16]);
    let mut bytes_b = [0u8; 16];
    bytes_b[0] = 0x80; // Differ in MSB
    let id_b = KadId(bytes_b);

    assert_eq!(id_a.xor_distance(&id_b), id_b);
    assert_eq!(id_b.leading_zeros(), 0);
    assert_eq!(id_a.bucket_index_for(&id_b), Some(127));

    let mut bytes_c = [0u8; 16];
    bytes_c[15] = 0x01; // Differ in LSB
    let id_c = KadId(bytes_c);
    assert_eq!(id_c.leading_zeros(), 127);
    assert_eq!(id_a.bucket_index_for(&id_c), Some(0));

    assert_eq!(id_a.bucket_index_for(&id_a), None); // Self distance 0
}

#[test]
fn test_routing_table_insertion_and_capacity() {
    let local_id = KadId([0u8; 16]);
    let mut table = KBucketTable::new(local_id);

    // Insert 20 contacts into bucket 127
    for i in 0..K_BUCKET_SIZE {
        let mut b = [0u8; 16];
        b[0] = 0x80;
        b[15] = (i + 1) as u8;
        let c = KadContact::new(KadId(b), Ipv4Addr::new(10, 0, 0, (i + 1) as u8), 4672, None, 2, 1000);
        let evict_candidate = table.insert_or_update(c);
        assert!(evict_candidate.is_none());
    }

    assert_eq!(table.total_contacts(), K_BUCKET_SIZE);
    assert_eq!(table.total_replacements(), 0);

    // 21st contact into bucket 127 -> goes to replacement cache, returns head candidate
    let mut b21 = [0u8; 16];
    b21[0] = 0x80;
    b21[15] = 255;
    let c21 = KadContact::new(KadId(b21), Ipv4Addr::new(10, 0, 0, 255), 4672, None, 2, 1000);
    let evict_candidate = table.insert_or_update(c21);
    assert!(evict_candidate.is_some());
    assert_eq!(table.total_contacts(), K_BUCKET_SIZE);
    assert_eq!(table.total_replacements(), 1);

    // Removing the head of the bucket should promote the replacement
    let head_id = evict_candidate.unwrap().kad_id;
    let removed = table.remove(&head_id);
    assert!(removed.is_some());
    assert_eq!(table.total_contacts(), K_BUCKET_SIZE);
    assert_eq!(table.total_replacements(), 0);
    assert!(table.get(&KadId(b21)).is_some());
}

#[test]
fn test_sqlite_kad_contacts_persistence() {
    let mut db = Database::open_in_memory().unwrap();

    let c1 = KadContact::new(KadId([1u8; 16]), Ipv4Addr::new(1, 2, 3, 4), 4672, Some(4662), 2, 1000);
    let c2 = KadContact::new(KadId([2u8; 16]), Ipv4Addr::new(5, 6, 7, 8), 1234, None, 1, 2000);

    db.batch_upsert_kad_contacts(&[c1.clone(), c2.clone()]).unwrap();
    assert_eq!(db.count_kad_contacts().unwrap(), 2);

    let list = db.list_kad_contacts(10).unwrap();
    assert_eq!(list.len(), 2);
    // Ordered by last_seen DESC -> c2 first
    assert_eq!(list[0].kad_id, KadId([2u8; 16]));
    assert_eq!(list[1].kad_id, KadId([1u8; 16]));
    assert_eq!(list[1].tcp_port, Some(4662));

    db.clear_kad_contacts().unwrap();
    assert_eq!(db.count_kad_contacts().unwrap(), 0);
}
