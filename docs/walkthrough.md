# Guide d'Exécution, de Validation et de Dépannage — Phase 3C : Durcissement Produit

Ce document constitue le guide technique de référence pour la **Phase 3C** du projet **Emule-Sion** (Durcissement produit & préparation à la production locale). Il détaille l'architecture de supervision, la gestion des sessions locales, les verrous d'exclusivité, la chaîne de packaging Windows sécurisée, les suites de tests et le guide complet de dépannage.

L'ensemble des procédures et démonstrations présentées ici est **100 % déterministe, reproductible et exécutable hors-ligne**, sans solliciter de serveurs eD2K ou nœuds Kad publics.

---

## 1. Architecture & Nouveautés de la Phase 3C

La Phase 3C consolide l'intégration entre le démon Rust (`mule-daemon`) et l'interface utilisateur Flutter Desktop (`apps/desktop`), assurant un cycle de vie robuste et sécurisé :

```
┌────────────────────────────────────────────────────────┐
│                   apps/desktop (Flutter)              │
│  ┌──────────────────────────────────────────────────┐  │
│  │                 DaemonSupervisor                 │  │
│  │  - Découverte via session.json + auth.token      │  │
│  │  - Auto-démarrage mule-daemon.exe si absent     │  │
│  │  - Résolution de course (2 UI simultanées)       │  │
│  │  - Reconnexion automatique (backoff exp)        │  │
│  │  - Détection d'incompatibilité protocolaire      │  │
│  └───────────▲───────────────────────────┬──────────┘  │
│              │ session.json              │ WS JSON-RPC │
└──────────────┼───────────────────────────┼─────────────┘
               │                           │
┌──────────────┼───────────────────────────▼─────────────┐
│  %APPDATA%/EmuleSion (Répertoire de travail local)     │
│  ├── mule.lock          (Verrou exclusif fs4)          │
│  ├── session.json       (Métadonnées d'instance publiques)│
│  ├── auth.token         (Token secret isolé, ACL local)│
│  ├── mule.db / -wal     (Persistance SQLite WAL)       │
│  └── part/              (Fichiers temporaires .part)   │
└──────────────▲─────────────────────────────────────────┘
               │
┌──────────────┴─────────────────────────────────────────┐
│               crates/mule-daemon (Rust)                │
│  - Verrou exclusif de dossier de travail (mule.lock)   │
│  - Repli dynamique sur port éphémère si 4660 occupé    │
│  - Écriture atomique (session.json.tmp -> sync -> ren) │
│  - Rejet des liens symboliques sur auth.token          │
│  - Nettoyage ordonné des sessions à l'arrêt (Ctrl+C)  │
└────────────────────────────────────────────────────────┘
```

### Garanties techniques validées
1. **Verrou exclusif d'instance (`crates/mule-daemon/src/lock.rs`)** :
   - Protégé par l'API `fs4::FileExt::try_lock()` sur `mule.lock`.
   - Empêche strictement deux démons d'ouvrir simultanément la même base SQLite ou les mêmes fichiers `.part`.
   - Si une seconde instance tente de démarrer, elle s'arrête immédiatement avec l'erreur `DaemonError::AlreadyRunning`.

2. **Découverte de session atomique et durabilité Windows (`crates/mule-daemon/src/session.rs`)** :
   - `session.json` contient : `protocolVersion`, `instanceId`, `pid`, `endpoint`, `startedAtUnixMs`.
   - Écriture atomique avec `sync_all()` et boucle de réessais (retries avec backoff de 20ms) lors du remplacement sous Windows pour éliminer tout conflit avec des verrous de partage temporaires (indexeur de fichiers, antivirus, lecteur concurrent).
   - Côté Flutter, lecture tolérante aux écritures en cours : un fichier vide ou en cours de remplacement `.tmp` déclenche des réessais courts au lieu d'interpréter hâtivement l'absence du démon.
   - Suppression propre de `session.json` lors de l'arrêt gracieux du démon.

