# Notes de Release — eMuleRust 0.1.2-lab

## 1. Vue d'Ensemble & Contexte

La version **0.1.2-lab** d'**eMuleRust** constitue le jalon de consolidation fonctionnelle et de certification qualité (QA) locale. Cette version transforme le projet en un produit QA portable, reproductible et vérifiable de bout en bout, de la génération de fixtures jusqu'au fichier finalisé, sans recourir à un terminal ni ouvrir de socket réseau sur l'extérieur.

> **Formulation d'attestation d'interopérabilité :**  
> *« eMuleRust est une application de laboratoire locale. La compatibilité eD2K est validée sur corpus statique et pair de référence émulé en loopback fermé. Aucun serveur eD2K public, nœud Kad ou peer externe n’est contacté. »*

---

## 2. Nouveautés & Évolutions Majeures

### 2.1 Séparation Étanche entre `data/` et `downloads/`
Afin de garantir une organisation propre et conforme aux attentes des utilisateurs et des environnements portables :
- **Dossier interne `data/`** : Réservé strictement à l'état local du démon et de l'interface :
  - Base de données SQLite WAL (`mule.db`, `mule.db-wal`, `mule.db-shm`).
  - Découverte et jeton de session (`session.json`, `auth.token`).
  - Verrou d'exclusivité d'instance (`mule.lock`).
  - Fichiers d'état temporaires et éparses en cours d'assemblage (`.part` et `.part.met`).
- **Dossier de destination `downloads/`** : Réservé exclusivement aux fichiers téléchargés complétés, vérifiés par empreinte cryptographique eD2K/AICH et finalisés (`target_path`).
- En mode portable, le dossier par défaut des téléchargements est automatiquement résolu vers `<bundleRoot>/downloads`, distinct de `<bundleRoot>/data`.

### 2.2 Sélection Personnalisée du Dossier de Destination (`Browse...`)
- L'interface Flutter Desktop propose désormais un sélecteur natif de dossier dans la boîte de dialogue d'ajout (`AddDownloadDialog`).
- L'utilisateur peut choisir n'importe quel dossier local valide, ou laisser le dossier `downloads/` par défaut.
- Le chemin sélectionné est validé, normalisé et transmis de manière sécurisée au démon via la méthode RPC `downloads.addEd2kLink`.

### 2.3 Persistance Définitive de la Suppression dans SQLite
- La suppression d'un téléchargement via l'interface utilisateur ou l'API JSON-RPC (`downloads.remove`) exécute désormais une instruction SQL `DELETE FROM downloads WHERE id = ?`.
- Les fichiers temporaires associés (`.part` et `.part.met`) sont immédiatement et physiquement supprimés du disque dans `data/`.
- La tâche supprimée ne réapparaît plus jamais après un redémarrage du démon (`test_downloads_remove_persistence_across_daemon_restart`).

### 2.4 Étanchéité du Nettoyage de Laboratoire (`lab.cleanupTestData`)
- Le mécanisme de purge des données de test supprime uniquement les fichiers de fixtures (`data/fixtures/`) et les fragments de téléchargements incomplets.
- **Règle absolue** : Les fichiers finalisés situés dans le dossier `downloads/` (ou tout dossier personnalisé) ne sont jamais touchés par le nettoyage de laboratoire.

### 2.5 Ergonomie de l'Interface & Diagnostic Sécurisé
- **Chips de capacités du démon** : L'en-tête de l'application affiche des puces visuelles (`Chip`) indiquant les capacités actives du démon connecté (`downloads`, `download-events`, `lab-tools`).
- **Abréviation des identifiants** : Les identifiants longs UUIDv4 des téléchargements sont abrégés sous forme ergonomique (ex. `#abc1...`) dans le tableau principal.
- **Diagnostic enrichi et sécurisé** : La boîte de dialogue `DiagnosticDialog` affiche la version du démon, les statuts de supervision, et caviarde rigoureusement les données sensibles (chemins système d'utilisateur et jetons d'authentification `auth.token`).

