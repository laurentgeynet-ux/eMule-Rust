# Notes de Compatibilité & Variantes Historiques eD2K

Ce document consigne les particularités de format, variantes d'encodage et divergences observées entre les implémentations historiques eMule, aMule et les spécifications d'origine eDonkey2000.

---

## 1. Identifiants Numériques vs Noms de Tags Textuels

Dans eDonkey2000 et les versions initiales d'eMule :
- Le nom d'un tag commence par un entier 16-bit little-endian indiquant sa longueur en octets.
- Si cette longueur vaut **1**, l'octet suivant n'est pas une chaîne ASCII d'un caractère, mais un **identifiant spécial standardisé** (`Special Tag ID`) :
  - `0x01` : Nom d'utilisateur / Nom de fichier (`CT_NAME` / `FT_FILENAME`)
  - `0x02` : Taille de fichier (`FT_FILESIZE`)
  - `0x03` : Type de fichier (`FT_FILETYPE`)
  - `0x11` : Version du client (`CT_VERSION`)
  - `0x5A` : Version eMule compatible (`CT_EMULE_VERSION`)
  - `0x5B` : Drapeaux de capacités étendues (`CT_EMULE_MISCOPTIONS1`)
  - `0x5C` : Drapeaux de capacités étendues 2 (`CT_EMULE_MISCOPTIONS2`)
  - `0xFB` : Port UDP eMule (`CT_EMULE_UDPOPTIONS`)
- Si la longueur est supérieure à 1, il s'agit d'une chaîne textuelle (ex: `"muleversion"` ou `"modversion"`).

### Évolution eMule 0.42+ (Extended Tags)
Dans les versions modernes d'eMule, un mécanisme d'optimisation compacte les tags en encodant des drapeaux binaires dans le bit de poids fort du type de tag (`tag_type & 0x80`), évitant d'écrire la longueur 1 pour les identifiants spéciaux.
Notre décodeur passif supporte à la fois la syntaxe classique à préfixe de longueur et capture les balises étendues.

---

## 2. Encodage des Chaînes de Caractères

Historiquement :
- Les clients eDonkey originaux utilisaient l'encodage ANSI/Windows-1252 ou ISO-8859-1 local de l'utilisateur.
- À partir d'eMule 0.47a et avec aMule 2.x, l'encodage standard par défaut pour les pseudonymes et les noms de fichiers est devenu **UTF-8**.
- **Gestion eMule-Rust** : Le décodeur tente une conversion stricte UTF-8. En cas de séquence invalide, il recourt à une conversion avec remplacement sans échec fatal (`String::from_utf8_lossy`), garantissant la résilience face à de vieux pairs ANSI.

---

## 3. HighID vs LowID dans le Client ID

Le champ `Client ID` (32-bit little-endian) transporte l'adresse réseau du pair :
- **HighID** : Le pair est directement joignable depuis l'extérieur sur son port TCP. Le `Client ID` est égal à l'adresse IPv4 publique du pair réordonnée en entier 32-bit little-endian. Une valeur $\ge 16\,777\,216$ ($2^{24}$) caractérise un HighID.
- **LowID** : Le pair est derrière un pare-feu ou un routeur NAT sans redirection de port. Le `Client ID` est un identifiant arbitraire attribué par le serveur eD2K ($< 16\,777\,216$). Deux pairs en LowID ne peuvent pas initier de connexion directe sans passer par un serveur intermédiaire.

---

## 4. IP et Port du Serveur en Fin de Handshake

Dans `OP_HELLO` et `OP_HELLOANSWER` :
- Après la liste des tags, eMule transmet l'adresse IPv4 (4 octets LE) et le port (2 octets LE) du serveur eD2K auquel le client est actuellement connecté.
- Si le client fonctionne sans être connecté à un serveur central (mode Kademlia pur ou hors-ligne), eMule et aMule écrivent `0.0.0.0:0` (6 octets nuls).
- Notre décodeur passif extrait ces champs optionnels sans présumer de leur signification réseau.

---

## 5. Écarts assumés face à eMule 0.72a

Deviations volontaires ou simplifications documentées après l'audit de
fidélité (`AUD-Bxx` — référence `srchybrid`, eMule 0.72a) :

