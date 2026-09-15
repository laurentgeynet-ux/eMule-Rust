# Walkthrough — Phase 12 : Kademlia (Kad DHT) Minimal & Transport Loopback

Ce document consigne l'architecture, les spécifications protocolaires, les implémentations Rust et Flutter, ainsi que la validation complète de la **Phase 12** d'Emule-Sion (Lots 12.0a, 12.0b et 12.1).

---

## 1. Vue d'Ensemble & Périmètre Réalisé

La Phase 12 dote Emule-Sion du support complet de la table de routage distribuée Kademlia (Kad v2), strictement confiné en boucle locale (`127.0.0.1` / profil `LocalLab`), avec zéro fuite WAN non sollicitée et un kill switch instantané.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        Interface Desktop (Flutter)                     │
│  ┌───────────────────────────────────────────────────────────────────┐  │
│  │                            KadView                                │  │
│  │  - Statut UDP (Actif / Inactif), Bannière Confinement Loopback    │  │
│  │  - ID Kad Local (128 bits), Compteurs Paquets TX / RX            │  │
│  │  - Statistiques Buckets (128 buckets), Contacts Actifs & Base     │  │
│  │  - Barre d'actions responsive (Amorcer Bootstrap, Importer .dat)  │  │
│  │  - Table paginée & filtrable des contacts avec Ping de latence    │  │
│  └──────────────────────────────────┬────────────────────────────────┘  │
└─────────────────────────────────────┼───────────────────────────────────┘
                                      │ WebSocket JSON-RPC
┌─────────────────────────────────────▼───────────────────────────────────┐
│                     mule-daemon (KadService)                            │
│  ┌─────────────────────────────┐     ┌───────────────────────────────┐  │
│  │       KadUdpService         │     │         KadService            │  │
│  │ - tokio::net::UdpSocket     │     │ - KBucketTable (128 buckets)  │  │
│  │ - InFlight Transactions     │◄───►│ - SQLite: kad_contacts        │  │
│  │ - Timeout 5s, TX ID 32-bit  │     │ - Iterative Search (alpha = 3)│  │
│  │ - KillSwitch Integration    │     │ - Nodes.dat Parser v0/v1/v2   │  │
│  └──────────────┬──────────────┘     └───────────────────────────────┘  │
└─────────────────┼───────────────────────────────────────────────────────┘
                  │ Datagrammes UDP (≤ 1472 octets)
┌─────────────────▼───────────────────────────────────────────────────────┐
│                   mule-protocol (KadCodec)                              │
│  - PR_KADEMLIA (0xE4), PR_KADEMLIA_PACKED (0xE5)                        │
│  - KADEMLIA_PING (0x60), KADEMLIA_PONG (0x61)                           │
│  - KADEMLIA_HELLO_REQ (0x11), KADEMLIA_HELLO_RES (0x19)                 │
│  - KADEMLIA2_BOOTSTRAP_REQ (0x21), KADEMLIA2_BOOTSTRAP_RES (0x29)       │
│  - KADEMLIA2_REQ (0x21), KADEMLIA2_RES (0x29)                           │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Décomposition des Lots

### 2.1 Lot 12.0a — Données, Persistance, Fichier `nodes.dat` et Interface Kad (Zero Socket)
- **Modèle de Données & Routage (`mule-core`)** :
  - Identifiant Kad sur 128 bits (`KadId`) avec calcul de distance métrique XOR (`^`) et comptage de préfixe commun (`leading_zeros`).
  - `KBucketTable` : 128 buckets, capacité $K = 20$ contacts actifs par bucket + cache de 10 remplaçants (LRU et éviction contrôlée).
  - Persistance SQLite dans la table `kad_contacts` avec insertion batch, pagination, filtrage et purge.
- **Format `nodes.dat` (`mule-format`)** :
  - Décodeur/encodeur défensif pour les versions 0, 1 et 2 de `nodes.dat`.
  - Bornes strictes : taille de fichier $\le 5\text{ MiB}$, maximum 10 000 contacts, maximum 50 tags par contact.
- **Interface Utilisateur (`apps/desktop`)** :
  - Création de `KadView` et `KadStatusModel` avec affichage des métriques de table DHT.

