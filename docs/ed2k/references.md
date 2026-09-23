# Références Historiques et Codebases eD2K / eMule

Ce document recense les dépôts sources, révisions historiques et fichiers de référence exploités pour la spécification du protocole eD2K dans EMule-Rust.

---

## 1. Dépôts et Arbres Sources de Référence

### 1.1 eMule Official Archive (SourceForge)
- **Projet** : eMule (C++ / MFC pour Windows)
- **Version de référence** : v0.72a — copie locale vérifiée : `D:\eMule0.72a-Sources\eMule0.72a-Sources\srchybrid`
- **Source** : Archive officielle du projet eMule sur SourceForge (`https://sourceforge.net/projects/emule/`)
- **Fichiers clés étudiés** :
  - `srchybrid/ListenSocket.cpp` : Réception et aiguillage des trames réseau brutes, décodage de l'en-tête (protocol byte, packet len, opcode).
  - `srchybrid/BaseClient.cpp` : Logique pair partagée en 0.72a (`SendHelloPacket:678`, `ProcessHelloPacket:342`, `SendHelloAnswer:892`, `ProcessHelloAnswer:351`, SUI `Send/ProcessSecIdentStatePacket:2076-2143`). *(En 0.50a, cette logique était dans `UpDownClient.cpp`, absent de 0.72a.)*
  - `srchybrid/DownloadClient.cpp` : Téléchargement (`SendBlockRequests:812`, `ProcessBlockPacket:956`, `ProcessFileStatus:509`).
  - `srchybrid/Packet.cpp` : Format binaire des trames, écriture et lecture des types scalaires, little-endianness, structure des en-têtes.
  - `srchybrid/Tag.cpp` : Définition des formats TLV (Type-Length-Value) eD2K et des balises spéciales (1 octet vs nom textuel).
  - `srchybrid/Opcodes.h` : Catalogue exhaustif des opcodes pour les protocoles `0xE3`, `0xC5` et `0xD4`.

### 1.2 irwir/eMule (Continuation Open-Source moderne)
- **Dépôt** : `https://github.com/irwir/eMule`
- **Branches / Révisions examinées** :
  - Branche principale `master` (commits de maintenance 64-bit et corrections de compilation MSVC).
  - Référence d'alignement pour les modifications récentes sur les extensions eMule (AICH, protocoles étendus, négociation de paquets).
- **Fichiers clés** :
  - `src/UpDownClient.cpp` : Normalisation du handshake `OP_HELLO` et `OP_HELLOANSWER`.
  - `src/Tag.cpp` : Gestion des tags étendus eMule 0.42+ (MSB flag dans le type de tag).

### 1.3 aMule (Client multiplateforme C++ / wxWidgets)
- **Dépôt** : `https://github.com/amule-project/amule`
- **Fichiers clés étudiés** :
  - `src/DownloadClient.cpp` & `src/UploadClient.cpp` : Implémentation portable de la poignée de main, compatible Linux/macOS.
  - `src/ECPacket.cpp` & `src/Packet.cpp` : Endianness et sérialisation/désérialisation sûre des entiers little-endian.
  - `src/Tag.cpp` : Décodage portable des chaînes UTF-8 vs encodages système 8 bits (ISO-8859-1 / Windows-1252).

---

## 2. Table de Correspondance des Types Fondamentaux

| Concept Wire eD2K | Représentation Source C++ | Type EMule-Rust (`mule-protocol`) | Notes |
| :--- | :--- | :--- | :--- |
| **Protocol Byte** | `uint8` (`0xE3`, `0xC5`, `0xD4`) | `u8` (`PR_ED2K`, `PR_EMULE`, `PR_EMULE_COMPRESSED`) | 1er octet de chaque trame |
| **Packet Length** | `uint32` (little-endian) | `u32` | Longueur = 1 (opcode) + payload |
| **Opcode** | `uint8` | `u8` | Code d'opération |
| **User Hash / File Hash** | `uchar[16]` | `[u8; 16]` / `PeerId` | Empreinte MD4 historique |
| **Client ID** | `uint32` (little-endian) | `u32` | HighID (IPv4 little-endian) ou LowID (< 16777216) |
| **Port** | `uint16` (little-endian) | `u16` | Port TCP du pair distant |
| **Tag Count** | `uint32` (little-endian) | `u32` | Nombre de métadonnées TLV associées |
| **Tag Name** | `uint16 len` + `string` ou `uint8 special_id` | `TagName::Text(String)` ou `TagName::Id(u8)` | Nom de balise standard ou ID abrégé |
