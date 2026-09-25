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

- **Identité annoncée = eMule 0.72a** (`CT_EMULE_VERSION = 72<<10`,
  `client_version = 72`) alignée sur la référence — voir §13 pour les
  bits `MISCOPTIONS2` volontairement non annoncés.
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
- **NNP (No Needed Parts)** : la session tente d'abord un swap A4AF
  socket-level (§11) ; sans candidat elle est fermée et la source
  redemandée après ~58 min (`FILEREASKTIME * 2`), tandis qu'eMule garde
  `DS_NONEEDEDPARTS` sur le socket ouvert. Même cycle de reask.
- **A4AF socket-level implémenté (§11)** : `SwapToAnotherFile`/`DoSwap`
  porté — le socket réémet la séquence `SendFileRequest` pour le
  fichier cible au lieu de reconnecter.
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

- **Réponse `0x3B` sans tag discriminateur** : une entrée au format
  plat legacy (sans `PUBLISHINFO`/`KADAICHHASHRESULT`) reste décodée par
  le chemin de compatibilité pré-v9 ; eMule ≥ 0.50a émet toujours la
  taglist complète.
- **Table de routage plate 128 × K** au lieu de l'arbre de zones
  binaire eMule (`RoutingZone` — split borné par `KK=5`/`KBASE=4`,
  profondeur accrue près de notre propre KadID). La capacité par bin
  est alignée (`K=10`) mais la topologie de couverture diffère.
- **Tags publiés non persistés au redémarrage** : les entrées
  keywords réhydratées depuis SQLite retrouvent `file_name`/`file_size`/
  `source_count` mais pas leur taglist complète — les recherches
  restrictives sur des métadonnées tierces ne matcheront ces entrées
  qu'après republication (eMule ne persiste pas son index du tout).
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
- **Expressions restrictives appliquées** : `kad::search_expr`
  parse l'arbre `SSearchTerm` (`CreateSearchExpressionTree` — AND/OR/
  NOT binaires, feuilles string/meta/numeric u32+u64, profondeur max
  24) et l'évalue comme `CKeyEntry::SearchTermsMatch` : termes de nom
  de fichier pré-tokenisés (`TokenizeOptQuotedSearchTerm`, quotes
  préservées, sous-chaîne case-insensitive, multi-mots = AND), égalité
  de chaînes de métadonnées (cas spécial `fileformat` → extension du
  nom), relations entières sur les tags publiés avec `filesize`
  virtuel (`GetIntTagValue`). Expression malformée → datagramme
  rejeté comme le `try/catch` eMule ; `start_pos` pagine **après**
  filtrage (`Indexed.cpp` `SendValidKeywordResult`). La taglist
  publiée est conservée sur `KadKeywordEntryWire::tags` et réémise
  verbatim dans les résultats (`WriteTagList`).
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

---

## 7. PartFile — écarts assumés et corrections d'alignement (audit phase B, passe 3)

### Écarts assumés

- **Sélection de blocs** : `GetNextRequestedBlock`/`CreateBlockRequests`
  eMule (`DownloadClient.cpp:1950+`) combine fréquence de part,
  priorité preview, `sameChunk`, pourcentage de complétion, nombre de
  clients en transfert et `bandwidthScore`. Le séquenceur Rust
  (`ed2k/sequencer.rs`) applique un ordre « rareté / preview /
  progression » simplifié — comportement proche mais non identique ;
  pas de risque filaire (les pairs voient des requêtes licites).
- **Bannissement corruption** portée session uniquement : eMule
  `clientlist->AddBannedClient` est aussi borné à la session, mais
  `AddTrackClient` (suivi de score inter-fichiers) n'a pas
  d'équivalent — un pair banni reste `Rejected` par fichier et ne peut
  plus être ingéré ni dialé, sans score persistant.
- **`.part.met`** : les tags `FT_CORRUPTEDPARTS`, `FT_STATUS`,
  `FT_LASTSEENCOMPLETE`, `FT_DL_ACTIVE_TIME`, `FT_PARTFILENAME` et le
  blob `FT_AICHHASHSET` ne sont pas écrits — corruption et parts
  vérifiées vivent en SQLite ; le hashset AICH est réobtenu via
  recovery/votes si nécessaire après redémarrage.
- **Pas de `UncompressFile`/`SetZoneIdentifier`** à la complétion
  (attribut sparse NTFS + zone MOTW) — spécifique Windows, sans effet
  protocolaire.
- **Pas de buffering 30 s** (`m_BufferedData_list`/`FlushBuffer`) :
  les écritures sont synchrones avec flush immédiat — plus sûr
  (aucune donnée volatile), même sémantique observable.

### Corrections d'alignement

- **Collision de destination** : `CollisionStrategy::Overwrite`
  remplacé par `Rename` — `CPartFile::PerformFileComplete` génère
  `name(1).ext`, `name(2).ext`, … en incrémentant un suffixe `(N)`
  existant ; le chemin résolu est persisté (`target_path`) et
  enregistré dans `shared_files` sous son vrai nom.
- **Écriture sur part complète rejetée** : eMule `WriteToBuffer`
  ignore toute donnée touchant une part déjà hashée
  (`IsComplete(start|end part)` → `return 0`). `inject_range` fait de
  même pour toute part couverte par `ranges` — un bloc en vol arrivant
  après complétion de la part n'écrase plus des données vérifiées.
- **ICH** : la vérif MD4 de part est inconditionnelle (eMule
  `HashSinglePart` sur chaque part complétée) ; `ich_enabled` ne gate
  plus la vérif mais uniquement le choix entre file de récupération
  AICH par blocs et éviction complète
  (`IsCorruptedPart && IsICHEnabled`, `PartFile.cpp:4158`).
- **`CorruptionBlackBox`** (`CorruptionBlackBox.cpp`) : provenance
  `(plage, expéditeur)` par part, statuts
  `Received/Verified/Corrupted` avec découpe des enregistrements sur
  recouvrement ; `EvaluateData` condamne l'expéditeur quand
  `corrompu/(corrompu+vérifié) > CBB_BANTHRESHOLD = 32 %` (chaque
  enregistrement corrompu compté ≥ `EMBLOCKSIZE` = 180 Kio). Le ban
  déconnecte la session et marque la source `Rejected`.
- **`CanAddSource`** : une source égale à notre propre LowID (origine
  serveur) ou à notre `HighID:port` public est ignorée ; les clients
  bannis sont filtrés dès `upsert_peer_sources` et avant le dial
  (`IsBannedClient`).

## 8. Client UDP (`ClientUDPSocket.cpp`) — écarts assumés et corrections (audit phase B, passe 3b)