3. **Sécurité et isolation stricte de `auth.token`** :
   - **Non-exposition** : Le secret d'authentification n'est **jamais** placé dans `session.json` ni affiché dans les flux `stdout` du démon (`(redacted)`).
   - **Protection contre les liens** : Rejet formel des liens symboliques ou reparse points Windows (`symlink_metadata`).
   - **Jeton à haute entropie CSPRNG** : Généré directement à partir de 32 octets aléatoires issus du générateur cryptographique du système d'exploitation (`rand::rng().fill_bytes(&mut bytes)`), encodé sous la forme stricte de 64 caractères hexadécimaux (`AUTH_TOKEN_HEX_LEN = 64`). Tout token non conforme (taille != 64 ou caractères non hexadécimaux) est immédiatement rejeté par Rust et Flutter.
   - **Zéro fuite côté client** : Le client Flutter et son superviseur ne journalisent jamais le token dans les messages d'erreur ou traces d'exceptions.

4. **Superviseur Flutter résilient (`apps/desktop/lib/services/daemon_supervisor.dart`)** :
   - **Priorité aux binaires packagés** : Recherche de `mule-daemon.exe` aux côtés de l'exécutable Flutter (`Platform.resolvedExecutable`), dans `bin/`, puis dans les cibles de développement.
   - **Résolution de la course au démarrage** : Si deux clients Flutter sont lancés simultanément sur un dossier vierge, l'un lance le démon ; le second processus démon sort avec `AlreadyRunning`, et la seconde UI Flutter détecte l'apparition du `session.json` créé par le premier et s'y connecte de manière transparente.
   - **Récupération des sessions obsolètes avec garde anti-course atomique** : Si `session.json` pointe vers un démon inaccessible (port fermé), Flutter effectue une relecture de confirmation et compare le contenu avant toute suppression pour éviter d'écraser une session valide concurrente, puis démarre une nouvelle instance.
   - **Reconnexion avec backoff exponentiel** : En cas de coupure socket inopinée, le superviseur tente de se reconnecter (500ms, 1s, 2s, 4s... plafonné à 10s).
   - **Détection des versions incompatibles** : Affichage d'une bannière bloquante (`IncompatibleVersionBanner`) si `protocolVersion` diffère de la version 1 supportée.
   - **Politique de cycle de vie et supervision** : Si l'UI Flutter a elle-même instancié le démon enfant, elle l'arrête proprement lors de sa fermeture (`stop()`). À l'inverse, si elle s'est connectée à un démon autonome existant (service local ou lancé via la CLI), ce dernier continue de vivre indépendamment après la fermeture de l'UI.

5. **Pipeline de build et packaging sécurisé (`scripts/build_windows_release.ps1`)** :
   - Conforme à la politique d'exécution PowerShell sans recours à `-ExecutionPolicy Bypass`.
   - Assemble le bundle autonome dans `dist/EmuleSion-Windows/` avec le manifeste de traçabilité `version.json`.

---

## 2. Prérequis & Environnement

| Composant | Version minimale recommandée | Remarque |
| :--- | :--- | :--- |
| **Système d'exploitation** | Windows 10/11 (x64) | Support des sockets TCP locaux, verrous `fs4` et desktop Flutter |
| **Rust & Cargo** | 1.80.0+ (stable) | Rust 1.98.1 validé avec `clippy` et `rustfmt` |
| **Flutter SDK** | 3.24.0+ (stable) | Flutter 3.47.2 validé avec support bureau activé (`flutter config --enable-windows-desktop`) |
| **Visual Studio C++** | Build Tools 2022 | Composants desktop C++ requis par Flutter Windows |

Vérifier les prérequis en ligne de commande :
```powershell
rustc --version
cargo --version
flutter --version
flutter doctor
```

---

## 3. Commandes de Vérification Qualité (100% Offline)

Toutes les commandes ci-dessous doivent s'exécuter avec succès :

