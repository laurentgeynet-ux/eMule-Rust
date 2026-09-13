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
| **`OP_COMPRESSEDPART`** | Extension eMule zlib | `PR_EMULE_COMPRESSED` (`0xD4`) | `0x40` | Source $\rightarrow$ Client | Hash MD4 (16B), start (4B LE), payload compressé zlib. | **Rejet Explicite** | Erreur typée `UnsupportedProtocolByte(0xD4)` tant que les gardes de mémoire de décompression ne sont pas auditées. |
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