### Écarts assumés

- **Datagrammes toujours en clair** : eMule peut chiffrer le canal
  client-UDP (`ShouldReceiveCryptUDPPackets` + `CreateSafeKeyAndIV`) ;
  nous émettons en clair et ne décodons pas les datagrammes obfusqués
  entrants (loggés en `debug`, ignorés).
- **Pas d'émission `OP_REASKCALLBACKUDP`** : pour un re-ask d'une
  source **LowID** en file, eMule relaie via le buddy de la source.
  Nos sessions en file gardent leur socket TCP — le re-ask TCP couvre
  le cas (le callback initial `kadcb:` passe déjà par
  `KADEMLIA_CALLBACK_REQ`).
- **Ext-info du ping inbound non appliquée** : le part-status /
  complete-sources du demandeur (`ProcessExtendedInfo`) n'est pas
  stocké — il n'a d'effet que sur la disponibilité annoncée aux autres
  clients de notre upload queue, statistique non exposée.
- **`OP_PORTTEST` sans réponse** : eMule accuse le test `0x12` via sa
  socket TCP de test dédiée (`SendPortTestReply('1')`), pas en UDP ;
  nous ne livrons pas l'outil de test de ports — décodé, journalisé.
- **Purge en masse vs par unité** : eMule retire **1** source
  `RemoteQueueFull` par minute (et 1 NNP / 40 s, 1 LowToLowIP / 30 s)
  via `m_lastpurgetime` partagé ; notre passe `SOURCE_PURGE_INTERVAL`
  (60 s) retire d'un coup tout le budget `count - seuil + 1` —
  cadence identique, descente vers `seuil - 1` plus rapide.

### Corrections d'alignement

- **`OP_REASKFILEPING`** : le rang n'est plus révélé à tout pair
  partageant l'IP — eMule `GetWaitingClientByIP_UDP` + contrôle
  `md4equ(reqfilehash, sender->GetUploadFileID())` reproduit :
  correspondance exacte IP **+ port UDP annoncé** (stocké par entrée
  de file), tolérance « unique IP », silence sur ambiguïté multi-NAT
  (`bSenderMultipleIpUnknown`) et sur mismatch de hash. La rangée
  `client_udp_port` est alimentée par le HELLO du client.
- **`SetLastUpRequest`** : un ping accepté rafraîchit
  `last_request_unix_ms` de l'entrée — la purge
  `MAX_PURGEQUEUETIME` (1 h) ne frappe plus un client qui ne survit
  que par re-asks UDP.
- **`OP_REASKACK` UDP ver > 3** : le bloc part-status
  (`u16 count` + bitfield) est décodé (`UdpPartStatus`) et appliqué à
  la disponibilité de la source (`ProcessFileStatus` eMule) —
  rareté `m_SrcPartFrequency` + persistance `availability`.
- **`OP_QUEUEFULL`** : marque `remote_queue_full` + rang 0 sur la
  session en file (`SetRemoteQueueFull` + `UDPReaskACK(0)`) — avant,
  le résultat était stocké sans consommateur.
- **`OP_FILENOTFOUND`** : deadlist — la source passe `Failed`
  (persisté, hors srclist) et la session en file est terminée
  (`UDPReaskFNF` → `RemoveSource` + `m_DeadSourceList`).
- **`OP_DIRECTCALLBACKREQ` (0x95)** : décodé
  (`u16 tcp_port + userhash16 + u8 conn_opts`), limité à 1 requête/IP/
  180 s (`AllowCalbackRequest`), accepté seulement quand notre état
  firewall UDP Kad est `Firewalled` (`CKademlia::IsFirewalled()`) —
  alors `connect_back` compose le pair en sortant
  (`TryToConnect`) et sert la session comme un inbound.
- **Purge `RemoteQueueFull` à 80 %** (`PartFile.cpp:2285-2295`) : les
  sessions `remote_queue_full` (flag levé par `OP_QUEUEFULL` UDP)
  rejoignent les critères de `purge_excess_sources` — suppression de la
  ligne source + `disconnect_peer` de la session live (`RemoveSource`
  détruit le download client). Correspondance adresse exacte puis IP
  seule pour les sources `lowid:`/`kadcb:`. Correction annexe : à
  égalité `count == seuil` eMule purge encore → budget
  `count - seuil + 1`.

## 9. Bande passante / USS (`UploadBandwidthThrottler.cpp`, `UploadQueue.cpp`, `LastCommonRouteFinder.cpp`) — audit phase B, passe 4

### Fidélité vérifiée (aucune correction nécessaire)

- **Loi de contrôle USS** (`services/uss.rs`) : `soll_ping`, `hping`,
  diviseurs, médiane glissante, baseline de 10 sondes, plancher 20 ms,
  tolérance %/ms, clamps min/max, re-baselining après 60 échecs —
  conforme à `LastCommonRouteFinder.cpp` (`Ping()` + thread de contrôle).
- **Cadence de sondage** : ~1 % du débit upload, bornée 125–1000 ms
  (identique à eMule).
- **Découverte de cible** : traceroute ICMP à TTL croissants via
  `IcmpSendEcho` (API non privilégiée Windows, comme eMule), dernier saut
  commun aux routes — `uss_icmp::last_common_host`.
- **`AcceptNewClient`** (`upload_queue.rs::can_open_slot`) : veto USS,
  4 slots garantis, formule du débit cible par client
  (`GetTargetClientDataRate`), garde débit mesuré vs débit minimal —
  porté fidèlement.
- **`MinUpload`** : `MIN_UPLOAD_BPS = 1024` = défaut eMule `MinUpload`
  (1 Ko/s, clampé ≥ 1 dans `Preferences.cpp:1942`).
- **Ratio UL/DL** (`emule_ud_ratio`) : coefficients 3/4/5 et seuil
  20 Ko/s — miroir de `GetMaxDownloadInBytesPerSec`.
- **USS désactivé → restauration de la limite manuelle** sur le bucket
  upload ; bucket download réduit via le ratio à chaque tick USS.

### Corrections d'alignement

- **Pacing `ForceNewClient`** (`UploadQueue.cpp:417-421`) : 1 s minimum
  entre deux ouvertures de slot tant que le débit mesuré < 100 Ko/s
  (`m_nLastStartUpload` porté par `last_upload_start_ms`) — avant, une
  file pleine ouvrait tous les slots permis en rafale au démarrage.
  Jamais de temporisation sous `MIN_UP_CLIENTS_ALLOWED` (2) ni au-dessus
  de 100 Ko/s mesurés. Appliqué aux deux chemins d'ouverture
  (`AddClientToQueue` direct + promotion depuis la file).

