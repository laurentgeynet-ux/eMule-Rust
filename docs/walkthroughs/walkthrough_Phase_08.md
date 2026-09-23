# Walkthrough — Phase 8.1 & 8.2 : Inspection, Destinations, Historique et Statistiques Locales

Phase 8 enrichit le client de bureau **eMule-Rust** avec une gestion locale complète des transferts et de l'audit, tout en maintenant strictement la frontière locale loopback (`127.0.0.1`), la séparation entre les données temporaires de travail (`data/temp/`) et les répertoires finaux (`downloads/`), ainsi qu'une politique de zéro télémétrie.

---

## 1. Vue détaillée de téléchargement (`DownloadDetailDialog` - Phase 8.1)

Accessible via l'icône d'inspection sur chaque ligne de téléchargement ou via le bouton **"Inspect Full Details"** du panneau de sélection.

### Fonctionnalités principales :
- **Identifiants & Liens eD2K canoniques** :
  - UUID complet du téléchargement avec copie dans le presse-papier.
  - Hash eD2K MD4 avec copie immédiate.
  - Lien eD2K canonique (`ed2k://|file|<nom>|<taille>|<hash>|/`) avec copie en un clic.
- **Métriques de transfert en direct** :
  - Tailles : totale, reçue, restante, et octets vérifiés par hash.
  - Taux de transfert instantané, vitesse moyenne de la session, et estimation du temps restant (ETA).
  - Nombre de tentatives de connexion et affichage de la dernière erreur éventuelle.
- **Carte des parties eD2K (Part Map)** :
  - Découpage visuel en blocs standards de 9 728 000 octets (y compris la dernière partie résiduelle, ex: 9 728 001 octets).
  - Code couleur et légende interactive :
    - `TealAccent` : Vérifié (hash validé)
    - `BlueAccent` : Complété (en attente de validation)
    - `AmberAccent` : En cours de téléchargement
    - `White12 / Border` : Manquant
  - Tooltips interactifs affichant l'indice de partie, la plage d'octets (`startOffset - endOffset`), la taille et le statut.
- **Chemins de stockage & Destination cible** :
  - Chemin temporaire confiné : `data/temp/<id>.part` (lecture seule sécurisée).
  - Chemin cible finalisé : modification via dialogue avec test d'écriture (write-probe).
  - Information explicite : la modification de destination n'impacte que l'assemblage final ; le fichier `.part` reste confiné dans `data/temp/` pendant le transfert.
- **Export de diagnostic nettoyé (Sanitized Diagnostics)** :
  - Génération d'un rapport de diagnostic expurgé de tout jeton d'authentification (`auth.token`), avec anonymisation des répertoires utilisateurs (`[DOWNLOAD_DIR]`) et confirmation de confinement loopback.
- **Barre d'actions isolée (Action Bar)** :
  - **Mettre en pause / Reprendre**.
  - **Réessayer la source loopback** (`downloads.retryConnection`).
  - **Ouvrir l'emplacement finalisé** : strictement désactivé tant que le téléchargement n'a pas atteint le statut `completed`.
  - **Supprimer le téléchargement** : bouton de suppression isolé, affichant une boîte de dialogue de confirmation détaillant la suppression de l'enregistrement SQLite et des fichiers `.part` / `.part.met` temporaires, tout en préservant le fichier finalisé si déjà complété.

---

## 2. Paramètres de destination à deux niveaux (`SettingsDialog` - Phase 8.1)

Accessible depuis la barre latérale gauche (`DownloadsSidebar`) via l'entrée dédiée **Settings**.

### 1. Répertoire par défaut global :
- Stocké de manière persistante dans la table SQLite `settings` (`default_download_directory`).
- Validé avant application par une sonde d'écriture temporaire (`.mule_write_probe_<uuid>`) assurant la permission d'écriture.
- Bouton **"Reset to Portable Default"** : réinitialise immédiatement le chemin vers `<bundleRoot>/downloads`.
- Bouton **"Open Folder"** : ouvre le répertoire de destination dans l'explorateur du système d'exploitation.

### 2. Destination par téléchargement :
- Configurable dès l'ajout via l'URL eD2K (`downloads.addEd2kLink` avec `targetDirectory`).
- Modifiable en cours de route via `downloads.setDestination`, mettant à jour le chemin d'assemblage final sans déplacer le fichier `.part` en cours d'écriture dans `data/temp/`.

