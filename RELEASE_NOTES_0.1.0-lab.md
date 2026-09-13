# Notes de Release — Emule-Sion 0.1.0-lab

## 1. Vue d'Ensemble

La release **0.1.0-lab** constitue le premier jalon stable et figé du projet **Emule-Sion**. Elle formalise et verrouille l'ensemble des travaux de décodage filaire eD2K, le moteur de stockage éparse, la supervision locale et les mécanismes de validation d'interopérabilité sans dépendre d'aucun réseau externe.

---

## 2. Attestation d'Interopérabilité Honnête

L'état d'attestation du projet repose sur une distinction stricte et formelle en trois paliers :

| Palier | Intitulé | Statut | Ce qui est prouvé | Ce qui n'est pas affirmé |
| :--- | :--- | :--- | :--- | :--- |
| **Tier 1** | Corpus Statique | **Validé (100%)** | Conformité syntaxique des décodeurs/encodeurs sur des fixtures d'octets figées et réelles. | Pas d'interaction dynamique avec un processus en cours d'exécution. |
| **Tier 2** | Pair Émulé | **Validé (100%)** | Conformité d'échange (handshake, statut, parts, sentinelles, résidus) avec un modèle simulant les variantes officielles en loopback. | Pas d'interopérabilité observée avec un binaire officiel compilé. |
| **Tier 3** | Binaire Externe Épinglé | **Harnais Prêt** | L'harnais d'orchestration hermétique, de neutralisation de config et de confinement loopback est audité. | Aucun test live n'est exécuté sans un binaire externe vérifié (test ignoré par défaut). |

> **Formulation d'attestation obligatoire :**
> *« Compatibilité eD2K expérimentale validée sur corpus statique et pair de référence émulé, en environnement loopback fermé. Le harnais Tier 3 pour un binaire eMule/aMule externe épinglé est disponible, mais son test live reste ignoré par défaut tant qu’un binaire vérifié n’est pas fourni. »*

---

## 3. Éléments Volontairement Absents

Pour préserver l'intégrité de l'environnement de recherche et la sécurité absolue de l'hôte, les composants suivants sont **strictement et volontairement absents** de cette version :
1. **Zéro connectivité réseau publique** : toutes les sockets d'écoute et de connexion sont confinées inconditionnellement à l'interface `127.0.0.1`. Toute tentative de liaison vers une adresse non-loopback déclenche une erreur immédiate.
2. **Zéro routage Kad / Kademlia / DHT** : aucun échange DHT n'est implémenté ni émis.
3. **Zéro serveur d'indexation eD2K** : aucun protocole de synchronisation avec des serveurs eD2K publics.
4. **Zéro upload / seeding** : aucune routine d'envoi ou de partage spontané de blocs vers d'autres pairs.
5. **Zéro fuite d'identité hôte** : les traces normalisées JSON et rapports CI sont audités contre 13 vecteurs de fuite (chemins Windows/Unix, tokens d'authentification, bidi overrides, caractères invisibles).

---

## 4. Dépendances Critiques & Versions Épinglées

Les composants du workspace sont compilés et verrouillés avec les versions exactes suivantes (`Cargo.lock`) :

| Dépendance | Version épinglée | Utilisation |
| :--- | :--- | :--- |
| `tokio` | `1.53.1` | Runtime asynchrone multi-thread pour le démon et les tests loopback |
| `axum` | `0.8.9` | Serveur WebSocket local pour l'API JSON-RPC du démon |
| `rusqlite` | `0.33.0` | Persistance locale SQLite (mode WAL, intégrité vérifiée) |
| `md4` | `0.10.2` | Calcul des hashsets et hashes de fichiers eD2K historiques |
| `sha1` | `0.10.7` | Calcul des arbres AICH de vérification d'intégrité de blocs |
| `fs4` | `1.1.0` | Verrouillage exclusif inter-processus (`mule.lock`) |
| `bytes` | `1.12.1` | Tampons mémoire haute performance pour le framing filaire |
| `unicode-normalization` | `0.1.25` | Normalisation Unicode canonique (Form C / NFC) |
| `serde` / `serde_json` | `1.0.229` / `1.0.139` | Sérialisation et traces normalisées JSON machine-readable |
| `rand` | `0.9.5` | Génération CSPRNG du token secret d'authentification (`auth.token`) |
| `uuid` | `1.26.1` | Identifiants uniques des téléchargements |
| `tempfile` | `3.27.0` | Isolation hermétique des tests et écriture sécurisée d'artefacts CI |

