# Guide d'Exécution & Validation — Phase 11 : Swarm Multi-Sources, Allocateur Équitable & Démarrage Basse Latence

Ce document constitue le guide technique de référence pour la **Phase 11** (versions `v0.1.4` et `v0.1.5`) du projet **eMuleRust**, dédiée à l'orchestration dynamique d'essaims de pairs et à l'accélération du démarrage des téléchargements :
* **SwarmCoordinator Autonome** : Gestion d'essaims multi-sources, auto-découverte cadencée hybride (Serveurs eD2K + Kad DHT) et supervision du cycle de vie des pairs.
* **Allocateur Dynamique Water-Filling** : Algorithme **Weighted Max-Min Fair-Share** pour la répartition optimale et équitable des connexions réseau globales entre fichiers concurrents.
* **Optimisation de la Latence de Démarrage** : Détection accélérée des sources (30s / 60s / 300s), tolérance douce aux timeouts de statut (`Tried` avec réessai à 45s) et réinterrogation rapide de la file d'attente (3 min).
* **Fidélité Wire & Rétrocompatibilité** : Support des tags eD2K de type fixe `0x11..0x7F` (`TAGTYPE_STR1..n`), décodage Kad legacy `0x00` et parallélisation du bootstrap.

---

## 1. Architecture du Swarm Multi-Sources

```mermaid
flowchart TB
    subgraph Engine ["Démon eMuleRust (Tokio Runtime)"]
        SC["SwarmCoordinator (Boucle autonome 3s)"]
        SA["SwarmAllocator (Weighted Max-Min Fair-Share Water-filling)"]
        PM["PeerSessionManager (Connexions TCP Parallèles)"]
        KadSvc["KadService (Découverte DHT)"]
        ServerSvc["ServerSessionManager (Découverte Serveur)"]
    end

    subgraph Storage ["Persistance & Données"]
        DB[(SQLite WAL : downloads, peer_sources)]
        PartFiles["data/temp/*.part (Sparse Files)"]
    end

    subgraph External ["Pairs Distants (Réseau eD2K & Kad)"]
        PeerA["Pair A (eMule 0.50a - HighID)"]
        PeerB["Pair B (aMule 2.3.3)"]
        PeerC["Pair C (eMule Community)"]
    end

    SC -->|1. Évalue la demande par fichier| SA
    SA -->|2. Alloue les quotas de slots| SC
    SC -->|3. Sélectionne les sources viables| DB
    SC -->|4. Lance les sessions nécessaires| PM
    PM <==>|TCP eD2K / Obfuscation RC4| PeerA & PeerB & PeerC
    PM -->|5. Écrit les blocs reçus (180 Ko)| PartFiles

    SC -.->|Auto-découverte cadencée 30s/60s/300s| KadSvc & ServerSvc
    KadSvc & ServerSvc -.->|Nouvelles sources découvertes| DB
```

---

## 2. Algorithme d'Allocation : Weighted Max-Min Fair-Share Water-Filling

