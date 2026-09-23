# Matrice des Opcodes et Protocoles eD2K / eMule

## 1. Contexte & Historique des Variantes

Le protocole eDonkey2000 originel et ses extensions successives (eMule, aMule, cDonkey, MLDonkey) ont introduit des évolutions, des réassignations et des formats alternatifs selon les générations logicielles et les octets de protocole (`0xE3`, `0xC5`, `0xD4`).

Cette matrice formalise les attributions exactes, les directions, les formats sur le fil et le niveau de support dans **eMule-Rust**, en se fondant sur les sources de référence versionnées :
- **eMule 0.72a** *(référence de vérité actuelle)* : `D:\eMule0.72a-Sources\eMule0.72a-Sources\srchybrid` (`opcodes.h`, `DownloadClient.cpp`, `ListenSocket.cpp`, `UploadClient.cpp`, `kademlia/`).
- **eMule 0.50a** *(références historiques citées dans les tableaux)* : `SomeSupport/eMule` (`srchybrid/opcodes.h`, `DownloadClient.cpp`, `ListenSocket.cpp`, `UploadClient.cpp`).
- **aMule 2.3.3** : `src/DownloadClient.cpp`, `src/UploadClient.cpp`.
- **eDonkey2000** : Spécifications historiques reverse-engineérées du protocole passif v1.

---

## 2. Matrice Comparative des Opcodes et Formats Wire

