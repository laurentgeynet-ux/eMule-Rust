use sha1::{Digest, Sha1};
use thiserror::Error;

use crate::aich::hash_tree::{AichHash, AichNode, AICH_BLOCK_SIZE};

/// Position of the sibling relative to the current verification node.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SiblingPosition {
    Left,
    Right,
}

/// A sibling in an AICH Merkle audit path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AichSibling {
    pub hash: [u8; 20],
    pub position: SiblingPosition,
}

/// A structurally and cryptographically auditable AICH Merkle proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AichProof {
    pub leaf_index: u32,
    pub leaf_count: u32,
    pub siblings: Vec<AichSibling>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AichProofError {
    #[error("leaf count must be at least 1")]
    EmptyTree,

    #[error("leaf index {index} is out of bounds (leaf count is {count})")]
    LeafIndexOutOfBounds { index: u32, count: u32 },

    #[error("invalid proof depth: expected {expected} siblings, got {actual}")]
    InvalidDepth { expected: usize, actual: usize },

    #[error("sibling position mismatch at step {step}: expected {expected:?}, got {actual:?}")]
    InvalidSiblingPosition {
        step: usize,
        expected: SiblingPosition,
        actual: SiblingPosition,
    },

    #[error("leaf data length {size} exceeds maximum AICH block size {max}")]
    LeafTooLarge { size: usize, max: usize },

    #[error("Merkle root mismatch: expected {expected}, computed {computed}")]
    RootMismatch { expected: String, computed: String },
}

/// Computes the sequence of expected sibling positions from leaf to root (bottom-up).
pub fn compute_expected_path(
    mut index: u32,
    mut count: u32,
) -> Result<Vec<SiblingPosition>, AichProofError> {
    if count == 0 {
        return Err(AichProofError::EmptyTree);
    }
    if index >= count {
        return Err(AichProofError::LeafIndexOutOfBounds { index, count });
    }

    let mut path_from_root = Vec::new();
    let mut is_left_branch = true;

    while count > 1 {
        let left_count = if is_left_branch {
            count.div_ceil(2)
        } else {
            count / 2
        };

        if index < left_count {
            // Target is in left child, sibling is right child
            path_from_root.push(SiblingPosition::Right);
            count = left_count;
            is_left_branch = true;
        } else {
            // Target is in right child, sibling is left child
            path_from_root.push(SiblingPosition::Left);
            index -= left_count;
            count -= left_count;
            is_left_branch = false;
        }
    }

    // Reverse to get bottom-up (leaf to root) order
    path_from_root.reverse();
    Ok(path_from_root)
}

/// Verifies an AICH Merkle proof structurally and cryptographically against an expected root.
pub fn verify_aich_proof(
    expected_root: [u8; 20],
    leaf_bytes: &[u8],
    proof: &AichProof,
) -> Result<(), AichProofError> {
    if proof.leaf_count == 0 {
        return Err(AichProofError::EmptyTree);
    }
    if proof.leaf_index >= proof.leaf_count {
        return Err(AichProofError::LeafIndexOutOfBounds {
            index: proof.leaf_index,
            count: proof.leaf_count,
        });
    }
    if leaf_bytes.len() > AICH_BLOCK_SIZE {
        return Err(AichProofError::LeafTooLarge {
            size: leaf_bytes.len(),
            max: AICH_BLOCK_SIZE,
        });
    }

    let expected_positions = compute_expected_path(proof.leaf_index, proof.leaf_count)?;
    if proof.siblings.len() != expected_positions.len() {
        return Err(AichProofError::InvalidDepth {
            expected: expected_positions.len(),
            actual: proof.siblings.len(),
        });
    }

    for (step, (sibling, &expected_pos)) in
        proof.siblings.iter().zip(&expected_positions).enumerate()
    {
        if sibling.position != expected_pos {
            return Err(AichProofError::InvalidSiblingPosition {
                step,
                expected: expected_pos,
                actual: sibling.position,
            });
        }
    }

    // Cryptographic validation
    let mut hasher = Sha1::new();
    hasher.update(leaf_bytes);
    let mut current_hash: [u8; 20] = hasher.finalize().into();

    for sibling in &proof.siblings {
        let mut parent_hasher = Sha1::new();
        match sibling.position {
            SiblingPosition::Left => {
                parent_hasher.update(sibling.hash);
                parent_hasher.update(current_hash);
            }
            SiblingPosition::Right => {
                parent_hasher.update(current_hash);
                parent_hasher.update(sibling.hash);
            }
        }
        current_hash = parent_hasher.finalize().into();
    }

    if current_hash != expected_root {
        return Err(AichProofError::RootMismatch {
            expected: AichHash(expected_root).to_base32(),
            computed: AichHash(current_hash).to_base32(),
        });
    }

    Ok(())
}

impl AichNode {
    /// Generates an AICH Merkle proof for the leaf at `leaf_index`.
    pub fn generate_leaf_proof(&self, leaf_index: u32, total_leaves: u32) -> Option<AichProof> {
        let mut siblings = Vec::new();
        if self.collect_leaf_proof(leaf_index, total_leaves, &mut siblings) {
            Some(AichProof {
                leaf_index,
                leaf_count: total_leaves,
                siblings,
            })
        } else {
            None
        }
    }

    fn collect_leaf_proof(&self, index: u32, count: u32, siblings: &mut Vec<AichSibling>) -> bool {
        if count <= 1 {
            return index == 0;
        }

        let (left, right) = match (&self.left, &self.right) {
            (Some(l), Some(r)) => (l, r),
            _ => return false,
        };

        let left_count = if self.is_left_branch {
            count.div_ceil(2)
        } else {
            count / 2
        };

        if index < left_count {
            if left.collect_leaf_proof(index, left_count, siblings) {
                if let Some(right_hash) = right.hash {
                    siblings.push(AichSibling {
                        hash: right_hash.0,
                        position: SiblingPosition::Right,
                    });
                    return true;
                }
            }
        } else {
            let right_count = count - left_count;
            if right.collect_leaf_proof(index - left_count, right_count, siblings) {
                if let Some(left_hash) = left.hash {
                    siblings.push(AichSibling {
                        hash: left_hash.0,
                        position: SiblingPosition::Left,
                    });
                    return true;
                }
            }
        }

        false
    }
}