### 3.1 Workspace Rust (Format, Clippy, Tests)
```powershell
# 1. Vérification du formatage du code
& "$env:USERPROFILE\.cargo\bin\cargo.exe" fmt --check

# 2. Analyse statique stricte (zéro warning toléré)
& "$env:USERPROFILE\.cargo\bin\cargo.exe" clippy --workspace -- -D warnings

# 3. Exécution complète des 69 tests d'intégration et unitaires Rust (avec support lab)
& "$env:USERPROFILE\.cargo\bin\cargo.exe" test --workspace --all-features
```

*Résultats validés :*
- `mule-format` : 19 tests passés (AICH, eD2K hashing, liens parsing/normalisation, .met parsing & offsets).
- `mule-core` : 9 tests passés (RangeSet, insertion, fusion, PartFile persistance, restauration SQLite `get_all_downloads` & corruption recovery).
- `mule-daemon` : 7 tests passés (Verrouillage exclusif, session atomique avec retries & sync_all, validation token, port de repli éphémère, contrat JSON-RPC, transfert inter-démons loopback avec crash à 50% et reprise intègre).
- `mule-protocol` : 34 tests passés (Framing eD2K zéro unsafe, fragmentation octet par octet, limites strictes, paquets malformés, machine à états de lab, tracker in-flight, indexation des hashes hébergés, et transfert TCP loopback avec blocs désordonnés).

### 3.2 Client Flutter Desktop (Analyse & Tests)
```powershell
cd d:\Projet\Emule-Sion\apps\desktop

# 1. Analyse statique Flutter
& "C:\Users\Lou\flutter\bin\flutter.bat" analyze

# 2. Exécution des 31 tests Flutter
& "C:\Users\Lou\flutter\bin\flutter.bat" test
```

*Résultats validés :*
- `No issues found!`
- `All tests passed! (31/31 passed)` comprenant :
  - Tests unitaires de modèles, DTOs et formatters.
  - Tests de contrats WebSocket JSON-RPC et streaming d'événements.
  - Tests de widgets, virtualisation de liste (500 items sans lag), filtrage et dialogue d'ajout de liens.
  - Tests du superviseur : auto-start, réutilisation de session, gestion de course simultanée, session tronquée, session obsolète, refus de version incompatible et gestion d'erreur de token sans fuite de secret.
  - Test E2E complet contre le vrai binaire `mule-daemon.exe`.

---

## 4. Modes de Lancement en Développement

### Option A : Mode Automatique / Production Locale (Recommandé)
Dans ce mode, Flutter supervise directement le démon :
```powershell
cd d:\Projet\Emule-Sion\apps\desktop
flutter run -d windows
```
- Flutter découvre `%APPDATA%\EmuleSion\session.json`.
- Si aucune instance n'est trouvée, il démarre automatiquement `mule-daemon.exe` en arrière-plan.
- La connexion WebSocket JSON-RPC 2.0 s'établit et la pastille de statut passe au vert (`Connecté`).

### Option B : Démon Rust Autonome + Flutter Développeur
Idéal pour observer les logs bruts du démon en temps réel :

**Terminal 1 (Démon Rust) :**
```powershell
cd d:\Projet\Emule-Sion
cargo run -p mule-daemon -- --workdir "$env:APPDATA\EmuleSion"
```
*Le démon affiche son PID, l'acquisition du verrou `mule.lock`, son port d'écoute et la création de `session.json`.*

**Terminal 2 (Application Flutter) :**
```powershell
cd d:\Projet\Emule-Sion\apps\desktop
flutter run -d windows
```
*Flutter détecte la session existante et s'y connecte immédiatement.*

### Option C : Mode Maquette Isolée (Fake Client)
Pour itérer rapidement sur l'interface graphique sans démon backend :
```powershell
cd d:\Projet\Emule-Sion\apps\desktop
flutter run -d windows --dart-define=USE_FAKE_DAEMON=true
```

---

## 5. Flux de Démonstration Pas-à-Pas (100% Hors-Ligne)

### Étape 1 : Vérification de l'interface et du statut de connexion
1. Lancer l'application (Mode A ou B).
2. Dans la barre latérale gauche, observer l'indicateur de statut : **Point vert « Connecté »**.
3. Le sous-titre affiche uniquement le point de terminaison public local (ex. `127.0.0.1:4660` ou port éphémère attribué) et la version du protocole `v1`. Aucun chemin de fichier sensible ni jeton secret n'est affiché.