### Écarts assumés

- **Pas de `GetSlotLimit` / round-robin par tick** : eMule ordonnance
  l'envoi socket par socket dans une fenêtre temporelle de 100 ms
  (jusqu'à `GetSlotLimit()` sockets standard par cycle, paquets
  ≥ 536/1300 o). Notre modèle : writers par connexion drainant un
  token bucket global partagé — saturation équivalente, équité par
  connexion différente (pas de round-robin explicite entre sockets
  standard et trickle).
- **Branche dynamique de `GetMaxDownloadInBytesPerSec(bDynamic)`** :
  eMule utilise le `GetDatarate()` **mesuré** quand DynUp est actif et
  la file non vide ; nous utilisons la limite configurée/courante USS
  (`ul_eff`). Écart faible : sous charge la valeur USS converge vers le
  débit mesuré.
- **Fallback de sonde USS = TCP-connect RTT** : eMule peut aussi pinger
  en UDP (socket UDP brute) quand ICMP échoue ; nous retombons sur une
  connexion TCP vers un pair Kad/serveur. Hors Windows, ICMP absent →
  fallback TCP systématique.
- **Comptabilisation du débit** : eMule distingue file-data et overhead
  protocolaire dans `GetDatarate` ; notre `effective_rate_bps` compte
  les octets socket (overhead inclus) — surestime légèrement le payload.

## 10. Persistance `.met` (`mule-format`) — audit phase B, passe 4

### Fidélité vérifiée

- **`.part.met`** : versions `0xE0`/`0xE1`/`0xE2` (large file),
  en-tête `mod_date + filehash + u16 part_count + part_hashes`,
  tags gap nommés `\x09<idx>`/`\x0A<idx>` (`SavePartFile`),
  `FT_FILESIZE_HI` fusionné en lecture, écriture atomique tmp+rename,
  `FT_AICH_HASH` base32, `FT_DLPRIORITY`/`FT_MAXSOURCES` écrits et
  relus à l'import (`PR_AUTO` restaure le mode auto). Import eMule →
  Rust : gaps inversés en ranges complétés, parts rehashés, mismatch
  évincé vers gaps — conforme à `LoadPartFile` + `UpdatePartsInfo`.
- **`server.met`** et **`nodes.dat`** : formats couverts.

### Écarts assumés (déjà documentés §7, rappel)

- Tags `.part.met` non écrits : `FT_STATUS`, `FT_CORRUPTEDPARTS`
  (corruption = gaps + blackbox SQLite), `FT_LASTSEENCOMPLETE`,
  `FT_DL_ACTIVE_TIME`, `FT_PARTFILENAME` (chemin cible en SQLite),
  `FT_AICHHASHSET` (hashset partiel reconstruit via récupération AICH).
  eMule lit nos fichiers sans ces tags (gaps + hashset MD4 suffisent à
  `UpdatePartsInfo`).
- **`known.met` non implémenté** : la persistance des fichiers partagés
  vit en SQLite (`shared_files`), pas de fichier eMule-compatible ni
  d'outil d'import `known.met` → Rust. Conséquence : les statistiques
  all-time (transferred/requested/accepted) et les timestamps de
  dernière publication Kad d'un eMule existant ne migrent pas — les
  fichiers sont repartagés frais (AICH root recalculé au premier hash
  complet).
- Pas de purge partielle `ShouldPartiallyPurgeFile` : sans `known.met`,
  pas de métadonnées longue durée à élaguer — le pruning des entrées
  SQLite suit la politique locale.

---

## 11. A4AF socket-level (`SwapToAnotherFile`/`DoSwap`) — audit phase B, passe 5

### Implémentation (`peer_session/a4af.rs`, `connection.rs`)

- `run_status_transfer_with_a4af` : boucle partagée par
  `run_peer_connection` et `run_callback_session` — sur `PeerError`
  `NoNeededParts` ou `FileNotFound` (`is_swap_eligible`, eMule appelle
  `SwapToAnotherFile` depuis `ProcessFileStatus`, `SendBlockRequests`,
  `UDPReaskFNF`, `OP_FILEREQANSNOFILE` `ListenSocket.cpp:428`), le même
  socket rejoue `query_file_status_and_enter_queue` pour le fichier
  cible au lieu de mourir. Borné à `MAX_A4AF_SWAPS = 4` swaps
  consécutifs (eMule n'a pas de borne — `DontSwapTo` + timing de
  reask ; notre cap couvre le ping-pong pathologique).
- `find_swap_target` : candidats = downloads actifs
  (`queued`/`waiting_for_sources`/`downloading`, `PS_READY|PS_EMPTY`
  eMule) listant l'adresse comme source, sous leur `max_sources`
  (`GetSourceCount() < GetMaxSources()`). Tri : sources propres
  (`m_OtherRequests_list`) avant NNP-flaggées
  (`m_OtherNoNeeded_list`), puis priorité de download
  (`RightFileHasHigherPrio`), puis hash déterministe.
  `Rejected`/`Filtered` exclus ; **cooldown appliqué à tous les
  statuts** — une source NNP (`Allowed` + 58 min) n'est pas re-élue
  dans la même chaîne (`DontSwapTo`/`GetTimeUntilReask`).
- `apply_swap` : bookkeeping `DoSwap` — FNF → `Rejected` (deadlist,
  `bRemoveCompletely=true`), NNP → `Allowed` + cooldown 58 min
  (`FILEREASKTIME * 2`), fichier cible → `Connected`
  (`SwapTo->srclist.AddTail(this)`).
- `try_a4af_swap` : purge les `in_flight_blocks` du pair
  (`m_PendingBlocks_list` abandonné), met à jour le snapshot session
  (`SetRequestFile`), émet l'événement.
- **Correction annexe** : le teardown de session écrit l'issue sur
  `snapshot.file_hash` (fichier servi en dernier), pas sur le hash
  dialé — eMule `RemoveSource` s'applique à `m_reqfile` ; sans ça le
  bookkeeping du swap était écrasé et la source cible restait
  `Connected` à jamais.
- **Correction annexe** : `WaitingForSources` sérialise
  `"waiting_for_sources"` — le filtre de statuts du swap matche les
  noms réels de `DownloadStatus::as_str`.

### Test d'intégration

`test_peer_connect_a4af_socket_swap_on_fnf` (mock TCP loopback) :
`OP_FILEREQANSNOFILE` sur A → même socket reçoit `OP_REQFILENAME`/`OP_SETREQFILEID`
pour B, transfert réel (≥1024 o injectés dans B), source A `rejected`.

### Écarts assumés

