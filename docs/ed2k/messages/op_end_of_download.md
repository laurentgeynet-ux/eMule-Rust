# Fiche Spécification : `OP_END_OF_DOWNLOAD` (`0x49`) / `OP_CHANGE_CLIENT_ID` (`0x4D`)

Référence de vérité : eMule 0.72a (`srchybrid/Opcodes.h`, `srchybrid/ListenSocket.cpp`).

## Problème de désambiguïsation

Deux couples d'opcodes partagent les mêmes valeurs filaires selon le profil client :

| Valeur | eMule 0.72a (`Opcodes.h`) | eDonkey2000 legacy |
| :--- | :--- | :--- |
| `0x49` | `OP_END_OF_DOWNLOAD` | `OP_FILESTATUSREQ` |
| `0x4D` | `OP_CHANGE_CLIENT_ID` | `OP_END_OF_DOWNLOAD` |

La résolution est exclusivement pilotée par `Ed2kCompatibilityProfile` — jamais par heuristique.

## `OP_END_OF_DOWNLOAD` — `0x49` (profils eMule/aMule)

### Layout

| Offset | Type | Champ | Notes |
| :--- | :--- | :--- | :--- |
| 0 | `u8` | Protocole | `0xE3` (`PR_ED2K`) |
| 1 | `u32` LE | Longueur payload | `17` (`1 + 16`) |
| 5 | `u8` | Opcode | `0x49` |
| 6 | `[u8; 16]` | Hash fichier | MD4 eD2K du fichier terminé |

### Sémantique eMule (`ListenSocket.cpp:537`)

- Le **downloader** notifie l'**uploader** qu'il a terminé le fichier.
- Réception : `if (size >= 16 && md4equ(client->GetUploadFileID(), packet))` → `RemoveFromUploadQueue(client)` ; sinon `CheckFailedFileIdReqs(packet)` (compteur anti-flood, 6 échecs → strike, 2 strikes → ban).
- eMule 0.72a **n'émet jamais** ce message (opcode legacy conservé en réception).
- La socket n'est **pas** fermée par eMule — la session continue.

### Sous profil `LegacyEdonkey`

La même valeur `0x49` est `OP_FILESTATUSREQ` (demande de statut des parts, payload `hash16` identique). Le décodeur `decode_file_status_req_with_profile` refuse `0x49` sous les profils eMule et refuse `0x4F` sous `LegacyEdonkey`.

## `OP_CHANGE_CLIENT_ID` — `0x4D` (profils eMule/aMule)

### Layout (`Opcodes.h:219` — `<ID_old 4><ID_new 4>`)

| Offset | Type | Champ | Notes |
| :--- | :--- | :--- | :--- |
| 6 | `u32` LE | `newUserID` | Nouvel ID ed2k du client |
| 10 | `u32` LE | `newServerIP` | IP du serveur ayant attribué l'ID |

### Sémantique eMule (`ListenSocket.cpp:590`)

- `IsLowID(newUserID)` → mise à jour de l'ID **seulement** si le serveur `newServerIP` est connu (`serverlist->GetServerByIP`).
- `newUserID == client->GetIP()` → le client a un HighID → `SetUserIDHybrid(ntohl(newUserID))`.
- Données additionnelles tolérées (log debug seulement).
- eMule 0.72a **n'émet jamais** ce message non plus.

## Politique d'émission côté Rust

- `OP_END_OF_DOWNLOAD` (`0x49`) est émis quand une session de download se termine (`peer_session/transfer.rs`) — le payload `hash16` permet à un uploader eMule/aMule de libérer notre entrée de file.
- `OP_CHANGE_CLIENT_ID` (`0x4D`) n'est **jamais émis** ; en réception il est décodé et journalisé (mise à jour d'ID non nécessaire : l'identité d'upload est indexée par `client_key` userhash/IP, pas par l'ID ed2k).

## Vecteur de test

`PR_ED2K` + `len=17` + `0x49` + `hash16` :

```
E3 11 00 00 00 49 4A 52 49 53 54 45 52 4D 49 4E 41 54 45 44 21 21
```

- Profil `Emule072a` : décodé comme notification end-of-download (pas `FileStatusReq`).
- Profil `LegacyEdonkey` : décodé comme `FileStatusReq`.

`PR_ED2K` + `len=9` + `0x4D` + `u32(0x01020304)` + `u32(0x0A0B0C0D)` :

```
E3 09 00 00 00 4D 04 03 02 01 0D 0C 0B 0A
```
