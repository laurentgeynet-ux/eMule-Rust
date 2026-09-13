# Fiche Spécification : `OP_REQUESTPARTS` (`0x4B`)

## 1. Vue d'Ensemble

Le message `OP_REQUESTPARTS` permet à un client eD2K de demander à un pair distant une ou plusieurs plages d'octets (blocs de 180 KiB = 184 320 octets max) pour un fichier donné, identifié par son hash MD4 de 16 octets.

Dans l'architecture Emule-Sion, ce message est **exclusivement utilisé pour les transferts sur boucle locale fermée (loopback)** via la feature `ed2k-loopback`. Il fait l'objet d'un suivi strict par le composant `BlockRequestTracker` selon une clé composite `[file_hash, start, end]`.

---

## 2. Propriétés du Protocole

| Propriété | Valeur |
| :--- | :--- |
| **Opcode Hexadécimal** | `0x47` (Canonique eMule `OP_REQUESTPARTS`), `0x4B` (Historique alternate) |
| **Nom Symbolique** | `OP_REQUESTPARTS` / `OP_REQUESTPARTS_EMULE` |
| **Protocole Réseau** | `PR_ED2K` (`0xE3`) |
| **Direction** | Client $\rightarrow$ Pair distant |
| **Statut d'Exécution** | Loopback transfer-only (Phase 6.4 / 6.5) |
| **Plafond de Bloc Unitaire** | 184 320 octets (180 KiB) |

---

## 3. Wire Format (Format sur le Fil)

Le format canonique d'eMule v0.50a structure la demande de jusqu'à 3 blocs simultanés dans une même trame de 40 octets de payload :

```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x47 (canonique eMule) ou 0x4B (historique)
1       [u8; 16]      File Hash              Empreinte MD4 (16 octets) du fichier requis
17      u32[3] LE     Start Offsets          3 offsets de début de blocs (u32 little-endian)
29      u32[3] LE     End Offsets            3 offsets de fin de blocs (u32 little-endian, exclusif)
```

- **Taille de trame standard (3 slots)** : 41 octets sur le fil (1 octet protocole `0xE3` + 4 octets de longueur `0x28, 0x00, 0x00, 0x00` (40) + 1 octet opcode `0x47` ou `0x4B` + 40 octets payload).
- **Sémantique des Bornes sur le Fil** : Dans les sources officielles d'eMule (`DownloadClient.cpp`, `ListenSocket.cpp`), `endpos` est exclusif : `endpos = block->EndOffset + 1`. La longueur du bloc transféré est exactement `end - start`.
  - Constructeur canonique dans `Ed2kBlockRange` : `from_wire_bounds(start, end)` mappant directement les valeurs du fil vers `[start, end)` après validation `start < end`.
  - Helpers de compatibilité : `from_legacy_inclusive_wire_bounds(start, end_inclusive)` pour d'anciennes variantes donkey transmettant une borne inclusive.
- **Sentinelles pour slots non utilisés** :
  - eMule transmet `start == 0 && end == 0` pour les slots résiduels non demandés.
  - Le décodeur accepte également la sentinelle `start == u32::MAX && end == u32::MAX`.
- **Format compact (1 slot, legacy Donkey)** : 25 octets sur le fil (longueur 24 : 16 octets hash + 4 octets start + 4 octets end).

---

## 4. Règles de Validation & Bornes Numériques

1. **Relation d'offset** : Pour chaque plage demandée, `start < end`.
2. **Borne de taille de bloc** : `end - start <= 184_320` octets (taille maximale standard d'un bloc eD2K).
3. **Borne de fichier** : `end <= file_size`.
4. **Hash Nul Rejeté** : Un `file_hash` de 16 octets à zéro `[0u8; 16]` est strictement rejeté (`ZeroUserHash` ou `MalformedField`).

---

## 5. Préconditions d'État & Machine à États

1. **Session `Ready`** : `OP_REQUESTPARTS` ne peut être émis que si la session est dans l'état `Ready { peer }`.
2. **Disponibilité Confirmée** : La part couvrant l'intervalle `[start..end]` doit avoir été confirmée disponible via `OP_FILESTATUS` préalablement reçu.
3. **In-Flight Tracker (`BlockRequestTracker`)** :
   - Chaque plage est indexée par `BlockRequestKey { file_hash, start, end }`.
   - Tout doublon ou chevauchement de plage en vol est rejeté avec `DuplicateBlockRequest`.
   - Plafond de requêtes en vol : maximum 8 simultanées. Dépassement rejeté avec `InFlightCapacityReached`.
   - En cas de déconnexion ou fermeture, toutes les requêtes en vol sont nettoyées (`cancel_all()`).

---

## 6. Références Historiques
- eMule 0.50a : `opcodes.h` : `#define OP_REQUESTPARTS 0x47`
- eMule 0.50a : `DownloadClient.cpp:CDownloadClient::SendBlockRequests()`
- eMule 0.50a : `ListenSocket.cpp:CListenSocket::ProcessPacket()` (case `OP_REQUESTPARTS`)
- aMule : `src/UploadClient.cpp:CUploadClient::ProcessBlockRequests()`
