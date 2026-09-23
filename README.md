# EMule-Rust 🚀

[![License: GPL-2.0-or-later](https://img.shields.io/badge/License-GPL--2.0--or--later-blue.svg)](LICENSE)
[![CI](https://github.com/VOTRE-USER/emule-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/VOTRE-USER/emule-rust/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/VOTRE-USER/emule-rust)](https://github.com/VOTRE-USER/emule-rust/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20x64-lightgrey)](README.md)

**EMule-Rust** est une réimplémentation moderne, hautement sécurisée et performante des protocoles pair-à-pair historiques **eD2K (eDonkey2000)** et **Kad DHT (Kademlia)**, associée à un serveur natif **Model Context Protocol (MCP)** pour agents d'intelligence artificielle.

Le projet est conçu en **Rust** (moteur de stockage éparse, démon Tokio asynchrone, codecs filaires bornés, allocateur de swarm dynamique) et en **Flutter Desktop** (interface Windows native réactive en Clean Architecture MVVM).

---

## ⚡ Caractéristiques Majeures

* **Moteur Rust Moderne & Sûr** : 100 % mémoire sûre (*memory safe*), zéro allocation sauvage, décompression continue zlib (`OP_COMPRESSEDPART`).
* **Swarm Multi-Sources Autonome** : Algorithme **Weighted Max-Min Fair-Share** (water-filling) distribuant les slots de connexion selon la rareté et les priorités des fichiers.
* **DHT Kademlia v2 Décentralisée** : Métrique de distance numérique 128-bit conforme à `CUInt128` d'eMule, K-Buckets avec cache LRU, recherche itérative avec convergence vers `SEARCHTOLERANCE`.
* **Serveur Model Context Protocol (MCP)** : **87 outils MCP natifs** permettant à des assistants IA (Claude Desktop, Cursor, Antigravity) de piloter intégralement le client.
* **Sécurité Défensive & Anti-SSRF** : Pipeline de contrôle réseau en 7 étapes, isolation loopback par défaut, Kill Switch atomique, filtrage IP rapide (`ipfilter.dat`) et pseudonymisation salée des pairs.
* **Fidélité Protocolaire eMule** : Interopérabilité validée face aux sources officielles **eMule 0.72a** (`srchybrid`), Source Exchange v4 (SX1/SX2), détection fine des files d'attente distantes (*Queue Ranking*).

---

## 📚 Documentation Complète

L'ensemble de la documentation technique, des guides d'exécution par phase et des spécifications filaires est centralisé dans le répertoire [`docs/`](docs/) :

| Section | Description |
| :--- | :--- |
| 📖 **[Portail de Documentation](docs/README.md)** | Point d'entrée principal et plan d'orientation complet. |
| 🧭 **[Walkthroughs par Phase](docs/walkthroughs/README.md)** | Guides d'exécution, d'interopérabilité et de validation technique (Phases 1 à 15). |
| 🏗️ **[Rapport d'Architecture](docs/architecture/rapport_architecture_et_avancement.md)** | État d'avancement technique et architecture modulaire détaillée. |
| 🗺️ **[Cartographie des Fichiers](docs/INDEX.md)** | Inventaire exhaustif et rôle de chaque fichier du dépôt. |
| 📝 **[Journal des Modifications](docs/CHANGELOG.md)** | Historique chronologique des versions et correctifs. |
| 📦 **[Notes de Version](docs/releases/)** | Notes détaillées de chaque version (`v0.1.0-lab` à `v0.2.0-RC1`). |
| 🧪 **[Assurance Qualité & Tests](docs/qa/)** | Protocole de réception QA, matrice de compatibilité et vecteurs de test. |
| 🔌 **[Spécifications eD2K & ADRs](docs/ed2k/)** | Cadrage filaire, catalogue d'opcodes et décisions de conception. |

---

## 🚀 Démarrage Rapide & Validation

### Prérequis
- Windows 10/11 (x64)
- Rust 1.80+ (stable)
- Flutter 3.24+ (support bureau Windows)
- Visual Studio C++ Build Tools 2022

### Commandes de Validation (100% Reproductibles Hors-Ligne)
```powershell
# Compilation et linter Rust
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Suite intégrale des 204+ tests unitaires et d'intégration Rust
cargo test --workspace --all-features

# Analyse statique et tests de l'interface Flutter Desktop
cd apps/desktop
flutter analyze
flutter test
```

### Packaging Windows Autonome
```powershell
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1
```
Le bundle autonome portable est généré dans `dist/eMuleRust-Windows/`.

---

## 🙏 Crédits & Attributions

EMule-Rust est une réimplémentation indépendante dont la fidélité
protocolaire s'appuie sur l'étude des sources open source **eMule**
(© 2002-2026 Merkur / eMule Dev Team) et **aMule** (© aMule Team, Angel
Vidal, Barry Dunne), toutes deux sous GPL-2.0+. Voir le fichier
[`NOTICE`](NOTICE) pour l'attribution complète et
[`docs/ed2k/references.md`](docs/ed2k/references.md) pour la
correspondance fichier-par-fichier.

## 📄 Licence

Distribué sous **GPL-2.0-or-later** — voir [`LICENSE`](LICENSE).