### Étape 2 : Ajout d'un lien eD2K local (Données de test simulées)
1. Cliquer sur le bouton **« + Ajouter un lien eD2K »** dans la barre supérieure.
2. Coller un lien eD2K de test syntaxiquement valide :
   `ed2k://|file|TestVideo_Offline_Demo.mkv|25000000|0123456789abcdef0123456789abcdef|/`
   > [!NOTE]
   > Cet exemple constitue une **URI syntaxiquement valide avec des données simulées**. La taille (23.84 MB) et le hash MD4 démontrent la validation locale de l'intégrité et du formatage sans présumer de la présence d'un pair réel sur un réseau distant.
3. Observer la validation dynamique : le nom, la taille et le hash MD4 s'affichent instantanément en vert.
4. Cliquer sur **« Ajouter le téléchargement »**.
5. Le téléchargement apparaît dans la liste avec le statut initial exact de l'enum RPC/UI : **`Queued`** (En attente).

### Étape 3 : Injection de données simulées
1. Cliquer sur le bouton **« Outils de simulation »** dans l'en-tête.
2. Sélectionner le téléchargement dans la liste.
3. Cliquer sur **« Injecter 10 % »** ou **« Injecter bloc de part »** :
   - La barre de progression avance en temps réel.
   - Le statut passe à `Downloading`.
   - Le débit de transfert et le temps restant (ETA) sont calculés de manière réactive.

### Étape 4 : Contrôle de flux (Pause / Reprise / Suppression)
1. Cliquer sur le bouton **Pause** : le statut passe à `Paused`, le débit tombe à 0 Ko/s.
2. Cliquer sur le bouton **Reprendre** : le statut repasse à `Downloading` et reprend à partir des octets persistés.
3. Cliquer sur **Supprimer** : confirmation modale, puis nettoyage immédiat des fichiers `.part` et des métadonnées SQLite.

### Étape 5 : Tests de Résilience et Crashs Indépendants

Pour valider l'indépendance et la robustesse des composants, trois scénarios de crash doivent être vérifiés séparément :

#### Cas A : UI tuée ou Démon autonome, Démon persistant
1. Lancer le démon en arrière-plan (Option B ou démon déjà actif) ou lancer l'application Flutter et injecter des données de test.
2. Fermer brutalement l'interface Flutter (`Alt+F4` ou via le Gestionnaire des tâches sans déclencher `stop()`), ou fermer une UI connectée à un démon autonome préexistant.
3. Constater que `mule-daemon.exe` continue de s'exécuter en arrière-plan et conserve son verrou exclusif `mule.lock`.
4. Relancer l'application Flutter : elle découvre immédiatement `session.json`, vérifie la validité du port et du token 64-hex, s'y rattache sans relancer de second démon et affiche l'état intact.
*(Note : lorsque l'UI est fermée normalement alors qu'elle supervisait elle-même le démon enfant, elle termine son processus enfant pour éviter les processus orphelins).*

#### Cas B : Démon tué, UI vivante
1. Alors que l'application Flutter est ouverte et connectée, tuer le processus démon en console :
   ```powershell
   Get-Process mule-daemon -ErrorAction SilentlyContinue | Stop-Process -Force
   ```
2. Observer l'interface Flutter :
   - La pastille passe instantanément à l'état `Reconnecting...`.
   - Le superviseur applique le backoff exponentiel sans saturer le processeur.
3. Dès que le démon est redémarré (ou auto-relancé par le superviseur) :
   - La connexion est rétablie automatiquement.
   - Le client détecte le nouvel `instanceId`, réinitialise son compteur de séquence et demande un snapshot complet (`downloads.list`), actualisant l'écran sans intervention manuelle.

#### Cas C : Crash total / Arrêt à froid
1. Tuer simultanément Flutter et le démon (`Ctrl+C` ou fermeture de session).
2. Relancer l'application Flutter :
   - Le démon démarre, ouvre `mule.db` (SQLite WAL), restaure les intervalles dans `RangeSet`, et l'UI affiche fidèlement l'avancement persisté.

