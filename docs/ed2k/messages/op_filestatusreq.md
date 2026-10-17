# Fiche Spécification : `OP_FILESTATUSREQ` (`0x49`)

## 1. Vue d'Ensemble

Le message `OP_FILESTATUSREQ` permet à un client eD2K d'interroger un pair distant sur sa possession et disponibilité des différentes parts (blocs de 9 728 000 octets) d'un fichier identifié par son hash MD4 de 16 octets.

Dans l'architecture eMuleRust, ce message est **exclusivement utilisé pour la découverte de disponibilité en boucle locale fermée (loopback)**. Il ne déclenche aucun transfert de données, aucune réservation de slot, et aucune écriture dans un fichier `.part`.

---

## 2. Propriétés du Protocole

| Propriété | Valeur |
| :--- | :--- |
| **Opcode Hexadécimal** | `0x4F` (Canonique eMule `OP_SETREQFILEID`), `0x49` (Historique alternate `OP_FILESTATUSREQ`) |
| **Nom Symbolique** | `OP_SETREQFILEID` / `OP_FILESTATUSREQ` |
| **Protocole Réseau** | `PR_ED2K` (`0xE3`) |
| **Direction** | Client $\rightarrow$ Pair distant |
| **Statut d'Exécution** | Loopback control-only (Phase 6.3 / 6.5) |
| **Transfert de Données Associé** | Aucun (disponibilité pure) |

---

## 3. Wire Format (Format sur le Fil)

Le format est strictement déterministe et de longueur fixe :

```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x4F (canonique eMule) ou 0x49 (historique)
1       [u8; 16]      File Hash              Empreinte MD4 (16 octets) du fichier requis
```

- **Taille du payload brut** : exactement 17 octets (1 octet d'opcode + 16 octets de hash).
- **Taille de la trame encapsulée** : 22 octets (1 octet protocole `0xE3` + 4 octets de taille LE `0x11, 0x00, 0x00, 0x00` + 17 octets de payload).

---

## 4. Préconditions d'État & Machine à États

1. **État Requis** : La session doit être strictement dans l'état `Ready { peer }`.
   - Si une émission de `OP_FILESTATUSREQ` est tentée avant l'état `Ready` (ex. pendant `Connecting`, `AwaitingHello` ou `AwaitingHelloAnswer`), la session rejette l'opération avec l'erreur `Ed2kSessionError::RequestBeforeReady`.
2. **Corrélation des Requêtes en Vol** :
   - Chaque requête est enregistrée dans le `FileStatusTracker` avec son instant d'envoi (`sent_at: Instant`).
   - L'émission d'une seconde requête pour un hash déjà en attente auprès du même pair est refusée avec `Ed2kSessionError::DuplicateFileStatusRequest`.
   - Le nombre maximal de requêtes en vol par pair est borné (plafond configurable, défaut : 16) pour éviter l'épuisement mémoire. Tout dépassement génère `Ed2kSessionError::TooManyPendingRequests`.

---

## 5. Décodage et Gestion des Erreurs

- **EOF Inattendu (Trame tronquée)** : Si le payload fait moins de 17 octets, retour d'erreur `Ed2kErrorKind::UnexpectedEof { needed: 17, offset: ... }`.
- **Octets Superflus (Trame trop longue)** : Tout octet présent au-delà des 16 octets de hash est considéré comme une anomalie de protocole et rejeté avec `Ed2kErrorKind::MalformedField`.
- **Hash Nul** : Un hash composé uniquement de zéros `[0u8; 16]` est rejeté comme invalide.

---

## 6. Fixtures de Test

- **`file_status_req.bin`** : Trame binaire valide de 22 octets avec hash `0xCC` répété 16 fois.
  - SHA-256 consigné dans `manifest.json`.
  - Provenance : généré selon la spécification formelle eD2K.

---

## 7. Références Historiques

- **eMule 0.72a** : `CUpDownClient::SendFileStatusRequest()` n'existe plus — `0x49` y est `OP_END_OF_DOWNLOAD` (`opcodes.h:215`). L'état fichier s'obtient via `OP_SETREQFILEID`, réponse `OP_FILESTATUS` (`ListenSocket.cpp:404`), traitée dans `DownloadClient.cpp:509` (`CUpDownClient::ProcessFileStatus`).
- **eMule 0.50a** *(historique)* : `srchybrid/UpDownClient.cpp` : `CUpDownClient::SendFileStatusRequest()`
  ```cpp
  Packet* packet = new Packet(OP_FILESTATUSREQ, 16);
  md4cpy(packet->pBuffer, reqfile->GetFileHash());
  SendPacket(packet, true);
  ```
- **aMule 2.3.3** : `src/DownloadClient.cpp` : `CDownloadClient::SendFileStatusRequest()`
