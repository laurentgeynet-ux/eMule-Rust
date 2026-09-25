# Étape 9 — Validation interop (transverse)

> **Nature** : continue — chaque étape livrée embarque ses tests ; cette étape couvre l'outillage transverse.
> **Contrainte** : tout est hors-ligne — loopback, corpus de fichiers, propriétés. Zéro trafic sortant.

## 9a. Golden packets (`mule-protocol/tests/`)

- Corpus `crates/mule-protocol/tests/fixtures/kad/*.bin` — captures réelles eMule/aMule (chiffrement retiré : `0xE4` en clair).
- Test : pour chaque fichier, `decode` → `encode` → bytes identiques (round-trip déterministe, exigé par `mule-protocol` AGENTS).
- Priorité aux paquets touchés par les étapes : `HELLO_*`, `SEARCH_*_RES` multi-formes, `FINDBUDDY_*`, `PUBLISH_*`.

## 9b. Tests de propriété (`proptest` déjà en dev-dep workspace)

| Propriété | Cible |
|---|---|
| `xor_distance` symétrique, non-négative, `dist(a,a)=0` | `kad_id.rs` |
| `bucket_index_for` ∈ `0..128`, monotone vs distance | `routing_table.rs` |
| `is_within_search_tolerance` ↔ `Get32BitChunk(0) ≤ SEARCHTOLERANCE` (boundary DWORD ordering) | `kad_id.rs` |
| `from_hash_be` ∘ `to_hash_be` = identité | `kad_id.rs` |
| Pagination : `start_pos` arbitraire → décodeur borne les entrées | `search_expr.rs`/decoder |

## 9c. Labo multi-nœuds loopback

Extension de `scripts/test_qa_loopback_e2e.ps1` + harness `mule-daemon` (feature `lab-loopback` uniquement) :

- **Perte** : drop aléatoire borné de datagrammes → la table survit (probing + mark_failed).
- **Duplication** : même paquet reçu 2× → dédup transaction, pas de double `mark_seen`.
- **Retard/réordonnancement** : paquets différés → fenêtre glissante collecte correctement.
- **Changement de NAT simulé** : réponse `FIREWALLED_RES` avec IP différente → revalidation déclenchée (étape 3).
- Assert final : convergence bootstrap jusqu'à un seuil de contacts vérifiés même avec seeds partiellement morts.

## 9d. Fuzzing borné des décodeurs

`cargo-fuzz` ou `proptest` byte-level sur les points d'entrée `kad/decoder/*` : tailles de champs, `tag counts`, profondeur d'arbre d'expression — jamais d'allocation proportionnelle à un champ non validé (`limits.rs`).

## Jalons par étape

| Étape | Ajout validation |
|---|---|
| 1–2 | Probe/HELLO loopback + corpus `HELLO_*` |
| 3 | Test `mark_stale` sur IP changée |
| 4 | Tests `assess` sur tables synthétiques |
| 5 | Scénarios perte/dup/réordonnancement |
| 7 | Corpus `SEARCH_*` multi-formes + round-trip expression |
| 8 | Scénario buddy complet loopback |

## Critère global de sortie

`scripts/verify_all.ps1` vert + zéro divergence encode/decode sur corpus réel + table Kad qui survit aux scénarios de chaos loopback.