- Pas de `ProcessA4AFClients` périodique (`ClientList.cpp:846`) ni de
  swap proactif à la découverte d'une nouvelle source
  (`CheckAndAddSource`/`CheckAndAddKnownSource`) : nos swaps restent
  déclenchés par NNP/FNF en session, pas par réévaluation périodique
  de la liste A4AF d'un client idle — couvert par le re-dial normal.
- Le mismatch de hash AICH (`ProcessAICHFileHash` eMule → swap
  `bRemoveCompletely`) ne swap pas : notre modèle AICH est par vote
  (`record_aich_root_vote`) — un vote divergent ne tue pas la session.
- Pas de `DontSwapTo` temporel ni d'`aggressiveSwapping` : le cap
  `MAX_A4AF_SWAPS` + exclusion du hash courant + cooldowns bornent
  déjà le ping-pong.

## 12. LowID↔LowID (`DS_LOWTOLOWIP`/`DS_WAITCALLBACK`, `CanDoCallback`, `IsLocalServer`) — audit phase B, passe 6

Références : `Emule.cpp:1056-1085` (`IsFirewalled`/`CanDoCallback`),
`BaseClient.cpp:1342-1457` (`TryToConnect`), `PartFile.cpp:2368-2403`
(`CanAddSource`, ingestion), `PartFile.cpp:2242-2261` (purge DS_LOWTOLOWIP
30 s), `ServerConnect.cpp:519` (`IsLocalServer`),
`DownloadClient.cpp:191-203` (parking `m_bReaskPending`).

### Implémenté

- **`CallbackCtx`** (`swarm_coordinator/lowid_callback.rs`) : contexte de
  joignabilité par tick — `server_connected`, `server_high_id`, endpoint
  du serveur connecté, `kad_open` (`CKademlia::IsConnected() &&
  !IsFirewalled()` = socket UDP actif + `UdpFirewallStatus::Open`).
- **`can_do_callback`** : port exact de `CemuleApp::CanDoCallback` —
  sans Kad ouvert : `server_high_id` seul ; Kad ouvert + nous LowID :
  refus des LowID **du même serveur** (relay sur nous-mêmes = violation
  de protocole, ban chez eMule).
- **`is_local_server`** : les LowIDs ne sont uniques que par serveur —
  `OP_CALLBACKREQUEST` n'est émis que pour les sources dont le serveur
  enregistré (`PeerSource.server_ip`/`server_port`, nouveaux champs
  persistés, remplis par SX filaire et par l'endpoint du serveur
  répondant TCP/UDP) matche notre serveur connecté. `server_ip == 0` →
  local seulement si `origin == Server` ; `server_port == 0` → match IP
  seule (réponses UDP globales).
- **`DS_LOWTOLOWIP`** : source `lowid:`/`kadcb:` sans `can_do_callback`
  → ignorée au dial (parquée, réévaluée à chaque tick quand notre état
  change — `DS_ONQUEUE` eMule). Les `lowid:` étrangères (serveur
  différent) alors qu'on est joignable → `Rejected` (deadlist), comme
  eMule `Disconnected("LowID: No Callback Option available")`.
- **Refus à l'ingestion** (`IsFirewalled` = ni HighID ni Kad ouvert) :
  `DownloadService.accept_lowid_sources` rafraîchi par le coordinateur,
  appliqué dans le funnel `upsert_peer_sources` pour tous les canaux
  (`CPartFile::CanAddSource` `IsLowID && IsFirewalled` → drop) ; le
  filtre SX existant gagne aussi l'exemption `kad_open` (`SxLocalIds`).
- **`DS_WAITCALLBACK`** : pending callbacks existants (TTL 5 min,
  adoption inbound par `client_id`/`IP`) — inchangé, conforme.
- **Purge** : `unreachable_lowid` étendu à `!can_do_callback` (couvre le
  cas même-serveur-LowID) ; eMule purge les DS_LOWTOLOWIP à ≥80 %.

### Écarts assumés

- **`OP_DIRECTCALLBACKREQ` sortant non implémenté** : eMule essaie en
  premier un callback UDP direct sur `GetConnectIP():GetKadPort()` quand
  la source a déjà annoncé sa vraie IP + `SupportsDirectUDPCallback` —
  informations qu'une source `lowid:` apprise par SX/serveur ne porte
  pas (`GetConnectIP() == 0` chez eMule aussi). Le cas ne vaut que pour
  des LowID déjà connectés inbound (leur vraie IP connue) — chez nous
  ils deviennent des sources `ip:port`. Inbound `OP_DIRECTCALLBACKREQ`
  est lui déjà traité (rate-limit, gate Kad-firewalled, connect-back).
- **`m_bReaskPending`** (`DownloadClient.cpp:193-198`) : eMule retarde
  le re-ask download d'un LowID **dans notre file d'upload** en
  espérant sa connexion inbound réutilisable — notre modèle n'a qu'un
  rôle par socket (upload XOR download), la connexion inbound ne peut
  pas servir de session download ; parquer équivaut au résultat.
- **Cadence de purge unifiée 60 s** vs eMule 30 s (LOWTOLOWIP) / 40 s
  (NNP) / 60 s (RQF) — eMule partage `m_lastpurgetime` entre les trois
  critères, notre intervalle unique est entre les valeurs.
- Pending callbacks serveur non purgés à la reconnexion serveur — le
  TTL 5 min couvre le changement de namespace LowID.

---

## 13. AICH-keyword / Kad v9 / identité 0.72a — audit phase B, passe 7

### Implémenté

- **`KADEMLIA_VERSION = 0x0a`** (`KADEMLIA_VERSION_CURRENT = 10`,
  `Opcodes.h:32`) : annoncé dans le hello Kad, `OUR_KAD_VERSION`,
  `KadContactWire.version` et les bits `MISCOPTIONS2`.
- **Identité 0.72a** : `CT_EMULE_VERSION = 72<<10` et
  `client_version = 72` (`m_uCurVersionShort = MAKELONG(72,0)`).
- **`TAG_KADAICHHASHPUB (0x36)`** : BSOB de 20 octets publié dans
  `KADEMLIA2_PUBLISH_KEY_REQ` uniquement vers les contacts
  `version >= KADEMLIA_VERSION_AICH (9)` (`Search.cpp` —
  `PreparePacketForTags`) quand le fichier partagé a un `aich_root`.
- **`TAG_PUBLISHINFO (0x33)`** : `UINT32` émis dans les résultats de
  recherche uniquement — compte des éditeurs connus
  (`Indexed.cpp` `WriteTagListWithPublishInfo`).
