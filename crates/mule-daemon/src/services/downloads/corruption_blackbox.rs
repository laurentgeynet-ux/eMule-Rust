// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Provenance des données reçues pour attribuer la corruption d'une part à
//! l'expéditeur — portage fidèle de `CCorruptionBlackBox` eMule 0.72a
//! (`CorruptionBlackBox.cpp`).
//!
//! Chaque bloc reçu est enregistré par part avec son expéditeur. Quand une
//! part est vérifiée ou jugée corrompue, les enregistrements chevauchants
//! sont marqués `Verified`/`Corrupted`. `evaluate` reproduit
//! `CCorruptionBlackBox::EvaluateData` : pour chaque expéditeur ayant au
//! moins un enregistrement corrompu dans la part fautive, le ratio
//! `corrompu / (corrompu + vérifié)` calculé sur **toutes** les parts du
//! fichier est comparé à `CBB_BANTHRESHOLD` (32 %) — au-delà, eMule bannit
//! le client (« Identified as a sender of corrupt data »).

use std::collections::{HashMap, HashSet};

/// eMule `CBB_BANTHRESHOLD` — pourcentage de données corrompues déclenchant
/// le bannissement de l'expéditeur.
pub const CBB_BANTHRESHOLD_PERCENT: u64 = 32;

/// eMule `EMBLOCKSIZE` — un enregistrement corrompu compte toujours pour au
/// moins un bloc complet (180 Kio), même si la plage corrompue est plus petite.
const EMBLOCKSIZE: u64 = 184_320;

/// Borne défensive sur les enregistrements conservés par part : une part de
/// 9 728 000 octets couvre ~54 blocs eMule ; les réécritures multiplient les
/// enregistrements par fragmentation, sans ordre de grandeur au-delà.
const MAX_RECORDS_PER_PART: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordStatus {
    /// Donnée reçue, pas encore évaluée (eMule `BBR_NONE`).
    Received,
    /// Donnée dont la part/le bloc a passé la vérif (eMule `BBR_VERIFIED`).
    Verified,
    /// Donnée appartenant à une plage corrompue (eMule `BBR_CORRUPTED`).
    Corrupted,
}

#[derive(Debug, Clone)]
struct CbbRecord {
    /// Plage demi-ouverte `[start, end)` en octets absolus du fichier.
    start: u64,
    end: u64,
    /// Adresse de l'expéditeur (`ip:port` ou `lowid:N`).
    sender: String,
    status: RecordStatus,
}

/// État du blackbox pour un téléchargement (eMule `m_CorruptionBlackBox`).
#[derive(Debug, Default)]
pub struct CorruptionBlackBox {
    /// Enregistrements par index de part (eMule `m_aaRecords`).
    records: HashMap<u32, Vec<CbbRecord>>,
}

impl CorruptionBlackBox {
    /// `CCorruptionBlackBox::ReceivedData` : enregistre la livraison de
    /// `[start, end)` par `sender`. Les enregistrements existants perdent la
    /// portion réécrite — les octets réécrits changent de responsable
    /// (eMule remplace les stats des plages recouvertes).
    pub fn received_data(&mut self, part: u32, start: u64, end: u64, sender: &str) {
        if end <= start || sender.is_empty() {
            return;
        }
        let recs = self.records.entry(part).or_default();
        subtract_range(recs, start, end);
        if recs.len() < MAX_RECORDS_PER_PART {
            recs.push(CbbRecord {
                start,
                end,
                sender: sender.to_string(),
                status: RecordStatus::Received,
            });
        }
    }

    /// `CCorruptionBlackBox::VerifiedData` : marque la portion `[start, end)`
    /// des enregistrements comme vérifiée.
    pub fn verified_data(&mut self, part: u32, start: u64, end: u64) {
        self.mark(part, start, end, RecordStatus::Verified);
    }

    /// `CCorruptionBlackBox::CorruptedData` : marque la portion `[start, end)`
    /// des enregistrements comme corrompue.
    pub fn corrupted_data(&mut self, part: u32, start: u64, end: u64) {
        self.mark(part, start, end, RecordStatus::Corrupted);
    }

