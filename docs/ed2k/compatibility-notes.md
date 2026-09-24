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

- **`OUR_KAD_VERSION = 8`** alors qu'eMule 0.72a annonce
  `KADEMLIA_VERSION = 0x0a` (`Opcodes.h:32`) : Kad 9 ajoute la gestion
  des hash AICH dans le stockage de mots-clés, non implémentée.
  Cohérent avec l'identité 0.70a annoncée ; annoncer 10 surestimerait
  les capacités réelles.
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