- **`TAG_KADAICHHASHRESULT (0x37)`** : histogramme BSOB
  `[count][popularity u8][hash20]×n` émis dans les résultats, plafonné
  à 11 entrées (borne eMule ~250 o de charge), votes à popularité nulle
  filtrés.
- **Modèle `CKeyEntry` dans `kad_udp::storage`** : par (keyword,
  file_hash) — registre d'éditeurs dédupliqués par IP (refresh au
  republish, TTL 24 h `KADEMLIAREPUBLISHTIMEK`, cap 100), votes AICH
  par éditeur agrégés en histogramme, popularité des noms de fichiers
  (`GetCommonFileName` — égalité → plus ancien), confiance
  `GetTrustValue` par agrégation `/24` (`PUBLISHPOINTSSPERSUBNET=10`)
  avec compteurs globaux cohérents au refresh/expiry/purge ; entrées
  trusted servies avant les non-trusted (`SendValidKeywordResult`,
  `start_pos` après filtrage).
- **Filtrage des tags résultat** : `0x33`/`0x37` (et doublons `0x36`
  au-delà du premier) extraits du stockage — `m_listTag` conserve
  `TAG_SOURCES` et les tags standards ; `FILENAME`/`FILESIZE` vivent
  dans les champs dédiés (`Indexed.cpp` interception au publish).
- **Ordre des tags à l'émission** (`WriteTagListWithPublishInfo`) :
  `FILENAME`, `FILESIZE`, taglist stockée, `PUBLISHINFO`,
  `KADAICHHASHRESULT`.

### Écarts assumés

- **Bits `MISCOPTIONS2` non annoncés** : bit 11 (Captcha — décodé mais
  pas de réponse implémentée) et bit 12 (`DirectUDPCallback` sortant —
  cf. §12). Annoncer une capacité non implémentée dégraderait
  l'interopérabilité réelle.
- **Vote AICH par IP, pas par publisher-ID** : le registre déduplique
  par adresse IP (eMule utilise `CKadEntry` indexé IP+port ; la
  déduplication IP seule est légèrement plus stricte — plusieurs
  clients NAT derrière une IP comptent pour un éditeur, comme l'agrégat
  `/24` eMule les rapproche déjà).
- **`GetCommonFileName` égalité → plus ancien** : eMule conserve le
  premier nom à popularité maximale — un renommage solitaire ne
  remplace pas le nom consensuel (test `keyword_store_lookup_and_dedup`
  aligné sur cette sémantique).

---

## 14. `known.met` — parseur et import de migration (audit phase B, passe 8)

### Implémenté

- **`mule_format::met::known_met`** — lecture fidèle de
  `CKnownFileList::LoadKnownFiles` (`KnownFileList.cpp:73`) :
  headers `MET_HEADER (0x0E)` / `MET_HEADER_I64TAGS (0x0F)`, layout
  `[date u32][hash16][u16 parts][hashes][u32 tagcount][tags]`,
  extraction de tous les champs de `LoadTagsFromFile` (FILENAME,
  FILESIZE, ATTRANSFERRED±HI, ATREQUESTED, ATACCEPTED, ULPRIORITY avec
  résolution `PR_AUTO → PR_HIGH`, KADLASTPUBLISHSRC/NOTES, FLAGS,
  AICH_HASH base32, LASTSHARED). Vérification finale fidèle
  (`HasExpectedMD4HashCount` : `floor(size/PARTSIZE)+1`, et
  `CalculateMD4HashByHashSet` : root = hash unique ou MD4 de la
  concaténation) — les records fautifs sont comptés `dropped_records`
  comme eMule les supprime. Bornes : `MAX_PART_HASH_COUNT`
  (`MAX_EMULE_FILE_SIZE/PARTSIZE+1`), records ≤ 500 k, tags ≤ 1 k.
- **`mule-cli InspectKnownMet`** — inspection du fichier (records,
  drops, métadonnées par entrée).
- **`shares.importKnownMet {metPath, directory?, verify?}`** —
  migration : jointure `(fileName, fileSize)` (seule clé commune,
  known.met ne stocke pas de chemin) sur les fichiers partageables du
  dossier ; enregistrement du partage sous le hash eMule, part hashes,
  `aich_root`, priorité upload (`PR_*` → chaînes) et statistiques
  `share_upload_stats`. Partages existants (match par hash) : enrichi
  `aich_root`+stats seulement, jamais de reset de la config utilisateur.

### Écarts assumés

- **`verify` par défaut `false`** : l'import fait confiance aux
  hashsets écrits par eMule — éviter de re-hacher tout le dossier
  Incoming est précisément l'intérêt de l'outil. eMule lui-même
  re-hache toujours (`CAddFileThread::Run` — `m_tUTClastModified` est
  ignoré) ; `verify=true` reproduit ce comportement et ne retient que
  les records dont le hash recalculé correspond (`mismatched`).
- **Pas de hash-cache persistant par chemin** : eMule n'en utilise pas
  non plus en 0.72a — `known.met` n'est consulté que comme index de
  fichiers déjà hachés, jamais pour sauter le hachage au démarrage.
- **`FT_AICHHASHSET` (0x35) lu mais non conservé** : le hashset de
  recovery AICH par part est vérifié par eMule puis servi aux pairs ;
  notre modèle ne conserve que la racine — le recovery set serait
  reconstruit depuis le fichier si nécessaire.

---

## 15. File d'attente distante — cadence, multipacket, identité et durée de vie socket (passe 9)

### Cadence du re-ask UDP — fidèle `PartFile.cpp:2291-2294`

Le ping `OP_REASKFILEPING` n'est plus sur une cadence fixe ~21 min :
`wait_in_queue` publie la deadline du prochain re-ask TCP
(`PeerSessionSnapshot.next_tcp_reask_unix_ms` +
`queued_peers[addr].next_tcp_reask_unix_ms`), et la stratégie n'émet le
ping que dans la fenêtre `(1 s, 2 min)` précédant cette deadline, avec
`LastTriedToConnect + 20 min` minimum et `!IsFirewalled()`. Les gardes
d'échec UDP (`> 3` tentatives, ratio `> 30 %`) sont conservés.

### `OP_MULTIPACKET*` — `SendFileRequest` groupé (`DownloadClient.cpp:266`)

- `PeerSxCaps.multipacket` (`MultiPacketLevel` Basic/Ext/Ext2) déduit des
  bits `SupportMultiPacket` (MISCOPTIONS1.1), `SupportExtMultiPacket`
  (MISCOPTIONS2.5), `SupportsFileIdentifiers` (MISCOPTIONS2.13) — la plus
  haute variante annoncée fait foi, comme eMule.