---

## 3. Historique & Statistiques Locales (Phase 8.2)

Phase 8.2 apporte une traçabilité complète et des métriques d'activité agrégées, stockées localement en base SQLite sans aucun composant de télémétrie ni fuite réseau.

### 1. Persistance SQLite (`download_history`) dans `mule-core` :
- **Table `download_history`** avec colonnes typées :
  - `id` (INTEGER PRIMARY KEY AUTOINCREMENT)
  - `download_id` (TEXT)
  - `file_name` (TEXT)
  - `total_bytes` (INTEGER)
  - `transferred_bytes` (INTEGER)
  - `ed2k_hash` (TEXT)
  - `event_type` (`completed`, `failed`, `removed`, `resumed`, `verified_pending_promotion`)
  - `status` (`downloading`, `paused`, `verifying`, `completed`, `failed`)
  - `duration_secs` (INTEGER)
  - `connection_attempts` (INTEGER)
  - `resume_count` (INTEGER)
  - `verification_result` (`verified_ok`, `hash_mismatch`, `unverified`, `skipped`)
  - `error_message` (TEXT nullable)
  - `timestamp_ms` (INTEGER)
- **Indexation performante** : `idx_history_timestamp` et `idx_history_event_type`.
- **Méthodes CRUD & Agrégations** :
  - `insert_history_entry(...)`
  - `query_history(filter)` avec pagination (`limit`, `offset`) et plages temporelles (`from_timestamp_ms`, `to_timestamp_ms`).
  - `get_history_stats(session_start_ms)` calculant les volumes et comptages sur les 4 fenêtres temporelles.
  - `prune_history(retention_days)` élaguant les enregistrements obsolètes.
  - `clear_history()` pour la purge volontaire par l'utilisateur.

### 2. Services et Points d'accès RPC dans `mule-daemon` :
- Enregistrement automatique des transitions :
  - `finalize_download` -> `event_type: completed`, `verification_result: verified_ok`.
  - `remove_download` -> `event_type: removed`.
  - `resume_download` -> `event_type: resumed`, incrémentant `resume_count`.
  - `update_status(Failed)` -> `event_type: failed`.
  - `update_status(Verifying)` -> `event_type: verified_pending_promotion`.
- Élagage automatique au démarrage selon la rétention configurée dans la table `settings` (`history_retention_days`).
- Endpoints JSON-RPC 2.0 :
  - `history.list` : liste filtrable et paginée.
  - `history.stats` : tableau de bord de métriques.
  - `history.clear` : purge complète avec confirmation.
  - `history.export` : génération de rapports CSV ou JSON désensibilisés.
  - `settings.setRetentionDays` : configuration de la durée de conservation (7, 30, 90 jours ou 0 = indéfini).
- Publicité de la capacité `"history"` dans `system.hello`.

### 3. Interface Desktop Flutter (`HistoryView`) :
- **Navigation latérale** : nouvelle section **"History & Statistics"** sous la rubrique `AUDIT & ACTIVITY`.
- **Tableau de bord statistique** :
  - *Total Completed* : nombre total de fichiers et volume finalisé cumulé.
  - *Activity Windows* : répartition par fenêtres (Session courante, Dernières 24h, 7 derniers jours).
  - *Resumes & Retries* : nombre total de reprises et échecs enregistrés.
  - *Integrity Health* : détection des erreurs de hash et taux d'intégrité (100% hash verified ou signalement).
- **Barre d'outils et de filtres** :
  - Filtrage dynamique par type d'événement (`All Events`, `Completed`, `Failed`, `Resumed`, `Verifying`, `Removed`).
  - Compteur d'enregistrements en temps réel.
  - Bouton **"Export Report"** (menu déroulant JSON / CSV).
  - Bouton destructif **"Clear History"** protégé par un dialogue de confirmation explicite.
- **Liste détaillée des événements** :
  - Badge coloré par statut / événement.
  - Nom du fichier et hash eD2K.
  - Volume transféré / total.
  - Durée formatée (`formatDuration`) et statistiques de reprise/tentatives.
  - Badge de résultat de vérification.
  - Horodatage lisible et bouton d'inspection modale détaillée.