    /// `CCorruptionBlackBox::EvaluateData` : retourne les expéditeurs dont le
    /// ratio de données corrompues dépasse `CBB_BANTHRESHOLD` (32 %).
    ///
    /// Les suspects sont les expéditeurs ayant au moins un enregistrement
    /// corrompu dans `part`. Pour chacun, le ratio est calculé sur **toutes**
    /// les parts du fichier : chaque enregistrement corrompu compte pour
    /// `max(longueur, EMBLOCKSIZE)` octets, chaque enregistrement vérifié
    /// pour sa longueur réelle.
    pub fn evaluate(&self, part: u32) -> Vec<String> {
        let suspects: HashSet<&str> = self
            .records
            .get(&part)
            .map(|recs| {
                recs.iter()
                    .filter(|r| r.status == RecordStatus::Corrupted)
                    .map(|r| r.sender.as_str())
                    .collect()
            })
            .unwrap_or_default();
        if suspects.is_empty() {
            return Vec::new();
        }

        let mut guilty = Vec::new();
        for sender in suspects {
            let mut corrupt_bytes = 0u64;
            let mut verified_bytes = 0u64;
            for recs in self.records.values() {
                for rec in recs {
                    if rec.sender != sender {
                        continue;
                    }
                    match rec.status {
                        RecordStatus::Corrupted => {
                            corrupt_bytes += (rec.end - rec.start).max(EMBLOCKSIZE)
                        }
                        RecordStatus::Verified => verified_bytes += rec.end - rec.start,
                        RecordStatus::Received => {}
                    }
                }
            }
            let total = corrupt_bytes + verified_bytes;
            // Comparaison entière équivalente à `100*corrupt/total > 32`.
            if total > 0 && corrupt_bytes * 100 > total * CBB_BANTHRESHOLD_PERCENT {
                guilty.push(sender.to_string());
            }
        }
        guilty
    }

    /// `m_CorruptionBlackBox.Free()` — libération à la complétion du fichier.
    pub fn clear(&mut self) {
        self.records.clear();
    }

    fn mark(&mut self, part: u32, start: u64, end: u64, status: RecordStatus) {
        if end <= start {
            return;
        }
        let Some(recs) = self.records.get_mut(&part) else {
            return;
        };
        let mut i = 0;
        while i < recs.len() {
            let rec = &recs[i];
            let ov_start = rec.start.max(start);
            let ov_end = rec.end.min(end);
            if ov_end <= ov_start {
                i += 1;
                continue;
            }
            // Découpe l'enregistrement en [avant] [chevauchant] [après] :
            // seule la portion chevauchante change de statut (eMule Split) —
            // les fragments externes conservent le statut d'origine.
            let before = (rec.start < ov_start).then_some((rec.start, ov_start));
            let after = (rec.end > ov_end).then_some((ov_end, rec.end));
            let mut rec = recs.remove(i);
            let old_status = rec.status;
            rec.start = ov_start;
            rec.end = ov_end;
            rec.status = status;
            if let Some((bs, be)) = before {
                recs.insert(
                    i,
                    CbbRecord {
                        start: bs,
                        end: be,
                        sender: rec.sender.clone(),
                        status: old_status,
                    },
                );
                i += 1;
            }
            recs.insert(i, rec);
            i += 1;
            if let Some((as_, ae)) = after {
                if recs.len() < MAX_RECORDS_PER_PART {
                    recs.insert(
                        i,
                        CbbRecord {
                            start: as_,
                            end: ae,
                            sender: recs[i - 1].sender.clone(),
                            status: old_status,
                        },
                    );
                }
                i += 1;
            }
        }
    }
}

