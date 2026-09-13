use sha1::{Digest, Sha1};

use crate::aich::hash_tree::{AichHash, AichNode};

/// A single step in an AICH Merkle proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AichProofStep {
    pub sibling_hash: AichHash,
    pub is_sibling_left: bool,
}

/// Merkle audit proof to verify an individual AICH block against a trusted root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AichProof {
    pub block_offset: u64,
    pub block_size: u64,
    pub steps: Vec<AichProofStep>,
}

impl AichProof {
    /// Verifies that `block_data` matches the `trusted_root` using this proof.
    pub fn verify(&self, block_data: &[u8], trusted_root: &AichHash) -> bool {
        if block_data.len() as u64 != self.block_size {
            return false;
        }

        let mut hasher = Sha1::new();
        hasher.update(block_data);
        let mut current_hash = AichHash(hasher.finalize().into());

        for step in &self.steps {
            let mut parent_hasher = Sha1::new();
            if step.is_sibling_left {
                parent_hasher.update(step.sibling_hash.as_bytes());
                parent_hasher.update(current_hash.as_bytes());
            } else {
                parent_hasher.update(current_hash.as_bytes());
                parent_hasher.update(step.sibling_hash.as_bytes());
            }
            current_hash = AichHash(parent_hasher.finalize().into());
        }

        &current_hash == trusted_root
    }
}

impl AichNode {
    /// Generates an audit proof for a leaf block at `start_pos` with `block_size`.
    pub fn generate_proof(&self, start_pos: u64, block_size: u64) -> Option<AichProof> {
        let mut steps = Vec::new();
        if self.collect_proof_steps(start_pos, block_size, &mut steps) {
            Some(AichProof {
                block_offset: start_pos,
                block_size,
                steps,
            })
        } else {
            None
        }
    }

    fn collect_proof_steps(
        &self,
        start_pos: u64,
        block_size: u64,
        steps: &mut Vec<AichProofStep>,
    ) -> bool {
        if self.left.is_none() && self.right.is_none() {
            return self.data_size == block_size && start_pos == 0;
        }

        let (left, right) = match (&self.left, &self.right) {
            (Some(l), Some(r)) => (l, r),
            _ => return false,
        };
        let left_size = left.data_size;

        if start_pos < left_size {
            if start_pos + block_size > left_size {
                return false;
            }
            if left.collect_proof_steps(start_pos, block_size, steps) {
                if let Some(sibling_hash) = right.hash {
                    steps.push(AichProofStep {
                        sibling_hash,
                        is_sibling_left: false,
                    });
                    return true;
                }
            }
        } else {
            let rel_pos = start_pos - left_size;
            if rel_pos + block_size > right.data_size {
                return false;
            }
            if right.collect_proof_steps(rel_pos, block_size, steps) {
                if let Some(sibling_hash) = left.hash {
                    steps.push(AichProofStep {
                        sibling_hash,
                        is_sibling_left: true,
                    });
                    return true;
                }
            }
        }

        false
    }
}