- **Export désensibilisé (Sanitized Export)** :
  - Anonymisation de tous les chemins locaux utilisateur (`[DOWNLOAD_DIR]/<filename>`).
  - Zéro fuite de tokens ou identifiants sensibles.
  - Boîte de dialogue avec prévisualisation et copie rapide dans le presse-papier.
- **Configuration de rétention dans `SettingsDialog`** :
  - Menu déroulant proposant 7 jours, 30 jours (défaut), 90 jours ou Indéfini.
  - Pruning automatique exécuté au démarrage du daemon.

---

## 4. Catégories Locales & Hiérarchie de Destination (Phase 8.3)

Phase 8.3 dote eMule-Rust d'un système complet d'organisation locale des téléchargements avec attribution de priorités et de destinations par défaut.

### 1. Invariant absolu de sécurité :
- **La suppression d'une catégorie ne supprime JAMAIS aucun fichier ni aucun téléchargement.**
- Les téléchargements rattachés à une catégorie supprimée reçoivent automatiquement `category_id = NULL` et rejoignent la vue virtuelle **« Sans catégorie »**.

### 2. Hiérarchie de résolution de destination à 4 niveaux :
$$\text{targetDirectory (explicite)} > \text{defaultDestination (catégorie)} > \text{defaultDownloadDir (global SQLite)} > \text{downloads/ (bundle portable)}$$

### 3. Persistance SQLite (`download_categories`) :
- Table dédiée avec `id`, `name`, `color` (`#RRGGBB`), `sort_order`, `default_destination`, `default_priority`, `created_at`, `updated_at`.
- Migration de la table `downloads` avec colonne `category_id TEXT`.
- Indexation par `sort_order ASC`.

### 4. Endpoints RPC Daemon :
- `categories.list`, `categories.create`, `categories.update`, `categories.delete`, `categories.reorder`.
- `downloads.setCategory(downloadId, categoryId)`.
- Support de `categoryId` dans `downloads.addEd2kLink`.
- Annonce de la capacité `"categories"` dans `system.hello`.

### 5. Intégration Flutter Desktop :
- Section **CATEGORIES** dans `DownloadsSidebar` avec bouton d'ajout `+`, vues virtuelles (« All Downloads », « Sans catégorie »), pastilles de couleurs, compteurs et menu contextuel (Modifier, Supprimer).
- Boîte de dialogue `CategoryEditDialog` avec palette de couleurs, saisie hexadécimale et sélecteur de dossier.
- Pastille de catégorie et menu contextuel d'attribution dans chaque ligne `DownloadRow`.
- Sélecteur de catégorie dans `DownloadDetailDialog` et `AddLinkDialog` (avec pré-remplissage du dossier cible).

---

## 5. Architecture & Frontière de sécurité

```
+-------------------------------------------------------------------------------------------------+
|                                         eMule-Rust GUI                                          |
|   +-----------------------+   +----------------------+   +-------------------+   +----------+   |
|   |   SettingsDialog      |   |   HistoryView        |   | DownloadDetail    |   | Category |   |
|   |   - Destination glob. |   |   - Stats 4 fenêtres |   |   - Part map      |   | Dialog   |   |
|   |   - Rétention hist.   |   |   - Filtres events   |   |   - Diagnostics   |   | - Nom    |   |
|   |   - Confinement local |   |   - Exports rapports |   |   - Catégorie &   |   | - Couleur|   |
|   +-----------------------+   +----------------------+   |     Destination   |   | - Dossier|   |
|                                                          +-------------------+   +----------+   |
+------------------------------------------------+------------------------------------------------+
                                                 | JSON-RPC WebSocket (127.0.0.1:4660)
+------------------------------------------------v------------------------------------------------+
|                                           mule-daemon                                           |
|   - RPC: categories.list, create, update, delete, reorder, downloads.setCategory                |
|   - RPC: history.list, history.stats, history.clear, history.export                             |
|   - RPC: settings.get, settings.set, settings.setRetentionDays                                  |
|   - Résolution hiérarchique de destination : Download > Category > Global SQLite > Bundle      |
|   - Confinement strict 127.0.0.1 et dissociation automatique lors de la suppression de catégorie|
+-------------------------+---------------------------------------------------+-------------------+
                          |                                                   |
+-------------------------v-------------------------+   +---------------------v-------------------+
|               data/mule.db (SQLite)               |   |                data/temp/               |
|   - Table downloads (category_id inclus)          |   |   (Fichiers .part strictement confinés) |
|   - Table download_categories (CRUD & ordre)      |   +-----------------------------------------+
|   - Table settings (chemins & rétention)          |   |                downloads/               |
|   - Table download_history (audit local)          |   |   (Fichiers finalisés après validation) |
+---------------------------------------------------+   +-----------------------------------------+
```

