# Fiche Spécification : Source Exchange (SX / SX2)

## 1. Vue d'Ensemble

Le Source Exchange permet à deux clients eD2K/eMule de s'échanger directement
les adresses de sources d'un fichier, sans passer par les serveurs ni Kad.
C'est le principal mécanisme de ventilation du swarm : chaque source contactée
redistribue les sources qu'elle connaît.

Référence de vérité : **eMule 0.72a** — `srchybrid/Opcodes.h:254-257`,
`ListenSocket.cpp:970-1011` (sous-opcodes multipacket) et `1165-1251`
(requête standalone), `PartFile.cpp:3602-3712` (`CreateSrcInfoPacket`),
`PartFile.cpp:3714-3890` (`AddClientSources`), `PartFile.cpp:2368-2405`
(`CanAddSource`), `DownloadClient.cpp:212-255` (`IsSourceRequestAllowed`).

---

## 2. Opcodes et Versionnement

| Opcode | Nom | Sens | Description |
| :--- | :--- | :--- | :--- |
| `0x81` | `OP_REQUESTSOURCES` | req | Requête SX1 : hash nu |
| `0x82` | `OP_ANSWERSOURCES` | resp | Réponse SX1 (versions 1-4 sniffées par taille d'entrée) |
| `0x83` | `OP_REQUESTSOURCES2` | req | Requête SX2 : `<ver><options><hash>` |
| `0x84` | `OP_ANSWERSOURCES2` | resp | Réponse SX2 : `<ver><hash><count>` |

- `SOURCEEXCHANGE2_VERSION = 4` (`Opcodes.h:254`).
- Protocole : `PR_EMULE` (`0xC5`) ; réponses compressées zlib en
  `PR_EMULE_COMPRESSED` (`0xD4`) quand le payload dépasse ~354 octets
  (`PackPacket`, `PartFile.cpp:3707-3708`).

### Négociation de la réponse (`PartFile.cpp:3629-3646`)

- `bIsSX2Packet && byRequestedVersion > 0` → `OP_ANSWERSOURCES2` à
  `byUsedVersion = min(byRequestedVersion, 4)`.
- Sinon → `OP_ANSWERSOURCES` (SX1) à `GetSourceExchange1Version()`
  (misc1 bits 12-15 ou tag `ET_SOURCEEXCHANGE` `0x23` de `OP_EMULEINFO`).
- Répondre SX2 v4 à un client SX1 est un écart filaire : il droppe le paquet.

### Version de la requête émise

- Le pair annonce SX2 (`SupportsSourceExchange2`, misc2 bit 10) →
  `OP_REQUESTSOURCES2` v4.
- Sinon SX1 (`GetSourceExchange1Version > 1`) → `OP_REQUESTSOURCES`.

---

## 3. Wire Format

### `OP_REQUESTSOURCES` (`0x81`)

```text
Offset  Type          Champ       Description
0       [u8; 16]      File Hash   Empreinte MD4 du fichier
```

### `OP_REQUESTSOURCES2` (`0x83`)

```text
Offset  Type          Champ       Description
0       u8            Version     Version SX demandée (typ. 4)
1       u16 LE        Options     Réservé (eMule envoie 0)
3       [u8; 16]      File Hash   Empreinte MD4 du fichier
```

### `OP_ANSWERSOURCES` (`0x82`, SX1)

```text
0       [u8; 16]      File Hash
16      u16 LE        Count       Nombre de sources
18      Entry[N]      Sources     N entrées (taille dépend de la version)
```

La version SX1 n'est pas sur le fil : eMule la déduit de la taille
(`PartFile.cpp:3777-3827`) — 12 B/entrée → v1, 28 B → v3, 29 B → v4.
Toute autre taille résiduelle → paquet droppé.

### `OP_ANSWERSOURCES2` (`0x84`, SX2)

```text
0       u8            Version     byUsedVersion (1-4, jamais 0 ni >4)
1       [u8; 16]      File Hash
17      u16 LE        Count
19      Entry[N]      Sources
```

### Entrée source

| Champ | Taille | Versions |
| :--- | :--- | :--- |
| Peer ID / IP | 4 B | toutes — network order si v<3, « hybrid » LE si v≥3 |
| TCP Port | 2 B LE | toutes |
| Server IP | 4 B LE | toutes |
| Server Port | 2 B LE | toutes |
| User Hash | 16 B | v≥2 (SX1 v3) |
| Crypt options | 1 B | v=4 |

Tailles : v1 = 12 B, v2/v3 = 28 B, v4 = 29 B.

Convention LowID : `id < 16 777 216` (`IsLowID`). Une entrée LowID désigne
un `client_id` serveur (champ ServerIP/Port = son serveur), pas une IP.

---

## 4. Sous-opcodes `OP_MULTIPACKET*`

eMule emballe ses requêtes fichier dans `OP_MULTIPACKET` (`0x92`),
`OP_MULTIPACKET_EXT` (`0xA4`, hash + taille u64) ou `OP_MULTIPACKET_EXT2`
(`0xA9`, `FileIdentifier`). Le hash de fichier vit dans l'**enveloppe** ;
les sous-opcodes `0x81` (sans payload) et `0x83` (`<ver u8><options u16>`,
sans hash) s'y réfèrent (`ListenSocket.cpp:970-1011`).

Sous-opcodes reconnus (`decode_multipacket_request`) :

| Opcode | Sub-request | Réponse attendue |
| :--- | :--- | :--- |
| `0x58` | `FileName` (+ ext-info part-status si `ExtendedRequestsVersion > 0`) | `OP_REQFILENAME_ANSWER` + `OP_FILESTATUS` en multipacket-answer |
| `0x4F` | `SetReqFileId` | — |
| `0x81` | `RequestSources` | `OP_ANSWERSOURCES` (paquet séparé) |
| `0x83` | `RequestSources2` | `OP_ANSWERSOURCES2` (paquet séparé) |
| `0x9E` | `AichFileHash` | `OP_AICHFILEHASHRES` en multipacket-answer |

Les réponses fichier sont groupées dans `OP_MULTIPACKETANSWER` (`0x93`) /
`OP_MULTIPACKETANSWER_EXT2` (`0xB0`) ; les réponses SX partent en paquets
indépendants comme eMule.

---

## 5. Filtrage à l'ingestion (`AddClientSources` / `CanAddSource`)

Appliqué dans `handle_sx_answer` (`mule-daemon/.../source_exchange.rs`) :

- hash du fichier ≠ hash attendu → paquet ignoré ;
- `version == 0` ou `> 4` (SX2), taille `count × entry_size ≠ len` → drop
  (`PartFile.cpp:3732-3770`) ;
- `user_hash == own_user_hash` → drop (`DownloadQueue.cpp:479-484`) ;
- HighID : `port == 0`, IP invalide/LAN/loopback (`IsGoodIP`, ici la
  policy réseau `mule-network-policy`, plus stricte), `ipfilter` → drop ;
- self-check : `client_id` HighID + notre port, ou LowID égal à notre
  `client_id` sur notre serveur → drop (`PartFile.cpp:2380-2403`) ;
- LowID quand nous sommes firewalled (`IsFirewalled`) → drop — le
  callback est irréalisable ; sinon persisté `lowid:<id>` ;
- cap : arrêt à `GetMaxSources()` (`max_sources` du fichier, défaut 400
  = `GetMaxSourcePerFileDefault`).

Les sources admises sont persistées dédupliquées (`PRIMARY KEY
file_hash+address`, upsert préservant `user_hash`/`crypt_options`/
`fail_count`/`cooldown`) puis ventilées par le swarm coordinator.

## 6. Sélection des sources servies (`CreateSrcInfoPacket`)

`select_sx_response_sources` :

- `!HasLowID() && IsValidSource()` : seules les sources vérifiées
  (états `Connected`/`Allowed`) avec IP filtrable valide ;
- `srcstatus` : masque de disponibilité présent et dont la longueur
  correspond à `ceil(part_count/8)` ;
- `reqstatus` : ≥ 1 part que le demandeur n'a pas (son `OP_FILESTATUS`
  ou l'ext-info multipacket `0x58`) ; si inconnu, ≥ 1 part tout court ;
- `nCount > 500` → 501 entrées maximum ;
- `!nCount` → **aucune réponse** ; fichier inconnu (ni partagé, ni
  download) → silence (`CheckFailedFileIdReqs`).

## 7. Throttles

| Timer | Valeur | Rôle |
| :--- | :--- | :--- |
| `SOURCECLIENTREASKS` | 40 min | cooldown requête par client (×4 = 160 min hors rare) |
| `SOURCECLIENTREASKF` | 5 min | cooldown réponse par fichier (×4 = 20 min hors rare) |
| `MINCOMMONPENALTY` | 4 | multiplicateur fichiers non rares |
| `RARE_FILE` | 50 | seuil « fichier rare » (sources connues) |
| `CONNECTION_LATENCY` | 22,05 s | marge sur les comparaisons de temps |

Côté **réponses** (`SetLastSrcReqTime`, `ListenSocket.cpp:1204-1215`) :
jamais répondu → OK ; part-file rare (≤ 50 sources) → > 40 min ;
sinon → > 160 min.

Côté **requêtes** (`IsSourceRequestAllowed`, `DownloadClient.cpp:212-255`) :
capacité SX requise, `GetSourceCount < GetMaxSourcePerFileSoft`
(`max_sources × 9/10`), clauses rare (`≤ RARE_FILE/5`, `≤ RARE_FILE`,
complétude du pair exclue) et commune (160 min + 20 min).

## 8. Re-ask périodique

eMule rattache `OP_REQUESTSOURCES` au cycle `SendFileRequest` (reask
~29-40 min). eMuleRust émet la requête sur les keep-alives de file
d'attente et chaque minute dans la boucle de transfert
(`maybe_send_sx_request`), uniquement si `IsSourceRequestAllowed` passe
— puis `SetLastAskedForSources` + `SetLastAnsweredTimeTimeout` miroir.

## 9. Décodage eMuleRust

- `decode_request_sources` : bornes strictes (≥16 B / ≥19 B), opcode
  vérifié, protocoles `PR_EMULE`/`PR_ED2K`.
- `decode_answer_sources` : version SX2 ∈ [1, 4] (0 et >4 droppés comme
  eMule), sniff SX1 par taille exacte, `count × entry_size == len` exigé,
  `count ≤ MAX_SX_SOURCES_PER_PACKET` (1000), endianness v<3 (network
  order → hybrid LE), décompression zlib `PR_EMULE_COMPRESSED`.
- `decode_multipacket_request` : enveloppe `Hash`/`HashAndSize`/
  `FileIdentifier`, ≤ 32 sous-opcodes, ext-info `0x58` lu selon
  `ExtendedRequestsVersion` négociée.
- `encode_answer_sources(ans, is_sx2, force_compress)` : opcode et layout
  selon la version négociée ; compression seulement si > 354 B sauf
  `force_compress`.

## 10. Références

- eMule 0.72a : `srchybrid/PartFile.cpp`, `ListenSocket.cpp`,
  `DownloadClient.cpp`, `Opcodes.h`, `DownloadQueue.cpp`.
- Implémentation : `crates/mule-protocol/src/ed2k/source_exchange/` (codec,
  types, tests round-trip) et
  `crates/mule-daemon/src/services/peer_session/source_exchange.rs`
  (ingestion, service, throttles, tests).