### Étape 6 : Course au démarrage de deux UI simultanées
1. Sur un répertoire de travail vierge sans démon actif, lancer simultanément deux fenêtres Flutter.
2. Constater qu'un seul `mule-daemon.exe` démarre et acquiert le verrou `mule.lock`.
3. Le second superviseur attend que `session.json` soit écrit et s'y connecte sans conflit ni message d'erreur.

---

## 6. Guide de Dépannage (Troubleshooting)

### Problème 1 : Message « Une autre instance du démon est déjà active sur ce répertoire »
- **Cause** : Un démon précédent retient le verrou exclusif `mule.lock`.
- **Solution** :
   ```powershell
   Get-Process mule-daemon -ErrorAction SilentlyContinue | Stop-Process -Force
   ```
   Le verrou se libère dès l'arrêt du processus.

### Problème 2 : Conflit sur le port réseau par défaut (`127.0.0.1:4660`)
- **Comportement automatique** : Le démon bascule automatiquement sur un port éphémère (`127.0.0.1:0`) et publie l'adresse effective dans `session.json`. L'application Flutter lit dynamiquement ce point d'accès.

### Problème 3 : Erreur d'authentification (`Authentication Failed`)
- **Cause** : Désynchronisation de `auth.token`.
- **Solution** :
   1. Arrêter le démon.
   2. Supprimer les fichiers de session :
      ```powershell
      Remove-Item "$env:APPDATA\EmuleSion\session.json" -ErrorAction SilentlyContinue
      Remove-Item "$env:APPDATA\EmuleSion\auth.token" -ErrorAction SilentlyContinue
      ```
   3. Relancer l'application.

### Problème 4 : Bannière rouge « Version du démon incompatible »
- **Cause** : `protocolVersion` incompatible entre le client et le démon.
- **Solution** : Recompiler le démon pour synchroniser les versions :
   ```powershell
   cargo build --release -p mule-daemon
   ```

### Problème 5 : Réinitialisation complète pour repartir d'un environnement propre
> [!CAUTION]
> Cette commande supprime irréversiblement l'ensemble des téléchargements en cours, la base SQLite locale et les fichiers temporaires `.part`. À n'utiliser qu'à des fins de réinitialisation de tests de laboratoire.

```powershell
# Confirmer la fermeture des processus
Get-Process mule-daemon,desktop,EmuleSion -ErrorAction SilentlyContinue | Stop-Process -Force

# Supprimer le répertoire de travail
Remove-Item -Recurse -Force "$env:APPDATA\EmuleSion"
```

---

## 7. Packaging et Déploiement de Release Windows

Pour générer le livrable complet sans contourner la politique de sécurité PowerShell du système :

```powershell
cd d:\Projet\Emule-Sion
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\build_windows_release.ps1
```

> [!TIP]
> **Pourquoi `RemoteSigned` plutôt que `Bypass` ?**
> `RemoteSigned` autorise l'exécution des scripts créés localement sur votre machine tout en exigeant une signature numérique de confiance pour les scripts téléchargés depuis Internet. Pour une distribution grand public, le script et les exécutables peuvent être signés à l'aide d'un certificat de signature de code Authenticode (`Set-AuthenticodeSignature`).

### Contenu du bundle généré dans `dist\EmuleSion-Windows\` :
- `EmuleSion.exe` : Exécutable Flutter Desktop principal.
- `bin/mule-daemon.exe` : Démon Core autonome packagé aux côtés de l'UI.
- `flutter_windows.dll` et dépendances natives.
- `data/` : Assets, icônes et polices de l'application.
- `version.json` : Manifeste de traçabilité incluant le SHA Git, la date de compilation et la version du protocole JSON-RPC.

---

## 8. Phase 4.1 : Socle Protocolaire eD2K Réseau (`crates/mule-protocol`)

Le composant `mule-protocol` constitue la fondation réseau du transfert pair-à-pair eD2K, garantissant une étanchéité absolue vis-à-vis du stockage (`mule-core`) et une robustesse totale face aux flux malveillants ou fragmentés.