### 2.6 Phase 7.3 : Pipeline E2E QA Automatisé (`scripts/test_qa_loopback_e2e.ps1`)
Un harnais automatisé en PowerShell permet d'exécuter un cycle QA complet dans un bac à sable temporaire externe :
1. Extraction du bundle scellé.
2. Démarrage autonome en mode portable avec vérification du socket loopback (`127.0.0.1`).
3. Téléchargement nominal d'une fixture de 1 Mo via le pair loopback émulé.
4. Téléchargement d'une fixture à la frontière de part (9 728 001 octets = 1 part complète de 9 728 000 octets + résidu de 1 octet).
5. Scénario de crash/reprise : interruption forcée du démon en cours de téléchargement, redémarrage, reprise sans perte des blocs déjà reçus, et finalisation avec vérification d'intégrité SHA-256.

---

## 3. Guide pour les Testeurs Internes

Pour valider le parcours QA complet directement depuis l'application de laboratoire (build `qa-loopback`), suivre la séquence guidée suivante :

```
┌────────────────────────┐     ┌────────────────────────┐     ┌────────────────────────┐
│ 1. Generate Fixture    │ ──> │ 2. Add to Downloads    │ ──> │ 3. Start Loopback Peer │
└────────────────────────┘     └────────────────────────┘     └────────────────────────┘
                                                                           │
                                                                           ▼
┌────────────────────────┐     ┌────────────────────────┐     ┌────────────────────────┐
│ 6. Cleanup Test Data   │ <── │ 5. Verify Completed    │ <── │ 4. Connect to Peer     │
└────────────────────────┘     └────────────────────────┘     └────────────────────────┘
```

1. **Générer une fixture** (`Generate Fixture`) : Créer une fixture synthétique déterministe (ex. `sample_1mb` ou `pattern_9728001`).
2. **Ajouter au téléchargement** (`Add to Downloads`) : Injecter le lien eD2K généré dans la file d'attente (avec dossier `downloads/` par défaut ou sélection personnalisée).
3. **Démarrer le pair loopback** (`Start Loopback Peer`) : Activer le pair de référence émulé sur `127.0.0.1`.
4. **Connecter le téléchargement** (`Connect Download to Peer`) : Initier le transfert loopback.
5. **Vérifier la complétion** (`Verify Completed`) : Observer la transition vers l'état *Completed* et la présence du fichier intègre dans le dossier `downloads/`.
6. **Nettoyer les données de test** (`Cleanup Test Data`) : Purger les fixtures temporaires sans affecter les fichiers finalisés.

---

## 4. Préparation de la Phase Tier 3 (Binaire Externe Épinglé)

Le projet intègre l'infrastructure nécessaire pour exécuter le harnais Tier 3 dès qu'un binaire eMule/aMule externe est disponible :
- **Binaire identifié & vérifié** : Exécutable officiel répertorié.
- **Empreinte SHA-256 épinglée** : Contrôle cryptographique strict avant tout lancement.
- **Configuration hermétique** : Fichiers `.ini` et répertoires de test isolés dans un bac à sable temporaire.
- **Confinement loopback exclusif** : `127.0.0.1` uniquement.
- **Désactivation totale des réseaux tiers** : Kademlia (Kad) désactivé, connexion automatique désactivée, liste de serveurs eD2K vidée.
- **Exécution sur demande** : Le test live Tier 3 n'est jamais exécuté dans la suite normale et requiert un drapeau explicite.

---

## 5. Spécifications des Deux Archives de Release

### 5.1 Composition Invariante (22 Fichiers par Bundle Post-Signing)
Chaque bundle Windows x64 contient exactement **22 fichiers** contrôlés dans `SHA256SUMS.txt` et `version.json` :
1. `bin/mule-cli.exe`
2. `bin/mule-daemon.exe`
3. `data/app.so`
4. `data/flutter_assets/AssetManifest.bin`
5. `data/flutter_assets/FontManifest.json`
6. `data/flutter_assets/fonts/MaterialIcons-Regular.otf`
7. `data/flutter_assets/NativeAssetsManifest.json`
8. `data/flutter_assets/NOTICES.Z`
9. `data/flutter_assets/packages/cupertino_icons/assets/CupertinoIcons.ttf`
10. `data/flutter_assets/shaders/ink_sparkle.frag`
11. `data/flutter_assets/shaders/stretch_effect.frag`
12. `data/icudtl.dat`
13. `eMuleRust.exe`
14. `flutter_windows.dll`
15. `native_assets.json`
16. `portable.flag`
17. `sbom/flutter.cdx.json`
18. `sbom/license-audit.json`
19. `sbom/license-audit.md`
20. `sbom/rust.cdx.json`
21. `sbom/sbom.cdx.json`
22. `sbom/windows-bundle.cdx.json`