/// Retire la portion `[start, end)` de chaque enregistrement (eMule
/// `ReceivedData` : les octets réécrits perdent leurs anciennes stats).
fn subtract_range(recs: &mut Vec<CbbRecord>, start: u64, end: u64) {
    let mut i = 0;
    while i < recs.len() {
        let rec = &recs[i];
        let ov_start = rec.start.max(start);
        let ov_end = rec.end.min(end);
        if ov_end <= ov_start {
            i += 1;
            continue;
        }
        let before = (rec.start < ov_start).then_some((rec.start, ov_start));
        let after = (rec.end > ov_end).then_some((ov_end, rec.end));
        let rec = recs.remove(i);
        match (before, after) {
            (Some((bs, be)), Some((as_, ae))) => {
                recs.insert(
                    i,
                    CbbRecord {
                        start: as_,
                        end: ae,
                        sender: rec.sender.clone(),
                        status: rec.status,
                    },
                );
                recs.insert(
                    i,
                    CbbRecord {
                        start: bs,
                        end: be,
                        sender: rec.sender,
                        status: rec.status,
                    },
                );
                i += 2;
            }
            (Some((bs, be)), None) => {
                recs.insert(
                    i,
                    CbbRecord {
                        start: bs,
                        end: be,
                        sender: rec.sender,
                        status: rec.status,
                    },
                );
                i += 1;
            }
            (None, Some((as_, ae))) => {
                recs.insert(
                    i,
                    CbbRecord {
                        start: as_,
                        end: ae,
                        sender: rec.sender,
                        status: rec.status,
                    },
                );
                i += 1;
            }
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// eMule `EvaluateData` : un expéditeur dont toute la livraison est
    /// corrompue dépasse 32 % — banni.
    #[test]
    fn single_sender_fully_corrupt_is_guilty() {
        let mut bb = CorruptionBlackBox::default();
        bb.received_data(0, 0, 9_728_000, "1.2.3.4:49662");
        bb.corrupted_data(0, 0, 9_728_000);
        assert_eq!(bb.evaluate(0), vec!["1.2.3.4:49662".to_string()]);
    }

    /// Un expéditeur suspect n'est banni que si `corrompu/(corrompu+vérifié)`
    /// dépasse 32 % — la dilution par des données vérifiées l'exonère.
    #[test]
    fn verified_data_dilutes_corruption_ratio() {
        let mut bb = CorruptionBlackBox::default();
        // 3 parts vérifiées (~29,2 Mio) + une part corrompue (~9,7 Mio)
        // → ratio ≈ 25 % < 32 % : soupçonné mais pas banni.
        for part in 0..3 {
            bb.received_data(part, part as u64 * 9_728_000, (part as u64 + 1) * 9_728_000, "1.2.3.4:49662");
            bb.verified_data(part, part as u64 * 9_728_000, (part as u64 + 1) * 9_728_000);
        }
        bb.received_data(3, 3 * 9_728_000, 4 * 9_728_000, "1.2.3.4:49662");
        bb.corrupted_data(3, 3 * 9_728_000, 4 * 9_728_000);
        assert!(bb.evaluate(3).is_empty());

        // Avec 2 parts vérifiées seulement : ~33 % > 32 % → banni.
        let mut bb2 = CorruptionBlackBox::default();
        for part in 0..2 {
            bb2.received_data(part, part as u64 * 9_728_000, (part as u64 + 1) * 9_728_000, "1.2.3.4:49662");
            bb2.verified_data(part, part as u64 * 9_728_000, (part as u64 + 1) * 9_728_000);
        }
        bb2.received_data(2, 2 * 9_728_000, 3 * 9_728_000, "1.2.3.4:49662");
        bb2.corrupted_data(2, 2 * 9_728_000, 3 * 9_728_000);
        assert_eq!(bb2.evaluate(2), vec!["1.2.3.4:49662".to_string()]);
    }

    /// `ReceivedData` remplace la provenance des octets réécrits : le
    /// réexpéditeur hérite de la responsabilité de la plage recouverte.
    #[test]
    fn rewritten_range_changes_attribution() {
        let mut bb = CorruptionBlackBox::default();
        bb.received_data(0, 0, 9_728_000, "bad:1");
        // La plage [100, 200) est re-livrée par un autre pair.
        bb.received_data(0, 100, 200, "good:1");
        // Corruption limitée à la plage réécrite → seul "good" est suspect.
        bb.corrupted_data(0, 100, 200);
        let guilty = bb.evaluate(0);
        assert_eq!(guilty, vec!["good:1".to_string()]);
    }

    /// Un enregistrement corrompu compte pour au moins `EMBLOCKSIZE` :
    /// une toute petite plage corrompue suffit à bannir un expéditeur
    /// qui n'a presque rien envoyé de vérifié.
    #[test]
    fn corrupted_record_counts_at_least_block_size() {
        let mut bb = CorruptionBlackBox::default();
        bb.received_data(0, 0, 1000, "1.2.3.4:49662");
        bb.received_data(0, 1000, 2000, "1.2.3.4:49662");
        bb.verified_data(0, 0, 1000);
        bb.corrupted_data(0, 1000, 2000);
        // corrompu compté = 184 320 (pas 1000) → 100 % > 32 %.
        assert_eq!(bb.evaluate(0), vec!["1.2.3.4:49662".to_string()]);
    }

    /// `evaluate` n'incrimine que les expéditeurs ayant livré des données
    /// corrompues dans LA part évaluée.
    #[test]
    fn only_suspects_of_failed_part_are_evaluated() {
        let mut bb = CorruptionBlackBox::default();
        bb.received_data(0, 0, 9_728_000, "innocent:1");
        bb.corrupted_data(0, 0, 9_728_000);
        // "innocent" est le seul suspect → banni ; un expéditeur d'une autre
        // part n'entre pas dans l'évaluation sauf s'il est aussi suspect.
        bb.received_data(1, 9_728_000, 2 * 9_728_000, "other:1");
        let guilty = bb.evaluate(0);
        assert_eq!(guilty, vec!["innocent:1".to_string()]);
    }
}
