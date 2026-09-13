# Spécification des Limites Strictes eD2K & Protection Anti-OOM

Ce document formalise les plafonds de taille et de cardinalité appliqués par les décodeurs de `mule-protocol::ed2k` pour se prémunir contre les entrées malveillantes, les débordements de mémoire (OOM) et les attaques par déni de service.

---

## 1. Principes Directeurs

1. **Vérification en Amont (Pre-allocation Check)** :
   Aucun vecteur dynamique ou chaîne de caractères n'est alloué sur la base d'un entier lu sur le réseau sans avoir été préalablement borné par une constante stricte.
2. **Bornes Spécifiques par Opcode** :
   Chaque message dispose d'un plafond correspondant à sa sémantique métier plutôt qu'à une limite globale trop généreuse.
3. **Absence d'Ambiguïté sur les Types Inconnus** :
   Les données inconnues sont limitées en taille maximale avant d'être encapsulées dans un conteneur brut (`RawTag`).

---

## 2. Table des Plafonds Appliqués

| Paramètre | Plafond Strict | Contexte / Opcode | Conséquence en cas de dépassement |
| :--- | :---: | :--- | :--- |
| **Taille de Trame Hello** | 65 536 octets (64 KiB) | `OP_HELLO`, `OP_HELLOANSWER` | `Ed2kParseError::FrameTooLarge` |
| **Taille de Trame Part / Data** | 262 144 octets (256 KiB) | `OP_SENDINGPART` | `Ed2kParseError::FrameTooLarge` |
| **Taille de Trame Métadonnées** | 65 536 octets (64 KiB) | `OP_FILESTATUS`, `OP_HASHSETANSWER` | `Ed2kParseError::FrameTooLarge` |
| **Nombre Maximal de Tags** | 256 tags | Tous messages contenant des tags | `Ed2kParseError::TooManyTags` |
| **Longueur de Chaîne Textuelle** | 1 024 octets | Noms d'utilisateur, logiciels, fichiers | `Ed2kParseError::OversizedString` |
| **Nombre Maximal de Parts** | 65 535 parts | `OP_FILESTATUS` | `Ed2kParseError::MalformedField` |
| **Taille Maximale de Blob de Tag**| 65 536 octets | Tags de type Blob (`0x07`) | `Ed2kParseError::FrameTooLarge` |

---

## 3. Détail des Vérifications lors du Parsing de Tags

Chaque tag TLV (Type-Length-Value) est décodé selon les étapes suivantes :
1. **Lecture du Type de Tag (1 octet)** :
   - Rejet si fin de flux inattendue (`UnexpectedEof`).
2. **Lecture du Nom de Tag** :
   - Longueur du nom de tag (2 octets LE).
   - Si $\text{longueur} == 1$ : lecture d'un identifiant numérique spécial (1 octet).
   - Si $\text{longueur} > 1$ : vérification que $\text{longueur} \le 1024$, puis extraction de la chaîne UTF-8 avec tolérance.
3. **Lecture de la Valeur selon le Type** :
   - `0x01` (Hash) : 16 octets fixes.
   - `0x02` (String) : lecture de la longueur (2 octets LE) $\le 1024$, puis des octets.
   - `0x03` (UInt32) : 4 octets LE.
   - `0x04` (Float32) : 4 octets LE.
   - `0x06` (Bool) : 1 octet.
   - `0x07` (Blob) : lecture de la longueur (4 octets LE) $\le 65536$, puis des octets.
   - `0x08` (UInt16) : 2 octets LE.
   - `0x09` (UInt8) : 1 octet.
   - `0x0A` (UInt64) : 8 octets LE.
   - Autre / Inconnu : le tag est conservé en `RawTag` si la longueur peut être délimitée, ou provoque une erreur `InvalidTagType`.
