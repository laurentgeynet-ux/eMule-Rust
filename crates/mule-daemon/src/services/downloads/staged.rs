// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Staging RAM des parts eD2K (modèle eMule `m_tData` / libtorrent
//! staging → hash → flush) : les octets injectés sont copiés dans un
//! buffer par part pour permettre la vérification MD4 **sans relecture
//! disque** à la complétion — supprime ~`total_size` octets de `pread`
//! par téléchargement.

use mule_core::RangeSet;
use std::collections::HashMap;

/// Borne mémoire : nombre maximum de parts stagées simultanément par
/// download (≈ 4 × 9,3 Mio). Au-delà, la vérification retombe sur la
/// relecture disque (`PartFile::read_range`).
pub(crate) const MAX_STAGED_PARTS: usize = 4;

/// Buffer d'assemblage d'une part en cours de réception.
///
/// `pub` requis par le champ publique `ActiveDownload::staged_parts` ;
/// toutes les opérations restent `pub(crate)`.
pub struct StagedPart {
    /// Octets de la part reçus cette session, indexés par offset local.
    /// Dimensionné à la taille réelle de la part (dernière part incluse).
    data: Vec<u8>,
    /// Couverture des octets effectivement stagés (offsets locaux).
    covered: RangeSet,
}

impl StagedPart {
    fn new(part_len: usize) -> Self {
        Self {
            data: vec![0u8; part_len],
            covered: RangeSet::new(),
        }
    }

    /// Copie des octets `[local_start, local_start+bytes)` dans le buffer.
    fn stage(&mut self, local_start: u64, bytes: &[u8]) {
        let s = local_start as usize;
        self.data[s..s + bytes.len()].copy_from_slice(bytes);
        let _ = self.covered.insert(local_start..local_start + bytes.len() as u64);
    }

    /// Tous les octets `[0, part_len)` ont été stagés cette session → la
    /// part est hashable entièrement en RAM.
    fn is_fully_staged(&self, part_len: u64) -> bool {
        self.covered.covered_len() == part_len
    }
}

/// Copie les octets d'une range injectée dans les buffers des parts
/// touchées. Les parts déjà complètes ne sont pas stagées (leur donnée
/// est ignorée en amont), et au-delà de [`MAX_STAGED_PARTS`] les nouvelles
/// parts ne sont pas stagées — la vérification relira le disque.
pub(crate) fn stage_range(
    staged: &mut HashMap<u32, StagedPart>,
    total_size: u64,
    part_size: u64,
    offset: u64,
    data: &[u8],
) {
    let end = offset + data.len() as u64;
    let first_part = offset / part_size;
    let last_part = (end - 1) / part_size;
    for p in first_part..=last_part {
        let ps = p * part_size;
        let pe = ((p + 1) * part_size).min(total_size);
        let s = offset.max(ps);
        let e = end.min(pe);
        if s >= e {
            continue;
        }
        let key = p as u32;
        if staged.len() >= MAX_STAGED_PARTS && !staged.contains_key(&key) {
            continue;
        }
        let part_len = (pe - ps) as usize;
        let sp = staged.entry(key).or_insert_with(|| StagedPart::new(part_len));
        let ds = (s - offset) as usize;
        let de = (e - offset) as usize;
        sp.stage(s - ps, &data[ds..de]);
    }
}

/// Hash MD4 de la part depuis son buffer stagé — `None` quand la part
/// n'est pas entièrement stagée (download repris à mi-part, borne
/// mémoire atteinte) et que l'appelant doit relire le disque.
pub(crate) fn verify_staged(
    staged: &mut HashMap<u32, StagedPart>,
    part_idx: u32,
    part_len: u64,
    expected_hash: &[u8; 16],
) -> Option<bool> {
    let sp = staged.get(&part_idx)?;
    if !sp.is_fully_staged(part_len) {
        return None;
    }
    Some(mule_core::verify_part_hash(&sp.data, expected_hash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use md4::{Digest, Md4};

    const PART: u64 = 100;
    const TOTAL: u64 = 250; // 3 parts : 100 + 100 + 50

    fn md4_of(data: &[u8]) -> [u8; 16] {
        Md4::digest(data).into()
    }

    #[test]
    fn stage_and_verify_full_part() {
        let mut staged = HashMap::new();
        let data: Vec<u8> = (0..TOTAL).map(|i| i as u8).collect();
        // Deux blocs pour la part 0, un bloc à cheval sur les parts 1-2.
        stage_range(&mut staged, TOTAL, PART, 0, &data[..60]);
        stage_range(&mut staged, TOTAL, PART, 60, &data[60..]);
        assert_eq!(staged.len(), 3);
        let expected = md4_of(&data[..100]);
        assert_eq!(
            verify_staged(&mut staged, 0, 100, &expected),
            Some(true),
            "part 0 entièrement stagée → hash vérifiable en RAM"
        );
    }

    #[test]
    fn verify_staged_returns_none_when_incomplete() {
        let mut staged = HashMap::new();
        let data = vec![7u8; 80];
        stage_range(&mut staged, TOTAL, PART, 0, &data);
        let expected = md4_of(&[7u8; 100]);
        assert_eq!(
            verify_staged(&mut staged, 0, 100, &expected),
            None,
            "part partiellement stagée → repli disque requis"
        );
    }

    #[test]
    fn verify_staged_detects_corruption() {
        let mut staged = HashMap::new();
        let data = vec![9u8; 100];
        stage_range(&mut staged, TOTAL, PART, 0, &data);
        let wrong = md4_of(&[0u8; 100]);
        assert_eq!(verify_staged(&mut staged, 0, 100, &wrong), Some(false));
    }

    #[test]
    fn staged_parts_bounded_by_max() {
        let mut staged = HashMap::new();
        let block = vec![1u8; 10];
        // Remplit MAX_STAGED_PARTS parts distinctes.
        for p in 0..MAX_STAGED_PARTS as u64 {
            stage_range(&mut staged, u64::MAX / 2, PART, p * PART, &block);
        }
        assert_eq!(staged.len(), MAX_STAGED_PARTS);
        // Une (MAX+1)-ième part n'est pas stagée.
        stage_range(
            &mut staged,
            u64::MAX / 2,
            PART,
            MAX_STAGED_PARTS as u64 * PART,
            &block,
        );
        assert_eq!(staged.len(), MAX_STAGED_PARTS);
        // Mais une part déjà présente continue d'absorber des blocs.
        stage_range(&mut staged, u64::MAX / 2, PART, 10, &block);
        assert_eq!(staged.len(), MAX_STAGED_PARTS);
    }

    #[test]
    fn range_spanning_part_boundary_splits() {
        let mut staged = HashMap::new();
        let data = vec![3u8; 30];
        // 85..115 couvre la fin de la part 0 et le début de la part 1.
        stage_range(&mut staged, TOTAL, PART, 85, &data);
        let p0 = staged.get(&0).expect("part 0 stagée");
        let p1 = staged.get(&1).expect("part 1 stagée");
        assert_eq!(p0.covered.ranges(), vec![85..100]);
        assert_eq!(p1.covered.ranges(), vec![0..15]);
        assert_eq!(&p0.data[85..100], &[3u8; 15]);
        assert_eq!(&p1.data[..15], &[3u8; 15]);
    }
}