### 5.2 Détails des Distributions

#### A. Build Standard Locale
- **Dossier d'assemblage** : `dist/eMuleRust-Standard-Windows/`
- **Archive scellée** : `eMuleRust-v0.1.2-lab-standard-windows-x64.zip` (14 599 546 octets)
- **Empreinte SHA-256** : `f7efc509c577ffa62240a0798e6b41993f0a5883683cca39bbdaaeca7800a7cf`
- **Commit provenance** : `76005757e488523dfe4960da4569f7257cb80c8b`
- **Capacités RPC annoncées** : `["downloads", "download-events"]`
- **Routes `lab.*`** : Inactives (rejet avec `-32601 MethodNotFound`).
- **Texte de diffusion recommandé** :
  > **eMuleRust v0.1.2-lab — Standard**  
  > Build interne locale sans outils de laboratoire compilés. Elle inclut le démon local, l’interface Flutter Desktop, la séparation `data/` vs `downloads/`, la persistance SQLite et la gestion de téléchargements. Les routes de test, le pair loopback et les fonctions de simulation ne sont pas présents.

#### B. Build QA Loopback
- **Dossier d'assemblage** : `dist/eMuleRust-Qa-Loopback-Windows/`
- **Archive scellée** : `eMuleRust-v0.1.2-lab-qa-loopback-windows-x64.zip` (14 815 380 octets)
- **Empreinte SHA-256** : `05f27c1e08af97278d28444eb5be24e002c3c4b157098d054e4458769642c2cc`
- **Commit provenance** : `76005757e488523dfe4960da4569f7257cb80c8b`
- **Capacités RPC annoncées** : `["downloads", "download-events", "lab-tools"]`
- **Routes `lab.*`** : Actives (`lab.status`, `lab.createFixture`, `lab.startLoopbackPeer`, etc.).
- **Texte de diffusion recommandé** :
  > **eMuleRust v0.1.2-lab — QA Loopback**  
  > Build interne de validation incluant les outils de laboratoire `lab-tools`. Les échanges de test sont strictement limités à `127.0.0.1` ; aucun serveur eD2K public, nœud Kad ou peer externe n’est contacté. Utiliser pour les scénarios QA guidés et les tests E2E automatisés.

---

## 6. Commandes de Validation & Reproduction

Toutes les vérifications s'exécutent hors-ligne et sans élévation de privilèges :

```powershell
# 1. Tests unitaires et d'intégration Rust
cargo test --workspace
cargo test -p mule-daemon --features lab-tools

# 2. Analyse statique et tests Flutter Desktop
cd apps/desktop
flutter analyze
flutter test
cd ../..

# 3. Assemblage des bundles Windows (Standard & QA Loopback)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1 -Flavor standard
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1 -Flavor qa-loopback

# 4. Suites de validation post-packaging et matrice de robustesse
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_packaged_bundle.ps1 -Flavor standard
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_packaged_bundle.ps1 -Flavor qa-loopback
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_rc_matrix.ps1

# 5. Pipeline E2E QA automatisé (Phase 7.3)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_qa_loopback_e2e.ps1
```

---

## 7. Scoreboard Final de Validation

```
=== SCOREBOARD eMuleRust 0.1.2-LAB ===

Rust Crates :
  - mule-core           :  12 tests réussis
  - mule-format         :  19 tests réussis
  - mule-protocol       : 160 tests réussis (+ 1 test Tier 3 live ignoré par défaut)
  - mule-daemon         :  13 tests réussis (nominal) / 27 tests réussis (--features lab-tools)
Total Rust              : 204 tests (nominal) / 218 tests (lab-tools), 0 échec

Flutter Desktop         :  53 tests réussis, 0 échec

Matrice Post-Packaging  : 7/7 tests réussis (espaces/unicode, port conflict, verrou exclusif, crash recovery, relocalisation, read-only, portable auto-résolution).
Pipeline E2E QA         : 100% réussi (fixture nominale 1 Mo, frontière 9 728 001 octets, reprise après crash, conformité downloads/ vs data/).
Diagnostic Redaction    : 100% garanti (aucun auth.token ni clé privée dans le presse-papier).
Politique d'Exécution   : 100% RemoteSigned, 0% Bypass.
```
