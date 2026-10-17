# Walkthrough - Interopérabilité Tier 3 avec eMule 0.50a Officiel

Ce document certifie et consigne la résolution complète et le durcissement du handshake eD2K réel (Tier 3) entre notre implémentation Rust **eMuleRust** et l'exécutable officiel de référence **eMule 0.50a** (`D:\eMule0.50a\emule.exe`, SHA-256 : `06cdf814387f627a4bd05a0c68211f715bfa952423e8e8a462e1f47c11a4d20e`), strictement confiné sur le loopback local `127.0.0.1`.

---

## 1. Diagnostics & Causes Racines Résolues

### Cause 1 : L'octet de longueur de hash `0x10` manquant dans `OP_HELLO`
- **Constat** : eMule fermait immédiatement la socket TCP après réception du premier paquet.
- **Explication** : Dans la spécification officielle eMule/eDonkey, le payload du paquet `OP_HELLO` (`0x01`) commence obligatoirement par un octet `0x10` (taille 16 en décimal) indiquant la longueur du User Hash qui suit immédiatement. À l'inverse, `OP_HELLOANSWER` (`0x4C`) ne possède pas ce préfixe.
- **Résolution** : Ajout du préfixe `0x10` dans `Ed2kEncoder::encode_hello` pour les profils eMule, portant le frame minimal à exactement 39 octets.

### Cause 2 : Mode répertoire portable (`UsePublicUserDirectories = 2`)
- **Constat** : Sur Windows moderne (Vista/7/10/11), eMule ignorait occasionnellement le `preferences.ini` local généré dans le bac à sable `%TEMP%\.tmp...` et basculait sur `%LOCALAPPDATA%\eMule\config`, liant un port aléatoire plutôt que le port éphémère du test.
- **Explication** : Le code source officiel d'eMule (`Preferences.cpp`) lit la valeur de registre `HKCU\Software\eMule\UsePublicUserDirectories` :
  - `0` : Mode multi-utilisateur (`%LOCALAPPDATA%`).
  - `1` : Mode utilisateur public.
  - `2` : Mode répertoire de l'exécutable (mode portable strict).
- **Résolution** : Le harnais applique automatiquement `reg add "HKCU\Software\eMule" /v UsePublicUserDirectories /t REG_DWORD /d 2 /f` avant le lancement, garantissant un confinement 100% hermétique dans le bac à sable temporaire.

### Cause 3 : Conflit d'instance unique sous Windows (Single-Instance Lock)
- **Constat** : Si un processus eMule orphelin existait en arrière-plan, toute nouvelle instance se terminait immédiatement avec le code de sortie 0.
- **Résolution** : Nettoyage proactif des processus orphelins avant lancement et dans la garde RAII `Tier3ProcessGuard`.

### Cause 4 : L'annonce concurrente `PR_EMULE` (`0xC5 0x01`) & Tags étendus
- **Constat** : eMule 0.50a émet simultanément `OP_EMULEINFO` (`0xC5 0x01`) et `OP_HELLOANSWER` (`0xE3 0x4C`).
- **Résolution** : Tolérance et conservation des paquets `PR_EMULE` dans l'état `AwaitingHelloAnswer`, décodage des tags canoniques eMule (`CTAG_EMULE_UDPPORTS = 0xF9`, `CTAG_EMULE_MISCOPTIONS1 = 0xFA`, `CTAG_EMULE_UDPOPTIONS = 0xFB`, `CTAG_EMULE_MISCOPTIONS2 = 0xFE`) et renseignement complet de `Ed2kPeerInfo`.

---

## 2. Structure Wire Validée du `OP_HELLO` (39 octets)

| Offset | Taille | Champ | Valeur sur le fil |
| :--- | :--- | :--- | :--- |
| **0** | 1 | Protocole eD2K | `0xE3` (`PR_ED2K`) |
| **1..4** | 4 | Longueur du payload (LE) | `0x22 0x00 0x00 0x00` (34 octets) |
| **5** | 1 | Opcode | `0x01` (`OP_HELLO`) |
| **6** | 1 | Préfixe taille User Hash | `0x10` (16 octets) |
| **7..22** | 16 | User Hash conforme eMule | `[.. 0x0E .. 0x6F ..]` (16 octets) |
| **23..26** | 4 | Client ID | `0x00 0x00 0x00 0x00` (Low ID / non assigné) |
| **27..28** | 2 | Port TCP local (LE) | Port éphémère alloué |
| **29..32** | 4 | Tag count | `0x00 0x00 0x00 0x00` (0 tags) |
| **33..36** | 4 | Server IP | `0x00 0x00 0x00 0x00` (aucun serveur) |
| **37..38** | 2 | Server Port | `0x00 0x00` (aucun port) |

**Total exact** : $1 + 4 + 34 = 39\text{ octets}$.

---

## 3. Traces Réelles d'Exécution Validée contre eMule 0.50a

Commande exécutée :
```powershell
powershell -ExecutionPolicy Bypass -File scratch\run_tier3.ps1
```

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.29s
     Running tests\tier3_loopback.rs (target\debug\deps\tier3_loopback-7797ba361086664e.exe)

running 1 test
test test_tier3_live_external_reference_loopback_handshake ... Target loopback addr: 127.0.0.1:57633
Reference binary work path: "%TEMP%\\.tmp<sandbox_handshake>"
Child PID: 18788
Attempt 1: child running = true
Attempt 1 failed: Timeout(500ms)
Attempt 2: child running = true
Attempt 2 failed: Timeout(500ms)
Attempt 3: child running = true
TCP connections for PID 18788:

LocalAddress LocalPort  State
------------ ---------  -----
0.0.0.0          57633 Listen