- **Zéro trafic WAN & Zéro télémétrie** : l'ensemble des données d'historique, de configuration et de catégories réside exclusivement dans la base SQLite locale `data/mule.db`.
- **Exports assainis** : chemins utilisateurs masqués sous `[DOWNLOAD_DIR]` et tokens d'authentification totalement exclus.

---

## 6. Résultats des vérifications & Tests

### Tests unitaires & intégration Rust (`cargo test --workspace`)
```
test result: ok. 7 passed (mule-cli make_link_tests)
test result: ok. 3 passed (mule-core part_file_and_persistence)
test result: ok. 7 passed (mule-core range_set)
test result: ok. 2 passed (mule-core range_set_properties)
test result: ok. 1 passed (mule-core sqlite_corruption)
test result: ok. 3 passed (mule-core sqlite_categories_tests)
test result: ok. 3 passed (mule-core sqlite_history_tests)
test result: ok. 2 passed (mule-core sqlite_settings_tests)
test result: ok. 4 passed (mule-daemon category_rpc_tests)
test result: ok. 3 passed (mule-daemon download_details_and_destination_tests)
test result: ok. 1 passed (mule-daemon dual_portable_bundles)
test result: ok. 1 passed (mule-daemon history_rpc_tests)
test result: ok. 6 passed (mule-daemon lifecycle_and_locking)
test result: ok. 2 passed (mule-daemon rpc_contract)
test result: ok. 18 passed (mule-protocol ed2k_corpus_tests)
test result: ok. 6 passed (mule-protocol ed2k_isolation_guard)
test result: ok. 45 passed (mule-protocol ed2k_roundtrip_tests)
test result: ok. 13 passed (mule-protocol ed2k_sequencer_tests)
test result: ok. 5 passed (mule-protocol malformed_packets)
test result: ok. 7 passed (mule-protocol packet_codec)
test result: ok. 6 passed (mule-protocol protocol_limits)
Total : Tous les tests Rust de l'espace de travail sont passés avec 100% de succès.
```

### Analyse statique Flutter (`dart analyze`)
```
Analyzing lib, test...
No issues found!
```

### Tests widgets & modèles Flutter (`flutter test`)
```
00:12 +70: All tests passed!
- test/ui/category_management_test.dart:
  - displays categories in sidebar with counts and virtual entries: OK
  - filters downloads when a category is selected in sidebar: OK
  - opens CategoryEditDialog and creates a new category: OK
  - assigns download category and updates category count: OK
  - deleting category preserves downloads and moves them to uncategorized: OK
- test/ui/history_view_test.dart:
  - navigates to History & Statistics section and displays dashboard: OK
  - filters history entries by event type: OK
  - shows sanitized export dialog: OK
  - clears history after destructive confirmation: OK
  - settings dialog displays history retention policy options: OK
- test/ui/download_detail_dialog_test.dart:
  - renders all metadata, part map, paths, and action bar: OK
  - copy redacted diagnostic generates safe sanitized output without secrets: OK
  - changing destination prompts dialog and updates destination: OK
  - remove download button opens isolated confirmation dialog: OK
- test/ui/settings_dialog_test.dart:
  - renders global download directory input and confinement notice: OK
  - saving new default directory updates setting and shows success: OK
  - resetting to portable default clears custom directory: OK
- test/ui/downloads_screen_test.dart:
  - renders sidebar with branding and download items: OK
  - filters list by status category: OK
  - search filters list by filename: OK
  - selecting item reveals details panel: OK
  - add eD2K link dialog validates and adds download: OK
  - displays status notification banner: OK
  - displays Loopback Lab Mode badge with tooltip and hides dev tools by default: OK
  - shows DEV badge and allows toggling debug bar when dev tools enabled: OK
  - renders informative empty state with dedicated icon and loopback notice: OK
  - dims sidebar filters with zero count except selected: OK
  - app exit request triggers supervisor stop cleanly: OK
- test/ui/large_list_performance_test.dart:
  - Large list virtualization with 500 items renders and scrolls without lag: OK
```

