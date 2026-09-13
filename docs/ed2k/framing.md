# Spécification de Cadrage (Framing) du Protocole eD2K

## 1. Structure Générale d'une Trame Réseau

Toute communication eD2K (TCP) est découpée en trames binaires préfixées par un en-tête de 5 octets :

```text
+---------------+-------------------------------+---------------+-----------------------+
| Protocol (u8) |    Packet Length (u32 LE)     |  Opcode (u8)  |    Payload Bytes      |
|    1 octet    |           4 octets            |    1 octet    |  (Length - 1) octets  |
+---------------+-------------------------------+---------------+-----------------------+
```

### 1.1 Champs de l'En-Tête (Header)
1. **Protocol Byte (1 octet)** :
   - `0xE3` : Protocole standard eDonkey2000 (`PR_ED2K` / `OP_EDONKEYPROT`).
   - `0xC5` : Extensions eMule (`PR_EMULE` / `OP_EMULEPROT`).
   - `0xD4` : Trames compressées par zlib/deflate (`PR_EMULE_COMPRESSED` / `OP_PACKEDPROT`).
2. **Packet Length (4 octets, entier non signé Little-Endian)** :
   - Définit le nombre total d'octets qui suivent le champ de longueur dans la trame.
   - **Formule essentielle** :
     $$\text{Packet Length} = 1 \text{ (Opcode)} + \text{longueur du Payload}$$
     $$\text{Taille totale sur le fil (Wire Length)} = 5 \text{ (Header)} + \text{Packet Length} = 6 + \text{longueur du Payload}$$
   - **Contrainte minimale** : Tout paquet valide doit contenir au moins 1 octet pour l'opcode. Une valeur de `Packet Length < 1` est immédiatement considérée comme une violation de protocole (`FrameTooSmall`).
3. **Opcode (1 octet)** :
   - Code d'opération désignant la sémantique du message.
4. **Payload (Taille variable)** :
   - Données applicatives sérialisées, de longueur $\text{Packet Length} - 1$.

---

## 2. Endianness (Boutisme)

Le protocole eDonkey2000 et ses dérivés eMule utilisent **exclusivement l'ordre des octets little-endian** pour tous les entiers numériques transmis sur le réseau :
- `u16` : 2 octets little-endian.
- `u32` : 4 octets little-endian (notamment les adresses IPv4 dans les Client IDs et Server IPs, ainsi que les longueurs de chaînes et compteurs de tags).
- `u64` : 8 octets little-endian (tailles de gros fichiers > 4 Go dans les extensions de tags).
- `f32` : Flottants IEEE 754 32-bit sous forme little-endian.

Les empreintes cryptographiques (hashes MD4 de 16 octets pour les utilisateurs et les fichiers) sont stockées et transmises comme des tableaux bruts d'octets sans réordonnancement (`[u8; 16]`).

---

## 3. Séparation des Limites : eD2K vs Laboratoire

Le banc d'essai de laboratoire (`mule-protocol::lab`) utilise un protocole simplifié `0xEE` borné pour des transferts de blocs de 180 KiB.
Le protocole eD2K historique (`0xE3`, `0xC5`, `0xD4`) possède des limites distinctes :

| Paramètre | Protocole Laboratoire (`lab`) | Protocole eD2K Passif (`ed2k`) | Justification eD2K |
| :--- | :--- | :--- | :--- |
| **Trame maximale par défaut** | 1 048 576 octets (1 MiB) | 262 144 octets (256 KiB) | Suffisant pour blocs 180 KiB + overhead |
| **Trame Hello maximale** | 65 536 octets (64 KiB) | 65 536 octets (64 KiB) | Prévention d'attaque par déni de service (tags abusifs) |
| **Taille maximale de chaîne** | 16 384 octets | 1 024 octets | Noms de fichiers et pseudonymes usuels ≤ 512 ou 1024 o |
| **Nombre maximal de tags** | 256 | 256 | Limite historique suffisante pour poignée de main |
| **Trames compressées (0xD4)** | Rejet explicite | Rejet explicite | Interdit jusqu'à la phase d'audit hostile de décompression |
