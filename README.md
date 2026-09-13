# Emule-Sion 🚀

**Emule-Sion** est une implémentation moderne, sécurisée et modulaire du protocole eD2K, architecturée en Rust (moteur de téléchargement, codec filaire et démon local) et en Flutter Desktop (interface utilisateur Windows).

Le projet est conçu selon un principe fondamental d'**isolation et de déterminisme absolu** : aucun paquet n'est échangé sur des réseaux publics, et toutes les procédures de validation sont 100 % reproductibles hors-ligne sur `127.0.0.1`.

---

## Attestation d'Interopérabilité Honnête

> **« Compatibilité eD2K expérimentale validée sur corpus statique et pair de référence émulé, en environnement loopback fermé. Le harnais Tier 3 pour un binaire eMule/aMule externe épinglé est disponible, mais son test live reste ignoré par défaut tant qu’un binaire vérifié n’est pas fourni. »**

Le projet distingue formellement trois paliers de preuve afin d'interdire tout raccourci heuristique :
- **Tier 1 (Corpus statique)** : **Validé (100%)** — Décodage/encodage canonique et invariant sur un corpus d'octets figés réels.
- **Tier 2 (Pair de référence émulé)** : **Validé (100%)** — Conformité dynamique d'échange (handshake, statut, parts, résidus, sentinelles) face à un pair simulé en loopback hermétique.
- **Tier 3 (Binaire externe épinglé)** : **Harnais Prêt** — Harnais d'orchestration prêt, sécurisé et audité ; test live ignoré par défaut tant qu'un binaire eMule/aMule vérifié et audité par SHA-256 n'est pas fourni.

---

## Statut Fonctionnel Exact (Release v0.1.0-lab)

| Domaine | État | Description |
| :--- | :--- | :--- |
| **UI Flutter Desktop** | **Fonctionnelle** | Interface réactive connectée au démon local via WebSocket JSON-RPC, avec fallback sur mock client en mode autonome |
| **Démon local (`mule-daemon`)** | **Supervisé & Verrouillé** | Verrou exclusif `fs4` (`mule.lock`), token secret CSPRNG à 64 caractères (`auth.token`), écriture atomique de session (`session.json`) |
| **Persistance & Stockage** | **Robuste** | SQLite WAL, fichiers éparses `.part`, gestion des manques par `RangeSet`, reprise après arrêt brutal |
| **Intégrité Cryptographique** | **Conforme** | Hachage eD2K (MD4), arbres de vérification AICH (SHA-1), validation stricte des offsets et longueurs |
| **Protocoles de laboratoire** | **Validés** | Banc de test loopback couvrant les cas nominaux et adverses (saturation, doublons, corruptions, timeouts) |
| **eD2K historique** | **Validé en loopback** | Codec filaire borné, handshake, statuts de fichiers et transferts de blocs complets et résiduels |
| **Profils de compatibilité** | **Explicites** | `Emule050a`, `Amule233`, `LegacyEdonkey`, `UnknownStrict` (désambiguïsation formelle des opcodes surchargés) |
| **Compression `0xD4`** | **Rejet explicite** | `CompressedProtocolUnsupported` inconditionnel en l'absence de décompresseur borné et audité |
| **Référence externe réelle** | **Harnais prêt** | Harnais d'orchestration Tier 3 disponible, test live ignoré par défaut |
| **Réseau public / Kad** | **Volontairement absent** | Zéro socket publique, zéro routage DHT/Kad, zéro serveur eD2K externe, zéro upload/seeding |

---

## Architecture du Workspace

```
Emule-Sion/
├── apps/
│   └── desktop/            # UI Bureau Flutter Desktop Windows
│       ├── lib/            # Écrans, services de supervision et client WebSocket JSON-RPC
│       └── test/           # 31 tests unitaires, de widgets et d'intégration E2E
├── crates/
│   ├── mule-format/        # Formats eD2K, parsing de liens, calcul de hash MD4 et arbres AICH
│   ├── mule-core/          # Moteur de téléchargement, stockage éparse et persistance SQLite WAL
│   ├── mule-protocol/      # Codec filaire borné, profils, state machine, traces normalisées et harnais Tier 3
│   ├── mule-daemon/        # Démon de fond local, supervision, RPC WebSocket et verrou d'instance
│   └── mule-cli/           # Outil en ligne de commande pour diagnostics locaux
└── scripts/
    └── build_windows_release.ps1  # Pipeline de packaging Windows autonome
```