### Pipeline QA Loopback automatisé (`scripts/test_qa_loopback_e2e.ps1`)
```
==================================================
  Phase 7.3 : Automated QA Loopback E2E Pipeline  
==================================================

[1/7] Creating isolated test sandbox at: C:\Users\Lou\AppData\Local\Temp\eMuleRust-QaE2E-bff77102
 -> portable.flag verified in bundle root.
[2/7] Booting packaged daemon binary in portable mode:
 -> Loopback confinement verified on ws://127.0.0.1:4660/rpc.
 -> system.hello handshake success (Version: 0.1.2, Mode: Portable).
[3/7] Testing Vector 1: Nominal Transfer (sample_1mb):
 -> Confirmed finalized file in default downloads/: sample_1mb.bin (1,048,576 bytes) in 169 ms.
[4/7] Boundary Transfer (9,728,001 bytes) in 451 ms:
 -> Confirmed boundary file in custom downloads/: fixture_part_9728001.bin (9,728,001 bytes) in 451 ms.
[5/7] Crash at ~50% and Resumption (part_9728000):
 -> Forcible kill simulated, .part file retained in temp/ (7,188,480 bytes).
 -> Daemon restarted: recovered from SQLite, reached 100% completion and hash verified!
[6/7] Verifying lab.cleanupTestData security and non-deletion of finalized files:
 -> Cleanup freed 20,504,577 bytes, all finalized files intact and untouched.
[7/7] Graceful shutdown via system.stop:
 -> Daemon exited cleanly with code 0.
==================================================
  PHASE 7.3 AUTOMATED QA LOOPBACK E2E PASSED!     
==================================================
```

---

## 5. Import expérimental de fichiers `.part.met` (Phase 8.4)

Phase 8.4 apporte l'importation défensive et sécurisée des téléchargements partiels ou complets issus d'eMule (`.part.met` et fichiers `.part` associés), sans aucune reprise automatique du réseau et avec confinement strict sous `data/temp/`.

### Principes et garanties de sécurité :
1. **Zéro activité réseau automatique** :
   - Tout téléchargement importé est initialisé impérativement en statut `status: Paused` avec `source_count: 0`.
   - Aucun paquet eD2K, aucune connexion TCP ou recherche de source n'est déclenchée lors de l'importation.
2. **Confinement et non-altération de la source (Sandbox Staging)** :
   - Le fichier source `.part` de l'utilisateur n'est jamais modifié, renommé ou déplacé.
   - Les données sont copiées de manière sécurisée sous `data/temp/<download_id>.part`.
3. **Validation défensive des formats binaires (`mule-format`)** :
   - Vérification stricte des octets magiques d'en-tête eMule (`0xE0` standard, `0xE1` grand fichier, `0xE2` nouveau format).
   - Inversion algorithmique des balises d'écarts (`gaps`) via `compute_completed_ranges` pour déterminer les intervalles d'octets réellement acquis.
   - Contrôle d'intégrité de la taille : le fichier `.part` physique ne peut excéder la taille totale déclarée dans le `.part.met`.
4. **Vérification cryptographique par partie (MD4)** :
   - Chaque partie complète de 9 728 000 octets est hachée (MD4) et comparée au tableau de `part_hashes` du `.part.met`.
   - Si le hash correspond, la partie est enregistrée comme vérifiée (`verified_parts`).
   - Si le hash échoue (données corrompues), la plage correspondante est automatiquement invalidée et évincée du `RangeSet` (reclassée en trou/gap), et incrémentée dans `corrupted_parts`.
5. **Persistance SQLite & Traçabilité locale** :
   - Enregistrement dans les tables `downloads`, `received_ranges` et `verified_parts`.
   - Création automatique d'une entrée dans la table d'audit `download_history` avec `event_type: "imported"`.
6. **Contrat JSON-RPC & Capabilities** :
   - `downloads.inspectPartMet` : inspection préliminaire non destructive retournant les métadonnées (nom, taille, hash, nombre de parties, octets estimés, existence du `.part`).
   - `downloads.importPartMet` : conversion, staging, vérification et enregistrement.
   - Capability `"import_part_met"` déclarée dans `system.hello` et `system.status`.