### 8.1 Garanties architecturales et sécurité
1. **Zéro `unsafe` (`#![forbid(unsafe_code)]`)** :
   Garantie au niveau de la compilation qu'aucun bloc mémoire non vérifié n'est manipulé.
2. **Découplage strict du Core** :
   Le protocole ignore l'existence de SQLite, des fichiers `.part` ou des structures internes (`RangeSet`). Toutes les interactions passent par des traits asynchrones purs (`DownloadSink` et `DownloadPlanner`).
3. **Plafonds stricts anti-déni de service (DoS)** :
   - `MAX_FRAME_BYTES = 1_048_576` (1 MiB) : rejet immédiat avant toute tentative d'allocation de buffers démesurés.
   - `MAX_BUFFERED_BYTES = 2_097_152` (2 MiB) : coupure préventive si un pair accumule des données partielles sans compléter de trame valide.
   - `MAX_STRING_BYTES = 16_384` (16 KiB) : protection contre les chaînes de tags surdimensionnées.
   - `MAX_TAGS_PER_MESSAGE = 256` : limitation du nombre de métadonnées par message.
   - `MAX_PEERS_PER_RESPONSE = 500` : limitation des réponses de sources de pairs.
4. **Codec de streaming `PacketCodec` & reconnaissance de protocoles** :
   - Décodage à coût zéro sur `bytes::BytesMut`.
   - Support natif des protocoles eD2K standard (`0xE3`), eMule étendu (`0xC5`), de laboratoire (`0xEE`), et framing reconnu pour le protocole compressé eMule (`0xD4`).
   - *Précision de sécurité sur 0xD4* : la décompression compatible et ses limites de sécurité post-décompression (ratio de décompression maximal plafonné contre les bombes de décompression, plafond par type de message et rejet strict de flux incomplets) seront intégrées dans une phase dédiée.
   - Trait d'abstraction `PacketLimits` prévoyant des plafonds contextuels par famille de messages.
   - Reconstitution déterministe lors d'une fragmentation extrême (validation octet par octet dans les tests).
   - Découpage ordonné de trames multiples contiguës au sein d'un même tampon réseau.
