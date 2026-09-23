# Notes de Release — eMule-Rust 0.1.1-lab

## 1. Vue d'Ensemble & Contexte

La version **0.1.1-lab** d'**eMule-Rust** constitue la Release Candidate (RC) consolidée et durcie pour les phases 7.1 (Mode Portable & Ergonomie Développeur) et 7.2 (Outils de Laboratoire Sécurisés & Expérimentation Locale).

Cette mise à jour apporte des garanties structurelles renforcées entre le produit nominal et les fonctionnalités de laboratoire, formalise la taxonomie d'erreurs JSON-RPC, interdit toute fuite de secret dans les outils de diagnostic, et établit une politique stricte d'exécution de scripts PowerShell (`-ExecutionPolicy RemoteSigned`).

---

## 2. Nouveautés & Évolutions Majeures

### 2.1 Résolution Hybride & Déterministe du Dossier de Travail (Workdir)
Une cascade unique de résolution a été formalisée et synchronisée de manière identique entre le démon Rust (`crates/mule-daemon/src/workdir.rs`) et l'application Flutter (`apps/desktop/lib/services/work_directory_resolver.dart`) :
1. Argument CLI `--workdir <chemin>` (priorité absolue : CI, diagnostic isolé, multi-instances).
2. Variable d'environnement `EMULE_SION_WORKDIR` (configuration développeur sans modifier les arguments).
3. Détection de `portable.flag` à la racine du bundle (mode autonome ZIP/clé USB/QA, dossier local `data/`).
4. Repli standard UAC Windows `%LOCALAPPDATA%\eMuleRust` (installation classique multi-utilisateurs).
*Rétrocompatibilité : l'ancien alias déprécié `MULE_DATA_DIR` est reconnu temporairement comme solution de repli avec avertissement de journalisation.*

### 2.2 Double Verrouillage des Capacités de Laboratoire
L'exposition des outils de test et de simulation locale dans l'interface utilisateur est conditionnée par un double verrou strict :
$$\text{isLabModeAvailable} = (\text{workdirMode} == \text{Portable}) \land (\text{capabilities}.\text{contains}(\text{"lab-tools"}))$$
- **Build Standard (Production)** : `default = []`. Le démon ne compile aucun code de simulation (`lab-loopback`, `ed2k-loopback`, `debug-rpc`), n'annonce que `["downloads", "download-events"]`, et rejette tout appel RPC `lab.*` avec le code `-32601` (`MethodNotFound`).
- **Build QA / Laboratoire (`-LabMode`)** : Compile la feature `lab-tools`, annonce `["downloads", "download-events", "lab-tools"]`.
- **Interface Flutter** : Le guide de test, l'assistant d'injection de liens eD2K synthétiques et les boutons de nettoyage sont strictement masqués en mode installé (`InstalledDefault`), même si un démon expérimental était accidentellement connecté.

### 2.3 Taxonomie Formelle des Erreurs JSON-RPC
Unification et codification des codes d'erreur selon la spécification JSON-RPC 2.0 et extension métier isolée :
- Codes standard : `-32700` (ParseError), `-32600` (InvalidRequest), `-32601` (MethodNotFound), `-32602` (InvalidParams), `-32603` (InternalError).
- Codes applicatifs eMule-Rust :
  - `-32000` : `CODE_AUTH_FAILED` — Authentification WebSocket invalide ou absente.
  - `-32001` : `CODE_PROTOCOL_MISMATCH` — Version protocolaire incompatible.
  - `-32002` : `CODE_FORBIDDEN_IN_INSTALLED_MODE` — Tentative d'exécution d'une commande lab en mode installé.
  - `-32003` : `CODE_UNSAFE_CLEANUP_TARGET` — Tentative de suppression hors du dossier de test autorisé ou cible avec traversée de chemin / lien symbolique.
  - `-32004` : `CODE_FIXTURE_DOWNLOAD_MISMATCH` — Téléchargement non éligible aux fixtures de test.
  - `-32005` : `CODE_LOOPBACK_PEER_UNAVAILABLE` — Pair de test loopback indisponible.
  - `-32010` : `CODE_BUSINESS_ERROR` — Erreur métier générique.