Le module [`crates/mule-daemon/src/services/swarm_allocator.rs`](file:///d:/Projet/eMuleRust/crates/mule-daemon/src/services/swarm_allocator.rs) résout le problème classique de contention des connexions réseau globales lorsque plusieurs téléchargements tournent simultanément.

### 2.1 Modèle de Demande (`FileDemand`)
Chaque téléchargement actif déclare sa demande :
* **Pondération par Priorité (`priority_weight`)** :
  * `Low` : poids 1
  * `Normal` : poids 2
  * `High` : poids 4
  * `Release` : poids 8
* **Plafond de viabilité (`effective_cap`)** :  
  $$\text{effective\_cap} = \min(\text{max\_peers\_per\_file},\ \text{viable\_sources\_count},\ \text{remaining\_parts} \times 3)$$
  * Si un fichier n'a plus que 2 parties de 9,28 Mo à terminer, il ne monopolise pas inutilement 40 slots : son allocation est plafonnée à $2 \times 3 = 6$ pairs (protection *endgame anti-hoarding*).

### 2.2 Procédure de Remplissage (Water-Filling)
1. **Attribution du plancher garanti** : Chaque fichier disposant de sources viables reçoit immédiatement un plancher de $\min(\text{min\_peers\_per\_file}, \text{effective\_cap})$.
2. **Distribution itérative de l'excédent** : Le quota global résiduel ($\text{budget\_restant} = \text{max\_global\_peers} - \sum \text{planchers}$) est distribué de manière continue et proportionnelle au poids de priorité des fichiers non encore saturés.
3. Dès qu'un fichier atteint son `effective_cap`, il est gelé et le surplus d'eau (slots) se déverse sur les autres téléchargements nécessiteux.

---

## 3. Accélération du Démarrage & Gestion Résiliente des Timeouts

### 3.1 Découverte Dynamique des Sources
Auparavant, le coordinateur interrogeait les sources toutes les 5 minutes fixes. Sur un fichier fraîchement ajouté avec 0 source, le téléchargement pouvait rester inactif plusieurs minutes. Le nouvel échéanceur dynamique adapte sa fréquence :
* **0 source connue** : réinterrogation immédiate toutes les **30 secondes**.
* **1 à 14 sources** : réinterrogation toutes les **60 secondes**.
* **15+ sources** (essaim bien nourri) : réinterrogation de croisière toutes les **300 secondes** (5 minutes).

### 3.2 Gestion Douce des Timeouts de Statut
Lorsqu'un pair distant ne répond pas immédiatement à la demande de disponibilité de parties (`OP_FILESTATUSREQ`) :
* Il n'est plus banni agressivement avec pénalité d'une heure.
* Il est rétrogradé dans l'état souple `Tried` avec un délai de réessai court de **45 secondes**.

### 3.3 Traitement Direct de la File d'Attente eMule
Lorsqu'un client distant reçoit notre demande mais que ses slots d'upload sont pleins, il répond immédiatement par `OP_QUEUERANKING` (notre rang dans sa file) ou `OP_ACCEPTUPLOADREQ`.
Ces paquets sont désormais interceptés directement dès la phase de handshake, éliminant les faux timeouts de 20 secondes qui conduisaient à des déconnexions intempestives.

---

## 4. Fidélité Wire & Rétrocompatibilité

### 4.1 Décodage des Chaînes Fixes `TAGTYPE_STR1..n` ([tags.rs](file:///d:/Projet/eMuleRust/crates/mule-protocol/src/ed2k/tags.rs))
* **Anomalie historique résolue** : Les serveurs eD2K Lugdunum émettent des paquets `OP_SERVERIDENT` contenant des tags de type `0x11` à `0x7F`. Ces types représentent des chaînes de taille fixe sans préfixe de longueur `u16`.
* **Règle wire** :  
  $$\text{Taille de la chaîne} = \text{TagType} - 0x10$$
  * Exemple : `0x19` correspond à une chaîne fixe de $0x19 - 0x10 = 9$ octets.

### 4.2 Amorçage Kad Legacy ([codec.rs](file:///d:/Projet/eMuleRust/crates/mule-protocol/src/kad/codec.rs))
* Support du paquet historique `KADEMLIA_BOOTSTRAP_REQ_OLD` (`0x00`) assurant la compatibilité avec les clients eMule ancestraux.
* Parallélisation du bootstrap (`bootstrap_active_contacts`) : interrogation concurrente via `futures_util::future::join_all` ramenant le temps d'amorçage de 50 secondes à **moins de 5 secondes**.

---

## 5. Commandes de Validation (100% Hors-Ligne)

```powershell
# 1. Tests unitaires de l'allocateur de swarm et de l'algorithme de water-filling
cargo test -p mule-daemon --lib services::swarm_allocator

# 2. Tests de découverte de sources et d'obfuscation eD2K
cargo test -p mule-daemon --test ed2k_source_discovery --test ed2k_obfuscation_tests

# 3. Tests de décodage des tags fixes et codecs Kad legacy
cargo test -p mule-protocol --test server_codec_tests --test kad_codec_tests

# 4. Tests d'intégration du coordinateur et des sessions de pairs
cargo test -p mule-daemon --test peer_connection_tests --test swarm_dual_peer_tests
```
