# Portail de Documentation — Emule-Sion 🚀

Bienvenue sur la documentation technique d'**Emule-Sion**, une réimplémentation moderne, modulaire et hautement sécurisée des protocoles **eD2K (eDonkey2000)** et **Kad DHT (Kademlia)**, associée à un serveur d'outils **Model Context Protocol (MCP)** pour agents IA.

Le projet est architecturé en **Rust** (moteur de téléchargement, codec filaire, démon asynchrone Tokio et serveur MCP) et en **Flutter Desktop** (interface graphique Windows en Clean Architecture MVVM).

---

## 🧭 Plan d'Orientation de la Documentation

Toute la documentation du projet est classée et organisée au sein de ce répertoire `docs/` :

```
docs/
├── README.md                      # Ce portail d'accueil général
├── INDEX.md                       # Inventaire cartographié de TOUS les fichiers du dépôt
├── CHANGELOG.md                   # Journal des modifications (Keep a Changelog)
│
├── architecture/                  # Architecture système & rapports techniques
│   └── rapport_architecture_et_avancement.md
│
├── walkthroughs/                  # Guides d'exécution & d'interopérabilité (Phases 1 à 15)
│   ├── README.md                  # Index maître des walkthroughs
│   ├── walkthrough_Phase_01_a_03c.md
│   ├── walkthrough_Tier3_eMule050a.md
│   ├── walkthrough_Phase_07.md
│   ├── walkthrough_Phase_08.md
│   ├── walkthrough_Phase_09.md
│   ├── walkthrough_Phase_10.md
│   ├── walkthrough_Phase_11.md
│   ├── walkthrough_Phase_12.md
│   ├── walkthrough_Phase_13.md
│   ├── walkthrough_Phase_14.md
│   └── walkthrough_Phase_15.md
│
├── releases/                      # Notes de version détaillées
│   ├── RELEASE_NOTES_0.1.0-lab.md
│   ├── RELEASE_NOTES_0.1.1-lab.md
│   ├── RELEASE_NOTES_0.1.2-lab.md
│   ├── RELEASE_NOTES_0.1.3-lab.md
│   └── RELEASE_NOTES_0.1.8.md
│
├── qa/                            # Qualification, protocoles de test & conformité
│   ├── QA_RECEPTION_PROTOCOL.md
│   ├── compatibility-matrix.md
│   └── test-vectors.md
│
└── ed2k/                          # Spécifications filaires eD2K & Décisions d'Architecture (ADR)
    ├── compatibility-notes.md
    ├── framing.md
    ├── limits.md
    ├── message-catalog.md
    ├── opcodes.md
    ├── opcodes_matrix.md
    ├── references.md
    ├── scope.md
    ├── decisions/
    └── messages/
```

---

## 📑 Accès Rapide par Domaine

### 1. Architecture Système & Démon
* **[Rapport Global d'Architecture & d'Avancement](architecture/rapport_architecture_et_avancement.md)** : Vue d'ensemble détaillée des 7 crates Rust, du superviseur Flutter et de la Clean Architecture.
* **[Protocole RPC du daemon](architecture/rpc_protocol.md)** : Contrat JSON-RPC 2.0/WebSocket — handshake, méthodes, events, codes d'erreur, versioning.
* **[Plan UI multiplateforme](architecture/plan_ui_multiplateforme.md)** : Roadmap desktop/mobile/web en 10 phases (packages partagés, Riverpod, i18n).
* **[Cartographie Globale des Fichiers](INDEX.md)** : Répertoire exhaustif de chaque fichier du dépôt.
* **[Journal des Changements](CHANGELOG.md)** : Historique des versions et évolutions.

### 2. Guides Techniques par Phase (Walkthroughs)
* **[Répertoire Maître des Walkthroughs](walkthroughs/README.md)**
  * [Phase 1 à 3C](walkthroughs/walkthrough_Phase_01_a_03c.md) : Fondations Rust, stockage éparse, démon local, verrous `fs4` et packaging.
  * [Tier 3 eMule 0.50a](walkthroughs/walkthrough_Tier3_eMule050a.md) : Qualification face au binaire officiel eMule 0.50a en loopback.
  * [Phase 7 (v0.1.2-lab)](walkthroughs/walkthrough_Phase_07.md) : Mode portable, étanchéité `data/` vs `downloads/` et persistance SQLite.
  * [Phase 8](walkthroughs/walkthrough_Phase_08.md) : Part Map bloc par bloc, historique & statistiques, catégories, import défensif `.part.met`.
  * [Phase 9](walkthroughs/walkthrough_Phase_09.md) : Politique réseau défensive en 7 couches, permis RAII, Kill Switch et filtre IP `ipfilter.dat`.
  * [Phase 10](walkthroughs/walkthrough_Phase_10.md) : Infrastructure serveurs eD2K (Lugdunum TCP, HighID/LowID, recherche et sources).
  * [Phase 11](walkthroughs/walkthrough_Phase_11.md) : Swarm multi-sources, allocateur Water-Filling et démarrage basse latence.
  * [Phase 12](walkthroughs/walkthrough_Phase_12.md) : Fondations Kademlia DHT (table K-Buckets LRU, transport UDP, bootstrap local).
  * [Phase 13](walkthroughs/walkthrough_Phase_13.md) : Source Exchange v4 (SX1/SX2), blocs compressés zlib et serveur MCP (83 outils IA).
  * [Phase 14](walkthroughs/walkthrough_Phase_14.md) : Convergence recherche Kad DHT (CUInt128, opcode universel 0x3B) et Clean Architecture.
  * [Phase 15](walkthroughs/walkthrough_Phase_15.md) : Swarm multi-pairs réel (`sync_transfer_rates`), respect des files d'attente eMule et logs stratégiques.

### 3. Assurance Qualité & Spécifications Filaire
* **[Protocole de Réception QA](qa/QA_RECEPTION_PROTOCOL.md)** : Guide de test pour la qualification d'une release.
* **[Matrice de Compatibilité Protocolaires](qa/compatibility-matrix.md)** : Synthèse des profils `Emule050a`, `Amule233`, etc.
* **[Spécifications eD2K & ADRs](ed2k/compatibility-notes.md)** : Décisions de conception et encodage filaire.

---

## 🛠️ Commandes de Vérification Globale (100% Hors-Ligne)

```powershell
# 1. Compilation et vérification du workspace complet
cargo check --workspace --all-targets --all-features

# 2. Linter Rust strict sans avertissement
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Suite intégrale des 204+ tests Rust
cargo test --workspace --all-features

# 4. Analyse statique et tests Flutter Desktop
cd ../apps/desktop
flutter analyze
flutter test
```
