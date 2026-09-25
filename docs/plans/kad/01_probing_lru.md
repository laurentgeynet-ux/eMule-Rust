# Étape 1 — Probing LRU (`ContactVerifier`)

> **Dépendances** : aucune (`KadUdpService::ping` existe déjà).
> **Risque interop** : nul — `PING`/`PONG` (0x60/0x61) sont standard eMule.

## Objectif

Quand un bucket est plein et qu'un nouveau contact y est proposé, `insert_or_update` place le nouveau dans le replacement cache et retourne le **LRU** (moins récemment vu) comme candidat de sonde. Aujourd'hui ce retour est ignoré partout : les contacts morts ne sont jamais remplacés par les candidats frais. C'est le défaut de liveness n°1.

## État vérifié

- `KBucket::insert_or_update` → `Option<KadContact>` : `Some(lru)` quand bucket plein — `routing_table.rs:82-95`.
- `KBucket::remove` promotionne déjà `replacements.pop_back()` → `contacts.push_back` — `routing_table.rs:98-108`.
- `KBucketTable::mark_failed` incrémente `fail_count`, évince au seuil — `routing_table.rs:170-203`.
- `mark_seen` réinitialise `verified`/`fail_count` — `routing_table.rs:207-213`.
- `udp.ping(addr)` complet avec transaction `Pong` — `routing_client.rs:20-88`.
- `learn_contact` (point chaud) : `handlers.rs:1005-1026`, appelé depuis ~4 sites, contexte handler struct ligne ~41.

## Changements

### 1a. Nouveau fichier `crates/mule-daemon/src/services/kad/contact_verifier.rs`

```rust
/// Candidat de sonde LRU retourné par un bucket plein.
pub struct ProbeQueue {
    queue: VecDeque<KadContact>,
    queued: HashSet<KadId>,              // dédup
    cooldowns: HashMap<KadId, Instant>,  // cooldown par contact
}
```

- `ProbeQueue::push(contact)` — ignore si déjà en file ou en cooldown ; borne `PROBE_QUEUE_CAP` (drop les plus anciens en tête si saturée).
- `ProbeQueue::take_next()` — retire et marque le cooldown.
- `ContactVerifier` : tâche Tokio — boucle `select!` sur cancel + réveil périodique ; extrait les candidats ; `Semaphore::new(PROBE_CONCURRENCY)` borne les `ping` concurrents ; à la réponse → `table.mark_seen(kad_id, now)` ; en échec → `table.mark_failed(kad_id)` (l'éviction + promotion sont automatiques dans `KBucket::remove`).

### 1b. Câblage

| Site | Action |
|---|---|
| `kad.rs` struct `KadService` | Champ `probe_queue: Arc<tokio::sync::Mutex<ProbeQueue>>` |
| `kad.rs:187, 290, 353, 539, 819` | `if let Some(lru) = table.insert_or_update(c) { probe_queue.push(lru) }` — pousser **après** libération du verrou table |
| `lookup.rs:152` | Idem |
| `handlers.rs` contexte (~l.41) | Champ `probe_queue` ; `learn_contact` (l.1025) capture le retour et pousse |
| `kad.rs:start_background_maintenance` | `tokio::spawn` du worker avec `cancel_token` |

**Ordre des verrous** : jamais `probe_queue.lock()` pendant `routing_table.lock()` — pousser dans une `Vec` tampon puis drainer hors verrou (cohérent avec la règle existante « `in_flight` libéré avant `learn_contact` »).

## Constantes (tête de `contact_verifier.rs`, documentées)

| Nom | Valeur | Rôle |
|---|---|---|
| `PROBE_CONCURRENCY` | `6` | Probes UDP simultanés max |
| `PROBE_COOLDOWN` | `15 min` | Délai min entre deux probes du même KadID |
| `PROBE_QUEUE_CAP` | `128` | File bornée — anti-flood sur paquets entrants |

## Tests

- `mule-core` (existant à compléter) : bucket plein → insert → `Some(lru)` retourné ; `mark_failed` répété → éviction → candidat du replacement cache promu dans `contacts`.
- `mule-daemon` (loopback) : socket UDP factice qui ne répond pas → le contact est évincé et remplacé ; socket qui répond `Pong` → `mark_seen` (verified).
- Cooldown : deux inserts du même candidat dans la fenêtre → un seul probe.

## Critères de fin

- [ ] Un LRU mort est remplacé par un candidat du replacement cache **sans action manuelle**.
- [ ] Un LRU vivant est `mark_seen` et reste.
- [ ] La file est bornée et dédupliquée ; aucun probe sous cooldown.
- [ ] Logs `tracing` structurés : `kad.probe.attempt/ok/fail/promoted`.

## Référence eMule

`CRoutingZone::OnBigTimer` / `CContact::CheckingType` — eMule sonde le LRU et démarre un timer d'éviction ; le nouveau contact attend dans la zone de remplacement.
