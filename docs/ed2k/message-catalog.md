# Fiches Normalisées des Messages eD2K Passifs

Ce catalogue décrit les formats sur le fil (wire layout), les bornes numériques, les règles de décodage et les références de code pour chaque message ciblé par la Phase 5.

---

## 1. Fiche : `OP_HELLO` (`0x01`)

### Direction
Bidirectionnel (généralement Client $\rightarrow$ Pair lors de l'établissement de connexion TCP).

### Protocol Byte
`0xE3` (`PR_ED2K`).

### Version et Extensions
- eDonkey2000 standard v1 : Hash utilisateur + Client ID + Port + Compteur de tags + Tags + Serveur IP/Port.
- Extensions eMule : inclusion de balises de version (`CT_VERSION`, `CT_EMULE_VERSION`), options diverses (`CT_EMULE_MISCOPTIONS`).

### Wire Format
```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x01 (OP_HELLO)
1       [u8; 16]      User Hash              Empreinte MD4 du client
17      u32 LE        Client ID              ID réseau eD2K (HighID ou LowID)
21      u16 LE        TCP Port               Port d'écoute TCP du client
23      u32 LE        Tag Count              Nombre de tags TLV suivants (N)
27      Tag[N]        Tags                   Séquence ordonnée de tags TLV
...     u32 LE        Server IP              IP du serveur connecté (ou 0.0.0.0 si non connecté)
...     u16 LE        Server Port            Port du serveur connecté (ou 0 si non connecté)
```

### Bornes & Limites
- Taille de trame maximale : 65 536 octets (64 KiB).
- Nombre maximal de tags : 256 tags.
- Longueur maximale d'une chaîne de tag : 1 024 octets.

### Décodage Emule-Sion
- Décodé dans la structure passive `Ed2kHello`.
- Les tags reconnus (`Name`, `Version`, `Port`) sont typés.
- Les tags inconnus sont préservés dans `unknown_tags: Vec<RawTag>`.
- Erreur si la trame est tronquée (`UnexpectedEof`), si le nombre de tags dépasse 256 (`TooManyTags`), ou si une chaîne dépasse 1024 octets (`OversizedString`).

### Fixtures Associées
- `hello_v1_minimal.bin` (SHA-256 dans le manifeste) : poignée de main valide avec 0 tag.
- `hello_v1_tags.bin` (SHA-256 dans le manifeste) : poignée de main avec tags nom, version et port.
- `malformed/hello_truncated_header.bin` : en-tête tronqué.
- `malformed/hello_truncated_tag.bin` : tag incomplet.
- `malformed/oversized_string.bin` : tag chaîne abusif (> 1024 octets).
- `malformed/too_many_tags.bin` : compteur de tags (> 256).

### Références
- eMule 0.50a : `srchybrid/UpDownClient.cpp:CUpDownClient::SendHelloPacket`, `CUpDownClient::ProcessHelloPacket`
- aMule : `src/DownloadClient.cpp:CDownloadClient::SendHelloPacket`

---

## 2. Fiche : `OP_HELLOANSWER` (`0x4C`)

### Direction
Pair distant $\rightarrow$ Client initiateur (en réponse à `OP_HELLO`).

### Protocol Byte
`0xE3` (`PR_ED2K`).

### Wire Format
Identique à `OP_HELLO`, avec l'opcode `0x4C` :
```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x4C (OP_HELLOANSWER)
1       [u8; 16]      User Hash              Empreinte MD4 du pair distant
17      u32 LE        Client ID              ID réseau eD2K du pair distant
21      u16 LE        TCP Port               Port d'écoute TCP du pair distant
23      u32 LE        Tag Count              Nombre de tags TLV suivants (N)
27      Tag[N]        Tags                   Séquence ordonnée de tags TLV
...     u32 LE        Server IP              IP du serveur eD2K auquel le pair est connecté
...     u16 LE        Server Port            Port du serveur eD2K
```

### Bornes & Limites
- Taille de trame maximale : 65 536 octets (64 KiB).
- Nombre maximal de tags : 256 tags.
- Longueur de chaîne maximale : 1 024 octets.

### Décodage Emule-Sion
- Décodé dans la structure `Ed2kHelloAnswer`.
- Même tolérance sur les tags inconnus (`unknown_tags: Vec<RawTag>`).

### Fixtures Associées
- `hello_answer.bin` (SHA-256 dans le manifeste) : réponse valide.

### Références
- eMule 0.50a : `srchybrid/UpDownClient.cpp:CUpDownClient::SendHelloAnswer`, `CUpDownClient::ProcessHelloAnswer`
- aMule : `src/DownloadClient.cpp:CDownloadClient::ProcessHelloAnswer`

---

## 3. Fiche : `OP_FILESTATUS` (`0x50`)

### Direction
Pair distant $\rightarrow$ Client (en réponse à `OP_FILESTATUSREQ`).

### Protocol Byte
`0xE3` (`PR_ED2K`).

### Wire Format
```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x50 (OP_FILESTATUS)
1       [u8; 16]      File Hash              Empreinte MD4 du fichier concerné
17      u16 LE        Part Count             Nombre total de parts (9 728 000 octets)
19      u8[M]         Status Bitfield        Masque binaire de disponibilité : ceil(Part Count / 8) octets
```

### Bornes & Limites
- `Part Count` maximal : 65 535 parts (représente jusqu'à ~637 Go pour des parts de 9,5 Mo).
- Taille minimale requise : $17 + \lceil\text{Part Count} / 8\rceil$ octets.
- Taille maximale de trame : 65 536 octets (suffisant pour $65535 / 8 = 8192$ octets de masque).

### Décodage Emule-Sion
- Décodé dans `Ed2kFileStatus { file_hash, part_count, status_bitfield }`.
- Si `Part Count == 0`, le masque est vide (le pair ne possède aucune part disponible).
- Rejet si la trame est tronquée par rapport à la taille déduite du `Part Count`.

### Fixtures Associées
- `file_status.bin` (SHA-256 dans le manifeste) : statut de parts d'un fichier multi-parts.

### Références
- eMule 0.50a : `srchybrid/UpDownClient.cpp:CUpDownClient::SendFileStatus`, `CUpDownClient::ProcessFileStatus`
- aMule : `src/UploadClient.cpp:CUploadClient::SendFileStatus`

---

## 4. Fiche : `OP_SETREQFILEID` (`0x47`)

### Direction
Client $\rightarrow$ Pair distant.

### Protocol Byte
`0xE3` (`PR_ED2K`).

### Wire Format
```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x47 (OP_SETREQFILEID)
1       [u8; 16]      File Hash              Empreinte MD4 du fichier demandé
```
Longueur totale de la trame : 17 octets (1 octet opcode + 16 octets hash).

---

## 5. Fiche : `OP_REQUESTPARTS` (`0x4B`)

### Direction
Client $\rightarrow$ Pair distant.

### Protocol Byte
`0xE3` (`PR_ED2K`).

### Wire Format
```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x4B (OP_REQUESTPARTS)
1       [u8; 16]      File Hash              Empreinte MD4 du fichier demandé
17      u32[3] LE     Start Offsets          3 offsets de début de blocs (180 KiB)
29      u32[3] LE     End Offsets            3 offsets de fin de blocs (180 KiB)
```
Taille totale sur le fil : 41 octets.

---

## 6. Fiche : `OP_SENDINGPART` (`0x46`)

### Direction
Pair distant $\rightarrow$ Client.

### Protocol Byte
`0xE3` (`PR_ED2K`).

### Wire Format
```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x46 (OP_SENDINGPART)
1       [u8; 16]      File Hash              Empreinte MD4 du fichier transféré
17      u32 LE        Block Start Offset     Offset de début du bloc dans le fichier
21      u32 LE        Block End Offset       Offset de fin du bloc dans le fichier
25      u8[...]       Payload Data           Octets bruts du bloc (End Offset - Start Offset)
```
Bornes maximales : bloc standard $\le$ 184 320 octets (180 KiB).
Taille maximale de trame : 262 144 octets (256 KiB).