---

## Principes de Sécurité et Garanties Techniques

1. **Isolation Loopback Stricte** :
   Toute socket réseau créée par le projet est confinée inconditionnellement à `127.0.0.1`. Toute tentative de liaison ou de connexion vers une adresse non-loopback échoue immédiatement.
2. **Propriété Formelle de Non-Mutation** :
   Le décodage d'une trame `Raw` (`0xC5 PR_EMULE`), d'un opcode inconnu (`0xFE`) ou ambigu n'altère en aucun cas l'état de session, le tracker de requêtes, la planification ou le stockage. Seul un compteur de diagnostic isolé est incrémenté.
3. **Sécurité Visuelle Unicode Anti-Spoofing** :
   Les octets bruts sont préservés pour la conformité filaire. L'affichage dans les rapports CI et terminaux applique la normalisation Unicode NFC et échappe systématiquement en `\u{XXXX}` les caractères de contrôle, les isolats/overrides bidirectionnels (`U+202E`, etc.) et les caractères invisibles zero-width pour interdire toute dissimulation visuelle.
4. **Zéro Fuite d'Identité Hôte** :
   Tous les rapports et traces CI sont validés mécaniquement contre 13 motifs de fuite d'environnement (chemins Windows/Unix, tokens d'authentification, mots de passe). En cas d'anomalie, la génération d'artefact avorte sans écrire aucun fichier.
5. **Garde Statique Rust** :
   Zéro appel à `unwrap()` ou `expect()` dans la logique principale de décodage filaire eD2K.

---

## Instructions de Reproduction & Validation (100% Hors-Ligne)

### Prérequis
- **OS** : Windows 10 / 11 (x64)
- **Rust** : 1.80.0+ (stable)
- **Flutter** : 3.24.0+ (avec support desktop Windows activé)
- **Visual Studio C++ Build Tools 2022**

### Commandes de Qualité
```powershell
# 1. Vérification de la compilation du workspace
cargo check --workspace --all-targets --all-features

# 2. Linter Rust strict sans avertissement
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Suite complète des 204 tests Rust (1 test Tier 3 live ignoré par défaut)
cargo test --workspace --all-features

# 4. Analyse statique Dart / Flutter
cd apps/desktop
flutter analyze

# 5. Suite des 31 tests Flutter Desktop
flutter test
```

### Packaging Windows Autonome & Release Candidate
```powershell
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1
```
Le bundle autonome est assemblé dans `dist/EmuleSion-Windows/` :
- Binaire principal `EmuleSion.exe` et démon autonome `bin/mule-daemon.exe`.
- Manifeste de provenance `version.json` et sommes de contrôle `SHA256SUMS.txt`.
- Inventaire SBOM CycloneDX 1.5 (`sbom/sbom.cdx.json`) et audit de conformité (`sbom/license-audit.md`).
- Archive de distribution scellée `dist/EmuleSion-v0.1.0-lab-windows-x64.zip` et son empreinte `.sha256`.

### Validation Post-Packaging & Protocole QA
```powershell
# 1. Smoke test post-packaging E2E en bac à sable temporaire hors dépôt
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_packaged_bundle.ps1

# 2. Matrice de supervision et d'environnement (Unicode, port 4660 occupé, lock, crash recovery)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_rc_matrix.ps1
```

Protocole détaillé de réception pour testeurs internes : [docs/QA_RECEPTION_PROTOCOL.md](docs/QA_RECEPTION_PROTOCOL.md).

---

## Historique et Versioning

Pour consulter l'historique complet des versions et la synthèse de la dernière release interne :
- [CHANGELOG.md](CHANGELOG.md)
- [RELEASE_NOTES_0.1.0-lab.md](RELEASE_NOTES_0.1.0-lab.md)
- [Protocole de Réception QA](docs/QA_RECEPTION_PROTOCOL.md)
- [Guide de Référence et Walkthrough](walkthrough.md)
