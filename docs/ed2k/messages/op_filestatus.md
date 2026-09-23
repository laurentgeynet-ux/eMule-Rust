# Fiche Spécification : `OP_FILESTATUS` (`0x50`)

## 1. Vue d'Ensemble

Le message `OP_FILESTATUS` est émis par un pair eD2K en réponse à une requête `OP_FILESTATUSREQ`. Il annonce le nombre total de parts constituant le fichier ainsi qu'un masque binaire (bitfield) indiquant quelles parts sont détenues et partagées par ce pair.

Dans l'architecture EMule-Rust, ce message joue deux rôles :

- **Production (`peer_session`)** : chaque `FILESTATUS` reçu — réponse initiale *et* keep-alive en file d'attente — enregistre le masque dans le store de disponibilité (`record_source_availability`) et déclenche `downloadUpdated`. C'est la source unique de la barre de parts (`availableSourceCount` → dégradé rouge/bleu façon `CPartFile::DrawStatusBar`) : seules les sources **ayant répondu** comptent (eMule `GetPartStatus() != NULL`), les masques périmés > 30 min sont ignorés, et `part_count == 0` est traité comme **source complète** (eMule `ProcessFileStatus` → `memset(m_abyPartStatus, 1, …)`).
- **Lab / contrôle (`Ed2kSession`)** : la machine d'états dédiée reste strictement d'observation sur boucle locale — elle alimente les statistiques de disponibilité (`sourceCount = 1`) sans jamais initier de demande de bloc (`OP_REQUESTPARTS`), sans allouer de tampon, et sans ouvrir ni écrire dans un fichier `.part`.

---

## 2. Propriétés du Protocole

| Propriété | Valeur |
| :--- | :--- |
| **Opcode Hexadécimal** | `0x50` |
| **Nom Symbolique** | `OP_FILESTATUS` |
| **Protocole Réseau** | `PR_ED2K` (`0xE3`) |
| **Direction** | Pair distant $\rightarrow$ Client |
| **Statut d'Exécution** | Production (`peer_session`, disponibilité des parts) + loopback control-only (`Ed2kSession`, Phase 6.3) |
| **Transfert de Données Associé** | Aucun (métadonnée pure) |

---

## 3. Wire Format (Format sur le Fil)

```text
Offset  Type          Champ                  Description
0       u8            Opcode                 0x50 (OP_FILESTATUS)
1       [u8; 16]      File Hash              Empreinte MD4 (16 octets) du fichier concerné
17      u16 LE        Part Count             Nombre total de parts du fichier (9 728 000 octets / part)
19      u8[M]         Availability Bitmask   Masque de bits : M = ceil(Part Count / 8) octets
```

### 3.1 Calcul de la Longueur du Masque
Pour un nombre de parts $N = \text{part\_count}$ :
$$\text{expected\_mask\_bytes} = \left\lceil \frac{N}{8} \right\rceil = \text{usize::from}(N).\text{div\_ceil}(8)$$

- Si $N = 0$, le masque attendu fait 0 octet (masque vide).
- Si $1 \le N \le 8$, le masque attendu fait 1 octet.
- Si $N = 12$, le masque attendu fait 2 octets.
- Si $N = 53$, le masque attendu fait 7 octets.

### 3.2 Ordre des Bits dans chaque Octet (LSB-First)
Dans le protocole standard eDonkey2000 / eMule, les parts sont indexées à partir de 0 et disposées selon l'ordre **LSB-first (Least Significant Bit first)** au sein de chaque octet :
- La part $i$ ($0 \le i < N$) est localisée à :
  - **Octet** : $i / 8$ (index 0-basé dans le masque binaire)
  - **Bit** : $i \pmod 8$ (bit de poids $2^{i \pmod 8}$)
- Formule de vérification :
  $$\text{has\_part}(i) = (\text{raw\_bytes}[i / 8] \ \& \ (1 \ll (i \pmod 8))) \neq 0$$

### 3.3 Règle Stricte des Bits Inutilisés (Trailing Bits)
Lorsque $N \pmod 8 \neq 0$, le dernier octet du masque contient $8 - (N \pmod 8)$ bits non assignés dans ses positions de poids fort.
- **Politique EMule-Rust** : **Rejet Strict**. Tous les bits au-delà de l'indice $(N \pmod 8) - 1$ dans le dernier octet **doivent obligatoirement être nuls** (`0`).
- Si un bit inutilisé est positionné à 1, le message est immédiatement rejeté avec l'erreur `Ed2kErrorKind::InvalidBitfieldTrailingBits`.

