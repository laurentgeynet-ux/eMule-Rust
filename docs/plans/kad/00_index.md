# Plan d'amélioration Kad — Index

> **Statut** : planifié — aucune étape implémentée.
> Chaque étape est un commit/PR indépendant, compilable et testé isolément.
> Contrainte absolue : **compatibilité filaire eMule 0.72a** — chaque changement est soit une politique locale invisible sur le fil, soit un comportement qu'eMule parle nativement (vérifié dans `D:\eMule0.72a-Sources\srchybrid`).

---

## Diagnostic vérifié (état des lieux)

### Conforme à eMule — ne pas toucher

| Comportement | Vérification |
|---|---|
| Datagrammes Kad UDP en clair (`0xE4`) | eMule n'obfusque pas le trafic Kad ordinaire ; RC4 réservé aux paquets à clé (firewall `FIREWALLUDP`, verify keys) |
| Pas de `SEARCHTOLERANCE` sur `SEARCH_*_REQ` entrants | `KademliaUDPListener.cpp:1052-1095` — eMule sert l'index sans check de distance ; la tolérance n'existe que dans les `PUBLISH_*_REQ` (lignes 1188, 1286, 1519) |
| Gate du drain bootstrap (`has_verified`) | `Kademlia.cpp:287` — `!IsConnected()` = `HasHadContact()` ; cadences 2 s/15 s identiques (`kad.rs:586-599`) |
| `KadId` aléatoire persisté, rotation uniquement via `identityRotationHours` (défaut 0) | eMule génère aussi un KadID aléatoire persisté |
| Promotion depuis replacement cache à l'éviction | `KBucket::remove` le fait déjà (`routing_table.rs:98-108`) |

### Manques réels — objet de ce plan

| Manque | Localisation |
|---|---|
| Candidat LRU retourné par `insert_or_update` jamais probé | `routing_table.rs:82-95` — retour consommé nulle part (7 sites : `kad.rs:187/290/353/539/819`, `handlers.rs:1025`, `lookup.rs:152`) |
| `HELLO_REQ` jamais émis (encodeur présent, zéro appelant) | `encoder.rs:124-163` vs `routing_client.rs` |
| Drain bootstrap stoppe au premier contact vérifié (fidèle mais fragile) | `kad.rs:586-595` |
| État firewall figé après conclusion (`!fw.verified` gate) | `kad.rs:694-705` |
| Fenêtre collecte search fixe 350 ms | `search_client.rs:19-47` |
| `send_search_sources` hardcode `start_pos=0` | `search_client.rs:85` |
| Recherche : un seul mot-clé, pas d'expression restrictive sortante, pas de pagination | `search_keywords.rs` |
| Coalescing `in_flight_searches` renvoie `Ok(vec![])` au duplicata | `kad.rs:45`, `search_sources.rs:24-54` |
| `publish_keyword` annonce `source_count: 1` constant | `search_keywords.rs:230` |
| Buddy : rôle relais seul, pas de rôle client LowID | `types.rs:251-346` |
| Matching transactions par IP seule | `kad_udp.rs` receive loop ~l.478 |
| `find_closest` = collecte + tri sous le Mutex global | `routing_table.rs:229-251` |

### Exclusions définitives (prouvées non-eMule ou impossibles)

- ❌ Tolérance sur recherches entrantes — eMule n'en fait aucune, en ajouter une diverge.
- ❌ Nonce de corrélation filaire — Kad2 ne transporte aucun nonce arbitraire.
- ❌ Obfuscation globale des datagrammes Kad — non standard, casse l'interop.
- ❌ Rotation périodique agressive du KadID — déjà désactivée par défaut.
- ❌ Enum `BucketInsertOutcome` — `Option<KadContact>` suffit ; le manque est le consommateur, pas le type.

---

## Ordre d'exécution

| # | Étape | Fichier | Dépendances |
|---|---|---|---|
| 1 | Probing LRU (`ContactVerifier`) | [01_probing_lru.md](01_probing_lru.md) | Aucune — `udp.ping()` existe |
| 2 | `HELLO_REQ` sortant | [02_hello_req.md](02_hello_req.md) | Étape 1 (le verifier héberge la logique) |
| 3 | Revalidation firewall | [03_firewall_revalidation.md](03_firewall_revalidation.md) | Aucune — livrable en quick win dès étape 1 |
| 4 | Santé bootstrap (`BootstrapHealth`) | [04_bootstrap_health.md](04_bootstrap_health.md) | Étapes 1–3 (métriques produites par le probing) |
| 5 | Robustesse UDP (erreurs typées, retry, fenêtre glissante, matching) | [05_udp_robustesse.md](05_udp_robustesse.md) | Étape 4 (la taxonomie d'erreurs sert à `Degraded`) |
| 6 | Contention (snapshot + tri hors verrou) | [06_contention_mutex.md](06_contention_mutex.md) | Aucune — petit, caser entre deux |
| 7 | Recherche (expression restrictive, multi-termes, pagination, coalescing) | [07_recherche.md](07_recherche.md) | Étape 5 (pagination dépend du transport fiable) |
| 8 | Buddy client LowID | [08_buddy_client.md](08_buddy_client.md) | Contacts vérifiés stables + matching durci (étapes 1, 5) |
| 9 | Validation interop (golden packets, proptest, labo loopback) | [09_validation_interop.md](09_validation_interop.md) | Transverse — jalons rattachés à chaque étape |

```text
1 Probing ──► 2 HELLO_REQ ──┐
3 Firewall  ────────────────┼─► 4 BootstrapHealth ──► 5 UDP ──► 7 Recherche ──► 8 Buddy
                            └─► 6 Contention (libre)      ▲
                            9 Validation (continue) ──────┘
```

---

## Contraintes transverses (rappel AGENTS.md)

- **Aucun magic number** : seuils/timeouts → `const` documentée en tête de fichier ; réglages opérationnels → `DaemonConfig` ; budgets réseau → `ResourceBudgets` ; bornes filaires → `limits.rs`.
- **Un composant par fichier** : `contact_verifier.rs`, `bootstrap_health.rs`, `buddy_client.rs` sont des fichiers dédiés.
- **Tests hors-ligne uniquement** : loopback, pas de trafic sortant.
- **`mule-protocol`** : `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, round-trip obligatoire, vérification contre `srchybrid` + `docs/ed2k/` avant tout changement filaire.
- **Sécurité** : ne jamais affaiblir anti-SSRF, isolation loopback, kill switch, pseudonymisation.
- **Code et doc en français**, `thiserror` pour les erreurs, `tracing` pour les logs, zéro warning.

## Validation par étape

```powershell
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
# + cargo test -p mule-protocol --all-features  (étapes touchant le protocole : 2, 7a, 8)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\verify_all.ps1
```