7. **Interface utilisateur Flutter (`apps/desktop`)** :
   - `ImportPartMetDialog` : saisie des chemins, auto-détection du `.part`, sélection de catégorie, résolution de destination cible (4-tier), prévisualisation d'inspection et bouton d'importation.
   - Bannière d'avertissement bilingue et de sécurité : « The download will be staged into data/temp/ and initialized in Paused state. L’import ne contacte aucun peer et le téléchargement restera en pause. »
   - Rapport d'audit d'importation structuré affiché immédiatement après succès (`import_audit_report_text`) avec statistiques détaillées (parties vérifiées, parties rejetées, statut de confinement, état réseau en pause).
   - Bouton de copie du rapport dans le presse-papier (`copy_report_button`) avec confirmation visuelle par SnackBar.
   - Bouton de finalisation « Done » (`finish_import_button`) fermant proprement la boîte de dialogue.
   - Déclencheurs accessibles dans l'état vide (`empty_import_part_met_btn`) et dans la barre latérale (`btn_import_part_met`).

---

## 6. Industrialisation, Packaging Windows & Matrice QA (Phase 8.5)

Phase 8.5 finalise le cycle produit complet en livrant des archives portables Windows x64 clé en main, auditées et conformes aux exigences strictes de sécurité et de reproductibilité.

### 1. Deux Saveurs Windows Distinctes

| Caractéristique | Saveur Standard (`standard`) | Saveur QA Loopback (`qa-loopback`) |
| :--- | :--- | :--- |
| **Archive livrable** | `eMuleRust-v0.1.2-lab-standard-windows-x64.zip` | `eMuleRust-v0.1.2-lab-qa-loopback-windows-x64.zip` |
| **Dossier de staging** | `dist/eMuleRust-Standard-Windows` | `dist/eMuleRust-Qa-Loopback-Windows` |
| **Feature `lab-tools`** | **Désactivée** (compilation sans code de test) | **Activée** (génération de fixtures et simulation) |
| **Méthodes RPC `lab.*`** | Absentes (-32601 MethodNotFound) | Présentes et fonctionnelles |
| **Capacités RPC** | `downloads`, `download-events`, `history`, `categories`, `import_part_met` | `downloads`, `download-events`, `history`, `categories`, `import_part_met`, `lab-tools` |
| **Usage** | Production utilisateur final | Tests d'intégration et qualification E2E |

### 2. Intégrité Cryptographique & SBOM CycloneDX 1.5

- **Génération SBOM quadripartite** :
  - `sbom/rust.cdx.json` : 191 dépendances Cargo auditées.
  - `sbom/flutter.cdx.json` : 33 composants Flutter / Dart.
  - `sbom/windows-bundle.cdx.json` : 16 artefacts Windows assemblés.
  - `sbom/sbom.cdx.json` : 240 composants consolidés au format CycloneDX 1.5.
- **Audit de licences** : 100% de conformité vis-à-vis des règles de `deny.toml` (`license-audit.json`, `license-audit.md`).
- **Contrôle d'intégrité 4-way** : vérification de concordance absolue entre les fichiers sur disque, `version.json`, `SHA256SUMS.txt` et le SBOM.

### 3. Migration Schéma SQLite & Préservation des Données (Phase 7 -> Phase 8.4)