- `file_request.rs` construit le bundle `SendFileRequest` unique :
  `REQFILENAME` (+extinfo quand `ext_req > 0`), `SETREQFILEID` **omis pour
  ≤ 1 part** (règle eMule conservée), `REQUESTSOURCES(2)` (planifié via
  `planned_sx_request` factorisé, mêmes gates `IsSourceRequestAllowed`),
  `AICHFILEHASHREQ`.
- `OP_MULTIPACKETANSWER*` entrant est éclaté en sous-paquets
  (`expand_multipacket_answer` + `VecDeque<RawPacket>`) qui réutilisent
  les handlers existants — preuve socket-level
  (`test_peer_connect_multipacket_file_request`).

### `OP_CHANGE_CLIENT_ID` appliqué (`ListenSocket.cpp:590`)

Le payload `[newUserID u32][newServerIP u32]` n'est plus seulement
journalisé : `LowID + serveur connu` (serverlist → port) ou
`newID == IP du pair` (HighID) mettent à jour `server_ip`/`server_port`
de la source (`lowid:<id>` et/ou `ip:port`) via
`update_peer_source_server` — les métadonnées utilisées par
`IsLocalServer`. Contenu inconnu ou tronqué → ignoré, comme eMule.

### Durée de vie de la socket en file — `CheckTimeOut` fidèle

- eMule tue la socket après `CONNECTION_TIMEOUT` (**40 s**) sans AUCUN
  trafic (`OnSend`/`OnReceive`/`SendPacket` resettent `timeout_timer`,
  `ListenSocket.cpp:120-155`) ; `DS_ONQUEUE` survit **logiquement** avec
  `socket == NULL`. Notre `queue_timeout` (ancien 15 min de silence
  post-reask) est repris sur cette sémantique : 40 s d'inactivité,
  resetté à chaque envoi ET réception.
- La mort de la socket en file (drop local ou fermeture distante) n'est
  **plus** un échec : la source reste `Allowed` avec cooldown =
  deadline de re-ask (`GetTimeUntilReask`), et l'entrée `queued_peers`
  conserve l'état logique — le ping UDP ne part que dans cet état
  (`!(socket && socket->IsConnected())`, `DownloadClient.cpp:1344`),
  c'est-à-dire jamais tant qu'une session TCP vit. Seuls FNF/NNP
  (issues terminales) suppriment l'entrée — eMule garde `DS_ONQUEUE`
  pour toute autre déconnexion (`BaseClient.cpp:1126-1135`).

### Écarts assumés

- **Socket partagée multi-rôle** : eMule réutilise une connexion encore
  vivante (upload/chat) pour le re-ask TCP sans reconnecter ; nos
  sessions sont mono-rôle — le re-ask est toujours une reconnexion,
  sauf si le pair arrose de paquets > 40 s (alors la branche re-ask
  interne agit comme le chemin `DS_CONNECTED` eMule).
- **`queued_peers` en mémoire seule** : le rang de file n'est pas
  persisté entre redémarrages (eMule non plus ne le restaure pas — un
  client redémarré repart à `TryToConnect`).
- **Backoff vs deadline sur mort en transfert** : une déconnexion en
  `DS_DOWNLOADING` repasse `DS_ONQUEUE` chez eMule (reask à deadline) ;
  nous gardons `Failed`+backoff (cadence comparable ≤ 20 min).

## 16. Migration eMule — `emfriends.met`, `cancelled.met`, identité (passe 10)

Trois formats de migration complètent `known.met` (§14) : amis, fichiers
annulés, et identité (clé RSA + user hash) — la continuité des crédits.

### `emfriends.met` → `friends`

- Parseur `mule_format::met::emfriends` fidèle à `CFriend::LoadFromFile`
  (`Friend.h:42-43`) : record `[userhash 16][lastUsedIP u32][lastUsedPort
  u16][lastSeen u32][lastChatted u32][tagCount u32][tags]` ; `FF_NAME`
  (0x01, première valeur retenue — `m_strName.IsEmpty()` eMule) et
  `FF_KADID` (0x02) reconnus, autres tags ignorés.
- `friends.importEmfriendsMet {metPath}` : records `!HasUserhash`
  (amis IP-only) rapportés mais non importés — notre slot ami est
  indexé par hash, comme la recherche `IsFriend` eMule. `add_friend`
  est un upsert : réimport rafraîchit le nom (`updated`), hashes
  enregistrés auprès de l'upload manager (`set_friend`).

### `cancelled.met` → `cancelled_files`

- Parseur `mule_format::met::cancelled_met` fidèle à `LoadCancelled`
  (`KnownFileList.cpp:167-194`) : `0x0F` = `[seed u32][count u32]
  {[hash 16][tagCount u8][tags]}`, `0x0E` déprécié = hashes réels.
  `cancelled_mask(seed, hash)` = `MD5(seed_le || hash)` — eMule ne
  stocke jamais le hash réel.
- Table `cancelled_files(masked_hash, seed)` : le seed **suit chaque
  entrée** car un fichier importé garde son `m_dwCancelledFilesSeed` ;
  les entrées `0x0E` sont masquées sous **notre** seed à l'import —
  conversion identique au chargement eMule.