---

## 4. Préconditions d'État & Règles de Session

1. **État Requis** : La session doit être dans l'état `Ready { peer }`. Tout statut reçu hors de cet état est rejeté (`Ed2kSessionError::ResponseBeforeReady`).
2. **Corrélation Stricte avec une Requête Pending** :
   - Le `file_hash` reçu doit correspondre exactement à une requête en attente dans le `FileStatusTracker`.
   - Si aucune requête n'est en attente pour ce hash, la réponse est rejetée comme non sollicitée (`Ed2kSessionError::UnsolicitedFileStatus`).
   - Une réponse valide retire immédiatement la requête du tracker. Une seconde réponse pour le même hash est donc rejetée comme doublon non sollicité.
3. **Plafond de Parts** :
   - `part_count` ne doit pas dépasser le plafond de sécurité configuré (`limits.max_parts`, maximum 65 535). Tout dépassement provoque `Ed2kErrorKind::MalformedField`.
4. **Cas Particulier `part_count == 0`** :
   - Décodage passif : accepté pour observation historique (retourne un masque vide).
   - Session lab (`Ed2kSession`) : rejeté avec `Ed2kSessionError::ZeroPartCountInActiveSession` car un fichier réel eD2K partagé possède au moins une part.
   - **Production (`peer_session`)** : interprété comme **source complète** — eMule `ProcessFileStatus` matérialise un masque plein (`memset(m_abyPartStatus, 1, …)`) ; `compute_part_summaries` compte une telle source pour **toutes** les parts (`availableSourceCount`).

---

## 5. Décodage et Gestion des Erreurs

| Condition Détectée | Erreur Typée | Action |
| :--- | :--- | :--- |
| Trame plus courte que $19 + \lceil N/8 \rceil$ | `Ed2kErrorKind::UnexpectedEof` | Échec de trame |
| Trame contenant des octets au-delà du masque | `Ed2kErrorKind::MalformedField` | Rejet de trame |
| $N > \text{limits.max\_parts}$ | `Ed2kErrorKind::MalformedField` | Rejet de trame |
| Bit résiduel à 1 dans le dernier octet | `Ed2kErrorKind::InvalidBitfieldTrailingBits` | Rejet de trame |
| Hash MD4 non demandé ou déjà résolu | `Ed2kSessionError::UnsolicitedFileStatus` | Rejet de session |
| Statut reçu avant achèvement du handshake | `Ed2kSessionError::ResponseBeforeReady` | Rejet de session |
| Protocole compressé 0xD4 | `Ed2kSessionError::CompressedProtocolUnsupported` | Rejet de session |

---

## 6. Fixtures de Test

- **`file_status.bin`** : Trame de 26 octets (part_count = 12, bitfield `ff0f`, parts 0..=11 disponibles).
- **`file_status_multi_parts.bin`** : Trame de 27 octets (part_count = 10, bitfield `8303`, parts 0, 1, 7, 8, 9 disponibles).
  - Octet 0 : `0x83` = `0b1000_0011` (bits 0, 1, 7)
  - Octet 1 : `0x03` = `0b0000_0011` (bits 0, 1 = parts 8, 9; bits 2..=7 nuls).
  - SHA-256 consigné dans `manifest.json`.

---

## 7. Références Historiques

- **eMule 0.72a** : envoi inline dans `srchybrid/ListenSocket.cpp:404` (réponse `OP_FILESTATUS` à `OP_SETREQFILEID`), réception `srchybrid/DownloadClient.cpp:509` : `CUpDownClient::ProcessFileStatus()`
- **eMule 0.50a** *(historique)* : `srchybrid/UpDownClient.cpp` : `CUpDownClient::SendFileStatus()`, `CUpDownClient::ProcessFileStatus()`
  ```cpp
  Packet* packet = new Packet(OP_FILESTATUS, 16 + 2 + reqfile->GetPartCount());
  // Remplit le hash MD4, partCount en uint16 LE, et le bitfield
  ```
- **aMule 2.3.3** : `src/UploadClient.cpp` : `CUploadClient::SendFileStatus()`
