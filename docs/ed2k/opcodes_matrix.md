# Matrice des Opcodes et Protocoles eD2K / eMule

## 1. Contexte & Historique des Variantes

Le protocole eDonkey2000 originel et ses extensions successives (eMule, aMule, cDonkey, MLDonkey) ont introduit des évolutions, des réassignations et des formats alternatifs selon les générations logicielles et les octets de protocole (`0xE3`, `0xC5`, `0xD4`).

Cette matrice formalise les attributions exactes, les directions, les formats sur le fil et le niveau de support dans **Emule-Sion**, en se fondant sur les sources de référence versionnées :
- **eMule 0.50a** : `SomeSupport/eMule` (`srchybrid/opcodes.h`, `DownloadClient.cpp`, `ListenSocket.cpp`, `UploadClient.cpp`).
- **aMule 2.3.3** : `src/DownloadClient.cpp`, `src/UploadClient.cpp`.
- **eDonkey2000** : Spécifications historiques reverse-engineérées du protocole passif v1.

---

## 2. Matrice Comparative des Opcodes et Formats Wire

| Message Symbolique | Variante & Source | Protocole Wire | Opcode Wire | Direction | Structure & Layout Payload | Support Emule-Sion | Règle / Invariant Associé |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **`OP_HELLO`** | Standard eD2K & eMule 0.50a | `PR_ED2K` (`0xE3`) | `0x01` | Client $\rightarrow$ Pair | Hash (16B), ClientID (4B LE), TCP Port (2B LE), Tag count (4B LE), Tags, Server IP (4B LE), Server Port (2B LE) | **Complet** | Rejet si hash nul ou version incompatible |
| **`OP_HELLO_ANSWER`** | Standard eD2K & eMule 0.50a | `PR_ED2K` (`0xE3`) | `0x4C` | Pair $\rightarrow$ Client | Hash (16B), ClientID (4B LE), TCP Port (2B LE), Tag count (4B LE), Tags, Server IP (4B LE), Server Port (2B LE) | **Complet** | Préservation des tags inconnus sans désérialisation non sécurisée |
| **`OP_SETREQFILEID`** | eMule 0.50a (`opcodes.h:73`) & aMule 2.3.3 | `PR_ED2K` (`0xE3`) | `0x4F` | Client $\rightarrow$ Source | Hash MD4 (16B). Déclenche l'envoi de `OP_FILESTATUS` (`0x50`) par la source. | **Complet** | Format canonique 17B (opcode + hash). Pas d'accès disque. |
| **`OP_FILESTATUSREQ`** | eDonkey legacy / docs alternatifs | `PR_ED2K` (`0xE3`) | `0x49` | Client $\rightarrow$ Source | Hash MD4 (16B). *(Dans eMule 0.50a, 0x49/0x4D = `OP_END_OF_DOWNLOAD`).* | **Complet (compatibilité)** | Décodé et encodé de manière transparente comme `FileStatusReq`. |
| **`OP_FILESTATUS`** | Standard eD2K & eMule 0.50a | `PR_ED2K` (`0xE3`) | `0x50` | Source $\rightarrow$ Client | Hash MD4 (16B), Part count (2B LE), Bitfield LSB-first ($\lceil part\_count / 8 \rceil$ bytes). | **Complet** | Validation stricte des bits de fin résiduels (doivent valoir 0). |
| **`OP_REQUESTPARTS`** | eMule 0.50a (`opcodes.h:71`) & aMule 2.3.3 | `PR_ED2K` (`0xE3`) | `0x47` | Client $\rightarrow$ Source | Hash MD4 (16B), 3× u32 LE start, 3× u32 LE end exclusif (`endpos = block->EndOffset + 1`). Sentinelles `(0, 0)` ou `(u32::MAX, u32::MAX)`. (40B total). | **Complet** | Tracké dans `BlockRequestTracker` (max 8 en vol, zéro chevauchement). |
| **`OP_REQUESTPARTS`** | eDonkey legacy / docs historiques | `PR_ED2K` (`0xE3`) | `0x4B` | Client $\rightarrow$ Source | 40B (3 slots) ou 24B (1 slot: hash 16B + start 4B + end 4B). *(Dans eMule 0.50a, 0x4B = `OP_ASKSHAREDFILESANSWER`).* | **Complet (compatibilité)** | Décodé de manière transparente dans les deux formats 24B et 40B. |
| **`OP_SENDINGPART`** | Standard eD2K & eMule 0.50a | `PR_ED2K` (`0xE3`) | `0x46` | Source $\rightarrow$ Client | Hash MD4 (16B), start (4B LE), end (4B LE exclusif), chunk bytes (`end - start` octets, max 180 KiB). | **Complet** | Écriture exclusive via `DownloadSink` dans `.part` sous verrou WAL. |
| **`OP_COMPRESSEDPART`** | Extension eMule zlib | `PR_EMULE` (`0xC5`) ou `PR_EMULE_COMPRESSED` (`0xD4`) | `0x40` (`0xA1` variante I64) | Source $\rightarrow$ Client | Hash MD4 (16B), start (4B/8B LE), taille compressée totale (4B LE), chunk zlib ≤ 10 240 B (envoyé sous `OP_EMULEPROT` chez eMule). | **Complet (bidirectionnel)** | Décodage incrémental `flate2` côté download ; émission zlib `Z_BEST_SPEED` côté upload si `nDataCompVer` (MiscOptions1 bits 20-23) ; repli `SENDINGPART` si non rentable. |
| **`OP_HASHSETREQUEST`** | eMule 0.72a | `PR_ED2K` (`0xE3`) | `0x51` | Client $\rightarrow$ Source | Hash MD4 (16B). Déclenche `OP_HASHSETANSWER`. | **Complet** | Envoyé à la connexion si le hashset est inconnu ; servi pour fichiers partagés avec vérification du hash racine. |
| **`OP_HASHSETANSWER`** | eMule 0.72a | `PR_ED2K` (`0xE3`) | `0x52` | Source $\rightarrow$ Client | Hash MD4 (16B), `u16` count, N× hash MD4 par part. | **Complet** | Persisté (`file_part_hashes`) ; vérification incrémentale et éviction des parts corrompues. |
| **`OP_AICHFILEHASHREQ` / `RES`** | eMule | `PR_EMULE` (`0xC5`) | `0x14` / `0x15` | Client $\leftrightarrow$ Source | Req : hash16. Res : hash16 + racine SHA-1 (20B). | **Complet** | Consensus de confiance `CAICHUntrustedHash` (≥10 IP uniques masquées, ≥92 %). |
| **`OP_AICHREQUEST` / `ANSWER`** | eMule | `PR_EMULE` (`0xC5`) | `0x12` / `0x13` | Client $\leftrightarrow$ Source | Req : hash16 + `u16` part + master20. Ans : entrées `(ident, hash)` 16/32-bit ou hash16 seul (échec). | **Complet** | Récupération au niveau bloc 180 Ko sur arbre de Merkle à double granularité. |
| **`OP_OFFERFILES`** | eD2K serveur | `PR_ED2K` (`0xE3`) | `0x15` | Client $\rightarrow$ Serveur | `u32` count + par fichier hash16 + `u32` client_id + `u16` port + tags. | **Complet** | Sémantique `CreateOfferedFilePacket` : marqueurs compression, HighID/LowID, tags `FT_FILESIZE_HI` > 4 Gio, keep-alive vide, plafond 200. |
| **`OP_CALLBACKREQUEST`** | eD2K serveur | `PR_ED2K` (`0xE3`) | `0x1C` | Client $\rightarrow$ Serveur | `u32` client_id nu. | **Complet** | Sources LowID `lowid:<id>` rappelées via relai serveur ; adoption des connexions entrantes correspondantes. |
| **`OP_CALLBACKREQUESTED` / `FAIL`** | eD2K serveur | `PR_ED2K` (`0xE3`) | `0x35` / `0x36` | Serveur $\rightarrow$ Client | `IP4 + port2` (+ optionnel `crypt u8 + userhash16`). | **Complet (décodage)** | Loggé et routé ; connexion sortante en réponse non implémentée. |
| **`OP_QUEUERANKING`** | eD2K | `PR_ED2K` (`0xE3`) | `0x60` | Source $\rightarrow$ Client | `u32` rang de file d'attente. | **Complet** | Propagé au snapshot de session (`queue_rank`) et aux événements. |
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
// srchybrid/DownloadClient.cpp: CDownloadClient::SendBlockRequests()
filedata->WriteUInt32(block->StartOffset);
filedata->WriteUInt32(block->EndOffset + 1);
```
La borne `EndOffset` en mémoire interne d'eMule est inclusive (`0..184319`), mais **la valeur écrite sur le fil est augmentée de 1 (`EndOffset + 1`)**, ce qui en fait une borne **strictement exclusive** sur le fil :
$$\text{bytes\_count} = \text{wire\_end} - \text{wire\_start}$$

### Prise en Compte dans Emule-Sion
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
