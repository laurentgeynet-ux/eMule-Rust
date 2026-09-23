# Répertoire Maître des Guides & Walkthroughs — EMule-Rust

Ce document constitue la table d'orientation générale et l'index chronologique exhaustif de l'ensemble des guides d'exécution, d'interopérabilité et de validation technique (**walkthroughs**) du projet **EMule-Rust**, de la Phase 1 à la Phase 15 (`v0.1.8`).

---

## 1. Vue d'Ensemble Synoptique des Phases & Documents

| Phase / Jalon | Version | Document de Walkthrough | Thématiques Clés | Statut de Validation |
| :--- | :--- | :--- | :--- | :--- |
| **Phases 1 à 3C** | `v0.1.0-lab` | [walkthrough_Phase_01_a_03c.md](walkthrough_Phase_01_a_03c.md) | Fondations Rust (`mule-format`, `mule-core`), démon Tokio, UI Flutter initiale, durcissement produit, verrous `fs4`, tokens CSPRNG, packaging Windows. | ✅ Certifié & Figé |
| **Tier 3.1 à 3.6** | `v0.1.3-lab` | [walkthrough_Tier3_eMule050a.md](walkthrough_Tier3_eMule050a.md) | **Interopérabilité eMule 0.50a Officiel** : handshake filaire, bitfields, streaming de blocs 9.28 Mo, résidu 1 octet, reprise crash et promotion atomique. | ✅ Certifié (100% Loopback) |
| **Phase 7.2** | `v0.1.2-lab` | [walkthrough_Phase_07.md](walkthrough_Phase_07.md) | Mode portable hybride, séparation étanche `data/` vs `downloads/`, persistance des suppressions dans SQLite, pipeline QA E2E automatisé. | ✅ Certifié & Scellé |
| **Phase 8** (8.1 à 8.5) | `v0.1.3-lab` | [walkthrough_Phase_08.md](walkthrough_Phase_08.md) | Part Map bloc par bloc, historique local & stats, catégories hiérarchiques à 4 niveaux, import défensif `.part.met`, packaging double saveur standard/qa-loopback. | ✅ Certifié & Scellé |
| **Phase 9** (9.0 & 9.1) | `v0.1.3-lab` | [walkthrough_Phase_09.md](walkthrough_Phase_09.md) | Politique réseau défensive en 7 étapes, permis RAII, Kill Switch atomique, anti-SSRF, filtrage IP dichotomique (`ipfilter.dat`), Token Bucket, pseudonymisation. | ✅ Validé (Crates dédiés) |
| **Phase 10** (10.0 à 10.2) | `v0.1.3-lab` | [walkthrough_Phase_10.md](walkthrough_Phase_10.md) | Décodage binaire `server.met` (`0xE0`), session TCP serveur eD2K réelle (Lugdunum), HighID/LowID, recherche sur catalogue serveur et résolution de sources. | ✅ Validé & Intégré UI |
| **Phase 11** | `v0.1.4` / `v0.1.5` | [walkthrough_Phase_11.md](walkthrough_Phase_11.md) | Swarm multi-sources autonome, allocateur dynamique **Weighted Max-Min Fair-Share Water-filling**, démarrage basse latence (30s/60s/300s), tags fixes eD2K `0x11..0x7F`. | ✅ Validé & Mergé |
| **Phase 12** | `v0.1.4` | [walkthrough_Phase_12.md](walkthrough_Phase_12.md) | Fondations Kademlia DHT : structure `KadId` 128-bit, table de 128 K-Buckets avec cache LRU, transport UDP asynchrone, amorçage bootstrap et écran Kad UI. | ✅ Validé & Scellé |
| **Phase 13** | `v0.1.6` | [walkthrough_Phase_13.md](walkthrough_Phase_13.md) | **Source Exchange v4 (SX1 & SX2)** décentralisé, transfert de blocs compressés zlib (`OP_COMPRESSEDPART`), et serveur Agent IA natif **MCP (83 outils)**. | ✅ Validé & Mergé |
| **Phase 14** | `v0.1.7` | [walkthrough_Phase_14.md](walkthrough_Phase_14.md) | **Convergence de la recherche Kad DHT** : métrique DWORDs `CUInt128`, correction opcode universel `0x3B`, élimination du stall decay, et **Clean Architecture** complète. | ✅ Validé & Mergé |
| **Phase 15** | `v0.1.8` | [walkthrough_Phase_15.md](walkthrough_Phase_15.md) | **Téléchargement multi-pairs réel** : agrégation de vitesse en direct (`sync_transfer_rates`), respect des files eMule (`Disconnected in queue` cooldown 10-15m), upload queue, logs stratégiques, `INDEX.md`. | ✅ Validé en Conditions Réelles |