- `IsCancelledFileByID` → `is_cancelled_file` : recalcule le masque
  sous chaque seed connu ; **toujours faux** si `remember_cancelled`
  est off (`IsRememberingCancelledFiles` eMule — même gate que
  l'historique).
- `AddCancelledFileID` → `remove_download` enregistre le hash masqué
  des téléchargements **non terminés** (`CPartFile::Delete` ne
  s'applique pas aux PartFile finalisés chez eMule).
- Sémantique eMule respectée : « cancelled » est un **marquage** des
  résultats de recherche (`downloads.isCancelled`), pas un blocage.

### Identité — `cryptkey.dat` + `preferences.dat`

- eMule génère le user hash **aléatoirement** et le persiste dans
  `preferences.dat` (`Preferences_Ext_Struct` : `[version u8][userhash
  16][WINDOWPLACEMENT]`, `Preferences.h:78-86`) — indépendant de la clé
  RSA. Notre user hash dérivé (`MD4(pubkey)` + magic bytes 0x0E/0x6F)
  ne peut donc pas retrouver un hash eMule existant : l'import lit les
  deux fichiers.
- `mule_format::met::preferences_dat` : extraction bornée du user hash
  (17 premiers octets) + `is_bad_hash` (`OtherFunctions.h:380-383`) —
  un hash dégénéré stocké = fichier corrompu, rejeté.
- `identity.importEmuleIdentity {cryptkeyPath?, preferencesDatPath?,
  replace?}` : `cryptkey.dat` validé (`Base64(PKCS#1 DER)` RSA-384,
  même format que le nôtre — copie atomique) ; user hash →
  `userhash.dat` (16 octets bruts). `replace:true` requis si les
  fichiers existent — écraser une identité perd ses crédits.
- `IdentityService::user_hash()` : l'override importé gagne (verbatim —
  les marqueurs eMule sont déjà dedans et c'est la clé des crédits) ;
  sinon le hash dérivé. `rotate_key` **supprime** `userhash.dat` : le
  hash importé appartient à la clé eMule remplacée.
- Propagation à chaud identique à `rotate_identity` (`propagate_identity`
  factorisé) : hellos, challenges SUI, firewall Kad, KadID — les
  crédits eMule sont valides sans redémarrage.

### Écarts assumés

- `emfriends.met` : `lastUsedIP/Port`, `lastSeen`, `lastChatted`,
  `kadId` lus mais non persistés — notre `Friend` ne les modélise pas.
- `preferences.dat` : seul le user hash est importé — le reste du
  struct (placement fenêtre) est sans valeur hors eMule.
- Marquage des résultats de recherche câblé : `emit_search_results`
  ajoute `"cancelled"` par fichier (TCP `OP_SEARCHRES` et UDP
  `OP_GLOBSEARCHRES` via `search_result_cancelled_flags` →
  `is_cancelled_file`) — le marquage eMule `IsCancelledFileByID`
  (`m_MyListCtrlItem`) ; l'API `downloads.isCancelled` reste dispo.

### `clients.met` → `peer_credits`

- Parseur `mule_format::met::clients_met` fidèle à `LoadList`
  (`ClientCredits.cpp:156-233`) : `[version u8][count u32]` puis
  `CreditStruct` packed — `0x12` : 119 B (`ClientCredits.h:41-52`),
  `0x11` (29a) : 38 B sans pubkey. Autre version → refus du fichier,
  comme eMule.
- `credits.importClientsMet {metPath}` : nos crédits sont indexés par
  `MD4(pubkey)` — `abySecureIdent[..nKeySize]` fournit exactement la
  clé RSA nécessaire ; records `nKeySize == 0` rapportés comme
  `skippedNoPubkey` (crédits faibles eMule, non réclamables sous SUI).
  Entrées `tLastSeen < now - 150 j` supprimées à l'import — le drop
  eMule au chargement (`ClientCredits.cpp:207-213`).
- C'est le chaînon qui rend `identity.importEmuleIdentity` utile : le
  user hash importé + la clé RSA importée permettent aux pairs de
  vérifier SUI, et `clients.met` restaure ce qu'ils nous doivent.

### Formats eMule couverts — bilan

| Fichier | Statut |
|---|---|
| `known.met` | ✅ parseur + `shares.importKnownMet` (§14) |
| `part.met` | ✅ parseur + `downloads.importPartMet` |
| `server.met` | ✅ parseur + `servers.importMet` |
| `nodes.dat` | ✅ parseur + `kad.importNodesDat` |
| `emfriends.met` | ✅ parseur + `friends.importEmfriendsMet` |
| `cancelled.met` | ✅ parseur + `downloads.importCancelledMet` |
| `clients.met` | ✅ parseur + `credits.importClientsMet` |
| `cryptkey.dat` | ✅ `identity.importEmuleIdentity` |
| `preferences.dat` | ✅ user hash via `identity.importEmuleIdentity` |
| `shareddir.dat` | ⚠️ non prévu — pas de feature « répertoires partagés » multiples |
| `preferences.ini` | ⚠️ non prévu — réglages GUI eMule hors scope (grosse surface, faible valeur) |
| `addresses.dat`, `webservices.dat`, `statistics.*`, `AC_*.dat` | ⚠️ non prévus — GUI/config eMule marginale |

## 17. Callback UDP direct (`OP_DIRECTCALLBACKREQ`) + `m_bReaskPending` — audit phase B, passe 11

### Sens du mécanisme

`OP_DIRECTCALLBACKREQ` (`0x95`, `Opcodes.h:296`) sert à joindre un pair
**TCP-injoignable mais UDP-ouvert** (NAT full-cone) : l'émetteur — lui-même
TCP-joignable — envoie un datagramme UDP sur le port Kad du destinataire,
qui compose alors une connexion TCP de rappel (`TryToConnect` step 4,
`BaseClient.cpp:1403-1415`).

### Émission (rôle du demandeur)

- Condition eMule : `SupportsDirectUDPCallback() && GetUDPPort() != 0 &&
  GetConnectIP() != 0` — `SupportsDirectUDPCallback()` exige
  `m_fDirectUDPCallback && HasValidHash() && GetKadPort()`
  (`updownclient.h:211`). Chez nous : `PeerSource.crypt_options & 0x08`
  (byte connect-options, bit 3) + `udp_port != 0` + user hash présent.
- Payload filaire `[PR_EMULE][0x95][tcp_port u16 LE][userhash 16][conn_opts u8]`
  (`CSafeMemFile` = little-endian) — encodeur
  `client_udp::encode_direct_callback_req`, constante
  `CONNECT_OPT_DIRECT_CALLBACK = 0x08`.
- Ordre eMule `TryToConnect` : **direct callback → server callback →
  buddy callback**. La stratégie tente le direct callback avant le relais
  buddy (`kadcb:`) et retombe sur le buddy si la socket UDP locale est
  absente (`send_direct_callback_req` → `false`).
- Le byte `conn_opts` = `GetMyConnectOptions(true, false)` : bits 0-2 crypt
  uniquement — le bit direct-callback reste à 0 (on demande à être appelé,
  on n'est pas le receveur de rappel).
- Chiffrement : eMule passe `bEncrypt = ShouldReceiveCryptUDPPackets()`
  (capacité UDP-crypt du **destinataire**) — notre `client_udp` envoie en
  clair comme le reste de ses datagrammes (réask ping inclus) : écart
  assumé et documenté, l'obfuscation UDP client n'est pas implémentée.

### Réception (rôle du pair firewalled)

- Gate eMule : `AllowCalbackRequest(ip)` (fenêtre 180 s/IP,
  `ClientList.cpp:913-928`) puis `CKademlia::IsRunning() &&
  CKademlia::IsFirewalled()` (`ClientUDPSocket.cpp:373`).
  `CKademlia::IsFirewalled()` = verdict **Kad-TCP** (`CPrefs::
  GetFirewalled`, `Kademlia.cpp:324`) — PAS le testeur UDP : le receveur
  type a le TCP mort et l'UDP ouvert, le statut UDP ne doit donc pas gater
  la réception. eMule vaut `firewalled` par défaut tant que 2 contacts
  n'ont pas confirmé le TCP ; notre proxy = `!is_high_id` (LowID
  serveur-assigné) — `UdpFirewallStatus` n'intervient plus (correction
  d'inversion : gate précédent exigeait `Firewalled` UDP, ce qui aurait
  exclu justement les receveurs cibles).
- Puis `FindClientByUserHash`/`AddClient` + `SetConnectOptions(opts,
  true, false)` + `SetDirectUDPCallbackSupport(false)` + `TryToConnect()`
  → chez nous : `InboundListenerService::connect_back` avec `our_id`
  (l'ID réel de la session serveur, comme eMule envoie son vrai ID dans
  `TryToConnect`).

### Capacité annoncée

- Sortante eMule (`BaseClient.cpp:1007`, `GetMyConnectOptions` bit 3 /
  MISCOPTIONS2 bit 12) : `theApp.IsFirewalled() && KadRunning &&
  !IsFirewalledUDP(true) && IsVerified()` — net : **pas de HighID eD2K**
  + UDP Kad testé ouvert. Bits 12 des hellos (outbound + answer inbound)
  et bit 3 du `TAG_ENCRYPTION` publié (`Search.cpp:692` —
  `GetMyConnectOptions(true, true)`) suivent cette condition.
- `PeerSource.udp_port` (colonne SQLite ajoutée, `ALTER TABLE` idempotent
  + `CASE WHEN excluded.udp_port != 0` sticky) = eMule `GetKadPort()` :
  alimenté par `KadSourceWire.udp_port` (recherche), le tag
  `CT_EMULE_UDPPORTS` du hello (`peer_udp_port`), 0 sinon.
- Ingest Kad : une source `tcp_port == 0` avec le bit direct-callback est
  conservée **sans buddy** (type 6, `DownloadQueue.cpp:1596`) — avant on
  exigeait `buddy_id`, ce qui dropait ces sources.

### `m_bReaskPending` (`DownloadClient.cpp:185-206`)

- Au re-ask d'un LowID **déjà sollicité** (`GetLastAskedTime() > 0` ↔
  entrée `queued_peers`) **et présent dans notre file d'upload**
  (`US_ONUPLOADQUEUE` ↔ `waiting_rank_by_ip`), eMule ne compose pas :
  `DS_ONQUEUE` + `m_bReaskPending`, et laisse au pair un cycle pour venir
  avec son propre re-ask d'upload. Le flag est consommé à la connexion
  entrante (`ConnectionEstablished` → `SendFileRequest` immédiat,
  `BaseClient.cpp:1552`) ; la branche `!CanDoCallback` (LowID↔LowID) est
  déjà couverte par notre gate `can_do_callback` (passe 9).