Connected on attempt 3!
Tier 3 live reference handshake successful! Peer info: Ed2kPeerInfo {
    user_hash: [REDACTED_PEER_HASH_16_BYTES],
    client_id: 0,
    tcp_port: 57633,
    name: Some("RefPeer050a"),
    version: Some(60),
    emule_version: None,
    misc_options1: Some(873677342),
    misc_options2: Some(11449),
    emule_udp_port: Some(57634),
    server_ip: 0,
    server_port: 0,
    tags: [
        Ed2kTag { name: Id(1), value: String("RefPeer050a") },
        Ed2kTag { name: Id(17), value: UInt32(60) },
        Ed2kTag { name: Id(249), value: UInt32(57634) },
        Ed2kTag { name: Id(250), value: UInt32(873677342) },
        Ed2kTag { name: Id(254), value: UInt32(11449) },
        Ed2kTag { name: Id(251), value: UInt32(51200) }
    ],
    unknown_tags: []
}
Files in work_path:
  "%TEMP%\\.tmp<sandbox_handshake>\\config"
  "%TEMP%\\.tmp<sandbox_handshake>\\emule.exe"
  "%TEMP%\\.tmp<sandbox_handshake>\\incoming"
  "%TEMP%\\.tmp<sandbox_handshake>\\lang"
  "%TEMP%\\.tmp<sandbox_handshake>\\preferences.ini"
  "%TEMP%\\.tmp<sandbox_handshake>\\skins"
  "%TEMP%\\.tmp<sandbox_handshake>\\temp"
Files in config_dir:
  "%TEMP%\\.tmp<sandbox_handshake>\\config\\cryptkey.dat"
  "%TEMP%\\.tmp<sandbox_handshake>\\config\\downloads.txt"
  "%TEMP%\\.tmp<sandbox_handshake>\\config\\preferences.ini"
  "%TEMP%\\.tmp<sandbox_handshake>\\config\\server.met"
ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 5.75s
```

---

## 4. Tests Unitaires de Gel de Preuve

Trois tests unitaires garantissent désormais la non-régression de cette compatibilité au niveau `cargo test --workspace` sans nécessiter de binaire externe :

1. **`test_tier3_op_hello_wire_framing_emule050a_compliance`** :
   Valide au bit près la sérialisation et désérialisation du paquet `OP_HELLO` de 39 octets sous le profil `Emule050a`.
2. **`test_tier3_op_hello_answer_wire_framing_emule050a_compliance`** :
   Valide l'absence du préfixe `0x10` dans `OP_HELLOANSWER` et le décodage sans perte des tags étendus eMule (`CTAG_EMULE_UDPPORTS`, `CTAG_EMULE_MISCOPTIONS1`).
3. **`test_session_awaiting_hello_answer_accepts_pr_emule_extension_before_hello_answer`** :
   Valide que la machine à états de session décode `Ed2kMessage::EmuleInfo` avant `OP_HELLOANSWER`, attache les capacités au pair et passe immédiatement à `Ready`.
4. **`test_tier3_op_emule_info_roundtrip_nominal`** :
   Valide le parsing et l'encodage hermétique d'`OP_EMULEINFO` et ses méthodes de détection de capacités (`has_compression_capability`, `has_source_exchange_capability`).
5. **`test_tier3_op_emule_info_oversized_tag_count_fails` & `test_tier3_op_emule_info_truncated_payload_fails`** :
   Garantit le rejet sécurisé et borné des paquets tronqués ou avec compteurs de tags abusifs.

---

## 5. Tier 3.1 : Post-Handshake Passif & Multi-Port

Le jalon **Tier 3.1** a été exécuté et validé avec succès sur 2 itérations consécutives sur ports et bacs à sable dynamiques :

### Capacités Réelles Annoncées par eMule 0.50a Officiel

Dans `OP_EMULEINFO` (`0xC5 0x01`), eMule 0.50a annonce :
- **`client_version` = 80** (`0x50` en hexadécimal, correspondant à eMule **0.50a** en notation BCD).
- **`protocol_version` = 1** (eMule compatible extensions version 1).
- **7 tags étendus décodés sans perte (0 tag inconnu)** :
  - `TAG_SPECIAL_EMULE_EXT_COMPRESSION` (`0x20` = 32) : valeur 1 (compression zlib disponible).
  - `TAG_SPECIAL_EMULE_EXT_UDPPORT` (`0x22` = 34) : valeur 4.
  - `TAG_SPECIAL_EMULE_EXT_UDPVER` (`0x21` = 33) : port UDP éphémère du test.
  - `TAG_SPECIAL_EMULE_EXT_SOURCEEXCHANGE` (`0x23` = 35) : version 3 (support SX).
  - `TAG_SPECIAL_EMULE_EXT_COMMENTS` (`0x24` = 36) : version 1.
  - `TAG_SPECIAL_EMULE_EXT_EXTENDEDREQUEST` (`0x25` = 37) : version 2.
  - `TAG_SPECIAL_EMULE_EXT_COMPATIBLECLIENT` (`0x27` = 39) : version 3.

### Invariants Vérifiés en Exécution Réelle

1. **Discipline Passive Pure** : Les capacités de compression et d'échange de sources sont observées via `peer_info.has_compression_capability()` et `peer_info.has_source_exchange_capability()`, mais le client reste en mode baseline et n'active aucune négociation spontanée.
2. **Watchdog Passif Bormé (1s)** : Maintien de la connexion TCP en écoute passive pendant 1 seconde sans trafic inattendu ni déconnexion de l'exécutable eMule.
3. **Confinement Réseau 100% Loopback** : Vérification par `Get-NetTCPConnection` qu'aucune connexion réseau non-loopback n'est établie par le processus cible.
4. **Fermeture TCP Gracieuce (FIN)** : Appel à `client.close().await` envoyant le paquet TCP `FIN` et fermant la session proprement.
5. **Teardown Hermétique & Multi-Sandbox** : Exécution sur 2 itérations indépendantes avec régénération des ports éphémères (ex: 60963/60964 puis 60971/60972) et purge des processus orphelins.

```text
running 2 tests
test test_tier3_live_external_reference_loopback_handshake ... ok
test test_tier3_live_passive_post_handshake_multi_port_and_teardown ... 
=== Starting Tier 3.1 Iteration 1/2 ===
Iteration 1: Assigned TCP port 60963, UDP port 60964
Iteration 1: Work path: "%TEMP%\\.tmp<sandbox_iter1>"
Iteration 1: Decoded OP_EMULEINFO with 7 tags, compression=true, source_exchange=true
Iteration 1: Holding passive connection for 1s watchdog...
=== Iteration 1/2 Completed Successfully ===

