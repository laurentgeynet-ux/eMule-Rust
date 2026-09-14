# Journal des Modifications — Emule-Sion

Toutes les modifications notables de ce projet sont documentées dans ce fichier.
Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et ce projet adhère à [Semantic Versioning](https://semver.org/lang/fr/).

---

## [0.1.1-lab] - 2026-09-14

Release Candidate consolidée pour le mode portable (Phase 7.1) et les outils de laboratoire durcis (Phase 7.2).

### Ajouté
- **Mode Portable Hybride & Cascade Déterministe** : Résolution unifiée `--workdir` > `EMULE_SION_WORKDIR` > `portable.flag` (`data/`) > `%LOCALAPPDATA%\EmuleSion` (avec alias déprécié `MULE_DATA_DIR`).
- **Double Verrouillage du Mode Lab** : Les fonctionnalités de laboratoire (`LabGuideDialog`, injection d'eD2K synthétique, nettoyage sécurisé) requièrent à la fois le mode portable et la feature compilée `lab-tools`.
- **Taxonomie JSON-RPC 2.0 Étendue** : Formalisation des codes `-32000` (AuthFailed), `-32001` (ProtocolMismatch), `-32002` (ForbiddenInInstalledMode), `-32003` (UnsafeCleanupTarget), `-32004` (FixtureDownloadMismatch), `-32005` (LoopbackPeerUnavailable), `-32010` (BusinessError).
- **Garantie Zéro Secret dans les Diagnostics** : Redaction systématique des jetons d'authentification (`auth.token`) et des clés privées dans `DiagnosticDialog`.
- **Politique PowerShell Durcie** : Transition stricte vers `-ExecutionPolicy RemoteSigned` (éradication de `-ExecutionPolicy Bypass`).

### Modifié
- **Épuration des Capacités Standard** : Suppression de `local-simulation` du build par défaut (`default = []`). Seules `["downloads", "download-events"]` sont annoncées par défaut.
- **Transparence de l'Inventaire du Bundle** : Transition de 22 à 21 fichiers dans `SHA256SUMS.txt` suite à l'adoption exclusive par Flutter Desktop de la version binaire `AssetManifest.bin` (remplaçant le format JSON redondant).

---

## [0.1.0-lab] - 2026-09-13

Première release interne de laboratoire pour **Emule-Sion** (client moderne eD2K offline/loopback).
Cette version fige l'état de validation d'interopérabilité locale, le pipeline de stockage, le protocole borné et la suite d'outils de test.

### Résumé des Niveaux d'Attestation
- **Tier 1 (Corpus statique)** : **100 % Validé** — Décodage/encodage canonique et déterministe sur fixtures d'octets figées (`ed2k_corpus_tests.rs`).
- **Tier 2 (Pair de référence émulé)** : **100 % Validé** — Handshake nominal, négociation de capacités, requêtes/réponses de statut et transfert de blocs complets et résiduels contre un pair de référence local simulé (`ed2k_reference_interop_tests.rs`).
- **Tier 3 (Binaire externe épinglé)** : **Harnais disponible & audité** — Harnais d'orchestration hermétique prêt (`tier3_loopback.rs`), test live ignoré par défaut tant qu'un binaire eMule/aMule vérifié et épinglé n'est pas fourni.

> **Formulation d'attestation d'interopérabilité :**
> *« Compatibilité eD2K expérimentale validée sur corpus statique et pair de référence émulé, en environnement loopback fermé. Le harnais Tier 3 pour un binaire eMule/aMule externe épinglé est disponible, mais son test live reste ignoré par défaut tant qu’un binaire vérifié n’est pas fourni. »*

### Éléments Volontairement Absents
- **Zéro connexion réseau publique** : toutes les sockets sont rigoureusement confinées à `127.0.0.1` (loopback strict).
- **Zéro protocole Kad / Kademlia / DHT** : aucune requête DHT active ni nœud de routage Kad.
- **Zéro connexion serveur eD2K** : aucun échange TCP/UDP vers des serveurs d'indexation publics.
- **Zéro upload / seeding** : aucune publication ou envoi spontané de fragments à des tiers.
- **Zéro fuite d'identité hôte** : anonymisation stricte de toutes les traces wire et rapports CI.

---

### Ajouts & Nouveautés

#### Sécurité Visuelle Unicode & Normalisation
- Intégration de `unicode-normalization` pour l'application canonique de la forme Unicode NFC sur les chaînes manipulées.
- Fonction [`sanitize_unicode_for_display`](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/ed2k/reference.rs) : conservation des valeurs brutes pour la comparaison octet par octet et échappement sécurisé en `\u{XXXX}` des caractères trompeurs pour les rapports CI et terminaux :
  - Overrides et isolats bidirectionnels (`U+202A`..=`U+202E`, `U+2066`..=`U+2069`, `U+061C`, `U+200E`, `U+200F`).
  - Caractères invisibles et espaces de largeur nulle (`U+200B`..=`U+200D`, `U+2060`, `U+FEFF`, `U+00AD`).
  - Caractères de contrôle non graphiques (`c.is_control()`).
- Détection et rejet automatique dans `TraceRecord::is_clean()` de toute trame contenant des bidi overrides ou des caractères invisibles non échappés.

#### Propriété Formelle de Non-Mutation
- Structure [`SessionSnapshot`](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/ed2k/session.rs) et méthode `Ed2kSession::snapshot()` pour figer l'état protocolaire complet d'une session.
- Compteur de diagnostics isolé `unhandled_packets_count`.
- Garantie formelle et testée : le traitement d'une trame `Raw` (`0xC5 PR_EMULE`, opcode ambigu pour le profil actif, ou opcode inconnu `0xFE`) n'altère en aucun cas l'état de session, le tracker de statut (`status_tracker`), le tracker de blocs (`block_tracker`), la planification de téléchargement, la persistance SQLite, ou les compteurs de progression.

#### Schéma de Traces Normalisées Machine-Readable
- Enregistrements de traces normalisées avec métadonnées d'attestation au niveau racine :
  - `schemaVersion: 1`
  - `testTier: 2` (ou `3` pour Tier 3)
  - `referenceKind: "emulated-reference-peer"` (ou `"external-reference-binary"`)
  - `compatibilityProfile: "Emule050a"` | `"Amule233"` | `"LegacyEdonkey"` | `"UnknownStrict"`
  - `protocolByte`, `opcode`, `direction`, `payloadLength`, `outcome`.
- Écriture sécurisée des artefacts CI via `DivergenceReport::write_ci_report_secure` dans des répertoires temporaires isolés (`tempfile::tempdir()`), avec échec pré-vol empêchant toute création de fichier si une trace est compromise.

#### Profils de Compatibilité eD2K Explicites
- Définition de l'enum `Ed2kCompatibilityProfile` :
  - `Emule050a` : opcodes documentés eMule 0.50a (`0x47` RequestParts, `0x4F` SetReqFileId).
  - `Amule233` : profil isolé pour variantes aMule.
  - `LegacyEdonkey` : opcodes historiques eDonkey 2000 (`0x4B` RequestParts, `0x49` FileStatusReq).
  - `UnknownStrict` : rejet immédiat de toute ambiguïté (`AmbiguousOpcodeForProfile`).
- Matrice de décodage multi-profil et gestion des sentinelles wire (`(0,0)` et `(u32::MAX, u32::MAX)`).

#### Pipeline de Stockage & Démon Local
- Moteur de téléchargement par intervalles disjoints (`RangeSet`) avec allocation éparse (`part_file.rs`).
- Support des blocs résiduels (ex: fichier de 9 728 001 octets composé d'une part de 9 728 000 octets et d'un résidu de 1 octet).
- Démon local supervisé `mule-daemon` avec persistance SQLite WAL, verrouillage exclusif `fs4::FileExt::try_lock()` sur `mule.lock`, et jeton secret CSPRNG à 64 caractères hexadécimaux (`auth.token`).
- Superviseur Flutter Desktop avec découverte automatique de session (`session.json`), reconnexion par backoff exponentiel et gestion de course au démarrage.

---

### Commandes de Validation (100% Hors-Ligne)

```powershell
# 1. Vérification statique et compilation
cargo check --workspace --all-targets --all-features

# 2. Linter sans avertissement
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Suite complète des 204 tests unitaires et d'intégration Rust (1 test Tier 3 live ignoré par défaut)
cargo test --workspace --all-features

# 4. Analyse statique Flutter Desktop
cd apps/desktop
flutter analyze

# 5. Suite des 31 tests Flutter Desktop
flutter test
```

### Décompte des Tests du Projet
- **Tests Rust** : **204 passés**, 1 ignoré (Tier 3 live), 0 échec.
- **Tests Flutter** : **31 passés**, 0 échec.
- **Total général actif** : **235 tests validés à 100 %**.