- Implémentation : `QueuedPeerState.reask_pending` +
  `peer_session.defer_reask_for_queued_peer(ip, hash, next)` (une seule
  pose, `!m_bReaskPending`) / `take_reask_pending(ip)` (consommé à la
  connexion entrante dans `dispatch_requests` → `OP_DIRECTCALLBACKREQ`
  immédiat + `register_pending_kad_callback`). La deadline
  `next_tcp_reask_unix_ms` est poussée d'un cycle `FILE_REASK_INTERVAL`
  pour garder la fenêtre UDP cohérente.
- Limite : n'opère que pour les sources à IP réelle connue (`kadcb:`) —
  les sources `lowid:` ne portent pas l'IP réelle du pair (le champ `ip`
  contient l'ID LowID) et ne peuvent pas être matchées à une entrée de
  file d'upload par IP, comme le `Compare` par userhash d'eMule le ferait
  si le hash était connu (les sources serveur n'en portent pas).

### Restes

- Le byte `TAG_ENCRYPTION` 0xF3 des contacts publiés (**bit 3**) est
  désormais honoré à l'ingest et à l'émission ; `KadPublishSourceReq.
  encryption` exposé au decode (round-trip testé).
- eMule partage le flag `m_bReaskPending` avec la socket sortante
  (`ConnectionEstablished` → `SendFileRequest` sur la socket entrante) ;
  notre split inbound/outbound fait l'équivalent par callback immédiat.

## 18. Parité `CT_EMULE_MISCOPTIONS2` sur tous les hellos — audit phase B, passe 12

### Divergence corrigée

Le hello du **connect-back** (`inbound_listener/connection.rs`, après un
`OP_CALLBACKREQUESTED`/`OP_DIRECTCALLBACKREQ`) ne posait que les bits
crypt (7-9) et omettait le tag entier sans obfuscation — le pair distant
voyait `dwEmuleTags` sans le bit misc2 → client classé « vieux eMule ».
eMule réutilise le même `SendHelloPacket` pour cette connexion.

- `services/hello_caps.rs::misc_options2()` — source unique du mot
  misc2 : `FileIdent<<13 | DirectUDPCallback<<12 | SrcExch2<<10 |
  Requires<<9 | Requests<<8 | Supports<<7 | ExtMultiPkt<<5 |
  LargeFiles<<4 | KadVersion<<0`. Bit 11 (chat CAPTCHA) toujours 0.
- Appliqué aux 3 hellos : outbound (`peer_session/handshake.rs`),
  réponse inbound et connect-back (`inbound_listener/connection.rs`).
- Bit 12 = `tcp_low_id && udp_verified_open` (verified && Open),
  sémantique `GetMyConnectOptions(true,true)` + `!IsFirewalledUDP &&
  IsVerified` de `BaseClient.cpp:1007`.
- Le hello minimal du test pare-feu UDP (`kad/firewall.rs`) reste
  allégé par design (socket de diagnostic, pas une session pair).

### Hygiène des fixtures de test

- Les littéraux de ports en range eMule (`4661`-`4665`, `4672`, `4673`)
  des fixtures **synthétiques** sont déplacés vers la plage éphémère
  `49661`-`49675` (≥ 49152) : ces ports sont historiquement filtrés par
  certains FAI et eMule lui-même tire un port aléatoire
  (`Preferences.cpp:1969`, `GetRandomTCPPort` = `4096+rand%61000`) au
  premier démarrage — jamais 4662 par défaut.
- Conservés volontairement : les vecteurs **capturés d'eMule réel**
  (`ed2k_corpus_tests.rs`, `manifest.json`, `server_codec_tests.rs`,
  `server_met_vectors.rs`) où le port fait partie de la fidélité filaire,
  et `4660` = le port RPC par défaut du daemon (hors protocole eMule).
- Défaut production inchangé : `DaemonConfig` écoute déjà sur
  `39735`/`39737` (hors range eMule, dans la bande de randomisation).
