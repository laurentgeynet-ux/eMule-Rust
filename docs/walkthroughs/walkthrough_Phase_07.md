# Walkthrough — Phase 7.2 : Parcours de test local guidé & Ergonomie RC

La **Phase 7.2** dote eMule-Rust d'un parcours de test local loopback de bout en bout, utilisable directement depuis l'interface graphique **sans jamais toucher à un terminal**, tout en garantissant une isolation stricte sur `127.0.0.1` et en protégeant les installations standards `%LOCALAPPDATA%`.

---

## 1. Détection bornée de la racine du bundle (`portable.flag`)

Pour éliminer tout risque de traversée incontrôlée de l'arborescence (anti-fuite de configuration), la détection de la racine du bundle est strictement bornée :
- Si `parent(executable)` se nomme `bin` (insensible à la casse) : `bundleRoot = parent(parent(executable))`.
- Sinon : `bundleRoot = parent(executable)`.
- Fichier cherché : `bundleRoot / portable.flag`.
- **Zéro traversée récursive ascendante** au-delà de cette règle.

### Vérification
- **Rust** : `test_workdir_non_bin_subfolder_does_not_traverse_parent_for_portable_flag` dans [crates/mule-daemon/tests/workdir_tests.rs](file:///d:/Projet/eMule-Rust/crates/mule-daemon/tests/workdir_tests.rs).
- **Dart** : `WorkDirectoryResolver` dans [apps/desktop/test/services/work_directory_resolver_test.dart](file:///d:/Projet/eMule-Rust/apps/desktop/test/services/work_directory_resolver_test.dart).

---

## 2. Nouvelles méthodes JSON-RPC de laboratoire (`mule-daemon`)

Le daemon expose désormais une suite complète d'opérations de laboratoire dédiées au QA local :

| Méthode RPC | Description | Sécurité & Gardes |
|---|---|---|
| `lab.status` | Retourne le mode de workdir, le chemin, l'indicateur portable, le pair loopback actif et la liste des fixtures. | Lecture seule. |
| `lab.createFixture` | Génère un fichier déterministe (`part_9728000`, `sample_1mb`, `text`) dans `<workdir>/fixtures/`, calcule son hash eD2K racine via `mule-format` et génère le lien eD2K canonique. | Confiné au dossier fixtures. |
| `lab.startLoopbackPeer` | Démarre une instance de `Ed2kTestPeer` sur `127.0.0.1:0` partageant toutes les fixtures générées. | Strictement lié à `127.0.0.1`. |
| `lab.stopLoopbackPeer` | Arrête le pair de test loopback et libère le socket TCP. | Nettoyage propre. |
| `lab.cleanupTestData` | Supprime les fixtures locales et purge les fichiers `.part` incomplets dans `<workdir>/temp` et `<workdir>/part`. | **Interdit et rejeté** en mode `InstalledDefault` (`%LOCALAPPDATA%`). |

### Vérification Rust
- Tests d'intégration complets dans [crates/mule-daemon/tests/lab_rpc_tests.rs](file:///d:/Projet/eMule-Rust/crates/mule-daemon/tests/lab_rpc_tests.rs) :
  - `test_lab_rpc_lifecycle_in_portable_mode` (création de fixtures, liaison canonique, démarrage/arrêt du pair, nettoyage).
  - `test_lab_cleanup_refused_in_installed_default_mode` (vérification de la protection anti-suppression en mode installé).
- **221 tests Rust validés**, Clippy : 0 avertissement.

---

## 3. Interface Utilisateur & Ergonomie RC

### Diagnostic Dialog (`DiagnosticDialog`)
Accessible en un clic sur le badge **« Loopback Lab Mode »** dans l'en-tête ou sur la carte de statut du daemon dans la barre latérale.

- **Confinement Réseau** : Carte visuelle rappelant la restriction stricte à `127.0.0.1` (zéro trafic WAN).
- **Workdir & Mode** : Badge coloré (`Portable`, `Explicit`, `Environment`, `InstalledDefault`) et chemin exact des données.
- **Processus & RPC** : PID du processus managé, endpoint WebSocket (`ws://127.0.0.1:4660/rpc`), version du protocole, identifiant d'instance et de session.
- **Copie rapide** : Bouton « Copy Diagnostics » pour copier l'ensemble des diagnostics dans le presse-papiers pour les rapports de bogue.
- **Accès direct** : Bouton « Open Lab Guide » menant directement au parcours guidé.

### Parcours de test local guidé (`LabGuideDialog`)
Un dialogue pas-à-pas en 5 étapes pour exécuter un cycle complet de test sans terminal :
1. **Étape 1 — Générer un fichier test** : Choix entre un échantillon de 1 Mo (`sample_1mb`), un chunk complet de 9,28 Mo (`part_9728000`), ou un texte léger (4 Ko).
2. **Étape 2 — Lien eD2K & Ajout** : Affichage du lien eD2K canonique, bouton « Copier », et bouton « Add to Downloads » qui injecte instantanément la tâche dans la file d'attente de l'application.
3. **Étape 3 — Pair de test loopback** : Bouton démarrer/arrêter le pair loopback sur `127.0.0.1` avec indicateur d'état dynamique et nombre de fichiers partagés.
4. **Étape 4 — Connecter le téléchargement** : Sélection de la tâche cible et connexion au pair `127.0.0.1` pour déclencher les transferts de blocs et la vérification d'intégrité.
5. **Étape 5 — Nettoyage des données de test** : Purge sécurisée des fichiers de fixtures et des téléchargements temporaires (avec dialogue de confirmation, réservé au mode portable).

### Gating et réactivité visuelle
- **Protection du mode installé** : Les outils de laboratoire sont masqués lorsque l'application s'exécute en mode standard `%LOCALAPPDATA%`.
- **Mise en page adaptative (`Wrap`)** : Tous les dialogues s'adaptent sans aucun débordement de rendu (`RenderFlex overflow`), y compris sur des résolutions compactes (800×600).
- **Accès depuis l'état vide** : Lorsqu'aucun téléchargement n'est présent, un bouton dédié « Guided Local Test » est proposé aux testeurs aux côtés de « Add eD2K Link ».

---

## 5. Correctif critique : Persistance de la suppression d'un téléchargement

### Problème identifié
En mode Standard (`%LOCALAPPDATA%`) ou en mode Portable, la suppression d'un téléchargement depuis l'interface le faisait disparaître visuellement de l'écran courant. En revanche, lors de la fermeture puis de la réouverture d'eMule-Rust, le téléchargement réapparaissait immédiatement dans la liste.

### Cause racine
1. Dans `crates/mule-daemon/src/services/downloads.rs`, `remove_download` retirait le téléchargement de la mémoire vive (`self.active.remove(id)`), mais **n'exécutait aucune requête SQL de suppression dans la base SQLite locale** (`mule.db`).
2. `Database` dans `crates/mule-core/src/persistence/sqlite.rs` ne disposait même pas d'une fonction `delete_download`. Les tables `downloads`, `received_ranges` et `verified_parts` conservaient indéfiniment l'enregistrement.
3. Au redémarrage du daemon, `db.get_all_downloads()` rechargeait l'entrée depuis la base, et Flutter la recevait à nouveau lors de `downloads.list`.
4. De plus, les fichiers temporaires `.part` restaient orphelins sur le disque si l'utilisateur ne pouvait pas choisir de les supprimer.

### Corrections implémentées
1. **Suppression atomique en base (`mule-core`)** :
   - Ajout de `delete_download(&mut self, download_id: &Uuid) -> Result<bool, CoreError>` dans [`sqlite.rs`](file:///d:/Projet/eMule-Rust/crates/mule-core/src/persistence/sqlite.rs).
   - Transaction SQLite atomique supprimant les lignes associées dans `received_ranges`, `verified_parts`, puis `downloads`.
   - Test unitaire dédié : `test_database_delete_download_persists_across_reopen` dans [`part_file_and_persistence.rs`](file:///d:/Projet/eMule-Rust/crates/mule-core/tests/part_file_and_persistence.rs).
2. **Gestion du descripteur et des fichiers temporaires (`mule-daemon`)** :
   - Fermeture explicite du descripteur de fichier Windows (`active.part_file.take()`) avant toute suppression sur le disque, prévenant l'erreur de verrouillage Windows `ERROR_SHARING_VIOLATION` (code 32).
   - Suppression systématique en base de données via `self.db.delete_download(id)`.
   - Suppression du fichier `.part` physique dans `temp/` si `delete_files == true`.
   - Test de contrat RPC : `test_downloads_remove_persistence_across_daemon_restart` dans [`rpc_contract.rs`](file:///d:/Projet/eMule-Rust/crates/mule-daemon/tests/rpc_contract.rs).
3. **Dialogue de confirmation enrichi (`apps/desktop`)** :
   - Mise à jour de [`download_row.dart`](file:///d:/Projet/eMule-Rust/apps/desktop/lib/ui/features/downloads/widgets/download_row.dart) avec une case à cocher « Supprimer également les fichiers temporaires/partiels du disque » (cochée par défaut).
   - Transmission du booléen `deleteFiles` au daemon.

---

## 6. Résultats des validations automatisées

```
==================================================
  Suite de tests Rust (cargo test --workspace --all-features)
==================================================
-> 222 tests passés, 0 échec, 1 ignoré (Tier 3 live ref)
-> Clippy (--workspace --all-targets --all-features) : 0 avertissement

==================================================
  Suite de tests Flutter (flutter test & analyze)
==================================================
-> 49 tests de widgets et d'intégration passés, 0 échec
-> Flutter analyze : No issues found!

==================================================
  Pipeline de packaging (scripts\build_windows_release.ps1)
==================================================
-> Bundle standard autonome : dist\eMuleRust-Standard-Windows (eMuleRust-v0.1.1-lab-standard-windows-x64.zip)
-> Bundle QA Loopback : dist\eMuleRust-Qa-Loopback-Windows (eMuleRust-v0.1.1-lab-qa-loopback-windows-x64.zip)
-> SBOM CycloneDX 1.5 consolidé : 240 composants audités
-> Hachages SHA-256 vérifiés et cohérents à 100%

==================================================
  Smoke test post-packaging (scripts\test_packaged_bundle.ps1)
==================================================
-> Décompression en sandbox stérile (%TEMP%)
-> Vérification d'intégrité SHA256SUMS.txt : 22/22 fichiers conformes
-> Démarrage daemon, vérification session loopback et JSON-RPC : SUCCÈS

==================================================
  Matrice d'environnement & supervision (scripts\test_rc_matrix.ps1)
==================================================
[1/7] Chemins avec espaces et caractères Unicode : OK
[2/7] Conflit de port TCP 4660 & repli éphémère : OK
[3/7] Verrou d'exclusivité & anti-collision : OK
[4/7] Récupération sur crash brutal & verrou orphelin : OK
[5/7] Bundle déplacé hors du dépôt : OK
[6/7] Répertoire non inscriptible : OK
[7/7] Mode portable via portable.flag : OK
```

---

## 5. Durcissement Sécurité & Gating Conditionnel (Validation Finale)

En réponse directe aux exigences de durcissement de production :

### A. Séparation stricte des features Cargo (`default = []`) & Capacités épurées
- `crates/mule-daemon/Cargo.toml` et `crates/mule-protocol/Cargo.toml` définissent désormais `default = []`.
- Les routes `lab.*` et le serveur loopback `Ed2kTestPeer` ne sont compilés **que** si la feature `lab-tools` est expressément demandée (`cargo build --features lab-tools`).
- `debug.injectRange` est conditionné par `#[cfg(any(feature = "debug-rpc", feature = "lab-tools"))]`.
- La capacité `local-simulation` a été totalement retirée du build standard :
  - **Build standard de production** : `capabilities = ["downloads", "download-events"]`
  - **Build QA portable (-LabMode)** : `capabilities = ["downloads", "download-events", "lab-tools"]`

### B. Taxonomie formelle des codes d'erreur JSON-RPC
Conformément aux exigences de stabilité du contrat d'API :
| Code | Nom canonique | Déclenchement / Description |
|---|---|---|
| `-32700` | `ParseError` | Trame JSON invalide. |
| `-32600` | `InvalidRequest` | Requête JSON-RPC non conforme. |
| `-32601` | `MethodNotFound` | Méthode inconnue ou route de lab appelée sur un build de production. |
| `-32602` | `InvalidParams` | Paramètres manquants ou mal typés. |
| `-32603` | `InternalError` | Erreur interne du daemon. |
| `-32000` | `AuthenticationFailed` | Jeton d'authentification invalide ou manquant. |
| `-32001` | `ProtocolMismatch` | Incompatibilité de version de protocole filaire. |
| `-32002` | `ForbiddenInInstalledMode` | Opération interdite car le workdir est en mode standard installé `%LOCALAPPDATA%`. |
| `-32003` | `UnsafeCleanupTarget` | Chemin hors liste blanche, traversée de répertoire (`..`) ou lien symbolique / point de jonction détecté. |
| `-32004` | `FixtureDownloadMismatch` | Incohérence entre la taille/le hash du téléchargement et la fixture. |
| `-32005` | `LoopbackPeerUnavailable` | Pair loopback inactif ou injoignable sur `127.0.0.1`. |
| `-32010` | `BusinessError` | Erreur métier générique. |

### C. Double condition d'affichage côté Flutter (Dual-Gating)
- L'interface Flutter n'affiche les outils de laboratoire que si deux conditions simultanées sont réunies :
  1. `workdirMode == 'Portable'`
  2. `daemonInfo.capabilities.contains('lab-tools')`

### D. Protection absolue contre la fuite de secrets (Diagnostics)
- **Export Partageable Sécurisé (« Copy Shareable (Sanitized) »)** :
  - Chemins masqués : `<portable-data>` ou `<installed-data>` (aucun nom d'utilisateur Windows ne fuite).
  - Identifiants tronqués : format `4b7e…8a31`.
  - Endpoint banalisé : `ws://loopback:4660/rpc`.
  - Token d'authentification : strictement exclu (zéro token).
- **Export Local Développeur (« Copy Full (Local Dev) »)** : conserve les chemins complets pour le débogage local en environnement contrôlé.
  - **Garantie de non-divulgation vérifiée par test automatisé** : un test de widget unitaire dédié dans [`test/ui/diagnostic_dialog_test.dart`](file:///d:/Projet/eMule-Rust/apps/desktop/test/ui/diagnostic_dialog_test.dart) valide formellement que ni `auth.token`, ni aucun jeton secret ou en-tête d'authentification n'apparaît dans le texte copié.

### E. Préconditions et chaîne de progression dans l'Étape 4 (`LabGuideDialog`)
- Vérification de la cohérence de taille entre le téléchargement sélectionné et la fixture générée (avertissement visuel immédiat en cas de divergence).
- Refus de connexion si le téléchargement est déjà `Completed` ou si le pair loopback est inactif.
- Bandeau de progression dynamique : état de la tâche, débit, octets transférés et badge de complétion.

### F. Nettoyage durci (`lab.cleanupTestData`)
- Blocage strict en mode `InstalledDefault` (code erreur JSON-RPC métier `-32002`).
- Rejet immédiat de toute tentative d'échappement hors du workdir canonique ou de traversée de lien symbolique (code d'erreur `-32003`).
- Liste blanche stricte : seuls `fixtures/`, `temp/` et `part/` sont nettoyés ; les dossiers `downloads/` et `incoming/` ne sont jamais altérés.
- Retour structuré d'un rapport détaillé (`cleanedFolders`, `deletedFilesCount`, `freedBytes`) affiché immédiatement au testeur.

---

## 6. Comparatif des Deux Livrables Découplés (v0.1.1-lab)

| Propriété / Critère | Build Standard de Production | Build QA Loopback (`-Flavor qa-loopback`) |
|---|---|---|
| **Commande de génération** | `powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1 -Flavor standard` | `powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1 -Flavor qa-loopback` |
| **Dossier d'assemblage** | `dist\eMuleRust-Standard-Windows` | `dist\eMuleRust-Qa-Loopback-Windows` |
| **Arbre Git requis** | 100% propre (`git status --porcelain` vide) | 100% propre (`git status --porcelain` vide) |
| **Manifeste `version.json`** | `"distributionFlavor": "standard"`, `"labToolsEnabled": false` | `"distributionFlavor": "qa-loopback"`, `"labToolsEnabled": true` |
| **Capacités annoncées** | `["downloads", "download-events"]` | `["downloads", "download-events", "lab-tools"]` |
| **Routes RPC `lab.*`** | Absentes (retourne `-32601 Method not found`) | Actives et enregistrées (`lab.status`, `lab.createFixture`, etc.) |
| **Pair loopback `Ed2kTestPeer`** | Non compilé | Présent, écoutant uniquement sur `127.0.0.1:0` |
| **Simulation `debug.injectRange`** | Non compilé | Présent pour tests de blocs locaux |
| **UI Flutter (Mode Portable)** | Outils de lab masqués (`isLabModeAvailable == false`) | Outils de lab et guide interactif actifs |
| **Vérification smoke test** | **Succès** (22/22 hachages conformes, session 127.0.0.1, -32601 vérifié) | **Succès** (22/22 hachages conformes, session 127.0.0.1, lab.status OK) |
| **Matrice de robustesse** | 7/7 tests réussis (`scripts\test_rc_matrix.ps1`) | 7/7 tests réussis (`scripts\test_rc_matrix.ps1`) |
| **Archive livrable** | `eMuleRust-v0.1.1-lab-standard-windows-x64.zip` | `eMuleRust-v0.1.1-lab-qa-loopback-windows-x64.zip` |
| **Empreinte SHA-256** | `2f258ef49383aa23e7ce7af350544466369ef57f69c5b3add7bfdd7d8ca33332` | `02e58f05f808d4a2726a5476f0fdf94f3c54e9fa09b38790cde8156ac05bbde4` |
| **Tag Git Scellé** | `v0.1.1-lab` (`6166c71`) | `v0.1.1-lab` (`6166c71`) |

---

## 7. Résolution du Diagnostic, Propagation DaemonInfo & Ergonomie de Téléchargement

### A. Cause racine diagnostiquée et corrigée pour les valeurs « N/A / Unknown »
1. **Rupture de propagation `DaemonInfo`** : `DownloadsViewModel` n'initialisait son champ interne `_daemonInfo` que dans la méthode `initialize()`. Or, lors du lancement nominal de l'application supervisée, `supervisor != null`, donc `initialize()` n'était jamais invoqué. `DaemonSupervisor` se connectait via `RpcDaemonClient.connect()` qui stockait bien les données dans `client.daemonInfo`, mais `DownloadsViewModel` ne lisait pas `client.daemonInfo` lors des événements `SupervisorState.connected` ou `instanceChanged`.
   - **Correction** :
     - Ajout du getter `DaemonInfo? get daemonInfo` sur l'interface abstraite [`MuleDaemonClient`](file:///d:/Projet/eMule-Rust/apps/desktop/lib/domain/services/mule_daemon_client.dart).
     - Écoute active dans `DownloadsViewModel` pour synchroniser `_daemonInfo = client.daemonInfo` lors de chaque connexion et rafraîchissement.
     - Conservation de l'instance active de session [`DaemonSessionFile`](file:///d:/Projet/eMule-Rust/apps/desktop/lib/services/daemon_supervisor.dart) dans le superviseur pour fournir les PID, endpoints et version du protocole instantanément.

2. **Préservation du `WorkDirMode` entre Flutter et le daemon enfant** :
   - Lorsque Flutter lançait `mule-daemon.exe --workdir <dataDir>`, le daemon interprétait la présence de `--workdir` comme `WorkDirMode::ExplicitCli`.
   - **Correction** :
     - Ajout de l'argument `--mode / --workdir-mode` dans le parsing CLI de [`mule-daemon`](file:///d:/Projet/eMule-Rust/crates/mule-daemon/src/main.rs).
     - Détection automatique dans `mule-daemon` : si `--workdir` est fourni sans `--mode`, le daemon vérifie si le parent direct contient `portable.flag` et promeut automatiquement le mode en `WorkDirMode::Portable`.
     - `DaemonSupervisor` passe explicitement `--mode Portable` au lancement.
     - `isLabModeAvailable` accepte à la fois la réponse du daemon et la détection locale du superviseur.

### B. Dialogue de diagnostic enrichi (`DiagnosticDialog`)
- **Puces de capacités (`Capabilities`)** : affichage élégant de chaque capacité sous forme de chip, avec mise en avant teal pour `lab-tools`.
- **Informations dynamiques complètes** : mode portable, chemin résolu, PID réel, endpoint WebSocket réel, version du daemon, identifiant d'instance et de session.
- **Badge d'en-tête adaptatif** :
  - Si le mode Lab est actif : **« Loopback Lab Mode »** en teal avec icône science, plus le bouton d'accès direct au guide de test (`Icons.biotech`).
  - Si le mode Lab est absent (build standard ou mode installé) : **« Diagnostics »** en blueGrey avec icône analytics.

### C. Abréviation ergonomique des identifiants de téléchargement
- Dans [`DownloadRow`](file:///d:/Projet/eMule-Rust/apps/desktop/lib/ui/features/downloads/widgets/download_row.dart), l'identifiant UUID brut de 36 caractères est désormais abrégé au format `ID: 4dab69c4…` (8 caractères + ellipse).
- **Tooltip natif** : survol affichant l'UUID complet (`Full ID: 4dab69c4-aa9e-46fa-81e0-0ea0b9974f07`).
- **Bouton de copie** : icône discrète permettant de copier l'UUID complet dans le presse-papiers en un clic avec confirmation SnackBar.

### D. Validation globale
- **Tests Flutter** : 50 tests passés avec succès (`flutter test`).
- **Analyse statique** : 0 avertissement, 0 erreur (`flutter analyze`).
- **Tests Rust** : 223 tests passés avec succès (`cargo test --features "lab-tools,ed2k-loopback"`).
- **Smoke tests post-packaging** : 100% validés sur `standard` et `qa-loopback`.

---

## 8. Phase 7.3 : Automatisation QA Loopback E2E & Améliorations de Confidentialité

### A. Confidentialité UI & Caviardage du Chemin de Données (`DiagnosticDialog`)
- **Caviardage automatique** : la méthode `_formatPathForDisplay` tronque les chemins de profils utilisateurs Windows (`C:\Users\…\Desktop\…\data`) et Unix (`/home/…/...`), évitant ainsi la divulgation involontaire du nom d'utilisateur lors des captures d'écran de bogue ou de QA.
- **Infobulle (`Tooltip`)** : le survol de la ligne `Path` révèle le chemin brut complet.
- **Bouton de copie inline** : un bouton compact `[Copy]` à droite de la ligne copie instantanément le chemin brut réel dans le presse-papiers avec notification visuelle (`Workdir path copied to clipboard`).

### B. Nouveau vecteur de frontière : `pattern_9728001` (9,28 Mo + 1 octet)
- **Rust (`mule-daemon`)** : ajout de `"pattern_9728001"` et `"part_9728001"` dans [`handle_lab_create_fixture`](file:///d:/Projet/eMule-Rust/crates/mule-daemon/src/rpc/methods.rs). Génère une fixture de 9 728 001 octets (exactement 1 part complète + 1 part résiduelle d'un octet) avec motif répétitif `(i % 256) as u8`.
- **UI (`LabGuideDialog`)** : puce dédiée `9.28 MB + 1 Byte (pattern_9728001)` à l'Étape 1 du guide.
- **Test Rust** : `test_lab_create_fixture_pattern_9728001` dans [`lab_rpc_tests.rs`](file:///d:/Projet/eMule-Rust/crates/mule-daemon/tests/lab_rpc_tests.rs).

### C. Pipeline d'automatisation QA de bout en bout (`scripts/test_qa_loopback_e2e.ps1`)
Un script automatisé complet qui valide sans aucune intervention humaine le bundle packagé réel :
1. **Extraction en sandbox stérile** : décompresse l'archive ZIP QA dans un répertoire temporaire isolé.
2. **Démarrage et confinement** : lance `mule-daemon.exe` en mode portable, valide l'exclusivité du socket `127.0.0.1` et l'annonce de `lab-tools`.
3. **Vecteur 1 — Transfert nominal (`sample_1mb`)** :
   - Création de la fixture, ajout du téléchargement, démarrage du pair loopback.
   - Connexion du téléchargement, transfert et transition vers `Completed` en ~170 ms.
   - Contrôle du fichier finalisé dans `<workdir>/downloads/` (taille et intégrité eD2K).
4. **Vecteur 2 — Frontière de part (`pattern_9728001`)** :
   - Création et transfert de la fixture de 9 728 001 octets.
   - Vérification de la complétion et du fichier finalisé.
5. **Vecteur 3 — Résilience, Crash brutal & Reprise (`part_9728000`)** :
   - Démarrage du transfert d'un chunk de 9,28 Mo.
   - Surveillance en temps réel jusqu'à ~50% (~4,6 Mo transférés).
   - Arrêt forcé brutal du daemon (`Stop-Process -Force`).
   - Vérification de la persistance du fichier `.part` et de l'état SQLite `mule.db`.
   - Relance du daemon packagé sur le même dossier de données : confirmation que la progression est conservée.
   - Reconnexion au pair loopback : reprise du transfert jusqu'à `Completed` (100%).
   - Validation du fichier finalisé.
6. **Sécurité du nettoyage (`lab.cleanupTestData`)** :
   - Exécution de `lab.cleanupTestData`.
   - Purge confirmée des fixtures et des fichiers `.part` temporaires.
   - **Contrôle critique anti-perte de données** : confirmation formelle que l'ensemble des 3 fichiers finalisés dans `downloads/` **sont intacts et n'ont jamais été supprimés**.
7. **Arrêt propre & intégrité** :
   - Arrêt contrôlé via `system.stop` et confirmation de zéro pollution du dépôt Git.

```
==================================================
  PHASE 7.3 AUTOMATED QA LOOPBACK E2E PASSED!     
==================================================
```

---

## 9. Séparation stricte de l'état interne (`data/`) et des téléchargements finaux (`downloads/`)

Pour garantir une séparation nette entre l'état d'exécution interne du daemon et les fichiers finaux visibles et manipulés par l'utilisateur, l'architecture sépare formellement `workdir` (`data/`) et `defaultDownloadDirectory` (`downloads/`) sur 4 paliers de priorité :

### A. Règle de résolution des répertoires (4 Paliers)

| Mode | Dossier interne `workdir` | Répertoire par défaut des téléchargements finaux |
|---|---|---|
| **1. CLI explicite** | Argument `--workdir <dir>` | Argument `--downloads-dir <dir>` ou `<workdir>/downloads` si omis |
| **2. Variables d'environnement** | `$EMULE_DATA_DIR` ou `$MULE_DATA_DIR` | `$EMULE_DOWNLOADS_DIR` ou `<workdir>/downloads` si omis |
| **3. Mode Portable (`portable.flag`)** | `<bundleRoot>/data` | `<bundleRoot>/downloads` (à côté du bundle mais distinct des binaires et de l'état interne) |
| **4. Mode Installé Standard** | `%LOCALAPPDATA%\eMuleRust` (Unix: `~/.local/share/eMuleRust`) | `%USERPROFILE%\Downloads\eMuleRust` (Unix: `~/Downloads/eMuleRust`) |

### B. Modifications Backend Rust (`mule-daemon`)
- **[`workdir.rs`](file:///d:/Projet/eMule-Rust/crates/mule-daemon/src/workdir.rs)** :
  - Introduction de la structure `ResolvedDirectories { workdir: ResolvedWorkDir, default_download_dir: PathBuf }`.
  - Implémentation de `WorkDirectoryResolver::resolve` et `resolve_with_context` appliquant les 4 paliers.
  - Tests unitaires complets dans `tests/workdir_tests.rs` (8 tests passés).
- **[`config.rs`](file:///d:/Projet/eMule-Rust/crates/mule-daemon/src/config.rs) & [`main.rs`](file:///d:/Projet/eMule-Rust/crates/mule-daemon/src/main.rs)** :
  - Support de l'argument CLI `--downloads-dir <path>`.
  - Intégration de `default_download_dir` dans `DaemonConfig`.
- **Méthodes RPC ([`methods.rs`](file:///d:/Projet/eMule-Rust/crates/mule-daemon/src/rpc/methods.rs))** :
  - `system.hello` et `system.status` annoncent désormais `"defaultDownloadDir"`.
  - `downloads.addEd2kLink` : si `targetDirectory` est omis, vide ou égal à `"."`, la cible est automatiquement résolue vers `state.config.default_download_dir`.
  - `lab.cleanupTestData` : ne touche jamais au répertoire `downloads/`.

### C. Modifications Interface Flutter (`apps/desktop`)
- **[`work_directory_resolver.dart`](file:///d:/Projet/eMule-Rust/apps/desktop/lib/services/work_directory_resolver.dart)** :
  - Alignement exact de la logique de résolution Dart avec la spécification Rust (4 paliers de priorité).
- **[`daemon_info.dart`](file:///d:/Projet/eMule-Rust/apps/desktop/lib/domain/models/daemon_info.dart) & DTOs** :
  - Réception et parsing du champ `defaultDownloadDir`.
- **Dialogue d'ajout de lien ([`AddLinkDialog`](file:///d:/Projet/eMule-Rust/apps/desktop/lib/ui/features/downloads/widgets/add_link_dialog.dart))** :
  - Pré-remplissage du champ dossier de destination avec le répertoire de téléchargement résolu par défaut.
  - Ajout du bouton natif `[ Browse... ]` via dialogue système PowerShell `FolderBrowserDialog` pour sélectionner confortablement un répertoire personnalisé.
- **Dialogue de diagnostic ([`DiagnosticDialog`](file:///d:/Projet/eMule-Rust/apps/desktop/lib/ui/features/downloads/widgets/diagnostic_dialog.dart))** :
  - Affichage dissocié de **« Internal Data »** (`.../data`) et de **« Downloads »** (`.../downloads`).
  - Boutons de copie individuels inline avec infobulles explicites.
- **Guide de laboratoire ([`LabGuideDialog`](file:///d:/Projet/eMule-Rust/apps/desktop/lib/ui/features/downloads/widgets/lab_guide_dialog.dart))** :
  - Carte récapitulative « Storage Overview » distinguant l'état interne (`data/`) et les fichiers complétés (`downloads/`), avec bouton direct « Open Folder ».
  - Affichage précis de la destination du fichier à l'Étape 4.
  - Mention explicite à l'Étape 5 que le nettoyage ne supprime pas les fichiers téléchargés dans `downloads/`.

### D. Validation des Scripts et Pipelines
- `scripts/test_packaged_bundle.ps1` : vérifie la présence et la validité de `defaultDownloadDir` dans `system.hello` et `system.status`.
- `scripts/test_qa_loopback_e2e.ps1` :
  - Valide le repli automatique vers `defaultDownloadDir` (Vecteur 1 et 3).
  - Valide la spécification d'un dossier personnalisé `targetDirectory` (Vecteur 2).
  - Contrôle formellement que `lab.cleanupTestData` ne supprime ni les fichiers de `default downloads/` ni ceux de `custom downloads/`.

```
==================================================
  RÉSULTATS DE VALIDATION COMPLÈTE
==================================================
-> Rust : 223 tests d'intégration passés (24/24 integration tests + unit tests)
-> Flutter : 69 tests passés (53 widgets/integration + 16 unit tests)
-> Flutter analyze : 0 issue
-> Smoke test QA Loopback : 100% réussi
-> Smoke test Standard : 100% réussi
-> QA Loopback E2E Pipeline (Phase 7.3) : 100% réussi (7/7 étapes)
```