=== Starting Tier 3.1 Iteration 2/2 ===
Iteration 2: Assigned TCP port 60971, UDP port 60972
Iteration 2: Work path: "%TEMP%\\.tmp<sandbox_iter2>"
Iteration 2: Decoded OP_EMULEINFO with 7 tags, compression=true, source_exchange=true
Iteration 2: Holding passive connection for 1s watchdog...
=== Iteration 2/2 Completed Successfully ===
ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 21.59s
```

---

## 6. Tier 3.2 : Requête de Disponibilité et Statut de Fichier (OP_SETREQFILEID_EMULE & OP_FILESTATUS)

Le jalon **Tier 3.2** franchit une étape essentielle : après le handshake eD2K, le harnais interroge l'exécutable officiel **eMule 0.50a** sur la disponibilité des parties d'un fichier partagé, puis sur un fichier inconnu, tout en restant strictement confiné au loopback `127.0.0.1` sous discipline passive absolue.

### Fixture Déterministe & Empreinte Scellée

Une fixture de test a été générée directement dans le répertoire partagé hermétique `%TEMP%\.tmp...\incoming\` (sans aucun contact avec les répertoires hôtes ou utilisateurs) :
- **Fichier** : `tier3_status_fixture_9728001.bin`
- **Taille exacte** : $9\,728\,001\text{ octets}$ ($1\text{ partie eD2K complète de } 9\,280\,000\text{ octets} + 448\,001\text{ octets de reliquat} = 2\text{ parties}$).
- **Motif déterministe** : octets consécutifs cycliques `(0..=255)`.
- **Empreinte eD2K canonique scellée** : `695b81b1727666818988524bd7fc823c`.

### Découverte Protocolaire Clé : Sémantique de `part_count == 0` dans eMule Officiel

L'exécution initiale a révélé un comportement protocolaire fondamental documenté dans le code source officiel d'eMule/aMule (`ClientTCPSocket.cpp` et `DownloadClient.cpp`) :

1. **Émission d'`OP_FILESTATUS` (`0x50`) par eMule** :
   ```cpp
   case OP_SETREQFILEID: {
       data_out.WriteUInt8(OP_FILESTATUS);
       if (reqfile->IsPartFile()) {
           static_cast<CPartFile*>(reqfile)->WritePartStatus(&data_out);
       } else {
           data_out.WriteUInt16(0);  // <-- CKnownFile (fichier terminé/partagé) écrit uint16(0) !
       }
       break;
   }
   ```
2. **Réception d'`OP_FILESTATUS` (`0x50`) par le client eMule** :
   ```cpp
   uint16 nED2KPartCount = data->ReadUInt16();
   if (!nED2KPartCount) {
       m_nPartCount = m_reqfile->GetPartCount();
       m_downPartStatus.setsize(m_nPartCount, 1); // Marque TOUTES les parts comme disponibles !
       m_bCompleteSource = true;                 // Pair identifié comme source complète
   }
   ```

**Règle protocolaire formelle** :
- Pour un fichier complet partagé (`CKnownFile`), eMule **n'envoie aucun masque binaire** sur le fil réseau. La trame mesure exactement **18 octets** (16 octets de hash MD4 + 2 octets nuls `0x00 0x00` pour `part_count = 0`).
- Dans la sémantique eD2K, `part_count == 0` ne signifie pas "aucun bloc disponible" mais au contraire : **"source complète possédant 100% des blocs"**.
- Seuls les fichiers en cours de téléchargement (`CPartFile`) transmettent un `part_count > 0` suivi de $\lceil\text{part\_count} / 8\rceil$ octets de masque binaire.

3. **Traitement de fichier inexistant (`OP_FILEREQANSNOFILE` `0x48`)** :
   Lorsque le hash demandé n'existe ni dans `sharedfiles` ni dans `downloadqueue`, eMule répond immédiatement avec l'opcode `0x48` (`OP_FILEREQANSNOFILE`) et le hash de 16 octets.

### Traces Réelles Validées contre eMule 0.50a Officiel

```powershell
powershell -ExecutionPolicy Bypass -File scratch\run_tier3.ps1
```

```text
running 3 tests
test test_tier3_live_external_reference_loopback_handshake ... ok
test test_tier3_live_file_status_request_and_parts_availability ... 
Tier 3.2: Created fixture at "%TEMP%\\.tmp<sandbox_fixture>\\incoming\\tier3_status_fixture_9728001.bin"
Tier 3.2: Sealed eD2K Hash: 695b81b1727666818988524bd7fc823c
Attempt 1: child running = true
Attempt 1 failed: Timeout(500ms)
Attempt 2: child running = true
Connected on attempt 2!
Tier 3.2: Handshake successful with peer: Some("RefPeer050a")
Tier 3.2: Sending OP_SETREQFILEID_EMULE (0x4F) for fixture...
Tier 3.2: Received OP_FILESTATUS:
  File Hash : 695b81b1727666818988524bd7fc823c
  Part Count: 0
  Bitfield  : []