- **Test automatisé de migration** : `test_phase7_to_phase8_schema_migration` dans `sqlite_categories_tests.rs`.
- Une base SQLite de Phase 7 (sans colonnes `category_id`, sans tables d'historique, de catégories ou de settings) est automatiquement mise à niveau par `Database::open` :
  - La colonne `category_id` est ajoutée de façon idempotente (`PRAGMA table_info`).
  - Les tables `download_history`, `download_categories`, `settings` et leurs index sont créés de manière transparente.
  - L'ensemble des téléchargements existants et leurs plages de données (`received_ranges`) sont conservés à 100% avec `category_id = NULL`.
- **Workflow de mise à niveau portable vérifié** : l'extraction de la nouvelle version sur une installation existante préserve intégralement le répertoire `data/` (`state.db`, `temp/`) et le dossier `downloads/`.

### 4. Résultats des Vérifications et Tests

#### Tests unitaires & intégration Rust (`cargo test --workspace`)
```
test result: ok. 100% passed across all crates:
- mule-cli: 7 passed
- mule-core: 21 passed (incluant migrations SQLite, catégories, historique, settings, range_set, corruption)
- mule-daemon: 20 passed (incluant RPC catégories, détails & destination, historique, import .part.met, locking)
- mule-format: 8 passed (vecteurs .part.met, tags eMule, gaps)
- mule-protocol: 95 passed (ed2k_corpus, isolation guard, roundtrip, sequencer, malformed, codec, limits)
Total : 151 tests Rust passés avec succès.
```

#### Analyse statique & Tests Flutter Desktop (`apps/desktop`)
```
flutter analyze : No issues found! (0 warnings, 0 errors, 0 lints)
flutter test    : 74 tests passés avec 100% de succès.
- import_part_met_dialog_test.dart : vérification de saisie, inspection, création confinée en statut Paused, affichage du rapport d'audit et copie dans le presse-papier.
- category_management_test.dart    : cycle de vie complet des catégories, dissociation non-destructive.
- download_detail_dialog_test.dart : métadonnées eD2K, part map, diagnostics désensibilisés, changement de destination.
- history_view_test.dart           : statistiques 4 fenêtres, filtres d'événements, exports assainis, rétention.
- settings_dialog_test.dart        : sonde d'écriture, répertoire par défaut global, reset portable.
- large_list_performance_test.dart : virtualisation fluide de 500 téléchargements simultanés.
```

#### Smoke Test Post-Packaging (`scripts/test_packaged_bundle.ps1`)
```
==================================================
  eMule-Rust Post-Packaging E2E Smoke Test        
  Flavors: standard & qa-loopback
==================================================
 -> Archive SHA-256 vérifié à 100% contre .zip.sha256.
 -> Extraction sterile en sandbox isolée sous AppData/Local/Temp.
 -> 22/22 hachages de fichiers validés avec exactitude via SHA256SUMS.txt.
 -> Processus eMuleRust.exe démarré et initialisé proprement sans plantage.
 -> Confinement loopback (127.0.0.1:4660) et token d'authentification 256 bits vérifiés.
 -> Contrats JSON-RPC validés (system.hello, system.status, system.stop).
 -> Absence absolue des méthodes lab.* dans la saveur standard (-32601 MethodNotFound).
 -> Présence des méthodes lab.* vérifiée dans la saveur qa-loopback.
 -> Arrêt gracieux et nettoyage du fichier session.json confirmé.
==================================================
  SMOKE TEST PASSED! Package is verified & sound. 
==================================================
```

#### Pipeline E2E QA Loopback (`scripts/test_qa_loopback_e2e.ps1`)
```
==================================================
  Phase 7.3 : Automated QA Loopback E2E Pipeline  
==================================================
[1/7] Extraction de l'archive QA Loopback et validation de portable.flag.
[2/7] Démarrage du daemon en mode portable avec confinement loopback strict (127.0.0.1).
[3/7] Transfert nominal (sample_1mb) : complété et vérifié en 181 ms dans default downloads/.
[4/7] Transfert de frontière (9 728 001 octets) : complété et vérifié en 439 ms dans custom downloads/.
[5/7] Simulation de crash à ~50% : kill forcé du daemon, persistance du .part sous data/temp/ et de l'état SQLite, redémarrage et reprise jusqu'à complétion 100% avec vérification d'intégrité !
[6/7] Sécurité lab.cleanupTestData : purge des fixtures temporaires, aucun fichier complété n'est supprimé.
[7/7] Arrêt gracieux via system.stop (code de sortie 0).
==================================================
  PHASE 7.3 AUTOMATED QA LOOPBACK E2E PASSED!     
==================================================
```

#### Matrice d'Environnement & Supervision RC (`scripts/test_rc_matrix.ps1`)
```
==================================================
  eMule-Rust RC Environment & Supervision Matrix  
==================================================
[1/6] Répertoires Unicode, caractères non-ASCII et espaces : PASS
[2/6] Conflit de port TCP 4660 & bascule dynamique sur port éphémère : PASS
[3/6] Verrou d'exclusivité & prévention de collision multi-instances : PASS
[4/6] Récupération robuste sur crash abrupt avec verrou obsolète : PASS
[5/6] Exécution autonome du bundle relocalisé hors du dépôt Git : PASS
[6/6] Gestion propre des erreurs sur répertoire en lecture seule : PASS
[7/7] Auto-résolution du mode portable via portable.flag sans argument CLI : PASS
==================================================
  ALL 7 ENVIRONMENT & SUPERVISION MATRIX TESTS OK! 
==================================================
```