---

## 2. Guide Thématique de Navigation

### A. Protocoles Réseau & Filaire
* Pour comprendre le protocole eD2K officiel et la qualification face au binaire eMule 0.50a :  
  👉 Consulter [walkthrough_Tier3_eMule050a.md](walkthrough_Tier3_eMule050a.md).
* Pour l'échange de sources décentralisé (SX1/SX2) et les blocs compressés zlib :  
  👉 Consulter [walkthrough_Phase_13.md](walkthrough_Phase_13.md).
* Pour l'infrastructure client-serveur eD2K (Lugdunum, login, HighID/LowID, recherche) :  
  👉 Consulter [walkthrough_Phase_10.md](walkthrough_Phase_10.md).
* Pour l'implémentation complète de Kademlia (Kad DHT) :  
  👉 Consulter [walkthrough_Phase_12.md](walkthrough_Phase_12.md) (Fondations) et [walkthrough_Phase_14.md](walkthrough_Phase_14.md) (Convergence & Opcode 0x3B).

### B. Moteur de Téléchargement & Algorithmes d'Essaim
* Pour le fonctionnement du stockage éparse (`PartFile`), des intervalles (`RangeSet`) et de la Part Map :  
  👉 Consulter [walkthrough_Phase_08.md](walkthrough_Phase_08.md).
* Pour l'allocateur de swarm et la répartition équitable Water-Filling :  
  👉 Consulter [walkthrough_Phase_11.md](walkthrough_Phase_11.md).
* Pour l'agrégation de vitesse en direct et la gestion des files d'attente distantes eMule :  
  👉 Consulter [walkthrough_Phase_15.md](walkthrough_Phase_15.md).

### C. Sécurité, Confinement & Protection des Données (DLP)
* Pour la politique réseau en 7 couches, l'anti-SSRF, le Kill Switch et le filtrage IP :  
  👉 Consulter [walkthrough_Phase_09.md](walkthrough_Phase_09.md).
* Pour la séparation étanche des répertoires de données (`data/`) et des fichiers promus (`downloads/`) :  
  👉 Consulter [walkthrough_Phase_07.md](walkthrough_Phase_07.md).

### D. Architecture Logicielle & Outils Agents IA
* Pour la refactorisation Clean Architecture et la modularisation de tous les crates :  
  👉 Consulter [walkthrough_Phase_14.md](walkthrough_Phase_14.md).
* Pour la boîte à outils Agent IA Model Context Protocol (MCP) :  
  👉 Consulter [walkthrough_Phase_13.md](walkthrough_Phase_13.md).
* Pour l'inventaire cartographié de chaque fichier du code source :  
  👉 Consulter [../INDEX.md](../INDEX.md).

---

## 3. Commandes Globales de Qualification (100% Reproductibles Hors-Ligne)

```powershell
# 1. Compilation et analyse statique stricte sans avertissement
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 2. Exécution intégrale de la suite de tests Rust (204+ tests unitaires et d'intégration)
cargo test --workspace --all-features

# 3. Analyse statique et validation de la suite de tests Flutter Desktop
cd ../../apps/desktop
flutter analyze
flutter test
```
