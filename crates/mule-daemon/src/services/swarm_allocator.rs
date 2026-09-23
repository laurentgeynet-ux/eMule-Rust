// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

//! Intelligent Swarm Slot Allocator
//!
//! Provides dynamic slot allocation using Weighted Max-Min Fair-Share with Water-filling.
//! It balances global slot limits across active downloads based on available viable sources,
//! remaining incomplete parts, and user-defined priority.

use std::collections::HashMap;

/// Configuration parameters for the dynamic swarm allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwarmAllocationConfig {
    /// Maximum concurrent peer connections across the entire daemon (default: 50).
    pub max_global_peers: usize,
    /// Maximum concurrent peer connections allowed for a single download file (default: 40).
    pub max_peers_per_file: usize,
    /// Minimum guaranteed slots for any active file that has viable sources (default: 2).
    pub min_peers_per_file: usize,
}

impl Default for SwarmAllocationConfig {
    fn default() -> Self {
        Self {
            max_global_peers: 500,
            max_peers_per_file: 400,
            min_peers_per_file: 5,
        }
    }
}

/// Swarm demand parameters for an individual active download.
#[derive(Debug, Clone)]
pub struct FileDemand {
    pub file_hash: String,
    /// Priority weight: Low=1, Normal=2, High=4, Release=8.
    pub priority_weight: u32,
    /// Number of viable candidate sources in DB (not currently failed/on cooldown).
    pub viable_sources_count: usize,
    /// Number of remaining incomplete parts (9.28 MiB each).
    pub remaining_parts: usize,
    /// Number of currently active peer connections.
    pub current_active_peers: usize,
}

impl FileDemand {
    pub fn new(
        file_hash: impl Into<String>,
        priority_str: &str,
        viable_sources_count: usize,
        remaining_parts: usize,
        current_active_peers: usize,
    ) -> Self {
        let priority_weight = match priority_str.to_lowercase().as_str() {
            "low" => 1,
            "high" => 4,
            "release" => 8,
            _ => 2, // "normal" or unspecified
        };
        Self {
            file_hash: file_hash.into(),
            priority_weight,
            viable_sources_count,
            remaining_parts,
            current_active_peers,
        }
    }

    /// Evaluates the true upper demand ceiling of this file.
    pub fn calculate_demand(&self, max_per_file: usize) -> usize {
        if self.viable_sources_count == 0 {
            return 0;
        }

        // Endgame awareness: if only 1 part remains, allocating 40 peers is wasteful.
        // We allow up to 3 peers per remaining part (with a minimum of 2).
        let part_demand = if self.remaining_parts == 0 {
            0
        } else {
            (self.remaining_parts * 3).max(2)
        };

        self.viable_sources_count.min(max_per_file).min(part_demand)
    }
}

/// Dynamic Swarm Slot Allocator
pub struct SwarmAllocator;

