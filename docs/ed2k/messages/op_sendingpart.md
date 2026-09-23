# Fiche Spécification : `OP_SENDINGPART` (`0x46`)

## 1. Vue d'Ensemble

Le message `OP_SENDINGPART` transporte un bloc de données binaires de fichier d'un pair distant vers le client récepteur, en réponse à une demande préalable formulée par `OP_REQUESTPARTS`.

Dans l'architecture EMule-Rust :
- Ce message est **strictement corrélé** avec une requête en vol enregistrée dans `BlockRequestTracker`.
- Les octets reçus sont validés puis écrits exclusivement au travers de l'abstraction de stockage `DownloadSink`.
- La taille du fichier physique `.part` et le calcul du hash eD2K progressent de manière déterministe et atomique via SQLite WAL.

---

## 2. Propriétés du Protocole

| Propriété | Valeur |
| :--- | :--- |
| **Opcode Hexadécimal** | `0x46` |
| **Nom Symbolique** | `OP_SENDINGPART` |
| **Protocole Réseau** | `PR_ED2K` (`0xE3`) |
| **Direction** | Pair distant $\rightarrow$ Client |
| **Statut d'Exécution** | Loopback transfer-only (Phase 6.4) |
| **Taille de Trame Maximale** | 262 144 octets (256 KiB) |
| **Taille de Bloc Standard** | $\le$ 184 320 octets (180 KiB) |

---

## 3. Wire Format (Format sur le Fil)

```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x46 (OP_SENDINGPART)
1       [u8; 16]      File Hash              Empreinte MD4 (16 octets) du fichier transféré
17      u32 LE        Block Start Offset     Offset de début du bloc dans le fichier (u32 little-endian)
21      u32 LE        Block End Offset       Offset de fin du bloc dans le fichier (u32 little-endian)
25      u8[...]       Payload Data           Octets bruts du bloc (longueur = End Offset - Start Offset)
```

- **Taille de l'en-tête de charge utile** : 24 octets (16 octets de hash + 4 octets start + 4 octets end).
- **Taille totale de la trame** : `5 (en-tête framing) + 1 (opcode) + 24 (en-tête block) + data.len() = 30 + data.len()` octets.
- **Vérification d'égalité stricte** : Le nombre d'octets de données restants dans la trame doit correspondre **exactement** à `end_offset - start_offset`. Tout écart déclenche l'erreur `BlockPayloadMismatch`.

---

## 4. Règles de Décodage & Rejets de Sécurité

1. **Trame Tronquée** : Si la charge utile fait moins de 24 octets, erreur `UnexpectedEof`.
2. **Ordre d'Offset Invalide** : Si `start_offset >= end_offset`, rejet immédiat avec `InvalidBlockOffsets`.
3. **Discordance de Taille** : Si `(end_offset - start_offset) as usize != data.len()`, rejet immédiat avec `BlockPayloadMismatch`.
4. **Dépassement de Capacité Trame** : Si la trame dépasse `ED2K_MAX_FRAME_DEFAULT` (256 KiB) ou si le bloc dépasse 184 320 octets, rejet avec `FrameTooLarge`.
5. **Bloc Non Sollicité** : Si aucun bloc correspondant à `(file_hash, start, end)` n'est en vol dans le tracker de session, rejet immédiat avec `UnsolicitedBlock`.
6. **Bloc Dupliqué** : Une seconde livraison pour une plage déjà traitée est ignorée et n'incrémente pas la progression.
7. **Trame Compressée 0xD4** : Rejet immédiat avec `CompressedProtocolUnsupported`.

---

## 5. Intégration `DownloadSink` & Persistance

Dès validation par la session et le `BlockRequestTracker` :
1. Le bloc est converti en `ReceivedBlock { file_hash, start_offset: start as u64, data }`.
2. Il est transmis au `DownloadSink::write_block()`.
3. L'écriture physique est réalisée dans le fichier `.part` temporaire.
4. L'intervalle est enregistré dans `RangeSet` et validé dans SQLite (WAL).
5. L'événement réactif `event.downloadUpdated` est notifié aux clients locaux.

---

## 6. Références Historiques
- eMule 0.72a : `srchybrid/ListenSocket.cpp:573` : `CListenSocket::ProcessPacket()` (case `OP_SENDINGPART`)
- eMule 0.72a : `srchybrid/DownloadClient.cpp:956` : `CUpDownClient::ProcessBlockPacket()`
- aMule : `src/DownloadClient.cpp:CDownloadClient::ProcessBlockPacket()`