### 2.2 Lot 12.0b — Transport UDP Loopback & Cycle de Vie des Transactions
- **Codecs Réseau (`mule-protocol`)** :
  - Magic bytes officiels : `PR_KADEMLIA` (`0xE4`) et `PR_KADEMLIA_PACKED` (`0xE5`).
  - Trames `KadPing`, `KadPong`, `KadHelloReq`, `KadHelloRes`.
  - Contrôle strict de la taille des datagrammes : rejet immédiat si $> 1472$ octets.
- **Gestionnaire UDP & Transactions (`mule-daemon`)** :
  - `KadUdpService` sur socket `UdpSocket` asynchrone lié sous `ListenPermit`.
  - Suivi des requêtes en vol avec ID de transaction aléatoire (32 bits), validation d'adresse de provenance et timeout de 5 secondes.
  - Mise à jour et vérification des contacts dans la table SQLite et les buckets lors de la réception d'un `Pong`.
  - Endpoints RPC : `kad.startUdp`, `kad.stopUdp`, `kad.pingContact`.

### 2.3 Lot 12.1 — Bootstrap Kad, Recherche Itérative & Nœuds Lab
- **Codecs Bootstrap & Find Node (`mule-protocol`)** :
  - Codecs pour `KADEMLIA2_BOOTSTRAP_REQ` (`0x21`), `KADEMLIA2_BOOTSTRAP_RES` (`0x29`), `KADEMLIA2_REQ` (`0x21`) et `KADEMLIA2_RES` (`0x29`).
  - Bornage strict : maximum 20 contacts par réponse, protection contre la troncature.
- **Moteur d'Amorçage & Recherche Itérative (`mule-daemon`)** :
  - `KadService::bootstrap_from_node` : amorçage direct via un nœud cible `IP:port` et enregistrement des contacts renvoyés.
  - `KadService::find_node` : recherche itérative avec parallélisme $\alpha = 3$, requêtage des contacts les plus proches en métrique XOR jusqu'à convergence.
  - Endpoints JSON-RPC : `kad.bootstrap` et `kad.findNode`.
- **UI & Dialogue Bootstrap (`apps/desktop`)** :
  - Dialogue interactif permettant de renseigner IP et port d'un nœud d'amorçage.
  - Barre d'outils responsive adaptée aux fenêtres compactes et larges.

---

## 3. Matrice des Endpoints RPC Kad

| Méthode RPC | Paramètres | Résultat | Description |
| :--- | :--- | :--- | :--- |
| `kad.status` | Aucun | `KadStatusModel` | Retourne l'état complet (ID, socket, compteurs, buckets) |
| `kad.startUdp` | `bindAddress` (opt) | `{ "boundAddress": "..." }` | Démarre le socket UDP (Loopback uniquement sous `LocalLab`) |
| `kad.stopUdp` | Aucun | `{ "stopped": true }` | Ferme immédiatement le socket UDP et annule les transactions |
| `kad.ping` | `ip`, `port`, `kadId` | `{ "success": bool, "latencyMs": u64 }` | Émet un datagramme `Ping` et mesure le RTT |
| `kad.bootstrap` | `ip`, `port` | `{ "status": "ok", "contactsDiscovered": u32 }` | Effectue l'amorçage initial auprès d'un nœud |
| `kad.findNode` | `targetKadId` | `[KadContactModel]` | Exécute une recherche itérative ($\alpha=3$) |
| `kad.listContacts` | `limit` (opt) | `[KadContactModel]` | Liste les contacts enregistrés en base SQLite |
| `kad.clearContacts` | Aucun | `{ "clearedCount": u32 }` | Vide la table des contacts |
| `kad.inspectNodesDat` | `filePath` | `NodesDatInspectReportModel` | Analyse défensive d'un fichier `nodes.dat` |
| `kad.importNodesDat` | `filePath` | `{ "importedCount": u32 }` | Importe les contacts d'un fichier `nodes.dat` |

---

## 4. Résultats de Validation & Intégrité

### A. Backend Rust
- `cargo test --workspace` : **100 % des tests unitaires, d'intégration et d'isolation réussis** sur l'ensemble des 6 crates du workspace.
- Zéro fuite de socket en dehors du loopback local `127.0.0.1`.

### B. Client Flutter
- `flutter analyze` : **0 avertissement, 0 erreur**.
- `flutter test` : **92/92 tests au vert**.