impl SwarmAllocator {
    /// Computes optimal slot quotas for a list of active download demands.
    /// Returns a map from file_hash to target slot quota.
    pub fn compute_quotas(
        demands: &[FileDemand],
        config: &SwarmAllocationConfig,
    ) -> HashMap<String, usize> {
        let mut quotas: HashMap<String, usize> = HashMap::new();
        if demands.is_empty() || config.max_global_peers == 0 {
            return quotas;
        }

        // 1. Calculate each file's demand ceiling.
        // Capped by max_peers_per_file and max_global_peers.
        let effective_max_per_file = config.max_peers_per_file.min(config.max_global_peers);

        let file_demands: Vec<(&FileDemand, usize)> = demands
            .iter()
            .map(|d| (d, d.calculate_demand(effective_max_per_file)))
            .collect();

        // 2. Baseline allocation (Floor)
        let mut total_allocated = 0usize;
        for (d, demand) in &file_demands {
            if *demand == 0 {
                quotas.insert(d.file_hash.clone(), 0);
            } else {
                let base = (*demand).min(config.min_peers_per_file);
                quotas.insert(d.file_hash.clone(), base);
                total_allocated += base;
            }
        }

        // If base allocation already meets or exceeds global budget, cap or trim.
        if total_allocated >= config.max_global_peers {
            let mut excess = total_allocated - config.max_global_peers;
            while excess > 0 {
                let mut reduced = false;
                for (d, _) in &file_demands {
                    if let Some(q) = quotas.get_mut(&d.file_hash) {
                        if *q > 1 {
                            *q -= 1;
                            excess -= 1;
                            reduced = true;
                            if excess == 0 {
                                break;
                            }
                        }
                    }
                }
                if !reduced {
                    break;
                }
            }
            return quotas;
        }

        // 3. Water-filling Phase: distribute remaining slots to files with unmet demand.
        let mut remaining_budget = config.max_global_peers - total_allocated;

        while remaining_budget > 0 {
            // Find all files that have unmet demand: demand > quota
            let mut candidates: Vec<(&FileDemand, usize)> = file_demands
                .iter()
                .filter_map(|(d, demand)| {
                    let current = *quotas.get(&d.file_hash).unwrap_or(&0);
                    if current < *demand {
                        Some((*d, demand - current))
                    } else {
                        None
                    }
                })
                .collect();

            if candidates.is_empty() {
                // All active demands are 100% satisfied!
                break;
            }

            // Distribute remaining budget among candidates weighted by priority.
            // Sort by priority descending, then unmet demand descending
            candidates.sort_by(|(d1, unmet1), (d2, unmet2)| {
                d2.priority_weight
                    .cmp(&d1.priority_weight)
                    .then_with(|| unmet2.cmp(unmet1))
            });

            let mut allocated_in_round = 0;
            for (d, unmet) in &candidates {
                if remaining_budget == 0 {
                    break;
                }
                // Allocate incremental slot proportional to priority weight
                let weight = (d.priority_weight as usize).max(1);
                let increment = (*unmet).min(weight).min(remaining_budget);
                if increment > 0 {
                    *quotas.entry(d.file_hash.clone()).or_insert(0) += increment;
                    remaining_budget -= increment;
                    allocated_in_round += increment;
                }
            }

            if allocated_in_round == 0 {
                break;
            }
        }

        quotas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asymmetric_swarms_waterfilling() {
        // User's exact scenario:
        // File A: 100 sources, 100 parts
        // File B: 3 sources, 50 parts
        let demands = vec![
            FileDemand::new("hash_a", "normal", 100, 100, 0),
            FileDemand::new("hash_b", "normal", 3, 50, 0),
        ];

        let config = SwarmAllocationConfig {
            max_global_peers: 50,
            max_peers_per_file: 40,
            min_peers_per_file: 2,
        };

        let quotas = SwarmAllocator::compute_quotas(&demands, &config);

        // File B has 3 sources, so demand is 3. It must get exactly 3 (100% utilized).
        assert_eq!(quotas.get("hash_b"), Some(&3));

        // File A has 100 sources, capped at max_peers_per_file (40).
        assert_eq!(quotas.get("hash_a"), Some(&40));

        let total: usize = quotas.values().sum();
        assert_eq!(total, 43);
    }

    #[test]
    fn test_single_file_takes_full_global_capacity() {
        let demands = vec![FileDemand::new("hash_solo", "normal", 80, 50, 0)];
        let config = SwarmAllocationConfig {
            max_global_peers: 50,
            max_peers_per_file: 50,
            min_peers_per_file: 2,
        };

        let quotas = SwarmAllocator::compute_quotas(&demands, &config);
        // When max_peers_per_file is at or above global, file absorbs all 50 slots!
        assert_eq!(quotas.get("hash_solo"), Some(&50));
    }

    #[test]
    fn test_single_file_capped_at_max_per_file() {
        let demands = vec![FileDemand::new("hash_solo", "normal", 80, 50, 0)];
        let config = SwarmAllocationConfig {
            max_global_peers: 50,
            max_peers_per_file: 40,
            min_peers_per_file: 2,
        };

        let quotas = SwarmAllocator::compute_quotas(&demands, &config);
        // Even solo file respects the hard max_peers_per_file ceiling!
        assert_eq!(quotas.get("hash_solo"), Some(&40));
    }

    #[test]
    fn test_heavy_competition_fair_sharing() {
        // 3 files with 50 sources each, competing for 50 slots
        let demands = vec![
            FileDemand::new("hash_1", "normal", 50, 50, 0),
            FileDemand::new("hash_2", "normal", 50, 50, 0),
            FileDemand::new("hash_3", "normal", 50, 50, 0),
        ];

        let config = SwarmAllocationConfig {
            max_global_peers: 50,
            max_peers_per_file: 40,
            min_peers_per_file: 2,
        };

        let quotas = SwarmAllocator::compute_quotas(&demands, &config);
        let total: usize = quotas.values().sum();
        assert_eq!(total, 50);

        // Each should get ~16 to 18 (50 / 3 = 16.67)
        for q in quotas.values() {
            assert!(*q >= 16 && *q <= 18);
        }
    }

    #[test]
    fn test_endgame_mode_prevents_slot_hoarding() {
        // File A has 100 sources but only 1 part left!
        // File B has 100 sources and 100 parts left.
        let demands = vec![
            FileDemand::new("hash_endgame", "normal", 100, 1, 0),
            FileDemand::new("hash_big", "normal", 100, 100, 0),
        ];

        let config = SwarmAllocationConfig {
            max_global_peers: 50,
            max_peers_per_file: 40,
            min_peers_per_file: 2,
        };

        let quotas = SwarmAllocator::compute_quotas(&demands, &config);

        // Endgame file demand = (1 * 3) = 3 peers max.
        assert_eq!(quotas.get("hash_endgame"), Some(&3));
        // Big file gets 40
        assert_eq!(quotas.get("hash_big"), Some(&40));
    }

    #[test]
    fn test_priority_bias() {
        // File High vs File Low competing for 10 slots
        let demands = vec![
            FileDemand::new("hash_high", "high", 20, 20, 0),
            FileDemand::new("hash_low", "low", 20, 20, 0),
        ];

        let config = SwarmAllocationConfig {
            max_global_peers: 10,
            max_peers_per_file: 10,
            min_peers_per_file: 2,
        };

        let quotas = SwarmAllocator::compute_quotas(&demands, &config);
        let high = *quotas.get("hash_high").unwrap();
        let low = *quotas.get("hash_low").unwrap();

        assert!(
            high > low,
            "High priority ({high}) should get more than low ({low})"
        );
        assert_eq!(high + low, 10);
    }

    #[test]
    fn test_zero_sources_gets_zero() {
        let demands = vec![
            FileDemand::new("hash_dead", "normal", 0, 50, 0),
            FileDemand::new("hash_alive", "normal", 10, 50, 0),
        ];

        let config = SwarmAllocationConfig {
            max_global_peers: 50,
            max_peers_per_file: 40,
            min_peers_per_file: 2,
        };

        let quotas = SwarmAllocator::compute_quotas(&demands, &config);
        assert_eq!(quotas.get("hash_dead"), Some(&0));
        assert_eq!(quotas.get("hash_alive"), Some(&10));
    }
}
