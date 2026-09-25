# Étape 6 — Contention : snapshot + tri hors verrou

> **Dépendances** : aucune. Petite étape chirurgicale — caser quand on veut.
> **Prérequis** : **mesurer d'abord** — ajouter un log de durée de détention du mutex avant d'optimiser plus loin.

## Objectif

`find_closest*` = `collect_all()` + tri complet **sous** le `Mutex` global (`routing_table.rs:229-251`). Avec ~1 300 contacts et des lookups/publications/bootstrap concurrents, la section critique est inutilement longue.

## Changements

### 6a. `mule-core/src/kad/routing_table.rs`

Rendre `collect_all` accessible : `pub fn snapshot_all(&self) -> Vec<KadContact>` (ou `pub` direct si cohérent avec l'API). Copie `Vec` sous verrou = O(n) memcpy — beaucoup plus court que collecte **+** tri.

### 6b. Callers — trier hors verrou

| Site | Avant | Après |
|---|---|---|
| `kad.rs` `bootstrap_active_contacts` (l.472-475) | `find_closest` sous verrou | `snapshot_all` sous verrou → `sort_by_key(\|c\| local_id.distance_to(c.kad_id))` + `take(n)` hors verrou |
| `lookup.rs` seeds (`converge_closest_contacts`) | idem | idem |
| `spread_verified` / sélections internes | idem | idem si sortie de verrou possible |

**Conserver les variantes `find_verified_closest`** pour les chemins où le filtre `verified` doit être cohérent avec la mutation (learn_contact sous le même lock) — décider au cas par cas, documenter.

### 6c. Instrumentation

`tracing::debug!` de la durée de détention du lock dans les sites chauds (avant/après) — la métrique décide si une étape ultérieure (`RwLock`, verrous par bucket) est justifiée. **Ne pas** implémenter de structure concurrente fine sans métrique la demandant : les invariants LRU/replacement cache valent plus qu'un gain spéculatif.

## Tests

- Mêmes tests que la table (ordre de tri identique avant/après — `sort_by_key` stable sur la même clé).
- Test de contention : N tâches en `snapshot_all` concurrent + inserts — pas de deadlock, résultats cohérents.

## Critères de fin

- [ ] Durée de détention du mutex mesurée et réduite sur les chemins chauds.
- [ ] Ordre des contacts retournés identique aux tests existants.
- [ ] Pas de `RwLock`/bucketing fin sans métrique le justifiant.