- **Sécurisation du champ `data`** : Le champ `data` des erreurs est toujours un objet structuré et assaini (`{"reason": "...", ...}`), excluant formellement tout token, secret, chemin d'utilisateur ou clé cryptographique.

### 2.4 Garantie Zéro Fuite de Secret dans les Outils de Diagnostic
- La fonction « Diagnostic » de l'interface utilisateur (`DiagnosticDialog`) produit des rapports complets sanitaires pour le débogage local.
- L'empreinte secrète `auth.token`, les jetons d'autorisation et les chemins contenant des noms d'utilisateurs système sont systématiquement caviardés (`[REDACTED]`).
- Un test unitaire automatisé (`test/ui/diagnostic_dialog_test.dart`) valide formellement qu'aucune valeur de token ni occurrence de `auth.token` n'apparaît dans le presse-papier copié.

### 2.5 Politique de Scripting PowerShell Durcie
- Bannissement total et inconditionnel de l'option `-ExecutionPolicy Bypass` dans toute la chaîne CI, de build, de test et de documentation.
- Adoption exclusive de `-ExecutionPolicy RemoteSigned` pour l'ensemble des scripts du dépôt (`scripts/build_windows_release.ps1`, `scripts/test_packaged_bundle.ps1`, `scripts/test_rc_matrix.ps1`, `scripts/generate_sbom.ps1`).

---

## 3. Livrables Distincts & Transparence de l'Inventaire (22 Fichiers par Bundle)

Afin d'éliminer toute ambiguïté de provenance et d'éviter l'écrasement mutuel des artefacts, le processus de build produit deux distributions scellées distinctes dans des répertoires et archives dédiés.

### 3.1 Composition Invariante (22 Artefacts Post-Signing)
Chaque bundle Windows x64 contient exactement **22 artefacts** référencés dans son propre fichier `SHA256SUMS.txt` et manifeste `version.json` :
1. `bin/mule-cli.exe`
2. `bin/mule-daemon.exe`
3. `data/app.so`
4. `data/flutter_assets/AssetManifest.bin` (remplace le format JSON redondant)
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
15. `native_assets.json` (résolution des assets natifs Flutter)
16. `portable.flag`
17. `sbom/flutter.cdx.json`
18. `sbom/license-audit.json`
19. `sbom/license-audit.md`
20. `sbom/rust.cdx.json`
21. `sbom/sbom.cdx.json`
22. `sbom/windows-bundle.cdx.json`

### 3.2 Spécifications des Deux Archives de Release

#### A. Build Standard Locale
- **Dossier d'assemblage** : `dist/eMuleRust-Standard-Windows/`
- **Archive scellée** : `eMuleRust-v0.1.1-lab-standard-windows-x64.zip` (14 488 636 octets)
- **Empreinte SHA-256** : `381239d6d94c3d74e00fe0d737644574d4083e1a1dbcfa1bc80b1df9e0f79f81`
- **Manifeste `version.json`** :
  ```json
  {
    "distributionFlavor": "standard",
    "labToolsEnabled": false
  }
  ```
- **Capacités RPC annoncées** : `["downloads", "download-events"]`
- **Routes `lab.*`** : Absentes (rejet avec `-32601 MethodNotFound`).
- **Texte de diffusion recommandé** :
  > **eMule-Rust v0.1.1-lab — Standard**  
  > Build interne locale sans outils de laboratoire compilés. Elle inclut le daemon local, l’interface Flutter Desktop, la persistance et les fonctions de gestion de téléchargements. Les routes de test, le pair loopback et les fonctions de simulation ne sont pas présents.

