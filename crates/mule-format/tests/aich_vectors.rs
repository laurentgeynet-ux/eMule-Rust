use mule_format::{
    calculate_aich_root, AichHash, AichNode, AICH_BLOCK_SIZE, BLOCKS_PER_PART, ED2K_PART_SIZE,
    LAST_BLOCK_SIZE_FULL_PART,
};
use sha1::{Digest, Sha1};

fn sha1_digest(data: &[u8]) -> [u8; 20] {
    let mut h = Sha1::new();
    h.update(data);
    h.finalize().into()
}

#[test]
fn test_aich_single_block() {
    let data = b"Hello, AICH!";
    let expected = sha1_digest(data);

    let root = calculate_aich_root(data).unwrap();
    assert_eq!(root.as_bytes(), &expected);
}

#[test]
fn test_aich_exact_block_size() {
    let data = vec![0x33; AICH_BLOCK_SIZE];
    let expected = sha1_digest(&data);

    let root = calculate_aich_root(&data).unwrap();
    assert_eq!(root.as_bytes(), &expected);
}

#[test]
fn test_aich_two_blocks() {
    let mut data = vec![0x11; AICH_BLOCK_SIZE];
    data.extend_from_slice(&[0x22; 100]); // 180 KiB + 100 bytes

    let b0_hash = sha1_digest(&data[..AICH_BLOCK_SIZE]);
    let b1_hash = sha1_digest(&data[AICH_BLOCK_SIZE..]);

    let mut parent = Sha1::new();
    parent.update(&b0_hash);
    parent.update(&b1_hash);
    let expected_root: [u8; 20] = parent.finalize().into();

    let root = calculate_aich_root(&data).unwrap();
    assert_eq!(root.as_bytes(), &expected_root);
}

#[test]
fn test_aich_full_part_53_blocks_structure() {
    // A full eD2K part: 9 728 000 bytes
    // 52 blocks of 184 320 bytes + 1 block of 143 360 bytes
    assert_eq!(
        52 * AICH_BLOCK_SIZE + LAST_BLOCK_SIZE_FULL_PART,
        ED2K_PART_SIZE
    );
    assert_eq!(BLOCKS_PER_PART, 53);

    let data = vec![0x7a; ED2K_PART_SIZE];
    let root = calculate_aich_root(&data).unwrap();

    // Verify that we can generate and verify a Merkle proof for the first block
    let mut tree = AichNode::new_tree(ED2K_PART_SIZE as u64).unwrap();
    let mut offset = 0;
    while offset < data.len() {
        let chunk_len = (data.len() - offset).min(AICH_BLOCK_SIZE);
        let block_bytes = &data[offset..offset + chunk_len];
        let digest = sha1_digest(block_bytes);
        tree.set_leaf_hash(offset as u64, chunk_len as u64, AichHash(digest));
        offset += chunk_len;
    }
    let calculated_root = tree.recalculate_hash().unwrap();
    assert_eq!(root, calculated_root);

    // Generate proof for block #0
    let proof0 = tree.generate_proof(0, AICH_BLOCK_SIZE as u64).unwrap();
    assert!(proof0.verify(&data[..AICH_BLOCK_SIZE], &root));

    // Generate proof for last block #52 (143 360 bytes)
    let last_offset = 52 * AICH_BLOCK_SIZE as u64;
    let proof_last = tree
        .generate_proof(last_offset, LAST_BLOCK_SIZE_FULL_PART as u64)
        .unwrap();
    assert!(proof_last.verify(&data[last_offset as usize..], &root));

    // Corrupted block must fail verification
    let mut corrupted_block = data[..AICH_BLOCK_SIZE].to_vec();
    corrupted_block[0] ^= 0xff;
    assert!(!proof0.verify(&corrupted_block, &root));
}
