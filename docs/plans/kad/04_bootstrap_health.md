# Étape 4 — Santé du bootstrap (`BootstrapHealth`)

> **Dépendances** : étapes 1–3 — le probing produit les métriques (âge `last_seen`, taux d'échec) dont la santé se nourrit.
> **Nature** : amélioration **au-delà** d'eMule, purement locale — le gate actuel est déjà eMule-fidèle (`Kademlia.cpp:287` : `!IsConnected()` = `HasHadContact()`, cadences 2 s/15 s) et la maintenance (30 s, `kad.rs:677-688`) élargit déjà tant que `0 < contacts < 400`. **Ce n'est pas un correctif de bug, c'est un raffinement de cadence.**

## Objectif

Remplacer le booléen `has_verified` (`kad.rs:586-595`) par un état de santé : un seul contact vérifié ne doit pas suffire à passer en cadence « healthy » — une table à 3 contacts mal distribués est fonctionnellement fragile même si un pair a répondu une fois.

## Changements

### 4a. Nouveau fichier `crates/mule-daemon/src/services/kad/bootstrap_health.rs`

```rust
pub enum BootstrapHealth { Empty, Fragile, Building, Healthy }

pub struct BootstrapHealthSnapshot {
    pub health: BootstrapHealth,
    pub verified: usize,
    pub nonempty_buckets: usize,
    pub distinct_subnet24: usize,
    pub recently_seen: usize,   // last_seen < RECENT_WINDOW
}

pub fn assess(table: &KBucketTable, local_id: &KadId) -> BootstrapHealthSnapshot
```

Critères indicatifs (constantes en tête de fichier — ajuster à la mesure) :

| État | Critère | Cadence drain |
|---|---|---|
| `Empty` | `verified == 0` | `BOOTSTRAP_DRAIN_EMPTY = 2 s` |
| `Fragile` | `1..15` vérifiés **ou** `nonempty_buckets < 8` **ou** `/24 < 4` | `BOOTSTRAP_DRAIN_FRAGILE = 5 s` |
| `Building` | `15..50` vérifiés | `BOOTSTRAP_DRAIN_BUILDING = 15 s` (eMule) |
| `Healthy` | `≥ 50` vérifiés, diversité suffisante | skip (`continue`, comportement actuel) |

### 4b. `kad.rs`

- **Drain** (l.586-625) : `let snap = assess(&table, &local_id)` ; `Healthy` → `continue` ; sinon intervalle par état au lieu du `min_interval` binaire.
- **Maintenance** (l.677) : seuil d'expansion modulé — `Fragile` → `bootstrap_active_contacts(BOOTSTRAP_SAMPLE_FRAGILE)` plus large que le `15` actuel.
- Log `kad.bootstrap.health = "fragile"` à chaque transition d'état.

### 4c. `Degraded` (état optionnel — après étape 5)

Une fois la taxonomie d'erreurs disponible : `Degraded` = chute récente de contacts vérifiés OU taux de timeout élevé → retour temporaire à cadence `Fragile`. Ne pas l'implémenter tant que l'étape 5 n'est pas faite.

## Constantes (`bootstrap_health.rs`)

`HEALTH_FRAGILE_MAX_VERIFIED`, `HEALTH_MIN_BUCKETS`, `HEALTH_MIN_SUBNETS`, `RECENT_WINDOW`, `BOOTSTRAP_DRAIN_*`, `BOOTSTRAP_SAMPLE_FRAGILE`.

## Tests

- `assess` : tables synthétiques → chaque état (vérifiés=0 ; 3 vérifiés/1 bucket → Fragile malgré >0 ; 60 vérifiés/30 buckets → Healthy).
- Drain : file de candidats non vide + état Fragile → drain continue (aujourd'hui `continue` dès le premier vérifié).

## Critères de fin

- [ ] Une table avec 1 vérifié continue de drainer.
- [ ] `Healthy` retrouvé = gate actuel inchangé (comportement eMule préservé).
- [ ] Transition d'état loggée une fois (pas à chaque tick).