- **Identité annoncée = eMule 0.70a** (`CT_EMULE_VERSION = 70<<10`) alors
  que la référence est 0.72a : cohérent avec `KadVersion = 8` annoncé
  (eMule 0.72a = Kad 10). Passer à 0.72a/Kad 10 exige d'abord la revue
  des features Kad v9/v10 (AUD-P03).
- **Timeout de file d'attente** : le daemon abandonne un pair en file
  silencieux après `FILEREASKTIME + 15 min` (`PeerTimeouts::queue_timeout`).
  eMule n'a aucun timeout de file — le socket vit jusqu'à la mort TCP.
  Approximation retenue pour borner les sockets dormants.
- **Reask UDP à cadence fixe (~21 min)** au lieu du modèle eMule (ping UDP
  dans la fenêtre de 2 min précédant le reask TCP, throttle ~20 min). La
  garde « >3 paquets et >30 % d'échecs → stop UDP » est en revanche
  fidèle (`DownloadClient.cpp:1340`).
- **Reasks standalone** : les reasks sont émis en paquets simples
  (`OP_REQFILENAME`, `OP_SETREQFILEID`, `OP_STARTUPLOADREQ`) plutôt que
  groupés dans `OP_MULTIPACKET*` — comportement que les pairs eMule
  attendent des clients qui n'annoncent pas toujours la capacité.
- **NNP (No Needed Parts)** : la session TCP est fermée et la source
  redemandée après ~58 min (`FILEREASKTIME * 2`), tandis qu'eMule garde
  `DS_NONEEDEDPARTS` sur le socket ouvert. Même cycle de reask, socket
  non conservé.
- **Pas d'A4AF socket-level** (`SwapToAnotherFile` sur un socket déjà
  connecté) : l'A4AF est réalisé au niveau allocation des sources
  (reconnexion), pas en réutilisant la session TCP du pair.
- **Chat et browsing de partages non implémentés** : `OP_MESSAGE`,
  `OP_ASKSHAREDFILES*`, `OP_QUEUERANK (0x5C)`, `OP_CHANGE_SLOT (0x5B)`
  sont décodés ou ignorés sans logique métier.
- **Preview non annoncée** (bit 0 `MiscOptions1` = 0) : contrairement à
  une version antérieure, la capacité n'est plus affichée puisque
  `OP_REQUESTPREVIEW`/`OP_PREVIEWANSWER` ne sont pas implémentés.
- **`OP_CHANGE_CLIENT_ID` (0x4D) journalisé, non appliqué** : eMule
  met à jour `m_nUserIDHybrid`/l'IP serveur du pair ; le daemon borne
  le parsing (2×u32) et log sans modifier l'état — suffisant car
  l'opcode n'est plus émis par les clients modernes.

### Corrections d'alignement (audit phase B)

- `OP_END_OF_DOWNLOAD` émis/routé en `0x49` (eMule) ; `0x4D` décodé
  comme `OP_CHANGE_CLIENT_ID` (`Opcodes.h:218-219`). Le profil eDonkey
  legacy conserve `0x49` = `OP_FILESTATUSREQ` via le dispatch par
  profil (`profile.rs`).
- Inbound `0x49` → retrait du pair de la file d'upload si le hash
  correspond au fichier uploadé (`ListenSocket.cpp:537`) ; `0x4F`
  reste le `SETREQFILEID` canonique.
- `OP_REQFILENAME (0x58)` standalone servi hors multipacket : réponse
  `OP_REQFILENAMEANSWER`, lecture bornée de l'ext-info part-status
  (`ProcessExtendedInfo`), `OP_FILEREQANSNOFILE` si le fichier est
  inconnu ou partiel sans part complète (`ListenSocket.cpp:310`),
  compteur anti-flood `CheckFailedFileIdReqs` → ban après 4 fautes.
- Handshake : `MultiPacket=1`, `FileIdent=1`, `Preview=0`,
  `ET_SOURCEEXCHANGE=3` (`BaseClient.cpp:715`) ; réponse inbound
  `EMULEINFO → HELLOANSWER` dans l'ordre eMule, `client_id` et champs
  serveur réels (ID assigné par le serveur courant).
