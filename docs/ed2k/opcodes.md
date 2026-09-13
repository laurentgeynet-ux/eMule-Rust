# Catalogue des Opcodes Historiques eD2K et eMule

Ce document répertorie les opcodes standards du protocole eDonkey2000 (`0xE3`), des extensions eMule (`0xC5`) et du protocole compressé (`0xD4`).

---

## 1. Protocole eDonkey2000 Standard (`PR_ED2K = 0xE3`)

Ces messages constituent le socle d'interconnexion pair-à-pair originel défini par MetaMachine et étendu par eMule.

| Opcode Hex | Nom Symbolique | Direction | Rôle & Sémantique |
| :---: | :--- | :--- | :--- |
| `0x01` | `OP_HELLO` | Bidirectionnel | Poignée de main initiale : annonce du hash utilisateur, port d'écoute et capacités |
| `0x4C` | `OP_HELLOANSWER` | Pair $\rightarrow$ Client | Réponse à la poignée de main initiale avec les informations du pair distant |
| `0x15` | `OP_OFFERFILES` | Client $\rightarrow$ Serveur | Annonce de fichiers partagés au serveur |
| `0x46` | `OP_SENDINGPART` | Pair $\rightarrow$ Client | Transmission d'un bloc de données de fichier (offset + payload) |
| `0x47` | `OP_SETREQFILEID` | Client $\rightarrow$ Pair | Déclaration du hash de fichier ciblé par le téléchargement |
| `0x48` | `OP_FILEREQANSNOFILE`| Pair $\rightarrow$ Client | Indique que le fichier demandé n'est pas ou plus partagé par le pair |
| `0x49` | `OP_FILESTATUSREQ` | Client $\rightarrow$ Pair | Demande des parts disponibles pour le fichier ciblé |
| `0x4B` | `OP_REQUESTPARTS` | Client $\rightarrow$ Pair | Requête d'une plage d'octets précise à l'intérieur d'une part |
| `0x4D` | `OP_END_OF_DOWNLOAD` | Client $\rightarrow$ Pair | Notification de fin de téléchargement complet du fichier |
| `0x50` | `OP_FILESTATUS` | Pair $\rightarrow$ Client | Envoi du masque de disponibilité des parts du fichier |
| `0x51` | `OP_HASHSETREQUEST` | Client $\rightarrow$ Pair | Demande de la liste des hashs de parts (hashset) pour un fichier multi-parts |
| `0x52` | `OP_HASHSETANSWER` | Pair $\rightarrow$ Client | Réponse contenant la liste des hashs MD4 de 16 octets des parts |
| `0x54` | `OP_STARTUPLOADREQ` | Client $\rightarrow$ Pair | Demande d'entrée dans la file d'attente d'envoi du pair distant |
| `0x55` | `OP_ACCEPTUPLOADREQ`| Pair $\rightarrow$ Client | Acceptation : un slot d'envoi est alloué et le transfert peut démarrer |
| `0x56` | `OP_CANCELTRANSFER` | Bidirectionnel | Annulation du transfert ou retrait de la file d'attente |
| `0x57` | `OP_OUTOFPARTS` | Client $\rightarrow$ Pair | Indique que le client ne recherche plus de parts auprès de ce pair |
| `0x58` | `OP_REQFILENAME` | Client $\rightarrow$ Pair | Demande du nom de fichier associé au hash |
| `0x59` | `OP_REQFILENAME_ANSWER` | Pair $\rightarrow$ Client | Réponse avec le nom de fichier |

---

## 2. Protocole eMule Étendu (`PR_EMULE = 0xC5`)

Introduit par les versions d'eMule pour ajouter des capacités avancées (compression, AICH, négociation de protocole et échanges de sources).

| Opcode Hex | Nom Symbolique | Direction | Rôle & Sémantique |
| :---: | :--- | :--- | :--- |
| `0x01` | `OP_EMULEINFO` | Bidirectionnel | Négociation des fonctionnalités modernes eMule (versions, options réseau) |
| `0x02` | `OP_EMULEINFOANSWER` | Pair $\rightarrow$ Client | Réponse à la négociation des fonctionnalités eMule |
| `0x12` | `OP_AICH_REQUEST` | Client $\rightarrow$ Pair | Demande du hash racine AICH (SHA-1) d'un fichier |
| `0x13` | `OP_AICH_ANSWER` | Pair $\rightarrow$ Client | Réponse fournissant le hash racine AICH |
| `0x14` | `OP_AICHFILEHASHREQ` | Client $\rightarrow$ Pair | Demande de l'arbre de preuve AICH pour récupérer une part corrompue |
| `0x15` | `OP_AICHFILEHASHRES` | Pair $\rightarrow$ Client | Transmission des hashs intermédiaires AICH de la part corrompue |
| `0x21` | `OP_SOURCESREQUEST` | Client $\rightarrow$ Pair | Demande de sources connues pour un fichier (Source Exchange) |
| `0x22` | `OP_SOURCESANSWER` | Pair $\rightarrow$ Client | Réponse contenant la liste de pairs pour un fichier |

---

## 3. Protocole Compressé (`PR_EMULE_COMPRESSED = 0xD4`)

| Opcode Hex | Nom Symbolique | Direction | Sémantique |
| :---: | :--- | :--- | :--- |
| `0x40` | `OP_COMPRESSEDPART` | Pair $\rightarrow$ Client | Bloc de données compressé avec l'algorithme zlib/deflate |

> [!CAUTION]
> Conformément aux décisions architecturales de la Phase 5, tout paquet portant le protocole `0xD4` est formellement rejeté par les décodeurs avec l'erreur `CompressedProtocolUnsupported` tant que les tests de décompression hostile et les plafonds stricts post-décompression ne sont pas audités.