5. **Couverture de tests exhaustifs (33/33 validés dans `mule-protocol`)** :
   - `tests/packet_codec.rs` : Aller-retour standard, extraction multi-paquets, trames à charge utile vide, blocs eD2K de 180 KiB (`184_320` octets), fragmentation unitaire.
   - `tests/protocol_limits.rs` : Rejet des longueurs nulles, dépassement de `MAX_FRAME_BYTES`, attaques par `u32::MAX`, débordement d'accumulateur `BufferOverflow`, respect des limites personnalisées.
   - `tests/malformed_packets.rs` : Rejet des octets de protocole inconnus (`0x00`, `0x42`, `0xFF`), gestion des en-têtes tronqués sans panique (`Ok(None)`), intégration asynchrone des traits `DownloadSink` et `DownloadPlanner`.
   - `tests/lab_state_machine.rs` : Handshake nominal client/serveur, rejet de version incompatible, rejet de nonce mismatch, rejet du double Hello, interdiction formelle des transferts avant l'état Ready, fermeture propre par Goodbye, sérialisation déterministe de tous les messages `LabMessage`.
   - `tests/lab_in_flight_and_blocks.rs` : Plafond strict des 8 requêtes en vol (`MAX_IN_FLIGHT_REQUESTS`), rejet des doublons, rejet des blocs non sollicités, rejet des discordances de longueur, service sécurisé en mémoire par `LabSourceEngine`.
   - `tests/loopback_transfer.rs` : Transfert TCP loopback complet (53 blocs d'une part eD2K de 9.5 MB) avec livraison intentionnellement dans le désordre, fragmentation TCP unitaire du handshake, détection de corruption de bloc empêchant la finalisation, et reprise après déconnexion inopinée.

---

## 9. Phase 4.2 : Machine à États P2P et Transfert Loopback Inter-Démons

La Phase 4.2 met en place un protocole de banc d'essai local, isolé et déterministe, garantissant qu'aucun socket ne dialogue avec le réseau eD2K public tant que l'intégrité et la résilience locale ne sont pas irréprochables.

### 9.1 Distinction Fondamentale
```
lab / loopback wire protocol (PR_LAB = 0xEE) ≠ eD2K / eMule historical wire protocol
```
- Le protocole de laboratoire opère sous le magic byte `PR_LAB = 0xEE`.
- Il utilise des messages typés explicites (`Hello`, `HelloAck`, `FileOffer`, `RequestBlock`, `BlockData`, `BlockRejected`, `Goodbye`, `ProtocolError`).
- Aucun chemin de fichier local (`PathBuf`) n'est jamais accepté depuis le réseau : le moteur de test [`LabSourceEngine`](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/lab/source_engine.rs) sert ses fixtures depuis un catalogue en mémoire indexé uniquement par hash 16 octets.

### 9.2 Machine à États Pure (`PeerStateMachine`)
- États mutuellement exclusifs : `Disconnected`, `Connecting`, `AwaitingHello`, `AwaitingHelloAck { local_nonce }`, `Ready { remote_peer_id, negotiated_version }`, `Closing`, `Closed`, `Failed { reason }`.
- Toute violation protocolaire ou erreur réseau entraîne une transition immédiate et irréversible vers `Failed`. Un état `Failed` ne redevient jamais `Ready`.
- Interdiction stricte de recevoir ou émettre `RequestBlock` ou `BlockData` avant d'avoir atteint l'état `Ready`.

### 9.3 Pipeline de Transfert et Intégration Core
1. **Contrôle de congestion et requêtes en vol** :
   Le composant [`InFlightTracker`](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/lab/in_flight.rs) borne les requêtes simultanées à `MAX_IN_FLIGHT_REQUESTS = 8`. Tout bloc reçu sans demande préalable est rejeté avec fermeture immédiate de la session.
2. **Tolérance prouvée à la livraison désordonnée** :
   Dans le test `test_nominal_loopback_transfer_with_shuffled_blocks`, les 53 blocs composant la part de 9 728 000 octets sont demandés et écrits dans le désordre. Le `RangeSet` de `mule-core` fusionne dynamiquement les intervalles au fur et à mesure de l'écriture sur le fichier `.part`.
3. **Validation cryptographique finale** :
   À l'issue des 53 blocs, le calcul du hash eD2K sur le fichier `.part` physique confirme une identité parfaite avec `FIXTURE_EXACT_ONE_PART` (`d815a0409aaa0b4279dd86e580bb043f`).
4. **Détection de corruption et reprise sur coupure** :
   - L'altération d'un seul octet dans un bloc transmis génère une divergence de digest immédiate, prévenant toute finalisation frauduleuse ou corrompue.
   - En cas d'interruption réseau après réception partielle (ex. 180 Ko reçus), la reconnexion interroge `RangeSet::missing_within` et ne redemande strictement que les octets manquants, permettant une reprise à chaud intègre.

---

## 10. Phase 4.3 : Transfert Loopback Inter-Démons Intégré, Feature Lab et Tolérance aux Pannes

La Phase 4.3 achève l'intégration de bout en bout du pipeline de transfert en exécutant un transfert complet entre deux processus `mule-daemon` distincts, tout en isolant le code de test derrière une feature Cargo dédiée et en prouvant la résilience du stockage en cas de crash brutal.

### 10.1 Feature Cargo `lab-loopback` & Sécurité des Livrables de Production
- **Isolation stricte** : Les fixtures synthétiques (`FIXTURE_EXACT_ONE_PART`), le moteur de source en mémoire et les routes RPC de test (`lab.addPeerSource`) sont strictement isolés derrière la feature Cargo `lab-loopback` dans `crates/mule-protocol` et `crates/mule-daemon`.
- **Livrables de production allégés et durcis** : Les builds standards (`cargo build --release`) conservent `default = []`. Ils ne compilent ni n'embarquent le moindre code de mock ou de serveur de test, garantissant une surface d'attaque minimale et aucun résidu de banc d'essai.

### 10.2 Architecture du Transfert Inter-Démons (`crates/mule-daemon`)
```
┌────────────────────────────────────────────────────────┐
│                   Démon A (Source Lab)                │
│  - Socket TCP loopback éphémère (127.0.0.1:0)          │
│  - Framing binaire PR_LAB (0xEE)                       │
│  - Sert FIXTURE_EXACT_ONE_PART (9 728 000 octets)      │
│  - Catalogue mémoire indexé par hash 16 octets         │
└───────────────────────────▲────────────────────────────┘
                            │ TCP Loopback (0xEE)
┌───────────────────────────▼────────────────────────────┐
│                  Démon B (Client Démon)                │
│  - Répertoire isolé (mule.lock, SQLite WAL, .part)     │
│  - JSON-RPC WebSocket sur port local éphémère          │
│  - Pipeline run_lab_download :                         │
│    • InFlightTracker (max 8 requêtes en vol)           │
│    • RangeSet::missing_within (requêtes ciblées)       │
│    • Écriture atomique dans .part et sync SQLite WAL   │
│    • Vérification racine eD2K puis finalisation .txt   │
└────────────────────────────────────────────────────────┘
```

1. **Cycle d'états streaming vérifié via WebSocket JSON-RPC** :
   - `Queued` : Le lien eD2K est soumis via `downloads.addEd2kLink`.
   - `WaitingForSources` : Le téléchargement attend qu'une source active soit identifiée.
   - `Downloading` : Connexion établie avec la source loopback, négociation de la machine à états (`Hello` / `HelloAck`), et streaming des blocs de 180 KiB.
   - `Verifying` : Dès la réception du dernier bloc couvrant les 9 728 000 octets, l'état bascule immédiatement sur `Verifying` pour empêcher toute utilisation prématurée avant le contrôle d'intégrité.
   - `Completed` : Le hash eD2K racine calculé sur le fichier physique `.part` correspond au hash de référence (`d815a0409aaa0b4279dd86e580bb043f`). Le fichier `.part` est renommé atomiquement vers le fichier de destination finale (`.txt`).

### 10.3 Test d'Interruption Brutale à ~50% et Reprise Intègre (`tests/inter_daemon_transfer.rs`)
Le test d'intégration `test_inter_daemon_loopback_transfer_and_interruption_resumption` valide de bout en bout la résistance aux pannes :
1. **Transfert initial jusqu'à mi-parcours** : Le Démon B télécharge les blocs séquentiellement jusqu'à dépasser ~4.5 Mo (sur 9.72 Mo).
2. **Crash brutal simulé** : Interruption immédiate du serveur Axum et annulation de toutes les tâches asynchrones d'arrière-plan via `AppState::abort_all_tasks()`.
3. **Persistance WAL vérifiée** : Inspection de la base SQLite et du fichier `.part` : les plages acquises (environ 4.5 Mo à 5.0 Mo) sont intactes et persistées sur disque.
4. **Redémarrage à froid (Cold Restart)** : Une nouvelle instance du Démon B redémarre sur le **même répertoire de travail**. Le `DownloadService` interroge `Database::get_all_downloads()` et restaure instantanément en mémoire l'état du téléchargement et ses plages acquises.
5. **Reprise sans re-téléchargement** : Le client WebSocket se reconnecte, réinjecte la source loopback. Le planificateur `missing_within` constate les plages déjà présentes et **ne redemande strictement aucun octet déjà écrit**.
6. **Finalisation intègre** : Les blocs restants sont reçus, le digest eD2K est validé et le fichier finalisé est strictement identique à la référence.

### 10.4 Alignement de l'Interface Flutter Desktop (`apps/desktop`)
- Prise en charge de l'état `DownloadStatus.waitingForSources` dans l'énumération de domaine, le DTO RPC (`RpcDto.parseStatus`), et le thème visuel (`AppTheme.getStatusColor`).
- Tests unitaires et d'intégration Flutter Desktop : 31/31 passés, `flutter analyze` : zéro warning.