Tier 3.2: Successfully received OP_FILEREQANSNOFILE (0x48) for unknown hash!
Tier 3.2: File status and parts availability successfully verified against eMule 0.50a!
ok
test test_tier3_live_passive_post_handshake_multi_port_and_teardown ... 
=== Starting Tier 3.1 Iteration 1/2 ===
Iteration 1: Assigned TCP port 55060, UDP port 55061
Iteration 1: Work path: "%TEMP%\\.tmp<sandbox_iter1>"
Iteration 1: Decoded OP_EMULEINFO with 7 tags, compression=true, source_exchange=true
Iteration 1: Holding passive connection for 1s watchdog...
=== Iteration 1/2 Completed Successfully ===
=== Starting Tier 3.1 Iteration 2/2 ===
Iteration 2: Assigned TCP port 55067, UDP port 55068
Iteration 2: Work path: "%TEMP%\\.tmp<sandbox_iter2>"
Iteration 2: Decoded OP_EMULEINFO with 7 tags, compression=true, source_exchange=true
Iteration 2: Holding passive connection for 1s watchdog...
=== Iteration 2/2 Completed Successfully ===
ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 30.50s
```

### Invariants et Périmètre Validés

| Domaine | Invariant Vérifié |
| :--- | :--- |
| **Requête Fixture** | Émission canonique de `OP_SETREQFILEID_EMULE` (`0x4F`) sur 21 octets (`0xE3 0x11 0x00 0x00 0x00 0x4F <HASH 16>`). |
| **Réponse Fichier Connu** | Réception et décodage d'`OP_FILESTATUS` (`0x50`) sur 18 octets avec `part_count = 0` et masque vide. |
| **Sémantique Disponibilité** | `status.is_complete() == true`, `status.has_part(0) == true`, `status.has_part(1) == true`. |
| **Réponse Fichier Inconnu** | Émission pour `[0xAA; 16]`, réception immédiate d'`OP_FILEREQANSNOFILE` (`0x48`), conversion en `FileNotFound` et nettoyage du traqueur en vol. |
| **Discipline Passive Stricte** | 0 requête de bloc (`OP_REQUESTPARTS`), 0 octet de payload téléchargé, 0 trafic UDP/Kad, 0 zlib (`0xD4`). |
| **Confinement Réseau** | Vérification automatique `Get-NetTCPConnection` confirmant 0 socket hors loopback. |
| **Isolation Hermétique** | Tous les fichiers de test sont créés dans des sous-dossiers éphémères `%TEMP%\.tmp...` nettoyés par RAII. |

---

## 7. Tier 3.3 : Séquencement de Téléchargement sans Payload (OP_REQUESTPARTS_EMULE Planning)

Le jalon **Tier 3.3** formalise, valide et scelle la logique complète de planification et de séquencement qui précède tout transfert de données :
- Sélection déterministe des parties disponibles en fonction du statut.
- Découpage en blocs standard eD2K ($\le 184\,320\text{ octets}$) et alignement strict sur la frontière de part ($9\,728\,000\text{ octets}$).
- Couverture complète de la première part et du reliquat d'un octet pour la fixture de $9\,728\,001\text{ octets}$.
- Validation stricte des bornes, débordements et interdiction de blocs à cheval sur deux parts.
- Preuve d'admissibilité de toutes les parts pour une source complète (`part_count == 0`).
- Preuve de rejet absolu de toute requête si `OP_FILEREQANSNOFILE` (`0x48`) a été reçu.
- **Discipline passive absolue** : encodage de la trame `OP_REQUESTPARTS_EMULE` (`0x47`) en mémoire locale uniquement, sans transmission vers eMule 0.50a.

### Architecture du Séquenceur (`Ed2kDownloadSequencer`)

Le nouveau composant `Ed2kDownloadSequencer` (`crates/mule-protocol/src/ed2k/sequencer.rs`) applique les invariants canoniques suivants :
1. **Source complète** : Lorsque `status.is_complete()` est vrai (`part_count == 0`), toutes les parts $0..\text{total\_parts}$ sont déclarées disponibles.
2. **Découpage de la part 0 ($9\,728\,000\text{ octets}$)** :
   - Décomposée en exactement 53 blocs : 52 blocs pleins de $184\,320\text{ octets}$ + 1 bloc résiduel de $143\,360\text{ octets}$ (`[9_584_640, 9_728_000)`).
   - Le bloc final s'aligne à l'octet près sur la frontière $9\,728\,000$, sans jamais déborder sur la part suivante.
3. **Découpage de la part 1 ($1\text{ octet}$ de reliquat)** :
   - Décomposée en 1 bloc de $1\text{ octet}$ (`[9_728_000, 9_728_001)`).
   - Couvre exactement le dernier octet de la fixture.
4. **Matrice de rejet strict** :
   - Rejet de `start >= end`.
   - Rejet de dépassement de la taille du fichier.
   - Rejet de blocs dépassant $184\,320\text{ octets}$.
   - Rejet immédiat de tout bloc traversant une frontière de part (ex: `[9_727_000, 9_729_000)` renvoie `CrossPartBoundary`).
   - Rejet des requêtes comportant plus de 3 plages (`TooManyRanges`).
5. **Rejet de fichier inconnu** :
   - Si le statut est `FileNotFound` (`OP_FILEREQANSNOFILE`), `Ed2kDownloadSequencer::from_file_status_result` échoue immédiatement et interdit toute planification de bloc.

### Traces Réelles Validées contre eMule 0.50a Officiel

```powershell
powershell -ExecutionPolicy Bypass -File scratch\run_tier3.ps1
```

```text
running 4 tests
test test_tier3_live_download_sequencing_without_payload ... 
Tier 3.3: Download sequencing and block request planning without payload successfully verified against eMule 0.50a!
ok
test test_tier3_live_external_reference_loopback_handshake ... ok
test test_tier3_live_file_status_request_and_parts_availability ... ok
test test_tier3_live_passive_post_handshake_multi_port_and_teardown ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 35.98s
```

### Invariants et Périmètre Validés

| Domaine | Invariant Vérifié |
| :--- | :--- |
| **Sélection Déterministe** | `sequencer.available_parts() == vec![0, 1]` pour le fichier de 9 728 001 octets issu du statut complet eMule. |
| **Alignement Frontière** | Bloc 52 de Part 0 termine à $9\,728\,000$ pile. Bloc 0 de Part 1 couvre $9\,728\,000..9\,728\,001$. |
| **Framing Wire Local** | Paquets `OP_REQUESTPARTS_EMULE` (`0x47`) construits sur 40 octets de payload (hash 16 + 3 start offsets + 3 end offsets), conformes eMule 0.50a. |
| **Discipline Passive Absolue** | 0 paquet de bloc émis sur la socket TCP ; `pending_block_count` de la session reste à 0. |
| **Rejet Fichier Inconnu** | Réception d'`OP_FILEREQANSNOFILE` (`0x48`) $\rightarrow$ refus strict de toute planification de bloc. |
| **Confinement Réseau** | Vérification `Get-NetTCPConnection` confirmant 0 socket hors loopback. |

---

## 8. Tier 3.4 : Transfert Réel d'un Premier Bloc Non Compressé Biparti (Single Bounded Block Transfer)

Le jalon **Tier 3.4** accomplit le premier échange réel de charge utile (payload de données) entre l'exécutable officiel **eMule 0.50a** et notre client **eMuleRust**, dans des conditions strictement bornées et hermétiques :
- Fichier synthétique scellé : `tier3_status_fixture_9728001.bin` ($9\,728\,001\text{ octets}$, hash eD2K scellé : `695b81b1727666818988524bd7fc823c`).
- Requête délimitée à un unique bloc standard : Part 0 Bloc 0, soit la plage `[0, 184_320)` ($184\,320\text{ octets}$).
- Protocole : `PR_ED2K` (`0xE3`), paquets `OP_SENDINGPART` (`0x46`).

### Découvertes Protocolaires Officielles eMule 0.50a

1. **Prérequis de la file d'attente d'upload (`UploadQueue`)** :
   Dans l'implémentation C++ d'eMule (`UploadQueue.cpp` et `ClientTCPSocket.cpp`), un client demandeur ne peut pas émettre directement `OP_REQUESTPARTS` sans avoir préalablement obtenu un slot d'upload (`US_UPLOADING`).
   - Le client transmet d'abord `OP_STARTUPLOADREQ` (`0x54`) avec le hash du fichier (17 octets).
   - eMule alloue le slot disponible et répond par `OP_ACCEPTUPLOADREQ` (`0x55`).
   - Une fois accepté, le client transmet sa requête de bloc `OP_REQUESTPARTS_EMULE` (`0x47`).

2. **Fragmentation des trames en tranches de 10 240 octets** :
   eMule 0.50a n'émet pas les $184\,320\text{ octets}$ en un seul paquet géant, mais découpe la transmission en tranches réseau de $10\,240\text{ octets}$ (`EMULE_DATA_BUFFER` = 10 KB) :
   $$\frac{184\,320}{10\,240} = 18\text{ tranches consécutives de }10\,240\text{ octets}$$
   Chaque tranche est encapsulée dans un paquet `OP_SENDINGPART` (`0x46`) avec ses offsets propres : `[0..10240)`, `[10240..20480)`, ..., `[174080..184320)`.

3. **Assemblage et Détection de Doublons (`Ed2kBlockSink`)** :
   Le nouveau récepteur mémoire `Ed2kBlockSink` alloue un tampon pré-dimensionné à la taille exacte du fichier ($9\,728\,001\text{ octets}$), valide chaque tranche via le séquenceur, refuse tout chevauchement ou doublon (`DuplicateBlock`), et certifie la couverture complète via `is_range_covered(0, 184_320)`.

### Traces Réelles Validées contre eMule 0.50a Officiel

```powershell
powershell -ExecutionPolicy Bypass -File scratch\run_tier3.ps1
```

```text
running 5 tests
test test_tier3_live_download_sequencing_without_payload ... Tier 3.3: Download sequencing and block request planning without payload successfully verified against eMule 0.50a!
ok
test test_tier3_live_external_reference_loopback_handshake ... ok
test test_tier3_live_file_status_request_and_parts_availability ... Tier 3.2: File status and parts availability successfully verified against eMule 0.50a!
ok
test test_tier3_live_passive_post_handshake_multi_port_and_teardown ... ok
test test_tier3_live_single_uncompressed_block_transfer_from_emule050a ... 
Tier 3.4: Requesting upload slot via OP_STARTUPLOADREQ (0x54)...
Tier 3.4: OP_ACCEPTUPLOADREQ (0x55) confirmed! Upload slot granted.
Tier 3.4: Transmitting single OP_REQUESTPARTS_EMULE (0x47) for [0, 184_320)...
Tier 3.4: Awaiting OP_SENDINGPART (0x46) chunks for [0, 184_320)...
Tier 3.4: Received OP_SENDINGPART chunk #1: [0..10240), 10240 bytes (total: 10240/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #2: [10240..20480), 10240 bytes (total: 20480/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #3: [20480..30720), 10240 bytes (total: 30720/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #4: [30720..40960), 10240 bytes (total: 40960/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #5: [40960..51200), 10240 bytes (total: 51200/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #6: [51200..61440), 10240 bytes (total: 61440/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #7: [61440..71680), 10240 bytes (total: 71680/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #8: [71680..81920), 10240 bytes (total: 81920/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #9: [81920..92160), 10240 bytes (total: 92160/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #10: [92160..102400), 10240 bytes (total: 102400/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #11: [102400..112640), 10240 bytes (total: 112640/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #12: [112640..122880), 10240 bytes (total: 122880/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #13: [122880..133120), 10240 bytes (total: 133120/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #14: [133120..143360), 10240 bytes (total: 143360/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #15: [143360..153600), 10240 bytes (total: 153600/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #16: [153600..163840), 10240 bytes (total: 163840/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #17: [163840..174080), 10240 bytes (total: 174080/184,320)
Tier 3.4: Received OP_SENDINGPART chunk #18: [174080..184320), 10240 bytes (total: 184320/184,320)
Tier 3.4: Block MD4 Oracle: 1d4196d9f9fa78d00b59ead840171f15
Tier 3.4: Single bounded block transfer successfully verified against eMule 0.50a!
ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 44.19s
```

### Invariants et Périmètre Validés

| Domaine | Invariant Vérifié |
| :--- | :--- |
| **Entrée en File d'Upload** | Émission d'`OP_STARTUPLOADREQ` (`0x54`), réception et validation d'`OP_ACCEPTUPLOADREQ` (`0x55`) par eMule 0.50a. |
| **Requête Unique** | Une seule requête `OP_REQUESTPARTS_EMULE` (`0x47`) pour le bloc `[0, 184_320)`. Invariants `pending_block_count == 1`. |
| **Réception & Découpage** | 18 tranches consécutives de 10 240 octets reçues sous `OP_SENDINGPART` (`0x46`), totalisant $184\,320\text{ octets}$. |
| **Conformité Byte-à-Byte** | Le bloc assemblé correspond à 100% au préfixe du fichier synthétique de test (`tier3_status_fixture_9728001.bin`). |
| **Oracle MD4 Local** | L'oracle MD4 sur le bloc reçu produit `1d4196d9f9fa78d00b59ead840171f15`, identique à l'oracle sur le préfixe de référence. |
| **Rejet de Doublon** | Toute tentative de réécriture dans `Ed2kBlockSink` est strictement rejetée avec `DuplicateBlock`. |
| **Discipline Passive Absolue** | Aucun bloc supplémentaire demandé, `pending_block_count == 0` à la fin du transfert. |
| **Confinement Réseau** | Scan `Get-NetTCPConnection` certifiant 0 socket en dehors de `127.0.0.1`. |
| **Fermeture Propre** | Envoi de TCP FIN (`client.close().await`) et arrêt propre du processus eMule. |

---

## 9. Tier 3.5 : Complétion de la Part 0 avec Reprise Contrôlée & Interruption Abrupte (Part 0 Transfer, Controlled Resume & Interruption)

Le jalon **Tier 3.5** valide pour la première fois le transfert complet d'une part eD2K intégrale (Part 0, soit $9\,728\,000\text{ octets} = 53\text{ blocs}$) depuis l'exécutable officiel **eMule 0.50a** (`D:\eMule0.50a\emule.exe`), avec interruption simulée, persistance physique dans un fichier `.part`, journal d'état atomique `.resume.json`, calcul de lacunes sans redondance, reprise multi-session et vérification d'intégrité MD4 diffusée depuis le disque.

### Périmètre et Invariants Tier 3.5

- **Part 0 Strictement Isolée** : 53 blocs ($9\,728\,000\text{ octets}$ = 52 blocs pleins de $184\,320\text{ octets}$ + 1 bloc résiduel de $143\,360\text{ octets}$).
- **Part 1 Résiduelle Exclue** : L'unique octet résiduel de la Part 1 (`[9_728_000, 9_728_001)`) n'a jamais été demandé et n'a jamais été écrit (réservé pour Tier 3.6).
- **Fenêtre Glissante (Pipelining)** : Au plus 3 requêtes de blocs actives en vol simultanément (`max_in_flight = 3`), respectant la borne de congestion eD2K officielle.
- **Stockage Physique `.part` & Journal `.resume.json`** :
  - `Ed2kPartFileSink` pré-alloue / dimensionne le fichier physique à la taille exacte ($9\,728\,001\text{ octets}$).
  - Les tranches reçues sont écrites sur disque à leur position absolue avec `seek(SeekFrom::Start(start))`.
  - `Ed2kPartResumeState` consigne les intervalles reçus canoniquement fusionnés (`completed_ranges`) et les octets cumulés.
  - Sauvegarde atomique par écriture dans un fichier temporaire (`.tmp`), `sync_all()`, puis renommage atomique vers `.resume.json` pour garantir l'immunité aux crashs.
- **Scénario d'Interruption Contrôlée (Phase 1)** :
  - Téléchargement exact des 20 premiers blocs ($3\,686\,400\text{ octets}$).
  - Coupure brutale de la connexion TCP (`client1.close().await`, fermeture de socket).
  - Validation sur disque : le fichier `.part` et le journal `.resume.json` contiennent fidèlement les $3\,686\,400\text{ octets}$ et l'intervalle unique fusionné `[(0, 3_686_400)]`.
- **Scénario de Reprise Hermétique (Phase 2)** :
  - Réouverture via `Ed2kPartFileSink::open_or_resume`, vérification de cohérence du hash eD2K et de la taille.
  - Calcul déterministe des lacunes via `sequencer.plan_missing_blocks(0, resumed_sink.completed_ranges())` :
    - Exactement 33 blocs manquants identifiés (blocs 20 à 52, offsets `[3_686_400..9_728_000)`).
    - Zéro requête émise pour les blocs déjà possédés 0..19.
  - Nouvelle connexion vers eMule 0.50a, réservation d'upload slot (`OP_STARTUPLOADREQ` -> `OP_ACCEPTUPLOADREQ`).
  - Téléchargement des 33 blocs restants via la fenêtre glissante.
  - Complétion 100% de la Part 0 ($9\,728\,000\text{ octets}$).
- **Validation Cryptographique MD4 Diffusée (Streamed Disk MD4)** :
  - La méthode `sink.compute_part_md4(0, &sequencer)` lit la Part 0 sur disque par tampons de 64 KiB directement dans le hasher MD4 sans saturer la RAM.
  - Résultat obtenu : `240568858a65f85133d858f538d17467`, strictement identique à l'oracle calculé sur les octets de référence de la fixture.

### Traces Réelles Validées contre eMule 0.50a Officiel

```powershell
powershell -ExecutionPolicy Bypass -File scratch\run_tier3.ps1
```

```text
running 6 tests
test test_tier3_live_download_sequencing_without_payload ... ok
test test_tier3_live_external_reference_loopback_handshake ... ok
test test_tier3_live_file_status_request_and_parts_availability ... ok
test test_tier3_live_passive_post_handshake_multi_port_and_teardown ... ok
test test_tier3_live_single_uncompressed_block_transfer_from_emule050a ... ok
test test_tier3_live_part0_transfer_with_resume_and_interruption ... 
Tier 3.5: Expected Part 0 MD4 oracle: 240568858a65f85133d858f538d17467
Tier 3.5: Session 1 requesting upload slot via OP_STARTUPLOADREQ (0x54)...
Tier 3.5: Session 1 upload slot granted!
Tier 3.5: Phase 1: Downloading first 20 blocks (3,686,400 bytes) with sliding window (max 3 in-flight)...
Tier 3.5: Phase 1 verified: exactly 20 blocks (3,686,400 bytes) written and verified on disk.
Tier 3.5: Simulating abrupt network interruption (closing Session 1 TCP connection)...
Tier 3.5: Atomic .resume.json journal verified on disk after interruption.
Tier 3.5: Phase 2: Opening existing .part file and .resume.json journal...
Tier 3.5: Gap calculation verified: exactly 33 missing blocks planned [3,686,400..9,728,000).
Tier 3.5: Session 2 requesting upload slot via OP_STARTUPLOADREQ (0x54)...
Tier 3.5: Session 2 upload slot granted!
Tier 3.5: Phase 2: Downloading remaining 33 blocks with sliding window (max 3 in-flight)...
Tier 3.5: Part 0 download 100% completed across 2 sessions!
Tier 3.5: Streaming Part 0 from disk in 64 KiB buffers to verify MD4 checksum...
Tier 3.5: Part 0 MD4 verified: 240568858a65f85133d858f538d17467 == 240568858a65f85133d858f538d17467
Tier 3.5: Part 0 transfer with resume and interruption successfully verified against eMule 0.50a!
ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 62.65s
```

### Synthèse des Garanties et Invariants Prouvés

| Domaine | Preuve / Invariant Validé |
| :--- | :--- |
| **Persistance Disque** | Fichier `.part` écrit par `Ed2kPartFileSink` avec positionnement `seek`, sans corruption ni troncature. |
| **Journal Atomique** | Fichier `.resume.json` mis à jour de façon transactionnelle (`.tmp` + `sync_all` + rename). |
| **Fusion d'Intervalles** | Les tranches adjacentes et consécutives sont automatiquement fusionnées en plages canoniques `(start, end)`. |
| **Résistance aux Crashs** | Après interruption TCP abrupte au bloc 20, réouverture exacte et reprise intègre. |
| **Calcul des Lacunes** | `plan_missing_blocks` calcule les 33 blocs restants (blocs 20..52) et exclut 100% des blocs déjà possédés (0..19). |
| **Pipelining / Fenêtre** | `download_blocks_pipelined` maintient une fenêtre active bornée ($\le 3$ requêtes simultanées). |
| **Part 0 Complétée** | Exactement $9\,728\,000\text{ octets}$ reçus et assemblés sur disque. |
| **Exclusion Part 1** | Aucun octet de la Part 1 (`[9_728_000..9_728_001)`) n'a été demandé ou écrit. |
| **Oracle MD4 Diffusé** | Le hash MD4 calculé en streaming depuis le disque est strictement identique à l'oracle fixture (`240568858a65f85133d858f538d17467`). |
| **Isolation & Pureté** | 0 panic, 0 `unwrap()`, 0 `expect()` dans `src/ed2k/`, 0 socket hors loopback `127.0.0.1`. |

---

## 10. Tier 3.6 : Finalisation de Frontière de Fichier, Calcul Racine eD2K Multi-Parts, Promotion Transactionnelle Crash-Résistante & Nettoyage (File Boundary Finalization, Multi-Part Root Hashing, Two-Phase Promotion & Cleanup)

Le jalon **Tier 3.6** conclut avec succès le cycle complet d'acquisition, de contrôle d'intégrité et de livraison physique d'un fichier multi-parts eD2K contre l'exécutable officiel **eMule 0.50a** (`D:\eMule0.50a\emule.exe`), strictement confiné sur `127.0.0.1` en loopback.

### Périmètre et Invariants Validés

1. **Franchissement et Respect de la Frontière de Part (Boundary Invariance)** :
   - Fichier cible : `tier3_status_fixture_9728001.bin` ($9\,728\,001\text{ octets}$, soit 2 parts eD2K).
   - Part 0 : $9\,728\,000\text{ octets}$ ($9\,500\text{ KiB}$, 53 blocs).
   - Part 1 : $[9\,728\,000, 9\,728\,001)$ ($1\text{ octet}$ résiduel unique `0x00`).
   - La requête `OP_REQUESTPARTS_EMULE` (`0x47`) est émise exclusivement pour `[9_728_000, 9_728_001)` : aucune requête ne traverse jamais la frontière des $9\,728\,000\text{ octets}$.
   - Réception d'`OP_SENDINGPART` (`0x46`) contenant exactement 1 octet de données.
   - Atteinte de la couverture intégrale du fichier : $9\,728\,001\text{ octets}$, intervalle fusionné `[(0, 9_728_001)]`.

2. **Calcul Cryptographique du Hash Racine eD2K Multi-Parts (Full Streamed Multi-Part Root Hashing)** :
   - Selon la spécification canonique eD2K / eMule :
     - Si `total_parts == 1`, le hash de fichier est l'empreinte MD4 de la Part 0.
     - Si `total_parts > 1`, chaque part physique (y compris la part résiduelle finale) est hachée individuellement par MD4 ($16\text{ octets}$). Les empreintes de parts concaténées ($N \times 16\text{ octets}$) sont alors hachées par un MD4 global pour produire le hash racine eD2K :
       $$\text{Root MD4} = \text{MD4}\Big(\text{MD4}(\text{Part } 0) \;\parallel\; \text{MD4}(\text{Part } 1) \;\parallel\; \dots \;\parallel\; \text{MD4}(\text{Part } N-1)\Big)$$
   - Calcul diffusé en streaming direct depuis le disque par blocs de 64 KiB sans mise en mémoire tampon complète :
     - Part 0 Streamed MD4 : `240568858a65f85133d858f538d17467`
     - Part 1 Streamed MD4 : `47c61a0fa8738ba77308a8a600f88e4b`
     - Hash Racine Calculé : `MD4(Part 0 || Part 1) = ` **`695b81b1727666818988524bd7fc823c`**
     - Correspondance 100% byte-à-byte avec le hash eD2K scellé de la fixture officielle.

3. **Machine à États de Promotion en Deux Phases & Résilience aux Crashs (`Ed2kPartDownloadStatus`)** :
   - États déclarés :
     - `Downloading` : Acquisition active des tranches dans le fichier `.part`.
     - `VerifiedPendingPromotion` : Téléchargement 100% achevé, hash racine eD2K vérifié sur disque avec succès, journal mis à jour et synchronisé (`sync_all`), fichier en attente de promotion locale.
     - `Promoted` : Fichier transféré dans son répertoire cible final (`downloads/`), journal `.resume.json` et fichier `.part` purgés avec succès.
   - **Phase 1 (Vérification & Consignation Atomique)** : `mark_verified_pending_promotion` bascule l'état dans le journal transactionnel `.resume.json`.
   - **Test de Crash Simulé** : Le processus client est brusquement interrompu (connexion TCP fermée, récepteur déchargé de la mémoire).
   - **Reprise Locale Hors-Réseau** : Au redémarrage, `Ed2kPartFileSink::open_or_resume` détecte l'état `VerifiedPendingPromotion` sur disque et permet de finaliser la promotion sans nécessiter la moindre reconnexion réseau ni redemander le moindre octet à eMule.
   - **Phase 2 (Promotion Atomique, Anti-Collision & Nettoyage)** :
     - `promote_to_destination` libère proprement le handle Windows (`file.take()`) pour prévenir tout verrouillage de fichier (`ERROR_SHARING_VIOLATION`).
     - Vérification anti-collision : refuse d'écraser un fichier existant avec une erreur explicite.
     - Déplacement atomique (via renommage intermédiaire `.part_promoted.tmp`) vers le répertoire `downloads/tier3_promoted_final.bin`.
     - Suppression du fichier d'origine `.part` et suppression du journal compagnon `.resume.json`.

### Traces Réelles d'Exécution contre eMule 0.50a Officiel

```powershell
$env:EMULE_REFERENCE_BIN = 'D:\eMule0.50a\emule.exe'; $env:EMULE_REFERENCE_SHA256 = '06cdf814387f627a4bd05a0c68211f715bfa952423e8e8a462e1f47c11a4d20e'; cargo test -p mule-protocol --test tier3_loopback --features ed2k-reference-loopback -- test_tier3_live_file_boundary_finalization_and_promotion --ignored --nocapture
```

```text
running 1 test
Tier 3.6: Launching reference eMule 0.50a with shared 9,728,001 bytes fixture...
Tier 3.6: Requesting upload slot via OP_STARTUPLOADREQ (0x54)...
Tier 3.6: Upload slot granted!
Tier 3.6: Phase 1: Downloading Part 0 (53 blocks, 9,728,000 bytes)...
Tier 3.6: Phase 1 verified: Part 0 (9,728,000 bytes) downloaded and present on disk.
Tier 3.6: Phase 2: Planning and requesting Part 1 residual block [9,728,000, 9,728,001)...
Tier 3.6: Phase 2 verified: Part 1 residual block received; 100% file coverage achieved!
Tier 3.6: Phase 3: Verifying in-disk multi-part eD2K root hash computation...
Tier 3.6: Phase 3 verified: In-disk root eD2K hash 695b81b1727666818988524bd7fc823c matches sealed oracle byte-for-byte!
Tier 3.6: Phase 4: Step A - Marking VerifiedPendingPromotion in atomic journal...
Tier 3.6: Phase 4: Step B - Simulating abrupt crash / network disconnection...
Tier 3.6: Phase 4: Step C - Recovering locally from disk without contacting eMule...
Tier 3.6: Phase 4: Step D - Atomically promoting .part to 'C:\Users\Lou\AppData\Local\Temp\.tmpGe9L2G\downloads'...
Tier 3.6: Phase 4 verified: Two-phase promotion and cleanup completed successfully!
Tier 3.6: File boundary finalization, full root eD2K hash, and two-phase promotion successfully verified against eMule 0.50a!
test test_tier3_live_file_boundary_finalization_and_promotion ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out; finished in 17.13s
```

### Exécution Globale de la Suite Complète Tier 3 (7 Tests Référence Live)

```powershell
powershell -ExecutionPolicy Bypass -File scratch\run_tier3.ps1
```

```text
running 7 tests
test test_tier3_environment_initialization_and_neutralized_configs ... ok
test test_tier3_live_download_sequencing_without_payload ... ok
test test_tier3_live_external_reference_loopback_handshake ... ok
test test_tier3_live_file_boundary_finalization_and_promotion ... ok
test test_tier3_live_file_status_request_and_parts_availability ... ok
test test_tier3_live_part0_transfer_with_resume_and_interruption ... ok
test test_tier3_live_single_uncompressed_block_transfer_from_emule050a ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 80.69s
```

### Tableau Récapitulatif des Invariants Validés

| Domaine | Preuve / Invariant Validé |
| :--- | :--- |
| **Frontière de Part** | Requête strictly bornée pour Part 1 `[9_728_000, 9_728_001)` (1 octet), sans franchissement de frontière de part. |
| **Complétion 100%** | Assemblage exact des $9\,728\,001\text{ octets}$ de la fixture sur disque dans l'intervalle canonique `[(0, 9_728_001)]`. |
| **Hash Racine Multi-Parts** | Calcul en streaming de $\text{MD4}(\text{Part 0}) \parallel \text{MD4}(\text{Part 1})$ donnant exactement `695b81b1727666818988524bd7fc823c`. |
| **Promotion Transactionnelle** | Transition sécurisée `Downloading` $\to$ `VerifiedPendingPromotion` $\to$ `Promoted`. |
| **Résilience aux Crashs** | Reprise après interruption brutale directement en `VerifiedPendingPromotion` sans appel réseau. |
| **Protection Anti-Collision** | Refus strict d'écraser un fichier de destination déjà présent. |
| **Gestion Windows Handles** | Libération du handle de fichier avant le renommage pour éviter tout verrouillage d'E/S. |
| **Nettoyage RAII** | Suppression automatique du fichier `.part` et du journal `.resume.json` une fois la promotion confirmée sur disque. |
| **Confinement Réseau** | Scan `Get-NetTCPConnection` certifiant 0 socket non-loopback pendant toute l'exécution. |
| **Qualité du Code** | 0 `unwrap()`, 0 `expect()`, 0 panique dans `src/ed2k/`, vérifié par `ed2k_isolation_guard`. |