- `OP_SETREQFILEID` non émis pour les fichiers ≤ 1 part
  (`DownloadClient.cpp:366`) — la réponse `OP_REQFILENAMEANSWER`
  suffit à établir la complétude.

---

## 6. Kad — écarts assumés et corrections d'alignement (audit phase B, passe 2)

### Écarts assumés

- **`OUR_KAD_VERSION = 8`** alors qu'eMule 0.72a annonce
  `KADEMLIA_VERSION = 0x0a` (`Opcodes.h:32`) : Kad 9 ajoute la gestion
  des hash AICH dans le stockage de mots-clés, non implémentée.
  Cohérent avec l'identité 0.70a annoncée ; annoncer 10 surestimerait
  les capacités réelles.
- **Table de routage plate 128 × K** au lieu de l'arbre de zones
  binaire eMule (`RoutingZone` — split borné par `KK=5`/`KBASE=4`,
  profondeur accrue près de notre propre KadID). La capacité par bin
  est alignée (`K=10`) mais la topologie de couverture diffère.
- **Expressions de recherche restrictives décodées mais non
  appliquées** : le flag `0x8000` de `KADEMLIA2_SEARCH_KEY_REQ` et les
  octets d'expression (`SSearchTerm`) sont conservés dans
  `KadSearchKeywordReq::search_expression`, mais les entrées servies
  ne sont pas filtrées (`StartSearchTermsMatch` non implémenté).
- **Convergence `find_node`** : arrêt anticipé dès que 12 candidats
  sont dans `SEARCHTOLERANCE` (≥ 2 itérations) ; eMule poursuit
  jusqu'à épuisement de `m_mapPossible` ou `SEARCHFILE_TOTAL=300`.
- **Réponse `0x3B` sans entrées ambiguë** : sans tag discriminateur,
  un résultat vide est décodé comme `SearchKeywordRes` — indifférent
  côté demandeur (0 entrée dans les deux cas).
- **`KADEMLIA2_PUBLISH_RES_ACK (0x4C)`** : émis en réponse quand le
  byte d'options le demande ; un `0x4C` entrant est rejeté « unknown
  opcode » comme chez eMule (« for future use »).

### Corrections d'alignement

- **`KADEMLIA2_SEARCH_KEY_REQ (0x33)`** : layout filaire corrigé en
  `[keyword_hash 16][start_pos u16]` — pas de `sender_id` terminal ;
  bit 15 = recherche restrictive, offset = `start_pos & 0x7FFF`
  (`KademliaUDPListener.cpp:1057` `Process_KADEMLIA2_SEARCH_KEY_REQ`).
- **`KADEMLIA2_PUBLISH_RES (0x4B)`** : byte d'options optionnel après
  `[hash16][status]` — bit 0 → renvoi d'un datagramme `0x4C` nu
  (`KademliaUDPListener.cpp:1438` `Process_KADEMLIA2_PUBLISH_RES`).
- **Fragmentation des réponses de recherche** : packing glouton des
  entrées en datagrammes `KADEMLIA2_SEARCH_RES` consécutifs ≤
  `UDP_KAD_MAXFRAGMENT = 1420` (`Opcodes.h:134`,
  `Indexed.cpp:601/707` `SendValid{Keyword,Source}Result`) —
  auparavant un datagramme unique de plusieurs Ko subissait la
  fragmentation IP.
- **Index de stockage** : TTL différenciés — sources 5 h
  (`KADEMLIAMAXINDEX=50 000` clés), mots-clés 24 h, notes 24 h ;
  bornes `KADEMLIAMAXSOURCEPERFILE=1 000`,
  `KADEMLIAMAXENTRIES=60 000`, `KADEMLIAMAXNOTESPERFILE=150`,
  résultats servis `iMaxResults=300` (`Opcodes.h:78-85`,
  `Indexed.cpp:619`).
- **Routage/lookup** : `K_BUCKET_SIZE = 10` (`Defines.h:42`, était 20),
  `ALPHA_QUERY = 3` (`:45`, était 5), graine de recherche 50 contacts
  (`Search.cpp:172` `GetClosestTo(3, …, 50, …)`, était 32).
