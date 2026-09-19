# Journal des Modifications — Emule-Sion

Toutes les modifications notables de ce projet sont documentées dans ce fichier.
Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et ce projet adhère à [Semantic Versioning](https://semver.org/lang/fr/).

## [0.1.8] - 2026-09-19

Release de performance et de fidélité protocolaire eMule : agrégation multi-pairs des débits de téléchargement, gestion conforme des files d'attente distantes (Queue Ranking) et préservation des quotas d'essaim.

### Ajouté & Optimisé
- **Agrégation Multi-Pairs & Stabilisation du Débit (`transfer.rs`)** :
  - Calcul et synchronisation en temps réel du débit cumulé de téléchargement sur l'ensemble des sessions actives pour un même fichier (`sync_transfer_rates`).
  - Suppression des artefacts d'affichage à 0 B/s : `RateResetGuard` recalcule le débit restant lors de la déconnexion d'un pair au lieu d'écraser la vitesse globale à zéro.
  - Journalisation de télémétrie `[DL_PROGRESS]` périodique claire affichant le débit global cumulé et la contribution par pair actif.
- **Gestion Conforme des Files d'Attente Distantes (`peer_session.rs`)** :
  - Détection appropriée de la déconnexion en file d'attente (`Disconnected in queue`) : lorsque les pairs eMule ferment le socket TCP pour préserver leurs ressources tout en maintenant notre rang (`OP_QUEUERANKING`), la source reste en statut `Allowed`.
  - Application du délai de réinterrogation conforme eMule (10 à 15 minutes) au lieu d'une re-connexion immédiate agressive à 60s, évitant les pénalités et bannissements de file.
- **Protection du Plafond de Quota de l'Essaim (`swarm_coordinator/strategy.rs`, `types.rs`)** :
  - Intégration des sources en file d'attente (`Candidate`, `Tried`, `Allowed`) dans le calcul des sources viables, empêchant l'effondrement du plafond d'allocation water-filling.
- **Inbound Listener & Upload Queue (`dispatcher.rs`, `upload_queue.rs`)** :
  - Traitement complet des requêtes de téléversement entrantes (`OP_STARTUPLOADREQ`, `OP_ACCEPTUPLOADREQ`) et libération dynamique des slots d'envoi.
  - Logs de diagnostic stratégiques (`[UPLOAD_REQUEST]`, `[UPLOAD_SLOT_GRANTED]`, `[UPLOAD_RELEASED]`).
- **Cartographie Globale du Dépôt (`INDEX.md`)** :
  - Inventaire exhaustif et documentation de tous les fichiers versionnés du projet Emule-Sion.

---

## [0.1.3-lab] - 2026-09-15

Release de consolidation majeure intégrant l'implémentation complète du protocole eD2K réel (Tier 3.1 à Tier 3.6), l'inspection détaillée et la Part Map (Phase 8.1), l'historique et statistiques locales avec exports caviardés (Phase 8.2), les catégories locales et la hiérarchie de destination à 4 niveaux (Phase 8.3), l'import défensif de fichiers `.part.met` eMule avec rapport d'audit (Phase 8.4), ainsi que le packaging multi-saveurs Windows et la migration de schéma SQLite (Phase 8.5).

> **Formulation d'attestation d'interopérabilité & confinement :**  
> *« Emule-Sion est une application de laboratoire locale. Les transferts eD2K, la reprise, la vérification par parties et la finalisation ont été validés contre la référence eMule 0.50a dans un environnement strictement confiné à 127.0.0.1. Aucun serveur eD2K public, nœud Kad, peer externe, trafic UDP ni compression de protocole n’est activé. »*

### Ajouté
- **Protocole eD2K Loopback & Qualification eMule 0.50a (Tier 3.1 – 3.6)** :
  - Handshake complet eD2K et négociation d'extensions eMule (`ET_MOD_VERSION`, `ET_VERSION`, `ET_EXT_FEATURES`).
  - Déclaration de disponibilité de fichier (`FILE_STATUS_REQ` et `FILE_STATUS` avec bitfield par bloc et source complète).
  - Séquençage et planification bornée en blocs de 9,28 Mo décomposés en sous-plages de requêtes `REQUESTPARTS`.
  - Écriture en flux continu via `SENDINGPART` dans un fichier temporaire `.part`.
  - Contrôle d'intégrité cryptographique MD4 par bloc et calcul de l'empreinte racine eD2K globale.
  - Gestion du résidu de frontière (ex. 9 728 001 octets) et promotion atomique vers `downloads/`.
  - Reprise résiliente après arrêt brutal à ~50% avec persistance SQLite et reprise intégrale.
- **Inspection Détaillée & Part Map Interactive (Phase 8.1)** :
  - `DownloadDetailDialog` : identifiants complets, lien canonique `ed2k://`, métriques de transfert en direct.
  - Visualisation Part Map en blocs colorés avec tooltips interactifs et offsets exacts.
  - Export de diagnostics caviardés sans aucun jeton `auth.token`.
  - Modification de destination cible avec sonde d'écriture (`write-probe`).
- **Historique Local & Statistiques d'Activité (Phase 8.2)** :
  - Table SQLite `download_history` immuable avec index temporels et de types d'événements.
  - Tableau de bord `HistoryView` avec agrégats sur 4 fenêtres (Total, 7 jours, 24h, Session courante).
  - Politique de rétention configurable (7, 30, 90 jours ou conservation indéfinie) avec élagage automatique.
  - Exports caviardés JSON / CSV sans fuite de chemins privés.
- **Catégories Locales & Résolution de Destination à 4 Niveaux (Phase 8.3)** :
  - Table SQLite `download_categories` et association `category_id` dans `downloads`.
  - Invariant absolu de non-destruction : la suppression d'une catégorie réassocie les téléchargements à `category_id = NULL` sans effacer aucun fichier.
  - Résolution de destination hiérarchique : tâche > catégorie > global SQLite > bundle.
- **Import Défensif de Fichiers `.part.met` (Phase 8.4)** :
  - Décodage des formats binaires eMule standard (`0xE0`), grands fichiers (`0xE1`) et nouveaux formats (`0xE2`).
  - Inversion algorithmique des gaps pour calculer les plages valides réellement acquises.
  - Hachage cryptographique MD4 par bloc avec invalidation et ré-évaluation en trou des blocs corrompus.
  - Confinement strict sous `data/temp/<id>.part` et initialisation impérative en statut `Paused` (`sourceCount: 0`).
  - Rapport d'audit lisible avec bouton de copie dans le presse-papier et bouton de clôture.
- **Packaging Windows Multi-Saveurs & Migration SQLite (Phase 8.5)** :
  - Saveur `standard` épurée des outils de laboratoire et saveur `qa-loopback` instrumentée.
  - Migration de schéma SQLite idempotente et non destructive depuis Phase 7 certifiée par test unitaire.
  - Portabilité et mise à niveau sans perte des dossiers `data/` et `downloads/`.

---

## [0.1.2-lab] - 2026-09-14

Release de consolidation intégrant la séparation étanche `data/` vs `downloads/`, la persistance de la suppression SQLite, la sélection personnalisée du dossier de téléchargement, l'enrichissement des diagnostics, et le pipeline QA E2E automatisé Phase 7.3.

> **Formulation d'attestation d'interopérabilité :**  
> *« Emule-Sion est une application de laboratoire locale. La compatibilité eD2K est validée sur corpus statique et pair de référence émulé en loopback fermé. Aucun serveur eD2K public, nœud Kad ou peer externe n’est contacté. »*

### Ajouté
- **Séparation Étanche de l'État Interne (`data/`) et des Fichiers Finalisés (`downloads/`)** :
  - `data/` contient exclusivement l'état interne : SQLite WAL (`mule.db`), session locale (`session.json`), jeton secret (`auth.token`), fichiers partiels éparses (`.part`).
  - `downloads/` héberge les fichiers finalisés et validés, avec création automatique si nécessaire.
  - En mode portable, le dossier par défaut de téléchargement est résolu vers `<bundleRoot>/downloads` distinct de `data/`.
- **Sélection Personnalisée du Dossier de Destination (Browse...)** :
  - Intégration du sélecteur de dossier natif dans la modale d'ajout de lien eD2K (`AddDownloadDialog`).
  - Prise en compte du chemin cible personnalisé dans la méthode RPC `downloads.addEd2kLink`.
- **Persistance de la Suppression de Téléchargement dans SQLite** :
  - La méthode RPC `downloads.remove` exécute un `DELETE FROM downloads WHERE id = ?` persisté dans SQLite.
  - Suppression physique automatique des fichiers d'état partiels associés (`.part` et `.part.met`).
  - Garantie de non-réapparition de la tâche après arrêt et redémarrage du démon (`test_downloads_remove_persistence_across_daemon_restart`).
- **Garantie d'Inviolabilité des Téléchargements lors du Nettoyage Lab** :
  - La méthode `lab.cleanupTestData` ne supprime que `<workdir>/fixtures/` et les fichiers incomplets temporaires ; les fichiers finalisés dans `downloads/` ne sont jamais altérés.
- **Ergonomie UI & Diagnostics Approfondis** :
  - Affichage sous forme de badges (`Chip`) des capacités actives annoncées par le démon (`downloads`, `download-events`, `lab-tools`).
  - Abréviation ergonomique des identifiants UUIDv4 (`#abc1...`) dans le tableau des téléchargements.
  - DiagnosticDialog enrichi avec masquage systématique des chemins sensibles et préservation du zéro-secret.
- **Phase 7.3 : Pipeline E2E QA Automatisé (`scripts/test_qa_loopback_e2e.ps1`)** :
  - Validation hermétique en environnement temporaire stérile hors-dépôt.
  - Couverture E2E complète : fixture nominale (1 Mo), fixture à la frontière de part (9 728 001 octets), arrêt brutal/reprise de téléchargement avec conservation des blocs déjà validés, et contrôle cryptographique final.

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