#### B. Build QA Loopback
- **Dossier d'assemblage** : `dist/eMuleRust-Qa-Loopback-Windows/`
- **Archive scellée** : `eMuleRust-v0.1.1-lab-qa-loopback-windows-x64.zip` (14 702 418 octets)
- **Empreinte SHA-256** : `2360a460bb4fdd179bb9c43a5e4f0ca0a552492fead22153fc3e12e81e90ed59`
- **Manifeste `version.json`** :
  ```json
  {
    "distributionFlavor": "qa-loopback",
    "labToolsEnabled": true
  }
  ```
- **Capacités RPC annoncées** : `["downloads", "download-events", "lab-tools"]`
- **Routes `lab.*`** : Actives (`lab.status`, `lab.createFixture`, etc.).
- **Texte de diffusion recommandé** :
  > **eMule-Rust v0.1.1-lab — QA Loopback**  
  > Build interne de validation incluant les outils de laboratoire `lab-tools`. Les échanges de test sont strictement limités à `127.0.0.1` ; aucun serveur eD2K public, nœud Kad ou peer externe n’est contacté. Utiliser uniquement pour les scénarios QA guidés.

---

## 4. Attestation d'Interopérabilité et Engagements Hermétiques

L'attestation formelle d'eMule-Rust reste rigoureusement inchangée :

> **Formulation d'attestation obligatoire :**
> *« Compatibilité eD2K expérimentale validée sur corpus statique et pair de référence émulé, en environnement loopback fermé. Le harnais Tier 3 pour un binaire eMule/aMule externe épinglé est disponible, mais son test live reste ignoré par défaut tant qu’un binaire vérifié n’est pas fourni. »*

Les éléments volontairement absents restent strictement maintenus :
- **Zéro trafic WAN / Internet** : Écoutes et connexions restreintes à `127.0.0.1` (loopback strict).
- **Zéro protocole Kad / DHT** : Aucune routine de routage distribué.
- **Zéro serveur central eD2K** : Aucun contact avec des serveurs publics.
- **Zéro upload / seeding** : Aucune diffusion spontanée de blocs vers l'extérieur.

---

## 5. Commandes de Validation & Reproduction

Toutes les vérifications s'exécutent hors-ligne et sans élévation de privilèges :

```powershell
# 1. Tests unitaires et d'intégration Rust (avec et sans lab-tools)
cargo test --workspace
cargo test -p mule-daemon --features lab-tools

# 2. Analyse statique et tests Flutter Desktop
cd apps/desktop
flutter analyze
flutter test
cd ../..

# 3. Génération du bundle standard scellé (arbre propre requis)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1 -Flavor standard

# 4. Génération du bundle QA expérimental (avec outils de laboratoire)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1 -Flavor qa-loopback

# 5. Tests automatisés post-packaging (par flavor) et matrice de robustesse
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_packaged_bundle.ps1 -Flavor standard
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_packaged_bundle.ps1 -Flavor qa-loopback
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_rc_matrix.ps1
```

---

## 6. Scoreboard Final de Validation

```
=== SCOREBOARD eMule-Rust 0.1.1-LAB ===

Rust Crates :
  - mule-core           :  12 tests réussis
  - mule-format         :  19 tests réussis
  - mule-protocol       : 160 tests réussis (+ 1 test Tier 3 live ignoré par défaut)
  - mule-daemon         :  13 tests réussis (nominal) / 24 tests réussis (--features lab-tools)
Total Rust              : 204 tests (nominal) / 215 tests (lab-tools), 0 échec

Flutter Desktop         :  49 tests réussis (+18 nouveaux tests couverture lab & diagnostic), 0 échec

Matrice Post-Packaging  : 7/7 tests réussis (espaces/unicode, port conflict, verrou exclusif, crash recovery, relocalisation, read-only, portable auto-résolution).
Diagnostic Redaction    : 100% garanti (aucun auth.token ni clé privée dans le presse-papier).
Politique d'Exécution   : 100% RemoteSigned, 0% Bypass.
```