---

## 5. Commandes Complètes de Validation (100% Offline)

Toutes les commandes ci-dessous doivent s'exécuter avec un taux de réussite de 100 % sans accès à Internet :

```powershell
# 1. Vérification de la compilation du workspace
cargo check --workspace --all-targets --all-features

# 2. Vérification statique du code (0 avertissement toléré)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Exécution des 204 tests unitaires, d'intégration et adversariaux Rust
cargo test --workspace --all-features

# 4. Analyse statique du code Flutter Desktop
cd apps/desktop
flutter analyze

# 5. Exécution des 31 tests Flutter Desktop
flutter test
```

---

## 6. Scoreboard Final des Validations

```
=== SCOREBOARD EMULE-SION 0.1.0-LAB ===

Rust Crates :
  - mule-core     :  12 tests réussis
  - mule-daemon   :  13 tests réussis
  - mule-format   :  19 tests réussis
  - mule-protocol : 160 tests réussis (+ 1 test Tier 3 live ignoré par défaut)
Total Rust        : 204 tests réussis, 0 échec

Flutter Desktop   :  31 tests réussis, 0 échec

TOTAL DU PROJET   : 235 tests actifs validés à 100 %.
GARDE STATIQUE    : 0 unwrap/expect dans mule-protocol::ed2k.
LINTING           : cargo clippy sans avertissement (-D warnings).
ISOLATION         : 100 % loopback 127.0.0.1 strict.
ANONYMISATION     : 100 % validée mécaniquement (Unicode NFC + bidi/ctrl échappés).
```

---

## 7. Livrables de la Release Candidate (Industrialisation Locale)

Le bundle autonome Windows x64 est généré par `scripts/build_windows_release.ps1` :
- **Exécutable Principal** : `EmuleSion.exe` (binaire UI Flutter Desktop Windows autonome).
- **Démon d'Arrière-Plan** : `bin/mule-daemon.exe` (moteur local supervisé).
- **Manifeste de Traçabilité** : `version.json` (commit Git exact, horodatage UTC, versions des toolchains, `gitDirty: false`).
- **Sommes de Contrôle** : `SHA256SUMS.txt` pour les fichiers du bundle.
- **Inventaire SBOM CycloneDX 1.5** :
  - `sbom/rust.cdx.json` : 191 composants Rust.
  - `sbom/flutter.cdx.json` : 33 composants Flutter.
  - `sbom/windows-bundle.cdx.json` : composants binaires Windows assemblés.
  - `sbom/sbom.cdx.json` : 239 composants consolidés.
  - `sbom/license-audit.md` : rapport de conformité confirmant 100% de compatibilité avec `deny.toml`.
- **Archive Scellée** : `EmuleSion-v0.1.0-lab-windows-x64.zip` (16 386 637 octets) et son empreinte cryptographique `EmuleSion-v0.1.0-lab-windows-x64.zip.sha256` :
  `09023bb37bfb45d5a028ada95f5681a4e42df451badd04ca4b9d4ecc99836245`
- **Validation QA** : Protocole détaillé dans [docs/QA_RECEPTION_PROTOCOL.md](docs/QA_RECEPTION_PROTOCOL.md) et scripts de test automatisés (`scripts/test_packaged_bundle.ps1`, `scripts/test_rc_matrix.ps1`).