| Message Symbolique | Variante & Source | Protocole Wire | Opcode Wire | Direction | Structure & Layout Payload | Support eMule-Rust | Règle / Invariant Associé |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`OP_HELLO`** | Standard eD2K & eMule 0.72a | `PR_ED2K` (`0xE3`) | `0x01` | Client $\rightarrow$ Pair | Hash (16B), ClientID (4B LE), TCP Port (2B LE), Tag count (4B LE), Tags, Server IP (4B LE), Server Port (2B LE) | **Complet** | Rejet si hash nul ou version incompatible |
| **`OP_HELLO_ANSWER`** | Standard eD2K & eMule 0.72a | `PR_ED2K` (`0xE3`) | `0x4C` | Pair $\rightarrow$ Client | Hash (16B), ClientID (4B LE), TCP Port (2B LE), Tag count (4B LE), Tags, Server IP (4B LE), Server Port (2B LE) | **Complet** | Préservation des tags inconnus sans désérialisation non sécurisée |
| **`OP_SETREQFILEID`** | eMule 0.72a (`opcodes.h:221`) & aMule 2.3.3 | `PR_ED2K` (`0xE3`) | `0x4F` | Client $\rightarrow$ Source | Hash MD4 (16B). Déclenche l'envoi de `OP_FILESTATUS` (`0x50`) par la source. | **Complet** | Format canonique 17B (opcode + hash). Pas d'accès disque. |
| **`OP_FILESTATUSREQ`** | eDonkey legacy / docs alternatifs | `PR_ED2K` (`0xE3`) | `0x49` | Client $\rightarrow$ Source | Hash MD4 (16B). *(Dans eMule 0.72a, 0x49 = `OP_END_OF_DOWNLOAD` et 0x4D = `OP_CHANGE_CLIENT_ID`, `opcodes.h:215,219`).* | **Complet (compatibilité)** | Décodé et encodé de manière transparente comme `FileStatusReq`. |
| **`OP_FILESTATUS`** | Standard eD2K & eMule 0.72a | `PR_ED2K` (`0xE3`) | `0x50` | Source $\rightarrow$ Client | Hash MD4 (16B), Part count (2B LE), Bitfield LSB-first ($\lceil part\_count / 8 \rceil$ bytes). | **Complet** | Validation stricte des bits de fin résiduels (doivent valoir 0). `part_count == 0` = source complète ; alimente `availableSourceCount` (barre de parts) via `record_source_availability`, initial + keep-alive en file. |
| **`OP_REQUESTPARTS`** | eMule 0.72a (`opcodes.h:213`) & aMule 2.3.3 | `PR_ED2K` (`0xE3`) | `0x47` | Client $\rightarrow$ Source | Hash MD4 (16B), 3× u32 LE start, 3× u32 LE end exclusif (`endpos = block->EndOffset + 1`). Sentinelles `(0, 0)` ou `(u32::MAX, u32::MAX)`. (40B total). | **Complet** | Tracké dans `BlockRequestTracker` (max 8 en vol, zéro chevauchement). |
| **`OP_REQUESTPARTS`** | eDonkey legacy / docs historiques | `PR_ED2K` (`0xE3`) | `0x4B` | Client $\rightarrow$ Source | 40B (3 slots) ou 24B (1 slot: hash 16B + start 4B + end 4B). *(Dans eMule 0.72a, 0x4B = `OP_ASKSHAREDFILESANSWER`, `opcodes.h:217`).* | **Complet (compatibilité)** | Décodé de manière transparente dans les deux formats 24B et 40B. |
| **`OP_SENDINGPART`** | Standard eD2K & eMule 0.72a | `PR_ED2K` (`0xE3`) | `0x46` | Source $\rightarrow$ Client | Hash MD4 (16B), start (4B LE), end (4B LE exclusif), chunk bytes (`end - start` octets, max 180 KiB). | **Complet** | Écriture exclusive via `DownloadSink` dans `.part` sous verrou WAL. |
| **`OP_COMPRESSEDPART`** | Extension eMule zlib | `PR_EMULE` (`0xC5`) ou `PR_EMULE_COMPRESSED` (`0xD4`) | `0x40` (`0xA1` variante I64) | Source $\rightarrow$ Client | Hash MD4 (16B), start (4B/8B LE), taille compressée totale (4B LE), chunk zlib ≤ 10 240 B (envoyé sous `OP_EMULEPROT` chez eMule). | **Complet (bidirectionnel)** | Décodage incrémental `flate2` côté download ; émission zlib `Z_BEST_SPEED` côté upload si `nDataCompVer` (MiscOptions1 bits 20-23) ; repli `SENDINGPART` si non rentable. |
| **`OP_HASHSETREQUEST`** | eMule 0.72a (*DEPRECATED* `opcodes.h:223`, remplacé par `OP_HASHSETREQUEST2` `0xB1`) | `PR_ED2K` (`0xE3`) | `0x51` | Client $\rightarrow$ Source | Hash MD4 (16B). Déclenche `OP_HASHSETANSWER`. | **Complet** | Envoyé à la connexion si le hashset est inconnu ; servi pour fichiers partagés avec vérification du hash racine. |
| **`OP_HASHSETANSWER`** | eMule 0.72a (*DEPRECATED* `opcodes.h:224`, remplacé par `OP_HASHSETANSWER2` `0xB2`) | `PR_ED2K` (`0xE3`) | `0x52` | Source $\rightarrow$ Client | Hash MD4 (16B), `u16` count, N× hash MD4 par part. | **Complet** | Persisté (`file_part_hashes`) ; vérification incrémentale et éviction des parts corrompues. |
| **`OP_AICHFILEHASHREQ` / `RES`** | eMule | `PR_EMULE` (`0xC5`) | `0x9E` / `0x9D` | Client $\leftrightarrow$ Source | Req : hash16. Res : hash16 + racine SHA-1 (20B). | **Complet** | Consensus de confiance `CAICHUntrustedHash` (≥10 IP uniques masquées, ≥92 %). |
| **`OP_AICHREQUEST` / `ANSWER`** | eMule | `PR_EMULE` (`0xC5`) | `0x9B` / `0x9C` | Client $\leftrightarrow$ Source | Req : hash16 + `u16` part + master20. Ans : entrées `(ident, hash)` 16/32-bit ou hash16 seul (échec). | **Complet** | Récupération au niveau bloc 180 Ko sur arbre de Merkle à double granularité. |
| **`OP_OFFERFILES`** | eD2K serveur | `PR_ED2K` (`0xE3`) | `0x15` | Client $\rightarrow$ Serveur | `u32` count + par fichier hash16 + `u32` client_id + `u16` port + tags. | **Complet** | Sémantique `CreateOfferedFilePacket` : marqueurs compression, HighID/LowID, tags `FT_FILESIZE_HI` > 4 Gio, keep-alive vide, plafond 200. |
| **`OP_CALLBACKREQUEST`** | eD2K serveur | `PR_ED2K` (`0xE3`) | `0x1C` | Client $\rightarrow$ Serveur | `u32` client_id nu. | **Complet** | Sources LowID `lowid:<id>` rappelées via relai serveur ; adoption des connexions entrantes correspondantes. |
| **`OP_CALLBACKREQUESTED` / `FAIL`** | eD2K serveur | `PR_ED2K` (`0xE3`) | `0x35` / `0x36` | Serveur $\rightarrow$ Client | `IP4 + port2` (+ optionnel `crypt u8 + userhash16`). | **Complet** | `CALLBACKREQUESTED` déclenche une connexion sortante (`connect_back` via `InboundListenerService`, handshake eD2K complet puis service des requêtes) ; `CALLBACK_FAIL` journalisé. |
| **`OP_QUEUERANKING`** | eMule 0.72a | `PR_EMULE` (`0xC5`) | `0x60` | Source $\rightarrow$ Client | Exactement 12 octets : `u16` rang + 10 octets réservés (`ProcessEmuleQueueRank`). | **Complet (codec)** | Décodé/encodé ; propagation session à brancher côté daemon. |
| **`OP_QUEUERANK`** | eD2K legacy | `PR_ED2K` (`0xE3`) | `0x5C` | Source $\rightarrow$ Client | `u32` rang de file d'attente. | **Complet (codec)** | Forme legacy du rang de file. |
| **`OP_CHANGE_SLOT`** | eD2K legacy | `PR_ED2K` (`0xE3`) | `0x5B` | Bidirectionnel | `hash16` — toléré/ignoré par eMule 0.72a. | **Complet (codec)** | Décodeur strict 16 octets. |
| **`OP_MESSAGE`** | eD2K | `PR_ED2K` (`0xE3`) | `0x4E` | Bidirectionnel | `u16` len + message ; `len` doit consommer le payload exactement. | **Complet (codec)** | Chat pair-à-pair. |
| **`OP_FILEDESC`** | eMule 0.72a | `PR_EMULE` (`0xC5`) | `0x61` | Source $\rightarrow$ Client | `u8` rating + `u16` len + description. | **Complet (codec)** | Notation/commentaire de fichier. |
| **`OP_PORTTEST`** | eD2K | `PR_ED2K` (`0xE3`) | `0xFE` | Bidirectionnel | `u8` statut (eMule répond `0x12`). | **Complet (codec)** | Test de connectivité TCP. |
| **`OP_CHATCAPTCHAREQ` / `RES`** | eMule 0.72a | `PR_EMULE` (`0xC5`) | `0xA5` / `0xA6` | Bidirectionnel | Req : `u8` tag_count + tags + bitmap BMP (128–4096 B chez eMule). Res : `u8` statut. | **Complet (codec)** | Anti-spam chat ; bornes bitmap à appliquer côté daemon. |
| **`OP_HASHSETREQUEST2` / `ANSWER2`** | eMule 0.72a (`opcodes.h:288-289`) | `PR_EMULE` (`0xC5`) | `0xB1` / `0xB2` | Client $\leftrightarrow$ Source | `<FileIdentifier>` + `u8` options (bit0 MD4, bit1 AICH) ; réponse : `FileIdentifier` + options + hashsets (`hash16+u16+N×16` / `hash20+u16+N×20`). | **Complet (codec)** | Remplace `0x51`/`0x52` deprecated ; options vides rejetées comme eMule. |
| **`OP_MULTIPACKET` / `EXT` / `EXT2`** | eMule 0.72a (`0x92`/`0xA4` deprecated, `0xA9` actif) | `PR_EMULE` (`0xC5`) | `0x92` / `0xA4` / `0xA9` | Client $\rightarrow$ Source | Référence fichier (`hash16` / `hash16+u64` / `FileIdentifier`) + sous-opcodes `0x58`(+ext-info conditionnel à `ExtendedRequestsVersion`)/`0x4F`/`0x81`/`0x83`(+ver u8+opts u16)/`0x9E`. | **Complet (codec)** | `DownloadClient.cpp:259-345`. |
| **`OP_MULTIPACKETANSWER` / `EXT2`** | eMule 0.72a | `PR_EMULE` (`0xC5`) | `0x93` / `0xB0` | Source $\rightarrow$ Client | Même enveloppe ; sous-réponses `0x59`(nom)/`0x50`(statut sans hash)/`0x9D`(racine AICH 20B). | **Complet (codec)** | `ListenSocket.cpp:1030-1101`. |
| **`OP_PEERCACHE_*`** | eMule (defunct 0.72a) | `PR_EMULE` (`0xC5`) | `0x94`–`0x96` | Bidirectionnel | Opaque — ignorés par eMule 0.72a. | **Complet (raw)** | Décodés en `RawPacket`, ignorés en amont. |
| **`OP_CANCELTRANSFER` / `OP_OUTOFPARTS`** | eD2K | `PR_ED2K` (`0xE3`) | `0x56` / `0x57` | Bidirectionnel | Payload vide. | **Complet** | Libération immédiate du slot upload ; fin de session gracieuse ; émis sur annulation et fin de demandes. |
| **`OP_REASKFILEPING` / `REASKACK` / `FILENOTFOUND` / `QUEUEFULL`** | Client UDP eMule (`ClientUDPSocket`) | `PR_EMULE` (`0xC5`) sur UDP | `0x90`–`0x93` | Bidirectionnel | Ping : `hash16` + `u16` status + `u16` complete. Ack : `[bitfield]` + `u16` rang. FNF/Full : vide. | **Complet** | Socket UDP partagée avec Kad ; keep-alive ~21 min vers sources `Queued` ; rang validé contre reasks en attente ; `QUEUEFULL` si file quasi pleine. |
| **`OP_SERVERLIST`** | eD2K serveur | `PR_ED2K` (`0xE3`) | `0x32` | Serveur $\rightarrow$ Client | `u8` count + `count × (u32 ip_le + u16 port)`. | **Complet** | Fusionné dans `ed2k_servers` (upsert dédupliqué, préférence 0) ; entrées invalides `ip=0`/`port=0` rejetées. |
| **`OP_GLOBGETSOURCES2` / `GLOBFOUNDSOURCES`** | Serveur UDP (`UDPSocket`) | `PR_ED2K` (`0xE3`) sur UDP | `0x94` / `0x9B` | Client $\leftrightarrow$ Serveur | Req : `hash16 + u32 taille`. Res : blocs `hash16 + u8 count + count × (u32 ip + u16 port)`. | **Complet** | Socket UDP dédiée (bind éphémère) ; requêtes vers serveur connecté (UDP = TCP+4) ; sources persistées comme `PeerSource` (mêmes filtres que TCP, LowID conservé). |
| **`OP_GLOBSERVSTATREQ` / `RES`** | Serveur UDP | `PR_ED2K` (`0xE3`) sur UDP | `0x96` / `0x97` | Client $\leftrightarrow$ Serveur | Req : vide. Res : `u32 users + u32 files`. | **Complet (codec)** | `RES` décodé et loggé ; `REQ` encodé mais non émis périodiquement. |
| **`OP_GLOBSEARCHREQ` / `RES`** | Serveur UDP | `PR_ED2K` (`0xE3`) sur UDP | `0x90`/`0x92`/`0x98` / `0x99` | Client $\leftrightarrow$ Serveur | Recherche globale UDP (payload = expression `OP_SEARCHREQUEST` verbatim ; RES layout `OP_SEARCHRESULT`). | **Complet (REQ2)** | `servers.search` émet `GLOBSEARCHREQ2` vers serveur connecté + serveurs `allowed` (UDP = TCP+4) ; `GLOBSEARCHRES` décodé et émis en `server.searchResult`. `GLOBSEARCHREQ3` (tags newtags) non émis. |
| **`OP_EMULE_INFO`** | Extension eMule | `PR_EMULE` (`0xC5`) | `0x01` | Bidirectionnel | Handshake étendu de capacités eMule. | **Non requis (Phase 6)** | Ignoré ou réservé pour négociations de fonctionnalités futures. |

