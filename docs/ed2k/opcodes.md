# Catalogue des Opcodes Historiques eD2K et eMule

Ce document répertorie les opcodes standards du protocole eDonkey2000 (`0xE3`), des extensions eMule (`0xC5`) et du protocole compressé (`0xD4`).

---

## 1. Protocole eDonkey2000 Standard (`PR_ED2K = 0xE3`)

Ces messages constituent le socle d'interconnexion pair-à-pair originel défini par MetaMachine et étendu par eMule.

| Opcode Hex | Nom Symbolique | Direction | Rôle & Sémantique |
| :---: | :--- | :--- | :--- |
| `0x01` | `OP_HELLO` | Bidirectionnel | Poignée de main initiale : annonce du hash utilisateur, port d'écoute et capacités |
| `0x1C` | `OP_CALLBACKREQUEST` | Client $\rightarrow$ Serveur | Demande au serveur de relayer une invitation vers un client LowID |
| `0x35` | `OP_CALLBACKREQUESTED` | Serveur $\rightarrow$ Client | Relai serveur : IP + port (optionnel : crypt options + user hash) d'un client LowID qui nous appelle |
| `0x32` | `OP_SERVERLIST` | Serveur $\rightarrow$ Client | Serveurs additionnels annoncés : `u8 count` puis `count × (u32 ip_le + u16 port)` |
| `0x36` | `OP_CALLBACK_FAIL` | Serveur $\rightarrow$ Client | Échec du relai de callback LowID |
| `0x94` | `OP_GLOBGETSOURCES2` | Client $\rightarrow$ Serveur (UDP) | Requête UDP de sources : `hash16 + u32 taille` (port serveur = TCP+4) |
| `0x96` | `OP_GLOBSERVSTATREQ` | Client $\rightarrow$ Serveur (UDP) | Requête UDP de statistiques serveur (payload vide) |
| `0x97` | `OP_GLOBSERVSTATRES` | Serveur $\rightarrow$ Client (UDP) | Statistiques : `u32 users + u32 files` |
| `0x99` | `OP_GLOBSEARCHRES` | Serveur $\rightarrow$ Client (UDP) | Résultats de recherche globale UDP (même layout que `OP_SEARCHRESULT`) |
| `0x9B` | `OP_GLOBFOUNDSOURCES` | Serveur $\rightarrow$ Client (UDP) | Sources par fichier : blocs répétés `hash16 + u8 count + count × (u32 ip + u16 port)` |
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
| `0x60` | `OP_QUEUERANKING` | Pair $\rightarrow$ Client | Position `u32` dans la file d'attente d'upload du pair distant |

---

## 2. Protocole eMule Étendu (`PR_EMULE = 0xC5`)

Introduit par les versions d'eMule pour ajouter des capacités avancées (compression, AICH, négociation de protocole et échanges de sources).

| Opcode Hex | Nom Symbolique | Direction | Rôle & Sémantique |
| :---: | :--- | :--- | :--- |
| `0x01` | `OP_EMULEINFO` | Bidirectionnel | Négociation des fonctionnalités modernes eMule (versions, options réseau) |
| `0x02` | `OP_EMULEINFOANSWER` | Pair $\rightarrow$ Client | Réponse à la négociation des fonctionnalités eMule |
| `0x12` | `OP_AICH_REQUEST` | Client $\rightarrow$ Pair | Demande de données de récupération AICH d'une part corrompue (`hash16 + u16 part + master_hash20`) |
| `0x13` | `OP_AICH_ANSWER` | Pair $\rightarrow$ Client | Données de récupération : entrées `(ident, hash SHA-1)` 16-bit/32-bit — ou `hash16` seul en cas d'échec |
| `0x14` | `OP_AICHFILEHASHREQ` | Client $\rightarrow$ Pair | Demande du hash racine AICH (SHA-1) d'un fichier |
| `0x15` | `OP_AICHFILEHASHRES` | Pair $\rightarrow$ Client | Réponse : hash de fichier + hash racine AICH (`hash16 + sha1_20`) |
| `0x40` | `OP_COMPRESSEDPART` | Pair $\rightarrow$ Client | Bloc zlib (`hash16 + u32 start + u32 taille compressée + chunk ≤ 10 240 o`), envoyé sous `OP_EMULEPROT` |
| `0x90` | `OP_REASKFILEPING` | Client $\rightarrow$ Pair (UDP) | Keep-alive vers une source en file : `hash16` + info étendue (`u16` part-status + `u16` complete sources pour UDP ver > 3) |
| `0x91` | `OP_REASKACK` | Pair $\rightarrow$ Client (UDP) | Rang de file d'attente : `[u16 count + bitfield]` (UDP ver > 3) + `u16` rang |
| `0x92` | `OP_FILENOTFOUND` | Pair $\rightarrow$ Client (UDP) | Le fichier demandé n'est pas partagé (payload vide) |
| `0x93` | `OP_QUEUEFULL` | Pair $\rightarrow$ Client (UDP) | File d'attente du pair saturée (payload vide) |
| `0x21` | `OP_SOURCESREQUEST` | Client $\rightarrow$ Pair | Demande de sources connues pour un fichier (Source Exchange) |
| `0x22` | `OP_SOURCESANSWER` | Pair $\rightarrow$ Client | Réponse contenant la liste de pairs pour un fichier |

---

## 3. Protocole Compressé (`PR_EMULE_COMPRESSED = 0xD4`)

| Opcode Hex | Nom Symbolique | Direction | Sémantique |
| :---: | :--- | :--- | :--- |
| `0x40` | `OP_COMPRESSEDPART` | Pair $\rightarrow$ Client | Bloc de données compressé avec l'algorithme zlib/deflate |

> [!NOTE]
> La décompression zlib `0xD4`/`OP_COMPRESSEDPART` est désormais supportée dans les deux sens : décodage côté téléchargement (flux incrémental `flate2`) et émission côté upload quand le pair annonce `nDataCompVer` (MiscOptions1 bits 20-23) — avec repli `OP_SENDINGPART` si la compression n'est pas rentable, fidèle à `CreatePackedPackets`.