---

## 3. Convention des Bornes de Blocs sur le Fil

### Formule eMule de Référence (`DownloadClient.cpp`)
Dans l'implémentation eMule officielle :
```cpp
// srchybrid/DownloadClient.cpp:812 — CUpDownClient::SendBlockRequests() (eMule 0.72a)
filedata->WriteUInt32(block->StartOffset);
filedata->WriteUInt32(block->EndOffset + 1);
```
La borne `EndOffset` en mémoire interne d'eMule est inclusive (`0..184319`), mais **la valeur écrite sur le fil est augmentée de 1 (`EndOffset + 1`)**, ce qui en fait une borne **strictement exclusive** sur le fil :
$$\text{bytes\_count} = \text{wire\_end} - \text{wire\_start}$$

### Prise en Compte dans eMule-Rust
- **Constructeur canonique** : `Ed2kBlockRange::from_wire_bounds(start, end)` vérifie `start < end` et transmet directement l'intervalle semi-ouvert `[start, end)`.
- **Compatibilité legacy** : `Ed2kBlockRange::from_legacy_inclusive_wire_bounds(start, end_inclusive)` permet de convertir des sources documentaires ou de vieux clients transmettant une borne inclusive (`end_inclusive.checked_add(1)`).
- **Bloc résiduel de fin de fichier** :
  Pour un fichier de 9 728 001 octets (`FIXTURE_ONE_PART_PLUS_ONE`), le dernier bloc est `[9728000, 9728001)` :
  - `start = 9_728_000`
  - `end = 9_728_001`
  - `len = 1` octet.

---

## 4. Tiers de Validation et Reproductibilité

1. **Tier 1 : Conformité Syntaxique de Corpus (Hors-Réseau)** :
   Vérification statique bit-à-bit contre le catalogue `manifest.json` avec sommes de contrôle SHA-256 (`crates/mule-protocol/tests/ed2k_corpus_tests.rs`).
2. **Tier 2 : Conformité par Émulation de Référence (`EmulatedReferencePeer`)** :
   Banc d'essai en processus sur socket loopback `127.0.0.1:0` reproduisant fidèlement les règles wire observées dans eMule 0.50a / aMule 2.3.3 (`crates/mule-protocol/tests/ed2k_reference_interop_tests.rs`).
3. **Tier 3 : Interopérabilité Observée contre Binaire Référence Externe** :
   Harnais automatisé pilotant un exécutable compilé et épinglé d'eMule ou aMule sur boucle locale avec profil de test éphémère (défini dans `reference_manifest.json`).
