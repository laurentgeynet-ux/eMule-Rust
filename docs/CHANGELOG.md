# Journal des Modifications — eMuleRust

Toutes les modifications notables de ce projet sont documentées dans ce fichier.
Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et ce projet adhère à [Semantic Versioning](https://semver.org/lang/fr/).

## [Unreleased]

Couche d'IA locale : un modèle loopback (Ollama/llama.cpp via `/api/chat`)
pilote le client à travers le registre d'outils MCP existant — aucun
doublon de logique métier, `mule-daemon` reste l'autorité de sécurité.

- **Pipeline d'écriture disque eD2K optimisé** (`mule-core`,
  `mule-daemon`, `mule-format`) — inspiré des constats I/O de
  libtorrent/rqbit :
  - **Staging RAM des parts** (`downloads/staged.rs`, modèle eMule
    `m_tData`) : les blocs reçus alimentent un buffer borné par part
    (`MAX_STAGED_PARTS` = 4, ≈ 37 Mio/download) ; la vérification MD4 à
    la complétion se fait en mémoire — plus de relecture de 9,28 Mio par
    part (`pread` ≈ 0 en réception). Libération à la complétion,
    l'éviction, la récupération AICH et la finalisation.
  - **Vérification finale sans relecture** : `try_verified_root` dérive
    la racine eD2K du hashset de parts déjà vérifiées
    (`ed2k_root_from_part_hashes`, mêmes règles que `Ed2kHasher` — part
    vide eMule aux multiples exacts) ; repli sur `hash_reader` quand le
    hashset est inconnu ou contredit. Supprime ~`total_size` de lecture
    par download.
  - **I/O synchrones hors executor Tokio** : `inject_range_brief`
    (chemins compressé et non compressé), le flush périodique
    (`flush_dirty_downloads` + `decay_stale_rates`) et
    `save_active_part_mets` passent par `spawn_blocking`/`blocking_lock`
    — fin du gel possible de l'executor sous pression disque.
  - **`received_ranges` en delta O(1)** : `append_received_range`
    (upsert `MAX(end_offset)`) remplace le DELETE+INSERT global par bloc
    dans le hot path ; la réécriture complète devient une compaction à
    cadence `PART_PERSIST_INTERVAL`.
  - **I/O positionnées** : `PartFile::write_segment`/`read_range` via
    `seek_write`/`seek_read` (boucles anti écritures/lectures
    partielles), plus de `seek` mutable sur le descripteur partagé ;
    compteurs `PartFileIoStats` (ops/octets lus, écrits, fsync) logués à
    la complétion.
  - **Pragmas SQLite** : `busy_timeout` 5 s, `temp_store=MEMORY`,
    `cache_size` 20 Mio, `wal_autocheckpoint` ~32 Mio.

- **Profils de filtrage des outils MCP** (`mule-mcp`) :
  - `ToolRisk { Read, Mutate, Destructive }` + `ToolProfile { read, safe,
    full }` classifiant les ~87 outils `emule_*` par verbe (avec table
    d'overrides pour les ambigus) ; `--tool-profile` en CLI, `full` par
    défaut (rétrocompatible).
  - Filtrage appliqué aux **deux** frontières : `tools/list` filtrée *et*
    `tools/call` refuse les outils hors profil — un client externe ne peut
    pas contourner une allowlist de présentation.
  - Test d'exhaustivité : tout nouvel outil non classé casse le build.

- **Nouveau crate `mule-assistant`** — orchestrateur LLM local :
  - Boucle agentique bornée (`MAX_TOOL_ROUNDS`) : `chat → tool_calls →
    policy → execute_tool → role:"tool" → repeat` ; le message assistant
    complet (avec ses `tool_calls`) est conservé dans l'historique avant
    les résultats, conformément au format natif Ollama (`tool_name`, ids
    internes optionnels).
  - **Policy** : outils visibles = `list_tools ∩ allowlist curée ∩
    capacités daemon (table préfixe→catégorie) ∩ profil MCP` ; revérifiée
    juste avant chaque exécution, confirmation utilisateur sur arguments
    exacts pour les outils `Destructive`.
  - **Backend Ollama** : POST `/api/chat` streamé, endpoint validé —
    HTTP, adresse IP littérale **loopback uniquement** (`127.0.0.1`,
    `::1`, refus des noms à résoudre), redirections désactivées
    (`redirect::Policy::none()`), proxys d'environnement désactivés,
    timeouts configurés ; `load_duration`/`eval_duration` logués.
  - **Streaming** : `AssistantEvent` (`token`/`toolStart`/`toolResult`/
    `final`/`notice`/`error`) — tokens affichés au fil de l'eau, appels
    d'outils assemblés *puis* validés *puis* exécutés ; `thinking` jamais
    exposé ; résultats d'outils tronqués pour le LLM, complets pour l'UI.
  - **Transport WS loopback** (`--serve`) : JSON-RPC `assistant.hello`
    (jeton de session daemon — pas d'auth parallèle), `assistant.chat`,
    `assistant.confirm`, `assistant.cancel` ; événements en notifications,
    drain des events en file à la fin de tour.
  - REPL (`chat`, `--prompt`) et `MockBackend` pour tests 100 % hors-ligne.

- **Panneau Flutter** (`app_shell`, MVVM) :
  - `AssistantClient` (mule_client) : client JSON-RPC/WS du protocole
    assistant ; modèles `AssistantEvent`/`AssistantConfirmRequest`/`ChatEntry`.
  - `AssistantController` : état de conversation, tokens streamés,
    confirmation, annulation ; jeton résolu depuis `<workdir>/auth.token`
    via `assistant_token(_io|_stub)` — indisponible sur web.
  - Section « Assistant IA » (route `/assistant`, sidebar) : bulles de
    chat, tuiles d'outils, bannière de confirmation, état hors-ligne avec
    retry ; design tokens uniquement.

- **Bureau Windows — systray et icône taskbar** (`apps/desktop`,
  `app_shell`) :
  - **Fermeture vers la zone de notification** (comportement type client
    P2P) : le bouton de fermeture / Alt+F4 masque la fenêtre dans le
    systray au lieu de quitter — l'application, l'état Flutter et
    `mule-daemon` restent actifs.
  - `setPreventClose` + nouveau coordinateur `AppQuit`
    (`app_shell/lib/core/app_quit.dart`, Dart pur) : le handler
    `onExitRequested` répond `cancel` et déclenche le masquage via
    `AppQuit.onHideToTray`. Nécessaire car le moteur Flutter consomme
    `WM_CLOSE` pour son mécanisme d'exit-request — l'événement `close` ne
    parvient jamais aux listeners `window_manager`.
  - Menu systray localisé (FR/EN) « Afficher eMuleRust » / « Quitter » ;
    clic gauche ou double-clic sur l'icône restaure la fenêtre avec le
    focus ; « Quitter » lève l'interception puis repasse par un close
    classique — l'arrêt gracieux du daemon (`supervisor.stop()` puis
    sortie) reste à un seul endroit.
  - Toast unique à la première fermeture signalant que l'application
    continue en arrière-plan (flag SharedPreferences persisté).
  - **Icône taskbar générique corrigée** : `local_notifier` déclare
    l'AUMID `eMuleRust`, que la taskbar résout via le raccourci
    `eMuleRust.lnk` du menu Démarrer — or WinToast ne recrée jamais un
    raccourci existant : après déplacement du bundle portable, la cible
    morte affichait l'icône générique. `DownloadNotifier` supprime
    désormais le `.lnk` quand il ne contient pas le chemin de
    l'exécutable courant (recherche ASCII et UTF-16LE), le laissant se
    recréer avec la bonne cible à chaque lancement.

## [0.2.0-RC2] - 2026-09-25

Deuxième release candidate — campagne d'audit filaire et comportemental
complète contre les sources officielles d'eMule 0.72a, validée par des
tests d'interopérabilité live contre le binaire réel.

- **Interopérabilité live eMule 0.72a validée — 7/7 tests** (`mule-protocol`) :
  le harnais Tier-3 (`EMULE_REFERENCE_BIN` + `EMULE_REFERENCE_PROFILE`)
  lance le vrai `emule.exe` 0.72a (SHA-256 épinglé) en profil isolé
  loopback et exécute handshake, file-status, transferts de blocs,
  reprise et finalisation — tous verts contre le binaire officiel.
  - **`OP_COMPRESSEDPART` (0x40) 32-bit** : décodeur borné
    (`<hash16><start u32><compressed_size u32><zlib>`) + encodeur
    symétrique sous `PR_EMULE` (`OP_EMULEPROT`, fidèle à
    `UploadDiskIOThread.cpp`). Correction d'un bug latent : l'encodeur
    I64 émettait `PR_EMULE_COMPRESSED` (0xD4) au lieu de `PR_EMULE`.
  - **Réassemblage zlib incrémental** côté client loopback :
    accumulateur par `(hash, début de bloc)` — eMule découpe le flux
    compressé en tranches de ≤ 10 240 octets portées par plusieurs
    paquets ; sous-plages validées dans le tracker de blocs.
  - **Bug de flux corrigé** dans `receive_sending_part` : après un
    paquet compressé, le code tombait dans le `read()` bloquant au lieu
    de drainer les parts déjà produites — deadlock sur tout transfert
    compressé multi-paquets.
  - Profil de référence paramétrable : `EMULE_REFERENCE_PROFILE`
    (`Emule<VV>a` → `client_version` attendu dérivé), `AppVersion`
    injectée dans le `preferences.ini` généré pour court-circuiter
    l'assistant de premier démarrage modal.

- **Anti-abuse aligné sur eMule 0.72a** (`mule-daemon`) :
  - **`CLIENTBANTIME` = 2 h** : les bans de sources IP expirent comme
    `IsBannedClient` (`ClientList.cpp`) au lieu d'être permanents.
  - **`BADCLIENTBAN`** : `AddRequestCount` (re-ask du même fichier <
    `MIN_REQUESTTIME` = 10 min, 4 strikes) produit désormais un vrai ban
    IP 2 h + coupure de la connexion (eMule `Ban()` →
    `AddBannedClient`), pas seulement un score à 0 — exemption
    `GetFriendSlot` respectée.
  - **`ComparePriorUserhash`** : les couples `(IP, port)` sont suivis 2 h
    (`KEEPTRACK_TIME`, enregistrés à la vérification SUI) — une IP:port
    revue avec un userhash différent est bannie et déconnectée.
  - **Filtrage UDP pair** : `ipfilter` + port 0 appliqués à chaque
    datagramme pair (`ClientUDPSocket.cpp:76`) et bannis rejetés ;
    le listener TCP vérifie l'IP bannie à l'accept et après le hello.

- **Couverture opcodes exhaustive** (`mule-protocol`, `mule-daemon`) —
  diff mécanique des ~95 `case OP_*` d'eMule (`ListenSocket`,
  `ClientUDPSocket`, `UDPSocket`, `ServerSocket`) : tout est couvert à
  l'exception d'`OP_PREVIEWANSWER` (nous annonçons preview=0 et
  n'émettons jamais `OP_REQUESTPREVIEW`, drop silencieux fidèle) :
  - **Browsing des partages** : `OP_ASKSHAREDFILES{,ANSWER}`,
    `OP_ASKSHAREDDIRS{,ANSWER}`, `OP_ASKSHAREDFILESDIR{,ANSWER}`.
  - **Verdict pare-feu Kad TCP** : `OP_FIREWALLED{2}_REQ` sortant +
    verdict `0xA8` entrant.
  - **Description serveur** : `OP_SERVER_DESC_REQ/RES` (Lugdunum).
  - **`OP_PUBLICIP_REQ`** entrant + **`OP_OUTOFPARTREQS`** sortant sur
    rotation de slot.
  - **`OP_PACKEDPROT` pair (0xD4)** : unpack entrant + réponses de
    sources packées ; Kad UDP packé (0xE5).
  - **Callbacks LowID** : `OP_PORTTEST` TCP, relais buddy
    `OP_REASKCALLBACKTCP`/`OP_CALLBACK` (check `~kad_id`), et
    `OP_DIRECTCALLBACKREQ` sortant avec re-ask différé
    (`m_bReaskPending`).
  - **Hashsets `FileIdentifier`** : `OP_HASHSETREQUEST2`/`ANSWER2`
    (identifiant composite + options MD4/AICH, dispatch sous PR_EMULE).
  - **Rang de file** : `OP_QUEUERANK` legacy (0x5C) entrant +
    correction `OP_QUEUERANKING` émis en `PR_EMULE` avec le payload
    eMule de 12 octets (`[rank u16][10 zéros]`) au lieu de `PR_ED2K`/u32.
  - **Hello connect-back** : `misc_options2` complet + ports de test
    hors range eMule.
  - **Opcodes 0x49/0x4D** (phase B) : handshake, requêtes standalone et
    reasks alignés sur `ListenSocket.cpp`/`ClientUDPSocket.cpp`.

- **File d'upload persistante aux déconnexions** (`mule-daemon`) :
  eMule `Disconnected` (`BaseClient.cpp:1169`) conserve un client en
  file (`US_ONUPLOADQUEUE` → pas de suppression) jusqu'à
  `MAX_PURGEQUEUETIME`. Désormais une fin de socket conserve l'entrée
  (`mark_disconnected`, score et wait intacts) alors que les fins
  explicites (CANCELTRANSFER, END_OF_DOWNLOAD, RPC) la retirent ;
  le meilleur injoignable dominant le meilleur connecté est marqué
  `add_next_connect` (`FindBestClientInQueue`) et sa reconnexion ouvre
  un slot immédiat (`AddClientToQueue`).

- **Comportement download** (`mule-daemon`) :
  - **Purge `RemoteQueueFull`** des sources à 80 % de `maxSources`
    (`PartFile.cpp`).
  - **A4AF au niveau socket** : `SwapToAnotherFile`/`DoSwap` sur la
    socket live.
  - **Cycle re-ask de file distante fidèle** : gestion de la socket et
    fenêtre UDP `(1 s, 2 min)` pré-reask TCP.

- **Kad aligné sur eMule 0.72a** (`mule-protocol`, `mule-core`) :
  expressions de recherche restrictives (`StartSearchTermsMatch`),
  publication de mots-clés AICH, modèle `CKeyEntry`, identité eMule
  0.72a, filaire et constantes réalignés.

- **Migration eMule complète** (`mule-format`, `mule-daemon`,
  `app_shell`) :
  - **`known.met`** : parser + import.
  - **`clients.met`** : import des crédits pairs vers `peer_credits`.
  - **`emfriends.met` + `cancelled.met`** + identité eMule
    (`cryptkey` + `userhash`).
  - **Assistant de migration** dans Paramètres → Avancé.
  - **Recherche annulée** : marquage `IsCancelledFileByID` + badge
    « Annulé » sur les résultats.

- **Refactor — mutualisation des tests** (`mule-test-support`,
  `mule-protocol`) : helpers d'intégration cross-crates dans
  `mule-test-support`, fixtures protocolaires communes
  (`tests/common/`), garde-fou anti-doublons
  (`scripts/test_duplicates.ps1`).

- **Priorités download/upload alignées sur eMule 0.72a** (`mule-daemon`,
  `app_shell`) :
  - **Priorité « auto » download** (`PR_AUTO` = 5) : acceptée par
    `downloads.setPriority`, persistée en DB et dans le `.part.met`
    (`FT_DLPRIORITY`, import `PR_AUTO` ↔ `"auto"` — une priorité
    automatique importée n'est plus convertie en manuelle). Le poids
    effectif est recalculé à chaque tick du coordinateur selon
    `CPartFile::UpdateAutoDownPriority` : plus de 100 sources connues →
    low, plus de 20 → normal, sinon → high ; il alimente le rebalance
    A4AF (`ProcessA4AFClients`). Les priorités manuelles ignorent le
    compte de sources.
  - **Priorité « auto » upload** : acceptée par
    `shares.setUploadPriority` ; un registre par fichier recalcule la
    priorité effective selon `CKnownFile::UpdateAutoUpPriority` à chaque
    entrée/sortie d'uploader (plus de 20 uploaders → low = 6, plus de
    1 → normal = 7, sinon → high = 9). Les scores de file sont
    rafraîchis à chaque tri (attente × crédits × priorité effective) —
    grants directs, file, reasks, promotions, rotation et publication
    serveur utilisent tous la valeur effective.
  - **Préférences eMule** : `autoPriorityNewDownloads` (eMule
    `DAPPref`) et `autoPriorityUploads` (`UAPPref`), toutes deux `true`
    par défaut comme eMule — les nouveaux downloads et partages
    démarrent en mode auto. Une priorité importée depuis un
    `.part.met` prime sur le défaut global.
  - **UI** : option « Auto » dans le sélecteur de priorité des
    téléchargements et des partages ; interrupteur
    `autoPriorityUploads` dans Paramètres.
  - Régressions couvertes : seuils `UpdateAutoDownPriority` bornes
    exactes (20/21/100/101), `UpdateAutoUpPriority` transitions (1/2/20/
    21 uploaders), manuel désactive auto, roundtrip `.part.met`
    `FT_DLPRIORITY`/`PR_AUTO`.

- **Alignement swarm sur eMule 0.72a** (`mule-daemon`) :
  - **Réinterrogation des sources par canal** (au lieu de la fenêtre
    adaptative 30 s/60 s/300 s) : TCP `OP_GETSOURCES` toutes les
    `SERVERREASKTIME` = 15 min si sources < `GetMaxSourcePerFileSoft`
    (90 % du cap, ≤ 750) ; UDP `OP_GLOBGETSOURCES2` toutes les
    `UDPSERVERREASKTIME` = 30 min si sources < `GetMaxSourcePerFileUDP`
    (75 % du cap, ≤ 50) ; Kad `search_sources` à `KADEMLIAREASKTIME` = 1 h
    × `m_TotalSearchesKad` (backoff jusqu'à 7 h), borné par
    `KADEMLIATOTALFILE` = 5 recherches simultanées et `KADEMLIAASKTIME`
    = 1 s entre démarrages.
  - **Suppression de l'allocateur water-filling** : plus de quota par
    fichier — chaque fichier connecte ses sources viables jusqu'à son
    plafond (`FT_MAXSOURCES` / `max_peers_per_download`), la limite
    globale `max_global_download_peers` jouant le rôle de
    `maxconnections` (`TooManySockets`). `swarm_allocator.rs` supprimé ;
    `min_peers_per_download` ne sert plus qu'au rebalance A4AF.

- **Observabilité locale complète** (`mule-daemon`, `mule-mcp`,
  `app_shell`, `mule_client`, `ui_kit`) :
  - **Logs rotatifs structurés** : `data/logs/daemon.<date>.log` en
    JSONL (rotation quotidienne, 7 fichiers retenus, writer
    non-bloquant `tracing-appender`) ; console lisible conservée ;
    l'ancien `daemon.log` plat est supprimé au démarrage.
  - **Capture des panics** : hook `std::panic` → `logs/crash.log`
    synchrone + `tracing::error!`.
  - **Niveau de log à chaud** : `settings.set` accepte `logLevel`
    (niveau simple ou spec `EnvFilter` complète), rechargé sans
    redémarrage et jamais persisté ; exposé via `settings.get` /
    `system.status` et via un sélecteur Paramètres → Avancé.
  - **Spans RPC** : `#[instrument]` sur le dispatch (`rpc.method`) et
    les handlers downloads (`download_id`, hash tronqué).
  - **mule-mcp** : layer fichier `logs/mcp.<date>.log` (même rotation),
    stdout réservé au JSON-RPC.
  - **Hygiène PII** : ~200 sites de logs convertis — adresses de
    pairs/serveurs pseudonymisées de façon déterministe par session
    (`peer-NN:port`, `server-NN:port`, `peer-NN` pour IPs nues) via
    `SessionPseudonymizer` (`mule-filter`) ; loopback lisible en clair.
    Helpers `log_peer_addr` / `log_server_addr` / `log_addr_str` /
    `log_peer_ip` + nouveau `pseudonymize_ip`. Script d'audit
    réutilisable `scripts/dev_audit_addr_logs.py`.
  - **Flutter** : `AppLogger` (`ui_kit`) — niveaux, ring buffer 500
    entrées, `debugPrint` dev-only, hooks `FlutterError.onError` /
    `PlatformDispatcher.onError` ; branché dans les `catch` des
    contrôleurs ; export diagnostics (compteurs en sanitized, 50
    dernières entrées en full).

- **Verbosity production réduite** (`mule-daemon`) : ~61 sites
  `info` → `debug` sur les chemins par paquet/par bloc — réceptions et
  envois Kad (`kad_udp`, `routing_client`, `search_client`), blocs
  upload/download, log par requête RPC (polling UI), tentatives de
  connexion du scheduler par tick et résumés `[SOURCE_SCHED]`. Le warn
  de fin de session pair est trié par gravité : `HashMismatch` /
  `InvalidRange` / `CorruptedData` restent `warn`, le churn normal
  (timeouts, refusés, `NoNeededParts`, `IpFilter`…) passe en `debug`.
  Restent en `info` : cycle de vie (connexions, handshakes, slots
  upload), jalons de download, buddy/callback Kad, failover serveur,
  drops IP-filter, heartbeat consolidé (1×/60 s).

- **Partages — scan récursif du dossier Incoming** (`mule-daemon`) :
  les fichiers dans les sous-dossiers du répertoire de téléchargement
  sont désormais partagés (sémantique eMule) ; symlinks et dossiers
  cachés ignorés. Régression : `test_shares_rescan_is_recursive`.

- **IP filter — corrections RPC** (`mule-daemon`) :
  `filter.importFile` renvoie désormais la même shape `FilterStatus`
  que `getStatus`/`setEnabled`/`clear` (le client Dart parsait le
  wrapper `{success, report, stats}` en status plat → l'import
  semblait sans effet) ; restauration au boot de `ipfilter_enabled=0`
  persisté (le défaut `true` réactivait le filtre au redémarrage).
  Régression couverte par `filter_rpc_tests.rs`.

- **Branding** : `EMule-Rust` → `eMuleRust` partout (~122 fichiers :
  UI, métadonnées Windows, docs, scripts). Identifiants techniques
  conservés (`eMuleRust.exe`, `emulerust`, `PR_EMULE`…) ;
  compatibilité UPnP avec l'ancien préfixe de description conservée
  (`LEGACY_DESCRIPTION_PREFIX`).

- **UI téléchargements — top bar harmonisée** (`app_shell`) : le titre
  de filtre « Tous les téléchargements » est retiré (redondant avec la
  sidebar et le hint de recherche) ; le champ de recherche n'a plus de
  taille fixe — il occupe 2/10 de l'espace libre via `Expanded(flex: 2)`
  pendant que le cluster d'actions garde 8/10 et ne scrolle qu'en
  fenêtre très étroite ; hauteurs harmonisées à 36 px (recherche,
  moniteur de transfert, bouton Ajouter). Ripple translucide des lignes
  de download supprimé (splash/highlight `InkWell` désactivés,
  sélection conservée).

- **Palette standard de la barre de parts** (`ui_kit`) : les teintes
  pastel Tailwind sont remplacées par des couleurs plates standard —
  bleu `#2196F3` reçu, vert `#4CAF50` vérifié/progression, rouge
  `#F44336` sans source, jaune `#FFEB3B` bloc en vol, et la formule de
  bleu authentique eMule `RGB(0, 210-22·(n-1), 255)` pour
  rare→commun. Partagé avec la carte part-map.

- **Build — déploiement automatique** (`scripts`) : l'étape Deploy de
  `build_windows_release.ps1` résout sa cible via `-DeployDir` >
  `$env:EMULERUST_DEPLOY_DIR` > fichier `.deploy-dir` à la racine
  (gitignoré, chemin machine) ; les processus lancés depuis la cible
  sont arrêtés avant la copie (exe verrouillés) ; le merge-copy
  préserve `data\`, `downloads\` et `ipfilter*`.

- **Partages — purge des fichiers supprimés du disque** (`mule-daemon`) :
  un fichier effacé physiquement dans le dossier de téléchargement
  restait listé dans Partages (la ligne `shared_files` survivait — le
  scan Incoming n'ajoutait que des entrées). Désormais
  `list_all_shared_files` filtre à la lecture les partages dont le
  fichier n'existe plus (ni UI, ni offre serveur, ni publication Kad
  n'annoncent un fichier mort), le download complété encore en mémoire
  applique la même garde `is_file()`, et `scan_download_dir_shares`
  supprime définitivement les entrées obsolètes sous le dossier Incoming
  — uniquement si celui-ci est joignable (un lecteur débranché ne vide
  pas la liste) et sans toucher les partages hors dossier. `shares.rescan`
  renvoie `{added, removed, dir}`. `get_shared_file` applique la même
  garde (l'échange de sources ne se déclare plus source d'un fichier
  absent). Régression :
  `test_shares_rescan_prunes_files_deleted_from_disk`.

- **Downloads — suppression du `.part` physique au boot** (`mule-daemon`) :
  un `.part` effacé pendant que le daemon était arrêté laissait les
  plages persistées en DB prétendre des données perdues (fichier recréé
  vide, zéros servis jusqu'à l'éviction ICH). Au démarrage, tout
  téléchargement non complété dont le `.part` a disparu — dossier parent
  joignable, donc suppression réelle et non lecteur hors-ligne — voit
  son entrée retirée (`delete_download` + `.part.met` orphelin nettoyé,
  event historique « removed »). Régression :
  `missing_part_file_removes_download_at_startup`.

- **Upload queue — alignement eMule 0.72a** (`mule-daemon`) : audit
  complet contre `UploadQueue.cpp`/`UploadClient.cpp` et corrections des
  divergences de comportement :
  - **Nombre de slots dynamique** (`AcceptNewClient`) : le plafond n'est
    plus un 100 fixe — au-delà des 4 slots garantis
    (`max(MIN_UP_CLIENTS_ALLOWED,4)`), un nouveau slot n'est ouvert que
    si le débit mesuré nourrit encore chaque client au plancher
    (`datarate / minTarget`, cible par client ≤3 slots → 3 KiB/s sinon
    `slots × 1 KiB/s` plafonné à 25 KiB/s) **et** si la capacité upload
    configurée le permet (`MaxSpeed / target` — sonde lue en direct sur
    `upload_bucket.rate_per_sec()`, donc la limite `limits.set` comme la
    valeur pilotée par USS s'appliquent). Avant : jusqu'à 100 slots
    simultanés → débit par client dilué et file jamais remplie.
  - **`Process()` périodique** (`process_tick`, 1 s, indépendant des
    abonnés UI) : purge des entrées sans re-ask depuis >1 h
    (`MAX_PURGEQUEUETIME`), promotion des meilleurs clients dès qu'un
    slot peut s'ouvrir, et **rotation des sessions** — un client ayant
    reçu plus de `SESSIONMAXTRANS` (~9,7 Mo) est renvoyé en file quand
    aucun slot ne peut s'ouvrir (`CheckForTimeOver` + `ForceNewClient`) ;
    son wait start d'origine est restauré à son re-ask (`rotation_grace`,
    équivalent du `credits` eMule — le score accumulé survit à la
    rotation).
  - **Promotion push** : le client promu reçoit `OP_ACCEPTUPLOADREQ`
    immédiatement (`AddUpNextClient` eMule écrit sur le socket en
    attente) via un `Notify` porté par l'entrée de file — avant, la
    promotion restait latente jusqu'au prochain re-ask du client (~30 min).
  - **Une entrée par client** : dedup sur `client_key` seul (eMule : un
    objet client = une entrée) ; le re-ask met à jour fichier/prio/crédits
    et rafraîchit `last_request`. Un client actif qui demande un autre
    fichier garde son slot (`IsDownloading` → `ACCEPTUPLOADREQ` +
    `SetUploadFileID`) au lieu d'être remis en file.
  - **Soft/hard limits** : `hard = soft + max(soft,800)/4` ; entre les
    deux, seuls les clients à slot ami ou dont
    `combined(prio × crédits)` dépasse la moyenne de la file sont admis
    (remplace la règle anti-leecher 90 %, non-eMule).
  - **Ordre de file recalculé à la demande** : tri par `effective_score`
    vivant avant promotion, rang rapporté et tick (eMule évalue
    `GetScore` en continu, pas à l'insertion).
  - **Anti-abus** : `MIN_REQUESTTIME` (10 min) / `BADCLIENTBAN` (4) —
    les re-asks agressifs du même fichier bannissent le client (score 0).
  - **`PR_POWER`** (powershare) → `GetFilePrioAsNumber` = 7 (branche
    `default` eMule) au lieu de 18.
  Régressions : 8 nouveaux tests `upload_queue` (plafond `MaxSpeed`,
  switch fichier, entrée unique, rotation + wait préservé, notify,
  purge 1 h, soft/hard, ban anti-abus).

- **Download queue & pipeline — alignement eMule 0.72a** (`mule-daemon`,
  `mule-protocol`) : audit contre `DownloadClient.cpp`/`DownloadQueue.cpp`
  et corrections des divergences de comportement :
  - **`SendBlockRequests` multi-blocs** (`plan_next_blocks_for_peer64` +
    `fill_and_send_block_requests`) : le daemon ne demandait qu'un bloc
    à la fois par source — débit plafonné à ~180 Ko/RTT. Désormais
    `blockCount` adaptatif au débit mesuré (clients eMule <4 KiB/s → 1,
    <9 KiB/s → 2 ; >150 KiB/s → 9, >75 KiB/s → 6, sinon 3) et jusqu'à
    **3 ranges par paquet** `OP_REQUESTPARTS`/`_I64` (format filaire
    eMule, slots inutilisés à zéro). Les blocs end-game restent un
    fallback unitaire (pas de duplication batchée).
  - **Re-ask de file 29 min** (`FILE_REASK_INTERVAL`, eMule
    `FILEREASKTIME`) : l'attente en upload queue réémet
    `OP_SETREQFILEID_EMULE` + `OP_STARTUPLOADREQ` toutes les 29 min —
    l'uploader distant purge les entrées dont `LastUpRequest` dépasse
    1 h (`MAX_PURGEQUEUETIME`) ; l'ancien keep-alive `SETREQFILEID` à
    60 s ne rafraîchissait **pas** ce compteur → notre slot était purgé
    au bout d'une heure chez les clients eMule officiels.
  - **Pas de timeout de file fixe** : remplacé par « silence pendant un
    cycle re-ask complet + `queue_timeout` » — tout paquet reçu (rang,
    promotion, réponse au re-ask) prolonge l'attente, comme eMule qui
    n'a aucun timeout de file.
  - **`DOWNLOADTIMEOUT` 100 s + re-queue même socket** : les timeouts
    `first_data`/`inactivity` passent de 25 s à 100 s ; un stall
    déclenche `OP_CANCELTRANSFER` puis **ré-entrée en file sur la même
    socket** (`wait_in_queue` partagé, bitmap de parts rafraîchi,
    séquenceur reconstruit) au lieu de tuer la connexion — 3 stalls
    consécutifs max avant abandon.
  - **In-flight sans réallocation prématurée** : TTL des blocs en vol
    porté à 100 s et **rafraîchi à chaque bloc reçu** — un bloc n'est
    plus redemandé à un autre pair pendant qu'une source lente le sert
    encore ; libération à la complétion, à la fin de session ou au
    re-queue.
  - **`ProcessBlockPacket` fidèle** : `size != (end−start)+header` ou
    `end ≤ start` → violation de protocole (déconnexion) ; bloc hors
    pending → ignoré ; `end` débordant la range → `RemoveBlockFromList`
    (bloc libéré) sans déconnecter.
  - **`Pending_Block_Struct`** : `queued`/`recv_to` par bloc ; streams
    zlib par bloc en vol (map par offset de début) au lieu d'un
    decompressor global.
  Régressions : `test_emule_pending_block_count_tiers`, 4 tests
  `plan_next_blocks_*` (disjoints, skip completed/in-flight, shadow,
  end-game unique), tests adversariaux et dual-peer adaptés.

- **UI web — état eD2K réel affiché à la connexion** (`app_shell`) :
  `AppController.refresh()` appelle désormais
  `serversController.refreshSession()` (RPC `servers.status`), donc
  l'UI reflète la session serveur courante même quand l'événement
  `server.sessionUpdated` a été broadcast avant l'attachement du
  client. Cas systématique en web : le daemon se connecte au serveur
  eD2K pendant le chargement de la page → l'UI restait affichée
  « Déconnecté » jusqu'à un clic manuel, alors que le daemon était
  déjà connecté. Régression couverte par `servers_connection_test.dart`.

## [0.2.0] - 2026-09-23

Version finale de la ligne 0.2.x — inclut l'intégralité de `0.2.0-RC1`
plus les ajouts suivants.

- **Publication open source** (dépôt) : ajout de `LICENSE`
  (GPL-2.0-or-later), `CONTRIBUTING.md`, `SECURITY.md` ; `.gitignore`
  généralisé à toutes les apps Flutter (`**/build/`, `**/.dart_tool/`,
  artefacts Android, keystores) ; dépersonnalisation des configs
  `.agents/` (exemples `*.example.json`) ; `DeployDir` du script de
  build rendu opt-in (plus de chemin personnel par défaut).

- **Interface web servie par le daemon — connexion zéro-config**
  (`mule-daemon`, `apps/web`, `app_shell`, `scripts`) :
  - **ServeDir embarqué** (`tower-http`) : le daemon sert le build
    Flutter web sur son port HTTP. Résolution du dossier :
    `--web-dir` / `MULE_WEB_DIR` > auto-détection `<exe>/../web`
    (bundle `bin/` → `web/`), `<exe>/web`, `../../apps/web/build/web`
    (dev cargo). `index.html` en fallback SPA (deep links
    `usePathUrlStrategy`).
  - **Auth transparente** : `GET /session-config.js` émet
    `window.MULE_SESSION` (endpoint + token) — réponse limitée aux
    binds **loopback** pour ne jamais exposer `auth_token` sur une
    interface réseau.
  - **App web** : lit `MULE_SESSION` au démarrage (connexion sans
    saisie quand servie par le daemon), nouveau param `?token=`,
    champ token optionnel dans le formulaire de connexion,
    `autoReconnect` activé sur le client RPC.
  - **Desktop** : bouton « Interface web » dans la sidebar → ouvre le
    navigateur sur l'URL HTTP dérivée de l'endpoint de session
    (`SystemActions.openUrl`, pattern conditionnel io/stub).
  - **Bundle** : `build_windows_release.ps1` compile `apps/web`
    (`-SkipWeb`, `-WebBaseHref`), copie les assets sous `web/`,
    génère `Interface Web.bat` à la racine (démarre le daemon si
    absent puis ouvre `http://127.0.0.1:4660/`), et ajoute
    `includesWebUi` à `version.json`.

- **Palette assouplie de la barre de progression des parts**
  (`ui_kit`, `app_shell`) : la sémantique `DrawStatusBar` eMule est
  conservée mais les primaires saturés sont remplacés par des teintes
  Tailwind déjà utilisées par `AppTheme` — Slate 400 = reçu,
  Teal 400 = vérifié, Red 400 = sans source, Amber 400 = bloc en vol,
  Green 400 = strip de progression, dégradé Sky 300→800 pour
  rare→commun. Les constantes `partHaveColor`/`partVerifiedColor`/
  `partMissingColor`/`partPendingColor`/`partProgressColor` sont
  publiques et partagées avec la carte part-map.

## [0.2.0-RC1] - 2026-09-23

- **Onglet Partages — catégories, recherche et stats d'upload**
  (`mule-core`, `mule-daemon`, `mule_client`, `app_shell`) :
  - **Catégorie par fichier partagé** : nouvelle colonne
    `shared_files.category_id` (migration `ALTER TABLE` idempotente) +
    RPC `shares.setCategory`. Un partage synthétisé depuis un download
    suit la catégorie de celui-ci (`shared_files` ou `downloads`
    selon la source) — menu « étiquette » sur chaque ligne pour
    assigner/retirer.
  - **Vue remaniée** : fichiers groupés par catégorie (en-têtes
    pastille + nom, ordre `sortOrder`), groupe « Sans catégorie » en
    dernier ; à l'intérieur de chaque groupe, tri par **total uploadé
    décroissant** (le plus servi en haut).
  - **Moteur de recherche** : champ dans la barre de titre, filtre sur
    nom de fichier et hash eD2K.
  - **Stats d'upload par fichier** : débit mesuré courant (↑ B/s) et
    total cumulé (Σ) affichés sur chaque ligne, rafraîchis via
    `uploadsEvents` debouncé.
  - `SharedFileModel` étendu (`categoryId`, `upPriority`,
    `uploadedBytes`, `uploadRateBps`) + client RPC/fake ; les
    mutations fake préservent tous les champs.

- **UI téléchargements — chip de priorité modernisée**
  (`app_shell`) : pill arrondie teintée + icônes sémantiques
  (double-flèche haut/bas, tiret) et **3 couleurs distinctes** —
  ambre = High, bleu ciel = Normal, ardoise = Low. Le menu popup
  reprend les mêmes icônes colorées.

- **UI téléchargements — barre de progression dédupliquée**
  (`app_shell`) : le `LinearProgressIndicator` au-dessus de
  `PartAvailabilityBar` était redondant — la barre de chunks dessine
  déjà le liseré vert d'avancement eMule (`DrawStatusBar`). Supprimée
  dans les layouts full et compact ; la barre de chunks passe à 14 px
  (12 px compact) pour occuper toute la hauteur.

- **Amorçage Kad résilient — file de candidats façon eMule
  `s_liBootstrapList`** (`mule-daemon`) : un timeout sur le premier
  bootstrap laissait Kad mort avec une table de routage vide — la
  maintenance ne s'activait que si `total_contacts > 0`, donc aucune
  retentative n'avait jamais lieu.
  - **File de candidats** (`bootstrap_candidates`, cap 50, dédupliquée) :
    chaque pair eD2K annonçant un port UDP Kad s'y enregistre au
    handshake (entrant comme sortant), filtré `kad_version >= 2`
    (`KADEMLIA_VERSION2_47a`) — les pairs pré-0.47a ne parlent pas
    KADEMLIA2.
  - **Boucle de drain** : tâche de fond qui vide la file à raison d'un
    `BOOTSTRAP_REQ` toutes les **2 s** tant que la table est vide,
    **15 s** quand elle a des contacts non vérifiés — calqué sur
    `CKademlia::Process` (Kademlia.cpp:287). Chaque candidat n'est tenté
    qu'une fois (`RemoveHead`) ; les handshakes ré-alimentent la file.
  - **Gate `!IsConnected()` fidèle** : le drain tourne tant qu'aucun
    contact n'a jamais répondu (`HasHadContact` eMule = contact
    *vérifié*), même si la table contient des zombies non vérifiés —
    avant, 50 contacts morts auraient figé le drain.
  - **Throttle partagé** `last_bootstrap` (= `m_tBootstrap` eMule) :
    envois immédiats au handshake (10 s) et drain partagent le même
    horodatage — pas de rafale de requêtes.
  - **`nodes.dat` auto-chargé** au démarrage (`<data_dir>/nodes.dat`) en
    candidats bootstrap : les contacts prouvent leur vivacité en
    répondant au drain au lieu d'entrer zombies dans la table.
  - **Événement `kad.statusChanged`** émis dès qu'un bootstrap apprend
    des contacts — le badge UI passe « Connecté » sans attendre le tick
    de 30 s.
  - Test : `test_bootstrap_drain_retries_candidates_until_success`
    (candidat mort → timeout → candidat vivant → table peuplée).

- **Bridages internes alignés sur eMule 0.72a** (`mule-daemon`) :
  chasse aux plafonds en dur hérités des premières versions de test.
  - **Slots upload** : `MAX_ACTIVE_UPLOADS` passe de **10 → 100** =
    `MAX_UP_CLIENTS_ALLOWED` d'eMule (`Opcodes.h`) — le cap de 10
    brisait le scaling sur connexions rapides.
  - **Slots garantis** : les 4 premiers slots s'ouvrent sans condition
    de débit (`max(MIN_UP_CLIENTS_ALLOWED, 4)` d'eMule
    `AcceptNewClient`) — auparavant le plancher `minUploadKbs` pouvait
    geler l'upload dès le 2ᵉ client.
  - **Promotion de file** : `promote_waiting` applique désormais la même
    éligibilité qu'`AcceptNewClient` (plancher de débit + veto USS) via
    `can_open_slot` partagé — avant, la promotion ignorait les deux
    contrôles.
  - **Pacing connexions sources** : `MAX_CONNECT_PER_TICK` 5 → 12 —
    suppression du double bridage (coordinateur 5/3s ≈ 100/min *plus
    restrictif* que le budget `rate_limit_per_min` 240/min). Un fichier
    seul atteint maintenant le rythme eMule `MaxConnPer5Sec`=20 (~4/s) ;
    le budget token-bucket reste le garde-fou global.
  - **Cap inbound** : `MAX_INBOUND_CONNECTIONS` (500, constante morte)
    supprimé — le listener suit le budget `max_active_peer_connections`
    live, comme eMule où toutes les connexions pairs comptent dans
    `maxconnections`. Le cap par IP (5, anti-flood) est conservé.
  - **Soft-cap SX** : `max_source_soft` et le cap d'ingestion des
    réponses SX retombent sur le réglage `maxPeersPerDownload` persisté
    au lieu de la constante 400 (eMule : `GetMaxSourcePerFile` →
    `thePrefs.maxsourceperfile`).
  - **Refactor** : la vérification des quotas de session (global +
    par fichier + doublon d'adresse), dupliquée entre `connect` et
    `accept_callback`, est factorisée en
    `SwarmInnerState::check_session_slot`.

- **Audit complet des réglages — câblage, doublons, bornes eMule 0.72a**
  (`mule-daemon`, `mule-network-policy`, `app_shell`, `SETTINGS.md`) :
  inventaire des 36 clés `settings.set`/`settings.get` et de tous les
  champs UI — chaque champ est câblé (persisté + appliqué à chaud ou
  relu au tick) et présent dans une seule section.
  - `maxConnections` : le plafond passe de **1500 → 65000**
    (`MAX_PEER_CONNECTIONS_CAP`) — eMule ne borne pas `maxconnections`
    sous Windows NT+ (`GetMaxWindowsTCPConnections()` → `UNLIMITED`) ;
    65000 reste un garde-fou anti-faute de frappe. La valeur est
    désormais **clampée avant persistance** (auparavant persistée brute
    puis rejetée par `set_budgets` → incohérence stockée/appliqué).
  - `maxUploadQueue` : clamp `[10, 5000]` (eMule `QueueSize`) appliqué
    avant persistance — stockée = appliquée.
  - `settings.get` renvoie les valeurs **effectives** (clampées) pour
    `maxConnections`, `maxConnPerMin`, `maxUploadQueue` — fini les
    valeurs persistées hors borne ré-affichées.
  - UI : bornes de saisie ajoutées (`maxQueue` ≤ 5000, quota ≤ 1 048 576
    Gio, `maxConnections` ≤ 65 000) ; hints clarifiés —
    `maxGlobalDownloadPeers` (quota d'allocation swarm) explicitement
    distingué de `maxConnections` (budget de sockets) ;
    `minUploadKbs = 0` documenté comme seuil désactivé (extension —
    eMule force `MinUpload ≥ 1`).
  - Écarts assumés et documentés dans `SETTINGS.md` : `kadPort = 0`
    n'est pas accepté — la désactivation UDP passe par `udpEnabled`
    (équivalent du « port 0 » d'eMule, en interrupteur dédié).

- **Liste des sources alignée sur eMule `srclist`/deadlist** (`mule-core`,
  `mule-daemon`, `app_shell`, `mule_client`) : le panneau déroulé d'un
  téléchargement affichait *toutes* les lignes persistées de
  `peer_sources`, y compris les échecs en cooldown, les sources rejetées
  (FNF) et les filtrées IP — eMule les retire de `srclist` et les place
  en `DeadSourceList` invisible (`CUpDownClient::Disconnected` →
  `RemoveSource` + `AddDeadSource`). Nouveau prédicat partagé
  `PeerSourceStatus::is_srclist_visible()` /
  `PeerSourceStatus.isListedInSourceList` : seuls `candidate`, `allowed`,
  `tried`, `connected` restent affichés ; `failed`, `rejected`,
  `filtered` restent persistés pour le backoff mais sont masqués. Les
  pairs déjà rendus dans la section « sessions actives » ne sont plus
  répétés dans la liste (`PeerSourcesController::getVisibleSourcesForHash`,
  dédupliqué par adresse). Le compteur `sourceCount` des snapshots ne
  compte désormais que les sources visibles (comme `GetSourceCount()`
  d'eMule) et est rafraîchi à chaque transition de statut ou suppression.

- **USS aligné sur eMule `LastCommonRouteFinder`** (`mule-daemon`) : la
  cible de ping USS n'est plus le serveur eD2K distant mais le **dernier
  routeur commun** — routes ICMP tracées (`IcmpSendEcho`, TTL croissant,
  non privilégié) vers un échantillon de pairs connus (serveur, clients
  d'upload, sources de download — IPs publiques, ≤ 8), puis ping du saut
  le plus profond partagé par toutes les routes (typiquement le routeur
  d'agrégation du FAI). Mesure la congestion du lien montant local comme
  eMule, au lieu d'un RTT de bout en bout. Cible cachée 15 min, re-résolue
  après re-baseline ; repli `uss.pingHost` puis sonde TCP-connect vers le
  serveur (plateformes sans `IcmpSendEcho`). `uss.pingTarget` expose
  désormais le schéma (`icmp:`/`tcp:`). Nouveau module `uss_icmp`,
  dépendance `windows` (target-specific) ajoutée.

- **Champ « limite d'envoi » qui revenait à sa valeur USS** (`mule-daemon`,
  `app_shell`, `mule_client`) : quand USS est activé, sa boucle de contrôle
  réécrit le bucket upload à chaque tick — un `limits.set` manuel était
  écrasé en ~1 s et le champ affichait la valeur effective (ex. 51 Ko/s)
  au lieu de la configurée. Deux correctifs : les champs de
  `RateLimitsTab` affichent désormais la limite *configurée* persistée
  (`uploadConfiguredBytesPerSec`/`downloadConfiguredBytesPerSec`) avec un
  helper indiquant la valeur effective (« Piloté par USS : X Ko/s » ou
  « plafonné par le ratio eD2K ») ; et côté daemon, `limits.set` sur
  l'upload met à jour `uss.max_upload_bps` quand USS est actif — la
  limite saisie devient le plafond du pilotage dynamique (sémantique
  eMule : `MaxUpload` borne DynUp, 0 = plafond illimité).

- **Guide lab masqué en release** (`app_shell`) : la tuile « Guide du
  mode laboratoire » de Settings → Avancé n'apparaît désormais que si
  `isLabModeAvailableProvider` est vrai (daemon portable avec capacité
  `lab-tools`, ou dev tools) — le flavor `standard` n'exposant aucune
  méthode `lab.*`, l'entrée était inutile en production.

- **Moniteur de transfert unifié dans la top bar** (`app_shell`,
  `ui_kit`) : les trois chips disjoints (débits live + sparkline, totaux
  de session, cumuls lifetime) sont fusionnés en une seule pill
  segmentée par des séparateurs hairline — `↓ DL/s ↑ UL/s │ sparkline │
  session │ Σ lifetime`. Chiffres en fonte tabulaire
  (`FontFeature.tabularFigures`) : la largeur ne bouge plus quand la
  valeur change. Kill switch actif → pill teintée rouge avec transition
  animée (`AnimatedContainer`) et segment session remplacé par l'état
  « Désactivé ». Sparkline enrichi : ligne de base discrète quand le
  graphe est vide + remplissage download en dégradé vertical au lieu
  d'un alpha plat.

- **Localisation FR complète de l'interface** (`app_shell`,
  `mule_client`) : `app_fr.arb` contenait ~340 valeurs encore en anglais
  (sidebar entière, badges, dialogues) — toutes traduites ; les valeurs
  françaises égarées dans `app_en.arb` sont remplacées par l'anglais
  réel. ~150 littéraux en dur (FR et EN mélangés) migrés vers
  `context.l10n`/`L10nLookup` — vues, dialogues, **et messages de
  statut/erreur des 9 contrôleurs** (résolus via `L10nLookup.current`
  synchronisé avec `LocaleController`, sans `BuildContext`).
  `l10n/enum_labels.dart` localise les enums de `mule_client`
  (`DownloadStatus`, `SupervisorState`, `NetworkProfile`, priorités…).
  Libellés techniques volontairement conservés : exports de diagnostic,
  identifiants protocolaires, chemins. Correctifs de layout au passage :
  les libellés FR plus longs faisaient déborder plusieurs `Row`
  (`history_filter_bar`, `add_link_dialog`, `import_part_met_dialog`,
  `diagnostic_dialog`, `import_inspection_card`) — rendus flexibles
  (`Flexible`/`Expanded`/`FittedBox` + ellipsis).

- **Nouvelle icône d'application** (`Asset/generated-image.png`,
  1254×1254 RGBA) déployée sur toutes les cibles : `app_icon.ico`
  multi-résolutions 16→256 px (exe Windows via `Runner.rc` + asset tray),
  icônes web 192/512 + maskables + favicon, `AppIcon.appiconset` macOS
  et iOS régénérés.

- **Ratio upload/download eMule** (`mule-network-policy`, `mule-daemon`,
  `mule_client`) : `emule_ud_ratio()` reproduit
  `Preferences::GetMaxDownloadInBytesPerSec` (« don't be a Lam3r :) ») —
  limite upload < 20 Ko/s → download effectif plafonné à 3× (<4 Ko/s),
  4× (<10 Ko/s) ou 5× (<20 Ko/s) l'upload ; download « illimité » plafonné
  aussi. La limite *configurée* persistée n'est jamais réécrite : le ratio
  est un plafond recalculé à chaque application (`limits.set`,
  `apply_capacity_clamp`, boot) et suit la limite dynamique USS à chaque
  tick (eMule `bDynamic` → débit mesuré). `limits.get` expose
  `downloadConfiguredBytesPerSec`/`uploadConfiguredBytesPerSec` et
  `downloadRatioApplied`.

- **Settings → Connexion : interrupteurs Réseau eD2K / Réseau Kad**
  (`app_shell`) : le sélecteur de profil à 3 options et la carte
  « Profil Réseau Actuel » sont remplacés par une carte « Réseaux » à
  deux interrupteurs — `Réseau eD2K` (off → confinement loopback
  `local-lab`) et `Réseau Kad` (on → `public-ed2k-kad-preview`, activer
  Kad active aussi eD2K). Le contrat de sécurité est inchangé : les
  switches projettent simplement le même `NetworkProfile` sous-jacent.

- **Carte daemon : statut réseau eD2K/Kad + serveur** (`app_shell`) : la
  carte « Daemon » en bas de sidebar affiche désormais deux pastilles
  colorées — `eD2K` (HighID vert, LowID/connexion orange, déconnecté
  rouge, depuis `server.session`) et `Kad` (connecté vert quand le socket
  UDP a des contacts, « Amorçage » orange, « Arrêté » rouge) — plus le
  nom du serveur eD2K courant (adresse en repli). Le tooltip du mode
  compact résume les trois informations.

- **Pseudo par défaut « eMuleRust »** (`mule-daemon`, `mule_client`,
  `app_shell`) : `current_nickname()`, `settings.get`, le fallback du
  champ vidé dans Settings et le HELLO firewall-test annoncent
  « eMuleRust ». Le `TAG_SPECIAL_NAME` du HELLOANSWER inbound reflète
  désormais le pseudo réellement configuré au lieu d'une constante.

- **Stats d'upload par fichier partagé (onglet Partages)** (`mule-core`,
  `mule-daemon`, `mule_client`, `app_shell`) : chaque ligne affiche le
  débit d'upload courant du fichier (`↑ …`, teal quand actif) et le total
  cumulé envoyé (`Σ …`) — colonnes « Upload speed » / « Transferred »
  d'eMule. Le débit est la somme des débits mesurés des slots actifs sur
  ce hash (`UploadManager::per_file_rates_bps`) ; le total est incrémenté
  à chaque bloc envoyé, hydraté au boot depuis la nouvelle table SQLite
  `share_upload_stats` et persisté par deltas à chaque `statsTick`
  (fonctionne aussi pour les partages partiels, qui n'ont pas de ligne
  `shared_files`). `shares.list` enrichit chaque entrée de
  `uploadRateBps`/`uploadedBytes` ; la vue se rafraîchit sur
  `upload.uploadsUpdated` (debounce 1 s) — le débit retombe à 0 quand le
  dernier slot se ferme.

- **Renommage de fichier (Téléchargements et Partages)** (`mule-core`,
  `mule-daemon`, `mule-mcp`, `mule_client`, `app_shell`) : icône crayon
  dans la barre d'actions de chaque téléchargement et sur chaque partage
  complet — dialogue pré-rempli avec validation (caractères interdits
  Windows, `.`/`..`). `downloads.rename` met à jour `display_name` +
  `target_path` (le `.part` UUID ne bouge pas ; le nouveau nom s'applique
  à la finalisation) ; pour un download terminé le fichier disque et
  l'entrée `shared_files` sont renommés. `shares.rename` fait de même
  pour les partages hors downloads et délègue à `downloads.rename` quand
  le hash adosse un download listé. Outils MCP `emule_rename_download` /
  `emule_rename_share` ajoutés. Les lignes gardent un alignement vertical
  constant (emplacement ETA réservé, slot pause/reprise toujours présent).

- **Barre du haut : totaux cumulés + graphe upload + Diagnostics retiré**
  (`mule-daemon`, `mule_client`, `app_shell`) :
  - le chip « Diagnostics » est retiré de la top bar (le dialogue reste
    accessible via la sidebar et Settings → Avancé) ;
  - le sparkline temps réel dessine désormais deux courbes sur la même
    échelle — download rempli (couleur statut) + upload en ligne teal ;
  - à droite des totaux de session, un second chip « Σ ↓ / ↑ » affiche
    les cumuls depuis l'installation, persistés en SQLite
    (`lifetime_downloaded_bytes` / `lifetime_uploaded_bytes`,
    accumulation par deltas de session à chaque `statsTick`) et
    transportés par `lifetimeDownloadedBytes` /
    `lifetimeUploadedBytes` dans `event.statsTick` ;
  - nouvelle RPC `stats.resetLifetimeTotals` + tuile « Remise à zéro
    des totaux cumulés » dans Settings → Avancé (avec confirmation) —
    les totaux de session sont conservés.

- **Suppression d'un fichier partagé depuis l'onglet Partages**
  (`mule-daemon`, `mule-mcp`, `mule_client`, `app_shell`) : chaque ligne
  de partage complet gagne une icône poubelle avec dialogue de
  confirmation. `shares.remove` accepte le nouveau paramètre
  `deleteFile` : le fichier est effacé du disque et l'entrée
  `shared_files` retirée ; quand le partage adosse un téléchargement
  complété, la suppression passe par `remove_download` (entrée de
  téléchargement retirée aussi — sinon elle ressuscitait le partage au
  redémarrage). Les partages partiels (downloads en cours) refusent la
  suppression et affichent une icône désactivée renvoyant vers l'onglet
  Téléchargements. Sans `deleteFile`, le comportement historique
  (unshare seul) est inchangé.

- **Totaux de session dans la barre du haut** (`mule-daemon`,
  `mule_client`, `app_shell`) : le chip « profil réseau »
  (`public-ed2k-kad-preview`…) — redondant avec Settings → Connexion —
  est remplacé par les totaux de session « ↓ téléchargé / ↑ envoyé ».
  `event.statsTick` transporte deux nouveaux champs
  (`sessionDownloadedBytes` = somme des deltas `covered -
  session_bytes_start` des downloads actifs ; `sessionUploadedBytes` =
  compteur `total_uploaded_bytes` de l'`UploadManager`), relayés par
  `DaemonStatsModel`. Le tap ouvre Historique & Statistiques ; quand le
  kill switch est engagé, le chip conserve son état rouge « Disabled »
  et renvoie vers Settings → Connexion.

- **Suppression des téléchargements repensée** (`mule-daemon`,
  `mule-mcp`, `app_shell`) : pour un téléchargement en cours, la
  confirmation retire désormais automatiquement les fichiers temporaires
  (`.part` + `.part.met`) sans case à cocher. Pour un téléchargement
  terminé, une case « Supprimer aussi le fichier partagé dans le dossier
  de destination » (décochée par défaut) supprime le fichier final ET
  son entrée `shared_files` (enregistrée à la complétion par
  `finalize_download`) — sans cette option, le fichier reste partagé.
  Le paramètre RPC `downloads.remove delete_files` couvre les deux cas ;
  le comportement `.part.met.keep` des fichiers conservés est inchangé.

- **Tri de la liste des téléchargements** (`app_shell`) : les éléments
  terminés coulent en bas de la liste ; les téléchargements en cours sont
  triés par avancement décroissant (les plus complets en haut), puis par
  date d'ajout récente à égalité. Nouveau comparateur partagé
  `compareDownloadsForList` appliqué à `filteredDownloadsProvider` et au
  getter legacy `AppController.filteredDownloads` (auparavant tri
  `createdAt` ou ordre d'insertion selon le chemin).

- **Débits d'upload temps réel corrigés** (`mule-daemon`) : la colonne
  « Débit » de la vue Uploads affichait une valeur figée — le dispatcher
  remontait `upload_bucket.rate_per_sec().max(length)`, c'est-à-dire la
  *limite configurée* du token bucket (0 = illimité → la taille du bloc,
  ~180–210 Kio) et non le débit réellement desservi. `UploadManager`
  mesure désormais le débit en interne par fenêtres d'échantillonnage
  (`RATE_WINDOW_MS` = 2 s, premier échantillon à 500 ms — même approche
  que `bytes_since_rate_check/elapsed` côté downloads), et
  `effective_rate_bps(now)` fait décroître le débit vers 0 quand un pair
  cesse de demander des blocs (convention `decay_stale_rates`). La boucle
  `statsTick` (2 s) émet en plus `upload.uploadsUpdated` tant que des
  slots sont actifs ou en attente, donc l'UI voit les débits vivre et
  retomber sans attendre un nouvel envoi. Bonus : `statsTick.uploadBps`,
  USS (`GetDatarate`) et le garde-fou `MinUpload` exploitent aussi le
  débit mesuré au lieu de la limite configurée.

- **Partages partiels, compteurs sidebar temps réel, pickers natifs et
  barre de parts eMule 0.72a byte-précise** (`mule-daemon`,
  `mule_client`, `app_shell`, `ui_kit`) :
  - **Téléchargements en cours toujours partagés** :
    `shared_entry_for_active` ne retourne plus `None` quand aucune part
    eD2K de 9,73 Mio n'est complète — tout download actif apparaît dans
    « Fichiers partagés » en `isPartial` dès sa création (eMule
    `CDownloadQueue::AddPartFilesToShare` : *« part files are always
    shared files »*) ; seules les parts complétées restent annoncées et
    servies (`PARTSTATUS`, refus de ranges non complétées, publish
    Kad/serveur inchangés).
  - **Compteurs sidebar temps réel** : nouveau `SharesController`
    (badge « Partages » live) ; `kad.statusChanged` est désormais émis
    par la boucle de maintenance Kad quand `total_contacts` change
    réellement (plus seulement via les handlers RPC — le badge se met
    à jour sans ouvrir la section, ≤30 s) ; nouveau nudge
    `shares.changed` sur `add`/`remove`/`rescan`/`setEnabled`/
    `setUploadPriority` et stream `sharesEvents` côté client.
  - **Notifications transitoires supprimées** : bannière de statut en
    haut (`StatusNotificationBanner` retiré) et tous les `SnackBar` en
    bas ; les erreurs d'ajout de lien deviennent inline dans le
    dialogue (les erreurs structurées `setError` et les bannières
    persistantes sont conservées).
  - **Sélecteurs de fichiers natifs** : helper `core/file_picker_helper`
    (`pickFile`, miroir de `pickFolder`, `file_picker` 12.3) — boutons
    « parcourir » sur dossier de téléchargement, dossier temporaire,
    `ipfilter.dat`, `server.met`, `nodes.dat` et l'import `.part.met`
    (champs `.met`, `.part`, dossier cible).
  - **Barre de parts fidèle à `CPartFile::DrawStatusBar`** :
    `PartAvailabilityBar` réécrite en `CustomPainter` byte-précis —
    ardoise = octets reçus (teal si part vérifiée), rouge = trou sans
    source confirmée, dégradé bleu `RGB(0, 210−22·(n−1), 255)`
    rare→commun, jaune `RGB(255,208,0)` = blocs en vol, strip vert
    3 px = progression ; palette atténuée en pause. Nouveaux champs
    snapshot : `PartProgressSummary.receivedMask` (bitmap 64 cellules
    intra-part, ≈152 Kio/cellule, calculé depuis le `RangeSet` réel) et
    `DownloadSnapshot.pendingRanges` (ranges des `in_flight_blocks`,
    enrichi dans `downloads.list`/`get`, `broadcast_download_update` et
    les sites d'émission `transfer.rs`/`strategy.rs`). Carte « eD2K Part
    Map » du détail alignée (remplissage intra-part, bande jaune pending,
    légende « No source »).
  - **Disponibilité réelle des parts** : le masque `OP_FILESTATUS`
    reçu de chaque pair (initial *et* keep-alive en file d'attente) est
    désormais enregistré dans `act.sources` via
    `record_source_availability` + emit `downloadUpdated` — auparavant
    il n'atteignait que le store SQLite du panneau Sources, laissant la
    barre entièrement rouge. `compute_part_summaries` applique la
    convention eD2K `part_count == 0` = source complète (eMule
    `ProcessFileStatus` → `memset(m_abyPartStatus, 1)`) et ignore les
    masques périmés (> 30 min, `SOURCE_FRESHNESS_MS` partagé avec
    `part_source_counts`). Comme eMule, les sources découvertes mais
    jamais contactées ne comptent pas — le rouge signifie « aucune
    source confirmée ».
  - **Tests WS durcis** : `ed2k_transfer_tests` et
    `daemon_adversarial` lisaient `read.next()` sans filtrer l'`id` —
    toute notification broadcast intercalée (events forwardés à toute
    session authentifiée) faisait paniquer l'`unwrap` ; helper
    `read_response`/`next_response` filtrant par `id` désormais utilisé
    partout dans ces fichiers.

- **Audit câblage réseau eD2K — injections manquantes, kill switch
  réversible et réglages live propagés** (`mule-daemon`, `mule-mcp`) —
  contrôle complet du chemin serveur/pairs (RPC → services → handshake)
  et corrections :
  - **Callback entrant désormais câblé** : `ServerSessionManager` reçoit
    l'`InboundListenerService` au boot — `OP_CALLBACKREQUESTED` (un pair
    HighID nous joint alors que nous sommes LowID) déclenche réellement
    le `connect_back` sortant implémenté dans le listener ; le code
    existait mais le service n'était jamais injecté en production.
  - **Kill switch réversible** : `network.disconnectAll` n'avorte plus
    les superviseurs (watcher de reconnexion, `SwarmCoordinator`,
    rotateur d'identité, quota d'upload) — ils s'auto-inhibent tant que
    l'état est `Disabled` — et les sockets UDP (Kad, serveur), non liés
    au token d'annulation, sont explicitement stoppés. `network.enable`
    crée un token neuf puis relance le listener TCP entrant, les sockets
    UDP, la maintenance Kad, USS et le refresh UPnP ; `last_target`
    étant conservé au kill (`disconnect_for_kill_switch`), le watcher
    restaure la session serveur comme eMule `Reconnect`. USS redevient
    relançable (le drapeau `running` est libéré à la sortie de boucle).
  - **`servers.connect` sémantique eMule** : bascule à chaud vers un
    autre serveur sans `disconnect` préalable ; la reconnexion vers la
    même adresse est idempotente (le snapshot courant est retourné).
  - **Réglages d'obfuscation propagés partout** : la préférence est
    appliquée à la session serveur au boot et sur `settings.set`
    (`set_obfuscation_preference` n'était jamais appelé en production) ;
    le coordinateur utilise `effective_obfuscation()` (valeurs
    persistées) pour `OP_GETSOURCES` et le dialing de pairs au lieu des
    valeurs `config.*` figées au boot ; le nickname réel
    (`current_nickname`) est utilisé dans le failover serveur et les
    hellos pairs (suppression des littéraux `http://emule-project.net`
    et `eMuleRust`).
  - **Dialing de pairs sans serveur (Kad-only)** : le coordinateur
    connecte les sources directes même déconnecté — `client_id=0` dans
    le hello, comme le chemin manuel `sources.connect` ; les sources
    `lowid:` et les callbacks serveur restent filtrés
    (`server_is_high_id=false`), injoignables par construction.
  - **MCP `emule_search`** : `network:"both"` (enum du schéma) active
    réellement Kad + serveur — auparavant seul `"all"` était reconnu et
    `"both"` retournait zéro résultat en silence.
  - **Tests** : `secure_ident_tests` ne fige plus — le partage pointe
    vers un vrai fichier temporaire au lieu du chemin fictif
    `C:\Shared\...` (une plage illisible reste sans réponse par
    conception eMule, le `read()` du test n'ayant pas de timeout) ;
    `server_connection_tests` couvre la nouvelle sémantique idempotente
    de `servers.connect`.

- **Liaison RPC auditée — `upnp.statusChanged` câblé de bout en bout**
  (`mule-daemon`, `mule-mcp`, `mule_client`, `app_shell`) — audit complet
  du contrat JSON-RPC/WebSocket (méthodes, events, handshake, supervision)
  et corrections :
  - **Daemon** : nouvelle variante typée `DaemonEvent::UpnpStatusChanged`
    (méthode `upnp.statusChanged`, topic `network`, payload = snapshot
    `UpnpStatus` complet). Les handlers `upnp.*` passent désormais par
    `DaemonEvent::send` — `services/events.rs` redevient la source unique
    du format wire pour **tous** les events.
  - **Client Dart** : nouveau stream `MuleDaemonClient.upnpStatusEvents`,
    handler `upnp.statusChanged` dans `RpcDaemonClient`, implémentations
    dans `FakeMuleDaemonClient` et les mocks de tests ; `UpnpStatusModel`
    expose `cgnatDetected` (champ du snapshot auparavant ignoré).
  - **UI** : la carte UPnP des Paramètres → Connexion s'abonne au flux et
    se rafraîchit sur tout changement asynchrone (retries de mappage en
    arrière-plan du daemon), plus seulement sur action utilisateur.
  - **mule-mcp** : suppression du listener mort `kad.searchResult`
    (jamais émis — les résultats `kad.searchKeywords` sont synchrones).
  - Contrôle complet : ~95 méthodes dispatchées, toutes celles appelées
    par le client Flutter ont un handler ; `cargo check`/`clippy`,
    `rpc_contract`, `flutter analyze`/`test` verts.

- **Cycle de vie `.part` / `.part.met` durci — parité `CPartFile` eMule**
  (`mule-daemon`, `mule-protocol`) — audit complet de la gestion des
  fichiers partiels et corrections :
  - **Pause effective en transfert actif** : les sessions pairs vérifient
    le statut du download avant chaque bloc planifié (début de session et
    boucle pipeline dans `transfer.rs`) et s'arrêtent proprement ;
    `inject_range` ne ramène vers `Downloading` que depuis
    `Queued`/`WaitingForSources` — un bloc en vol sur un download
    `Paused`/`Failed`/`Removed` s'écrit sans dépauser ni ressusciter
    (sémantique eMule : écritures en vol terminées, requêtes stoppées).
  - **Écriture garantie avant persistance** : si le descripteur `.part`
    est absent, `inject_range` le rouvre à la demande ; en cas d'échec,
    erreur remontée au lieu de marquer la plage reçue sans octets écrits
    (fini les boucles verify-fail fantômes à 100 %).
  - **Intégrité upload** : suppression du fallback synthétique `0x77` —
    une lecture disque défaillante sur un partage laisse la requête sans
    réponse (comportement eMule) au lieu d'empoisonner le demandeur.
  - **Scan d'orphelins au démarrage** : `recover_orphan_part_mets` scanne
    le temp dir pour les `*.part.met` inconnus de la base (DB perdue ou
    fichiers déposés par eMule) et les importe via le chemin sandboxé
    habituel — vérification des hashsets, éviction des parts corrompues,
    statut `Paused`, zéro réseau. Le `.part` est **adopté in-place**
    (`rename` atomique vers `{uuid}.part`, pas de copie), le met source
    consommé ; mets malformés ignorés avec `warn`, jamais de crash.
  - **Persistance continue des mets** : `save_active_part_mets` écrit les
    `.part.met` des downloads non terminaux — cadence ~60 s dans le
    heartbeat du `SwarmCoordinator` (eMule `SavePartFile`) + flush
    explicite dans le graceful shutdown — un crash ne laisse plus un met
    périmé.
  - **Suppression keep-files sans résurrection** : `downloads.remove`
    avec `deleteFiles=false` renomme le compagnon en `*.part.met.keep`
    (invisible au scan d'orphelins, toujours importable via
    `downloads.importPartMet` avec `partPath` explicite) ; code mort
    `cancel_download` supprimé.
  - **Pureté `mule-protocol` restaurée** (ADR 0003) : `ed2k::storage`
    (`Ed2kPartFileSink`, `.resume.json`) gaté derrière `ed2k-loopback`,
    `serde_json` rendu optionnel — la crate reste sans I/O en build
    standard.

- **Source Exchange (SX) aligné sur eMule 0.72a** (`mule-protocol`,
  `mule-daemon`) — audit complet contre `srchybrid`
  (`docs/ed2k/messages/source_exchange.md`) :
  - **Négociation version/opcode fidèle** : la réponse suit
    `byUsedVersion = min(byRequestedVersion, 4)` en SX2 et bascule en
    `OP_ANSWERSOURCES` SX1 (version `GetSourceExchange1Version`) quand le
    demandeur n'envoie pas SX2 — auparavant `OP_ANSWERSOURCES2` v4 était
    systématique et droppé par les clients SX1/SX2<v4
    (`PartFile.cpp:3629-3646`).
  - **Sous-opcodes `OP_MULTIPACKET*`** : les requêtes SX embarquées
    (`0x92`/`0xA4`/`0xA9`, hash porté par l'enveloppe) sont décodées et
    dispatchées vers le même handler — le cas typique des clients eMule,
    inbound comme sur les sessions download. `OP_REQFILENAME` /
    `OP_FILESTATUS` / `OP_AICHFILEHASHREQ` reçoivent une
    `OP_MULTIPACKETANSWER` groupée ; l'ext-info `0x58` (part-status du
    demandeur, lu selon la `ExtendedRequestsVersion` négociée) alimente
    le `reqstatus` SX.
  - **Filtrage à l'ingestion (`AddClientSources`/`CanAddSource`)** :
    `userhash == own`, port 0, IP mauvaise/LAN (policy réseau) ou filtrée
    (`ipfilter`), LowID quand on est firewalled, self `client_id`+serveur,
    cap `GetMaxSources` (défaut 400) ; persistance dédupliquée.
  - **Sélection des sources servies (`CreateSrcInfoPacket`)** : uniquement
    `!HasLowID && IsValidSource` avec masque compatible et ≥ 1 part utile
    au demandeur, cap 501 ; silence si `nCount == 0` ou fichier inconnu
    (shared **ou** download partiel servi).
  - **Throttles eMule** : réponses rate-limitées `SOURCECLIENTREASKS`
    40 min/client (×4 hors part-file rare ≤ `RARE_FILE` = 50) ; requêtes
    gouvernées par `IsSourceRequestAllowed` (`SOURCECLIENTREASKF`,
    `CONNECTION_LATENCY`, `GetMaxSourcePerFileSoft`).
  - **Re-ask périodique** : `OP_REQUESTSOURCES(2)` rattaché au cycle de
    reask (keep-alive queue + boucle de transfert) au lieu du seul
    handshake.
  - **Wire strict** : endianness SX1 v<3 en network order corrigée,
    version 0/>4 droppée, `count × entry_size == len` exigé.
  - **Échecs permanents** : refus de permis réseau (`AddressViolation`/
    `IpFiltered`) marque la source `Rejected` au lieu de boucler
    `Permit denied` dans le coordinateur.

- **Kad DHT — durcissement d'interopérabilité eMule 0.72a** (audit des
  sources officielles `srchybrid/kademlia/` + diagnostic du log WAN) :

  - *Réponses de routage conformes* : `KADEMLIA2_REQ` (`0x21`) reçoit
    désormais une vraie réponse `KADEMLIA2_RES` (`0x29`) avec les contacts
    **vérifiés** les plus proches de `target` (eMule `GetClosestTo` type 2),
    au lieu d'un `BOOTSTRAP_RES` vide qui était droppé comme non sollicité
    par le demandeur — le nœud n'est plus évincé des tables distantes.
    Comme eMule, la requête n'est servie que si le champ `uCheck` égale
    notre KadID (drop silencieux sinon). `BOOTSTRAP_REQ` reçoit notre
    KadID + port **TCP** eD2K (et non UDP — bug filaire corrigé, le
    paramètre encodeur est renommé `tcp_port`) + version Kad **8** +
    jusqu'à 20 contacts vérifiés diversifiés (`spread_verified`).
  - *Apprentissage des contacts entrants* : la table de routage est
    injectée dans le listener UDP (`set_routing_table`) ; les émetteurs
    de `HelloReq`, `HelloRes`, `BootstrapRes` et `PublishSourceReq`
    validé nourrissent la table — comme `AddContact` eMule qui apprend
    depuis chaque paquet.
  - *Sémantique `verified` fidèle* : les contacts appris dans les
    réponses de routage ne sont plus marqués vérifiés (un bootstrap
    malveillant ne peut plus injecter des « vérifiés » arbitraires) ;
    promotion uniquement par réponse sollicitée (`mark_seen`) ou
    handshake `HELLO_RES_ACK`. Le bootstrap n'assume `verified` que si
    la table est vide (`bAssumeVerified` = `GetNumContacts()==0`).
  - *Tolérance de publication* : `PublishSourceReq`/`PublishKeywordReq`/
    `PublishNotesReq` entrants sont droppés silencieusement quand la
    clé cible est hors `SEARCHTOLERANCE` de notre KadID (comportement
    `KademliaUDPListener.cpp`) — le nœud ne devient plus un puits
    d'index universel.
  - *Identité publiée correcte* : `publish_source` annonce l'IPv4
    **publique observée** (`FirewalledRes` → `observed_ip`, ou HighID
    de connexion serveur propagé via `set_public_ipv4`) et le port UDP
    effectif au lieu de l'adresse de bind — sinon les indexeurs eMule
    rejetaient nos publications (`contact.ip == IP source`). Version
    annoncée : 8.
  - *Lookups unifiés* : `find_node`, `search_notes` et `publish_note`
    passent par `converge_closest_contacts` (convergence itérative
    ALPHA=5) au lieu du parcours séquentiel / table locale.
  - *Deadlock corrigé* : cycle `in_flight` → `routing_table` (handlers
    `HelloRes`/`BootstrapRes` tenant le mutex des transactions pendant
    `learn_contact`) contre `routing_table` → `in_flight`
    (`kad.status` → `get_status` → `get_snapshot`) — gelait tout le
    pipeline Kad en session WAN réelle (recv loop morte, maintenance et
    recherches bloquées). Ordre des verrous corrigé des deux côtés :
    `learn_contact` appelé après relâchement d'`in_flight`, et
    `get_status` ne traverse plus aucun await en tenant `routing_table`.
  - *Fuite de transactions en vol* : `InFlightTxGuard` RAII retire
    l'entrée `in_flight` à l'abandon de la future `send_*` (le timeout
    externe de 2,5 s de `converge_closest_contacts` annulait la future
    avant son propre timeout interne de 3 s, laissant l'entrée jusqu'à
    saturation du cap 200 → silence total des envois). `insert_in_flight`
    balaye en prime les entrées âgées de plus de `timeout+2 s` ; les 4
    insertions inline qui contournaient le cap sont unifiées.
  - *Réponses de recherche multi-paquets* : les `SEARCH_*_RES` arrivent
    en plusieurs datagrammes (~50 entrées/paquet, jusqu'à ~300 résultats
    chez eMule) — la transaction n'est plus retirée au premier paquet.
    `InFlightTransaction.sender` passe en `mpsc` non borné ;
    `collect_search_batches` agrège les batches dans une fenêtre idle de
    350 ms (≤ 12 paquets, ≤ 300 entrées). ~91 % des résultats keyword
    étaient auparavant jetés comme non sollicités.

- **Options exclusives eMuleRust** (aucun équivalent eMule/aMule) :
  - `paranoidMode` : slots d'upload réservés aux pairs **obfusqués avec
    signature Secure Ident vérifiée** — applique le gate devant
    `request_upload_slot` (`OP_STARTUPLOADREQ`, `OP_REQUESTPARTS*`).
  - `identityRotationHours` : rotation périodique de l'identité —
    nouvelle clé RSA (`cryptkey.dat`), nouveau `user_hash` eD2K et
    nouveau `KadID` propagés à chaud (listener, SUI, firewall-check,
    table de routage Kad). Timestamp persisté
    (`identity_last_rotation_unix`), watcher 60 s. Les crédits sont
    volontairement détachés à chaque rotation.
  - `dailyUploadQuotaGb` : quota volumétrique d'upload par jour —
    compteur cumulatif persisté (`upload_quota_used_bytes`,
    `upload_quota_day`), gate atomique bloquant les nouveaux slots au
    seuil, remise à zéro au changement de jour UTC (watcher 30 s).
  - `swarmHealing` : biaise la planification des blocs vers les **parts
    les plus rares du swarm** (compteur de sources connues par part,
    `Ed2kDownloadSequencer::with_part_scarcity`) — le client devient
    source de secours avant la disparition d'une part. Les sources
    complètes (`part_count = 0`) couvrent toutes les parts ; les masques
    périmés (> 30 min) sont ignorés. eD2K reste pull-only : aucun push,
    tie-break sur octets reçus.

- **Persistance corrigée** :
  - `limits.set` persistait déjà les 5 buckets ; une mise à jour
    **partielle** réinitialisait en live les limiteurs non fournis aux
    défauts — la config part désormais du snapshot live des buckets.
    L'UI envoyait aussi `serverRequestsPerSec`/`kadPacketsPerSec` en dur
    (10/50) — elle envoie maintenant les valeurs courantes.
  - **Filtre IP** : `filter.setEnabled` et `filter.importFile` ne
    persistaient rien — `ipfilter_enabled` + `ipfilter_path` sont
    désormais écrits en SQLite et rechargés au boot
    (`filter.clear` oublie le chemin), comme eMule rechargeant
    `ipfilter.dat`.

- **Défauts alignés sur eMule 0.72a** : file d'upload 5000
  (`QueueSize`), `minUploadKbs` 1 (`MinUpload`), connexions/min 240
  (`MaxConnectionsPerFiveSeconds` = 20), UPnP désactivé par défaut,
  suppression des serveurs morts dès le 1er échec (`DeadServerRetry`).
  UI Connexion : boutons « Appliquer », capacités acceptent `0`
  (illimité).

- **Fix partages après redémarrage** : les téléchargements `Completed`
  persistés en SQLite mais absents de la map `active` (boot frais) sont
  désormais fusionnés dans `list_all_shared_files` et résolus par
  `get_shared_file` — comme eMule qui rehash ses fichiers finis dans
  `CSharedFileList` au démarrage. Sans ce fix, l'onglet Partages était
  vide et les fichiers complétés n'étaient pas serviables en upload après
  un restart. Le fichier cible doit exister sur disque.

- **Paramètres eMule complétés** (`docs/SETTINGS.md` pour la référence) :
  - **Serveurs** (section Connexion) : `serverReconnect` — watcher 15 s
    reconnectant au dernier serveur via le chemin RPC normal ;
    `serverAutoconnect` — connexion au meilleur serveur au démarrage ;
    `serverAutoremoveDead` — suppression après 3 échecs ;
    `serverAutoupdateList` + `serverAutoupdatePath` — import d'un
    `server.met` local au boot (pas de fetch HTTP, anti-SSRF) ;
    `safeConnect` — `downloads.resume` bloqué tant que ni eD2K ni Kad
    ne sont disponibles.
  - **Général** : `tempDir` (dossier `.part` configurable),
    `ichEnabled` (vérification MD4 des parts), `rememberCancelled` /
    `rememberDownloaded` (gating des entrées d'historique).
  - **Transferts** : `minUploadKbs` (plancher par slot upload, eMule
    `MinUpload` — n'ouvre pas de slot sous la moyenne configurée),
    `autoPriorityNewDownloads` (fichiers ≥ 1 part en priorité `high`).

- **Refonte complète des Paramètres** : le dialog fourre-tout est remplacé
  par un **onglet dédié `/settings`** (`SettingsView`) avec rail de sections
  à gauche, style « Préférences » eMule :
  - **Général** : dossier de téléchargement, bascule `shareDownloadDir`
    (partage auto du dossier Incoming — nouveau paramètre exposé via
    `settings.get`/`settings.set`), rétention de l'historique, langue.
  - **Connexion** : profil réseau lab/WAN, bannière kill switch, budgets de
    sockets, accès à la config UPnP/ports (ex-`NetworkPolicyDialog` +
    `NetworkModeDialog` fusionnés).
  - **Transferts** : `maxGlobalDownloadPeers`, `maxPeersPerDownload`,
    `minPeersPerDownload` — réglages RPC auparavant invisibles en UI.
  - **Sécurité** : filtre IP (`ipfilter.dat`) + limites de débit
    (ex-`IpFilterDialog` intégré) + notice anti-SSRF.
  - **Amis** : friend slots (section déplacée de l'ancien dialog).
  - **Avancé** : diagnostic, import `.part.met`, guide lab, barre debug,
    version du daemon.
  - Client : nouvelle méthode `updateSettings(patch)` → `settings.set`.
  - `SettingsDialog` supprimé (code mort) ; l'item « Settings » de la
    sidebar navigue vers `/settings`.

- **Stratégies « smart » eMule 0.72a** (download + upload) :
  - **Preview (first/last part)** : `Ed2kDownloadSequencer::with_preview_priority`
    sert les parts 0 et N-1 en priorité (`PR_VERYHIGH` eMule) pour les fichiers
    média/archives, permettant la prévisualisation pendant le téléchargement.
  - **End-game redundancy** : quand ≤ 3 blocs (≈ 552 Kio) restent, un bloc
    déjà en vol peut être re-demandé à un second pair (max 2 assignations,
    jamais pour le même pair ; hors end-game l'overlap reste interdit) —
    conforme `CPartFile::GetNextRequestedBlock`.
  - **Drop NoNeededParts** : `PeerFailureReason::NoNeededParts` — un pair dont
    le bitmask ne couvre aucun bloc manquant est libéré proprement et marqué
    `Rejected` temporairement.
  - **Priorités de fichiers** : `Download.priority` persisté (colonne SQLite
    `downloads.priority`, défaut `normal`), RPC `downloads.setPriority`,
    pondération de l'allocateur de slots (low×1/normal×2/high×4/release×8),
    sélecteur de priorité dans l'expansion Sources de l'UI.
  - **Crédits eMule persistés** : les octets téléchargés depuis un pair
    vérifié SUI sont comptabilisés sous `Md4(pubkey)` (flush par 8 Mio +
    fin de session) ; l'upload était déjà câblé — la formule canonique
    `min(10, 2·DL/UL)` s'applique désormais dans les deux sens.
  - **Trickle slots + slot focusing** : `UploadSlotState::Trickle` — un slot
    < 1 Kio/s après 60 s de grâce est rétrogradé (ne compte plus dans
    `max_active`) et le meilleur client en file est promu.
  - **Friend slots + anti-leecher minimal** : table `friends` (équivalent
    `friends.met`), RPC `friends.list`/`add`/`remove`, slot dédié bypassant
    la file (`uh:<userhash>`), rejet des clients non identifiés quand la file
    est ≥ 90 % pleine.
  - **A4AF (swap inter-fichiers)** : `a4af_rebalance` à chaque tick du
    coordinateur — un fichier sous `min_peers` réclame une adresse connectée
    à un fichier de priorité strictement inférieure et saturé (1 swap/tick,
    log `[A4AF]`). Nos sessions restent mono-fichier : le swap se fait par
    reconnexion, pas par multiplexage `OP_REQFILENAME`.

- **Dossier « Incoming » partagé** (`incoming_share.rs`) : au démarrage,
  chaque fichier régulier du répertoire de téléchargement est hashé en eD2K
  (MD4 + hashset persisté) et enregistré comme fichier partagé — même
  comportement que le dossier Incoming d'eMule. Les entrées existantes
  conservent leur priorité/état ; désactivable via le setting
  `share_download_dir = "0"`. Artefacts de téléchargement exclus
  (`.part`, `.met`, `.tmp`…). Log `[INCOMING_SHARE]`.
  - **Rescan manuel** : RPC `shares.rescan` + client `rescanSharedFiles()`
    ; le bouton refresh de l'onglet Partages déclenche désormais un vrai
    rescan du dossier Incoming (hash des nouveaux fichiers, réglages
    `enabled`/`upPriority` existants préservés) puis recharge la liste,
    avec SnackBar de résultat. Tests offline dans
    `upload_inbound_tests.rs` (découverte, préservation, opt-out).

- **Partage des téléchargements en cours (eMule `CPartFile`)** : un
  téléchargement actif devient serviable dès qu'une part eD2K complète
  (9 728 000 B) est reçue — `get_shared_file` résout un `SharedFile`
  `is_partial` pointant vers le `.part`, avec `completed_parts` = indices
  des parts vérifiées. Le dispatcher n'annonce que ces parts dans
  `PARTSTATUS` (`OP_FILESTATUS`), refuse tout range hors parts complétées
  (`OP_REQUESTPARTS`), et ne calcule jamais de hashset/AICH depuis un
  fichier incomplet (le hashset du `.met` reste servi, root AICH connu
  uniquement). `shares.list` inclut désormais les partiels (badge
  « partiel (n parts) » dans l'onglet Partages, contrôles
  priorité/activation désactivés dessus).

- **Paramètres eMule supplémentaires câblés** (`settings.get`/`settings.set`,
  appliqués à chaud) :
  - **Général** : `nickname` — pseudo annoncé aux serveurs et pairs
    (remplace les valeurs codées en dur « eMuleRust »).
  - **Connexion → Protocole & limites** : `obfuscationEnabled` /
    `obfuscationRequired` (protocol encryption — listener inbound live via
    `set_obfuscation`, connexions sortantes via `effective_obfuscation()`,
    byte connect-options buddy Kad mis à jour) ; `maxConnections` et
    `maxConnPerMin` (budgets `network_policy` appliqués immédiatement) ;
    `ussEnabled` (Upload Speed Sense, persisté dans `uss_config`).
  - **Transferts** : `maxUploadQueue` — taille de la file d'attente upload
    (eMule `QueueSize`, défaut 500, clamp 10–5000).
  - **Connexion → Capacités & ports** (panneau eMule complet) :
    `capacityDownloadKbs`/`capacityUploadKbs` (capacités de ligne, la
    capacité émission sert de référence à USS) ; `ed2kPort`/`kadPort`
    **rebind à chaud** (listener TCP stoppé/redémarré si actif, socket Kad
    UDP rebind, `kad.set_ed2k_tcp_port`, buddy connect-options,
    `upnp.set_ports`) ; `udpEnabled` (stoppe/démarre `kad_udp` +
    `server_udp`, eMule « UDP désactivé ») ; `upnpEnabled` (map/unmap
    immédiat). Ports effectifs en atomics sur `AppState`
    (`ed2k_port()`/`kad_port()`/`udp_enabled()`), persistés et appliqués au
    boot (`state.rs` lit les settings avant de binder) — tous les call
    sites (`servers`, `sources`, `swarm_coordinator`) utilisent désormais
    les valeurs effectives.

- **Alignement fin eMule 0.72a — file d'upload et gestion des sources** :
  - **Formule de crédit exacte** (`ClientCredits::GetScoreRatio`) :
    `min(2·DL/UL, √(DL/1Mio + 2), cap linéaire ~3,34 @ 9,2 Mio)` bornée
    `[1.0, 10.0]` — remplace le `2·DL/UL` seul qui sur-créditait les petits
    downloaders ; identités invalides/non vérifiées → 1.0.
  - **Priorité du fichier demandé dans le score de queue** : `file_prio_num`
    par fichier partagé (`veryhigh/powershare`=18, `high`=9, `normal`=7,
    `low`=6, `verylow`=2) appliqué au score d'attente ; colonne
    `shared_files.up_priority` + RPC `shares.setUploadPriority`.
  - **Bonus « downloading »** : un client en cours d'upload qui re-file
    reçoit ~+15 min de temps d'attente (`GetScore(isdownloading)`), ce qui
    stabilise les sessions actives.
  - **Score 0 bad-guy** : les clients bannis ou à identification rejetée
    (`IS_IDBADGUY`) restent en queue avec score 0 — jamais promus.
  - **Purge conditionnelle à 80 %** (`CPartFile::Process`) : quand le nombre
    de sources connues atteint 80 % du `maxSources` du fichier, purge à
    cadence 60 s des LowID non rappelables, des sources NNP (« no needed
    parts ») et des sources en échec répété (`fail_count ≥ 3`), log
    `[SOURCE_PURGE]`.
  - **UDP re-ask conditionné** : `OP_REASKFILEPING` n'est émis que si la
    session en file distante a plus de ~20 min (dernier essai de connexion
    ancien), en plus de la déduplication 21 min existante.
  - **`maxSources` par fichier** : `FT_MAXSOURCES` (0x13) lu à l'import
    `.part.met` et réécrit à la sauvegarde ; colonne `downloads.max_sources`,
    champ `maxSources` dans les snapshots, RPC `downloads.setMaxSources` ;
    plafonne le quota de pairs, le gate de re-requête et le seuil de purge.
  - **UI câblée** : champ « Src max » (dialog d'édition `FT_MAXSOURCES`)
    dans l'expansion Sources ; chip de priorité `DownloadPriorityChip`
    directement sur chaque ligne de download (layouts full et compact —
    plus besoin de déplier) ; onglet **« Partages »** dédié (`/shares`,
    `SharesView`) avec sélecteur de priorité d'upload (powershare) et
    switch activé/suspendu par fichier ; section « Amis » dans Paramètres
    (ajout/suppression de userhash → friend slot) ; colonnes « Score » et
    « Crédit ×n.nn » + badge « banni » dans la file d'upload.
  - **A4AF événementiel + périodique** : `a4af_event_pass` à chaque tick —
    une source marquée NNP/FNF voit son cooldown effacé sur les autres
    fichiers où elle est candidate (équiv. `SwapToAnotherFile` immédiat) ;
    le rebalance complet passe à la cadence eMule de 8 min
    (`A4AF_FULL_INTERVAL`).

- **Alignement protocolaire eMule 0.72a** (`mule-protocol`, `mule-daemon`) :
  - **Référence de vérité** : `D:\eMule0.72a-Sources\eMule0.72a-Sources\srchybrid`
    remplace eMule 0.50a comme référence normative (docs `docs/ed2k/`,
    `AGENTS.md`, README mis à jour ; les mentions 0.50a restantes sont de la
    provenance historique assumée).
  - **Profil `Emule072a`** : nouvelle variante `Ed2kCompatibilityProfile::Emule072a`,
    désormais le **profil par défaut**. `Emule050a` est conservée comme variante
    historique (assignations filaires identiques).
  - **Opcodes AICH corrigés** : `0x12`–`0x15` (erronés) → `0x9B`–`0x9E`
    (`opcodes.h:272-275`).
  - **Nouveaux codecs** : `OP_HASHSETREQUEST2`/`ANSWER2` (`0xB1`/`0xB2`,
    `FileIdentifier` + options), famille `OP_MULTIPACKET*` (`0x92`/`0x93`/`0xA4`/
    `0xA9`/`0xB0`), `OP_QUEUERANK`/`QUEUERANKING`/`CHANGE_SLOT`, `OP_MESSAGE`,
    `OP_FILEDESC`, `OP_PORTTEST`, `OP_CHATCAPTCHAREQ`/`RES`, `OP_PEERCACHE_*`
    (raw/ignorés). `0x51`/`0x52` marqués *DEPRECATED* comme dans `opcodes.h:223-224`.
  - **Ext-info `0x58` déterministe** : `Ed2kDecoder::extended_requests_version`
    porte la version `ExtendedRequestsVersion` négociée via `OP_EMULEINFO`
    (0 = absent, 1 = bitfield, ≥2 = bitfield + complete sources) au lieu de
    l'ancienne heuristique par sous-opcode. Câblée dans `peer_session`
    (`status_query`, `transfer`) avec la version du pair.
  - **Kad** : `send_bootstrap` insère le nœud répondeur dans les contacts
    (conforme eMule) ; `search_sources` déduplique les sources par `IP:port`
    rapportées par plusieurs contacts.
  - **Robustesse tests** : lecture des événements `downloadAdded`/
    `downloadUpdated` tolérante à l'ordre ; fixtures `PeerSource`
    (`software`/`availability`) ; masque `FILESTATUS` surdimensionné réellement
    rejeté ; clippy workspace à zéro warning sous `-D warnings`.

- **Façade AppController réduite** : ~31 membres délégués devenus
  inutilisés supprimés (getters history/kad/servers/search/netpol/
  settings exposés désormais via leurs providers). Restent : composition
  root (client, supervisor, sub-contrôleurs), `initialize`/`refresh`/
  `dispose`, et les membres encore consommés (tests, `DeepLinkHandler`,
  `filteredDownloads`).

- **Desktop : notifications natives de fin de téléchargement** :
  `DownloadNotifier` (`apps/desktop/lib/core/download_notifier.dart`)
  souscrit à `downloadLifecycleEvents` et affiche un toast OS via
  `local_notifier` (Windows/Linux/macOS) sur `completed` (nom du fichier)
  et `failed` (nom + raison). No-op sur les autres plateformes ou si le
  notifier OS est indisponible.

- **Web : bouton "Changer de daemon"** : nouveau provider
  `onChangeDaemonProvider` (null par défaut — desktop/mobile). La sidebar
  affiche une action `link_off` quand il est défini ; `eMuleRustApp`
  accepte `onChangeDaemon`. `apps/web` implémente `_disconnect()` :
  dispose le controller et revient au formulaire de connexion pré-rempli
  avec l'URL courante.

- **Migration AppController → providers (fin)** : les ~18 widgets restants
  (dialogs add-link/import/settings/network-policy/ip-filter/lab-guide/
  diagnostic/category/network-mode/download-detail, sidebar, top bar, list,
  row, empty state, details panel, downloads screen) consomment désormais
  les providers Riverpod (`ref.watch`/`ref.read`) au lieu de recevoir
  `AppController` en paramètre. `AppNavSection`/`DownloadFilter` déplacés
  dans `shell_controller.dart` (ré-exportés par `app_controller.dart` pour
  compat). `AppController.dispose` est idempotent — les sous-contrôleurs
  sont disposés par leurs `ChangeNotifierProvider`. `AppController` reste
  uniquement composition root (router, `eMuleRustApp`, tests).


- **E5 — Détail par pair + ETA** : `PeerSessionSnapshot.transferRateBps`
  existait déjà (affiché dans la carte sources du détail) ; ajout de
  `etaSeconds` au `DownloadSnapshot` léger (seconds restantes au débit
  courant, `None` si nul/terminé) — peuplé dans `DownloadItem.etaSeconds`
  sans appel `downloads.get`, affiché dans la ligne de téléchargement
  (`ETA: 12m 34s` sous le débit).

- **E4 — Fin du polling** : events daemon `kad.statusChanged`,
  `history.entryAdded`, `settings.changed {keys}`, `network.policyChanged`
  (topics `kad`/`history`/`settings`/`network`, payload nudge = refetch).
  `AppState::emit_event` = point d'entrée unique pour les handlers. Côté
  client : `kadStatusEvents`/`historyEvents`/`settingsEvents`/
  `networkPolicyEvents` + `SettingsChangedEvent`. Côté UI : le timer 3 s
  de `KadController` est supprimé (events + fetch à la connexion),
  `HistoryController`/`SettingsController`/`NetworkPolicyController` se
  rafraîchissent sur event → synchronisation multi-clients live.

- **E3 — Cycle de vie downloads** : events daemon `event.downloadAdded` /
  `downloadRemoved` / `downloadCompleted` / `downloadFailed` (topic
  `downloads`), émis centralement par `DownloadService` (`set_event_channel`
  — tous les chemins couverts : RPC, deep link, lab, transitions internes).
  Côté client `DownloadLifecycleEvent` + `downloadLifecycleEvents` ; côté UI
  `AppController` applique added/completed/failed via `upsert`, removed via
  `removeLocal`, et affiche bannière de fin / erreur d'échec (toast partagé
  desktop + web).

- **E2 — Tick de stats globales** : event daemon `event.statsTick`
  (topic `stats`, ~2 s, suspendu sans abonné) avec `downloadBps`,
  `uploadBps`, `downloadsActive`, `peersConnected`, `serverConnected`,
  `kadContacts`, `kadUdpBound`. Côté client `DaemonStatsModel` +
  `statsEvents` ; côté UI `StatsController` (provider dédié) et le
  `TransferRateTracker` préfère les débits agrégés du daemon quand un tick
  est disponible (fallback : sommes locales).

- **E1 — Uploads temps réel** : event daemon `upload.uploadsUpdated`
  (topic `uploads`, snapshot actif + file + débit agrégé, émis après chaque
  mutation de l'upload manager, progression throttlée ~500 ms). Côté client
  `MuleDaemonClient.uploadsEvents` + `UploadsSnapshotModel` ; le fake client
  simule un upload actif ~50–250 kB/s + 1 en file. Côté UI : nouveau
  `UploadsController` (provider dédié), section **Uploads** dans la sidebar
  (`/uploads`), `UploadsView` avec tables sessions actives / file d'attente
  (annulation via `uploads.cancel`), et le badge UL + sparkline de la top
  bar branchés sur le débit réel (fini le `() => 0`).

- **Architecture partagée — `packages/app_shell`** : tout le code UI/app
  (`AppController`, providers, router, vues/features, l10n) est extrait de
  `apps/desktop` vers `packages/app_shell` (platform-pure). Les points de
  contact OS passent par des abstractions : `DaemonLifecycle` (implémenté
  par `DaemonSupervisor` desktop), `WorkDirInfo`/`WorkDirMode`, et les
  helpers conditionnels `systemActions`/`pickFolder` (stub web / impl IO).
  `eMuleRustApp` est partagé ; `apps/desktop/lib/main.dart` devient un
  thin entry point (window/tray/deep-links + supervisor).
- **`apps/web` — remote controller** : nouvelle app Flutter web qui se
  connecte à un mule-daemon existant via WebSocket. Résolution de l'URL
  par ordre : `?daemon=ws://…` > `--dart-define=DAEMON_URL` > préférence
  `daemon.url` persistée > formulaire de connexion
  (`DaemonConnectScreen`, suggestion `ws://<hôte>:4660/rpc`, validation
  ws/wss, `?daemon=` vide réinitialise). `flutter build web` + dry-run
  WASM validés.

- **Desktop polish (P6, début)** :
  - `file_picker` natif remplace le hack PowerShell dans
    `lib/core/folder_picker.dart` (Windows/Linux/macOS/Android).
  - `window_manager` : `lib/core/window_setup.dart` restaure et persiste
    la géométrie de fenêtre (min 760×500, shared_preferences `window.*`).
  - `tray_manager` : `lib/core/tray_setup.dart` ajoute l'icône tray
    (Show/Quit) avec `assets/app_icon.ico`.
  - Routes profondes (`lib/app/router.dart`) : `/downloads/:id` ouvre le
    panneau détail d'un téléchargement (l'URL devient la source de vérité
    pour la sélection — clic row → `router.go`, fermeture → retour
    `/downloads`), `/settings` ouvre le dialog Settings par-dessus la
    section downloads, `usePathUrlStrategy()` activé sur le web.
  - Graphe de débit : `TransferRateTracker` (`lib/app/`) échantillonne le
    débit agrégé chaque seconde (fenêtre glissante 60 s, timer paresseux
    démarré au premier listener) ; `SpeedSparkline` (CustomPainter, sans
    dépendance chart) affiché dans le badge de vitesse de la top bar.
  - Liens `ed2k://` : `app_links` + `lib/core/deep_link_handler.dart`
    (lien initial + stream → `addEd2kLink`) ; enregistrement Windows via
    `windows/ed2k_protocol.reg` (ou l'installeur), `.desktop`
    `x-scheme-handler/ed2k` pour Linux, intent-filter/URL types pour
    Android/iOS.
  - Runners **Android** et **iOS** générés (`com.eMuleRust.app`) :
    `AndroidManifest.xml` avec permission `INTERNET` + intent-filter
    `ed2k`, `Info.plist` avec `CFBundleURLSchemes` `ed2k`, nom d'app
    `eMuleRust`. Les services desktop (`WindowSetup`, `TraySetup`) sont
    déjà no-op hors desktop ; `pickFolder` exclut iOS (pas de picker de
    dossier natif).

### Ajouté — Refonte UI multiplateforme (branche `ui-ux`)

- **`packages/mule_client`** : extraction du client daemon dans un package
  Dart partagé (interface `MuleDaemonClient`, transport `RpcDaemonClient`
  WS/JSON-RPC, `FakeMuleDaemonClient`, modèles de domaine, DTOs). Nouveau :
  reconnexion automatique opt-in (`autoReconnect`, backoff 0,5 s→10 s +
  re-`hello` + resync), keep-alive `system.status` toutes les 20 s
  (détection des connexions zombies), gestion `event.snapshotRequired`,
  tests client déplacés dans le package.
- **`packages/ui_kit`** : design system partagé — `AppTheme`,
  `Formatters`, `AppStrings`, `PartAvailabilityBar`.
- **Daemon — events RPC durcis** : `DaemonEvent` devient la source unique
  du format wire (`method()`, `topic()`, `send()` — utilisé par tous les
  émetteurs) ; `events.subscribe {topics}` filtre par session
  (`downloads`/`peers`/`servers`/`search`/`network`) ; un client qui lag
  reçoit `event.snapshotRequired {skipped}` au lieu de perdre des events
  silencieusement ; `DownloadUpdatedEvent` (doublon) supprimé.
- **UI desktop** : correction du débordement du sélecteur de cible de
  recherche (scroll horizontal en mode compact, espacements resserrés) ;
  watchdog de recherche 30 s annulé dès réception de résultats.
- **Navigation `go_router` + shell adaptatif (P3)** : `MaterialApp.router`
  avec routes déclaratives par section (`/downloads`, `/history`,
  `/servers`, `/search`, `/kad`), `ShellRoute` injectant le contenu dans le
  shell adaptatif (`DownloadsScreen` : sidebar ≥1100 px, rail compact
  750–1100 px, drawer <750 px). Le `ShellController` reste la source de
  vérité de la section : sync VM→URL par listener (`router.go`), sync
  URL→VM (deep links, prêts pour le web) via adoption de la route dans le
  shell. Les vues de section ont désormais chacune leur `ListenableBuilder`.
- **Clean architecture UI** : design tokens dans `ui_kit`
  (`AppColors`/`AppSpacing`/`AppRadius`/`AppTextStyles`/`AppBreakpoints`/
  `AppDecorations` — plus aucun `Colors.*` en dur dans les vues) ;
  `downloads_screen.dart` découpé en widgets dédiés (`DownloadsTopBar`,
  `DownloadsList`, `DownloadsEmptyState`, `DownloadDetailsPanel`,
  `StatusNotificationBanner`) ; conventions dans
  `docs/architecture/conventions_ui.md`.
- **i18n (P5)** : `gen_l10n` activé — `l10n.yaml`, ARB
  `lib/l10n/app_fr.arb` (template = libellés actuels) + `app_en.arb`
  (traduction), sortie `lib/l10n/generated/`. Sélecteur de langue FR/EN
  dans Settings (`LocaleController` + `localeControllerProvider`, persistance
  `shared_preferences` clé `app.locale`). Extension `context.l10n`
  (+ `DownloadFilterL10n`). `AppStrings` (ui_kit) est deprecated.
  **Passe complète** : ~400 littéraux visibles migrés vers `context.l10n`
  dans ~90 fichiers (dialogs, tables, toolbars, bannières, empty states),
  avec placeholders `{param}` pour les textes dynamiques. Les littéraux
  techniques (URLs `ed2k://`, chemins, formats purs interpolés) restent en
  dur volontairement. Les tests qui montent un `MaterialApp` brut doivent
  fournir `AppLocalizations.localizationsDelegates` + `supportedLocales`
  (+ `locale: Locale('fr')` quand ils assertent des libellés FR).
- **Migration Riverpod (P4, découpage du VM terminé)** : `ProviderScope`
  dans `eMuleRustApp` (`muleDaemonClientProvider`,
  `daemonSupervisorProvider`, `devToolsEnabledProvider`,
  `appControllerProvider`). L'ancien `DownloadsViewModel`, renommé
  `AppController` et déplacé dans `lib/app/app_controller.dart` (1781 → ~740
  lignes) est découpé en **10 contrôleurs par sous-domaine**, chacun dans
  son fichier `features/<domaine>/view_models/` et exposé via un
  `ChangeNotifierProvider` dédié : `DownloadsStoreController` (store,
  events, CRUD, sélection, part.met, diagnostic), `HistoryController`,
  `KadController`, `SearchController` (watchdog 30 s, dédup par hash),
  `ServersController` (liste + session), `PeerSourcesController`,
  `CategoriesController`, `NetworkPolicyController` (kill switch, IP
  filter, rate limits), `SettingsController`, `LabController`. Le VM
  reste un orchestrateur fin (connexion/supervisor, section, bannières)
  qui délègue et forward les notifications — aucune vue ni test cassé.
  Reste : basculer les vues sur les providers (`HistoryView` et `KadView`
  déjà en `Consumer*`), puis supprimer la façade.
- **Docs** : `docs/architecture/rpc_protocol.md` (contrat JSON-RPC complet)
  et `docs/architecture/plan_ui_multiplateforme.md` (roadmap desktop/
  mobile/web en 10 phases).

### Ajouté — Notes Kad (commentaires/évaluations de fichiers) et outillage MCP

- **Kad notes — modèle wire et codec bornés** (`crates/mule-protocol/src/kad`) : `KadNoteWire` (entrée `CEntry` avec `m_bSource = false`), décodeurs/encodeurs pour `KADEMLIA2_SEARCH_NOTES_REQ` (`0x35`, `[target:16][file_size:u64]`), `KADEMLIA2_PUBLISH_NOTES_REQ` (`0x45`, `[target][source:16][tag_count:u8][tags]`), les réponses notes sur le `KADEMLIA2_SEARCH_RES` unifié (`0x3B`, forme émise par `CIndexed::SendValidNoteResult`) et le `KADEMLIA_SEARCH_NOTES_RES` legacy (`0x3A`). Listes de tags génériques bornées : tags connus décodés en `Ed2kTag`, inconnus préservés en `RawTag`, compteur plafonné à 32 — les commentaires restent des données d'affichage non fiables.
- **Kad notes — index de stockage** (`kad_udp/storage.rs`) : map `target → source_id → entrée` (une note par publieur et par fichier), TTL 5 h, plafond 100 notes/fichier, filtrage `file_size` à la lecture (`SendValidNoteResult`), rapport `load` (0-100) dans `KADEMLIA2_PUBLISH_RES`. Handlers : `SearchNotesReq` servi, `PublishNotesReq` refusé quand le nœud est UDP-firewallé (garde eMule), réponse `0x4B` routée vers les transactions notes par `expected_response`.
- **Kad notes — persistance SQLite** : table `kad_notes_index` (`file_hash`,`source_id` PK) stockant la liste de tags en **blob wire** (`encode_note_tags_blob`/`decode_note_tags_blob`) — les tags arbitraires/inconnus traversent un redémarrage à l'identique ; purge TTL et réhydratation au démarrage de la socket Kad.
- **Kad notes — recherche et publication sortantes** : `send_search_notes`/`send_publish_notes` (transactions `KadUdpService` avec corrélation in-flight) ; `KadService::search_notes` interroge les 20 contacts les plus proches du hash fichier et agrège les notes dédupliquées par `source_id` ; `publish_note` pousse une liste de tags bornée (`filename`/`filesize`/`comment`/`rating`, commentaire ≤ 512 o). RPC : `kad.searchNotes`, `kad.publishNote`. Point subtil : `0x3B` étant partagé sources/keywords/notes, la boucle UDP **re-décode** le payload brut en forme notes quand une transaction attend `SearchNotesRes` (sinon les tag lists s'aplatiraient en entrées keyword).
- **MCP** : nouveaux outils `emule_kad_search_notes` et `emule_kad_publish_note` ; `DaemonClient::call` applique un timeout de 60 s aux méthodes `kad.search*`/`kad.publish*` (les recherches itératives dépassent 15 s en WAN), exposé via `call_with_timeout`.
- Tests : codec notes (roundtrips `0x35`/`0x45`/`0x3B`/`0x3A`, malformed, cap tags) ; store notes (upsert, cap, filtre taille, load) ; intégration `kad_udp` : publish → `0x4B` load → search → `0x3B` avec commentaire/rating → filtre taille.

### Ajouté — Recherche globale serveur `GLOBSEARCHREQ3` (eMule 0.72a)

- **Codec** (`mule-protocol/src/ed2k/server_udp.rs`) : `encode_glob_search_req3` (`0x90`) préfixe l'arbre de recherche REQ2 d'un tag `CT_SERVER_UDPSEARCH_FLAGS` (`0x0E`) = `SRVCAP_UDP_NEWTAGS_LARGEFILES` (`0x01`) écrit au format `WriteNewEd2kTag` (type `|0x80`, nom 1 octet) — déclare au serveur le support des tags nouveau format et des tailles 64 bits ; `OP_GLOBSERVSTATRES` décodé en entier (`challenge`, `users`, `files`, `udp_flags`, ports d'obfuscation TCP/UDP), constantes `SRV_UDPFLG_EXT_GETFILES`/`SRV_UDPFLG_LARGEFILES` exposées.
- **Daemon** (`services/server_udp.rs`) : cache des `udp_flags` par adresse UDP de serveur, alimenté par `OP_GLOBSERVSTATRES` (réponse acceptée uniquement si le challenge correspond à un `GLOBSERVSTATREQ` émis — garde anti-usurpation d'eMule) ; `send_servstat_req` probe un serveur aux capacités inconnues avant la recherche ; `send_glob_search` choisit `REQ3` si `EXT_GETFILES` **et** `LARGEFILES` (logique `SearchResultsWnd.cpp`), `REQ2` sinon, et retourne la forme émise.
- **RPC** : `servers.search` retourne désormais `udpTargets[]` (`{address, request: "REQ3"|"REQ2"|null}`) pour observer la sélection en test UI/MCP, et accepte `udpOnly` (portée « Global » d'eMule : fan-out UDP vers la liste de serveurs sans recherche TCP sur le serveur connecté).
- **UI desktop (harnais de test)** : troisième cible de recherche « Global UDP » (`udpOnly`), bandeau d'info sans connexion requise, et résumé `UDP: n×REQ3, m×REQ2` affiché dans la notification de statut après envoi.
- Tests : wire REQ3 (tag NewEd2k + payload verbatim), décodage `GLOBSERVSTATRES` complet et forme courte, sélection REQ3/REQ2/probe par socket UDP réelle en loopback.

### Corrigé — Recherche globale UDP

- **Sélection REQ1/REQ2/REQ3** : eMule envoie `OP_GLOBSEARCHREQ` (`0x98`) aux serveurs sans `SRV_UDPFLG_EXT_GETFILES` — ces serveurs ignoraient silencieusement le REQ2 qu'on leur adressait ; la sélection est désormais à trois niveaux (REQ3 → REQ2 → REQ1) et un serveur inconnu est probé par `GLOBSERVSTATREQ` puis reçoit REQ1.
- **`OP_GLOBSEARCHRES` mal décodé** : le payload UDP `0x99` est un flux d'entrées nu (`[hash16][clientid][port][tagcount][tags]` répété), sans le préfixe `u32` count du `OP_SEARCHRESULT` TCP — les datagrammes reçus étaient rejetés par le décodeur TCP ; nouveau `decode_glob_search_res` validé sur un datagramme réel capturé.
- **Observabilité** : pacing 50 ms entre envois UDP (eMule espace ses requêtes) et logs `unknown opcode`/`decode error` relevés en INFO sur le socket server-UDP.

### Ajouté — Disponibilité par part (carte eMule)

- `downloads.getDetails` expose `availableSourceCount` par part : nombre de sources connues dont le `PartAvailabilityMask` couvre la part (`0` = part introuvable → téléchargement bloqué).
- `downloads.list` retourne désormais les `DownloadDetail` complets (shape aplatie compatible) afin que la liste affiche la barre de disponibilité par part sans appel dédié.
- UI : `PartAvailabilityBar` (segment par part positionné à l'échelle des offsets) sous chaque ligne de téléchargement — rouge = aucune source, nuances de bleu = rare → commun, teal = vérifié ; tooltip `Part N · état · Sources: X`. Couleurs partagées avec la carte Part Map du détail.

### Modifié — Couche events RPC (pour `packages/mule_client`)

- `DaemonEvent` devient la source unique du format wire (`services/events.rs`) : chaque variante porte sa `method`, son `topic` de souscription et sérialise via `to_wire_json()` ; tous les émetteurs passent par `DaemonEvent::send` (`DownloadUpdatedEvent` supprimé).
- `events.subscribe` est réellement fonctionnel : intercepté dans `handle_socket`, il mut le filtre de topics **par session** (`downloads`, `peers`, `servers`, `search`, `network`, `other` ; `null` = tout, rétro-compat). Non authentifié → `-32000` comme toute autre méthode.
- `RecvError::Lagged` sur le broadcast n'est plus ignoré : le daemon émet `event.snapshotRequired {skipped}` pour forcer une resynchronisation complète côté client.
- Doc : `docs/architecture/rpc_protocol.md` §6 documente le tableau méthode → topic, la sémantique de `events.subscribe` et `snapshotRequired` ; §9 décrit `packages/mule_client` (reconnexion auto, keep-alive, resync).

### Corrigé — Audit et refonte UX de la page Paramètres (`/settings`)

Audit complet du câblage UI → client RPC → handler daemon → persistance →
consommation réelle (référence mise à jour : `docs/SETTINGS.md`).

- **UPnP** : `RpcDaemonClient.mapUpnpPorts` n'envoie plus les ports codés
  en dur `39735/39737` — en omettant les paramètres, le daemon mappe les
  ports effectifs du snapshot (suivis par `upnp.set_ports`).
- **`capacityDownloadKbs` fonctionnel** (était inerte) : clamp eMule
  (`Preferences.cpp` : limite DL ≤ capacité × 9/10, UL ≤ capacité × 4/5,
  `0` = illimité non borné) appliqué dans `limits.set`
  (`clamp_to_capacity`), au changement de capacité via `settings.set`
  (`apply_capacity_clamp`), et **normalisé au boot** dans `AppState::new`
  — une limite persistée au-delà de la capacité est bornée puis réécrite
  en SQLite pour refléter la valeur effective.
- **Saisies invalides remontées** : `int.tryParse` échouait silencieusement
  (clé omise du patch, toast « mis à jour » mensonger) — les sections
  Connexion/Transferts/Sécurité valident désormais chaque champ (bornes,
  non-vide) et remontent l'erreur dans la bannière globale.
- **Mineurs daemon** : write-probe sur `tempDir` (comme
  `defaultDownloadDir`), warning `tracing` sur les clés `settings.set`
  inconnues, `maxUploadQueue` borné UI ≥ 10.

### Modifié — Localisation complète de `/settings`

- ~120 nouvelles clés ARB FR/EN (`settings*`, `network*`, `filter*`,
  `upnp*`, `friends*`, descriptions de profils réseau) ; les 6 sections,
  le rail, les cartes réseau (`NetworkStateBanner`, `UpnpStatusCard`,
  `NetworkModeStatusCard`, `RateLimitsTab`, `NetworkProfileSelector`,
  `IpFilterTab`, rapport d'import) et les widgets réutilisés
  (`SettingsDirectorySection`, `SettingsFriendsSection`) passent par
  `context.l10n` — le toggle Langue traduit désormais tout l'écran.
- Messages de statut des contrôleurs (`SettingsController`,
  `NetworkPolicyController`) passés en français, cohérents avec le reste
  du shell.
- Corrections préexistantes : `btnCancel`/`cancel` FR étaient « Cancel »
  → « Annuler » ; `ajouter` EN était « Ajouter » → « Add ».

### Modifié — UX de sauvegarde et synchronisation des Paramètres

- **Affordances homogènes** : Entrée soumet dans tous les champs (y
  compris Transferts et les limites de débit) ; boutons « Appliquer » par
  carte en Connexion/Transferts, icône de sauvegarde sur les champs
  libres.
- **Dirty tracking + resync** : tous les `TextEditingController` se
  resynchronisent sur `settings.changed` / changements de politique via
  `ref.listen` — un champ en cours d'édition (dirty) n'est jamais
  écrasé par une valeur distante.
- **Dropdown rétention robuste** : une valeur persistée hors des items
  standards (ex. 60 jours via `emule_update_settings` MCP) s'affiche dans
  un item dédié au lieu d'asserter le `DropdownButton`.
- **Kill switch** : dialog de confirmation avant de couper toutes les
  sockets (`network.disconnectAll`).
- **Sécurité** : plus de spinner pleine section à chaque toggle — barre
  de progression linéaire pendant les opérations.
- **Divers** : section Amis sur `SettingsSectionScaffold`, `Scrollbar`
  explicite sur le panneau, switch barre debug désactivé quand les dev
  tools sont indisponibles, note explicative capacité ↔ limite dans
  l'onglet des débits.
- Tests UI mis à jour : `import_part_met_dialog_test` attend « Annuler »,
  `network_policy_ui_test` confirme le dialog du kill switch.

### Modifié — Partage partiel : publication serveur/Kad et protections

Fidélité eMule 0.72a pour les `.part` en cours (`CPartFile`) :

- **Offre serveur (`OP_OFFERFILES`)** : la liste envoyée au login est triée
  par priorité d'upload décroissante (`file_prio_num`, comme
  `SendListToServer`) et plafonnée à 200 ; les fichiers **incomplets**
  portent désormais le couple de marqueurs `0xFCFCFCFC`/`0xFCFC` (vs
  `0xFBFBFBFB`/`0xFBFB` pour les complets) quand le serveur supporte
  `SRV_TCPFLG_COMPRESSION` — `OfferedFile::for_server` prend un paramètre
  `is_partial` et exporte `OFFER_INCOMPLETE_CLIENT_ID`/`_PORT`.
- **Republication Kad** : les partiels sont publiés comme **sources**
  (`CKnownFile::PublishSrc` — les pairs peuvent récupérer les parts déjà
  complétées) mais la publication de *keywords* leur est refusée (« someone
  else should have the full file »).
- **Mutations refusées sur partiels** : `shares.setEnabled` et
  `shares.setUploadPriority` retournent une erreur métier sur un
  `is_partial` — persister un partage transitoire figerait son chemin
  `.part` dans `shared_files` et le servirait plus tard comme complet
  (eMule n'autorise pas le départage d'un part en cours).
- **`setUploadPriority` robustifié** : résolution `get_shared_file` puis
  upsert — fonctionne aussi sur les downloads `Completed` jamais persistés
  dans `shared_files`.
- **Fix client `shares.list`** : `RpcDaemonClient.listSharedFiles`
  attendait une liste JSON nue alors que le daemon renvoie
  `{sharedFiles: [...]}` — l'onglet Partages affichait une liste vide ;
  le parseur accepte désormais les deux formes.
- **UI Partages** : `SharesView` s'abonne aux événements lifecycle
  (rafraîchissement immédiat) et aux events de progression (debounce 3 s)
  pour suivre les `completedParts` des partiels.

## [0.1.9] - 2026-09-20

### Ajouté — Interopérabilité eMule 0.72a (fonctions essentielles du réseau eD2K/Kad)

Audit comparatif du daemon contre les sources officielles **eMule 0.72a** (`D:\eMule0.72a-Sources`), puis implémentation de toutes les fonctions bloquantes pour des transferts eD2K/Kad réels :

- **Vérification incrémentale par part (Hashset MD4)** : codec complet `OP_HASHSETREQUEST` (`0x51`) / `OP_HASHSETANSWER` (`0x52`) dans `mule-protocol` ; requête automatique à la connexion d'une source ; vérification MD4 à la complétion de chaque part de 9 728 000 octets avec éviction ciblée des parts corrompues (`CPartFile::IsCorruptedPart`) au lieu de l'échec global du fichier ; table SQLite `file_part_hashes` ; service `OP_HASHSETANSWER` côté upload pour les fichiers partagés (calcul `spawn_blocking` + vérification du hash racine).
- **AICH câblé au réseau** : codec des 4 opcodes (`OP_AICHREQUEST 0x12`, `OP_AICHANSWER 0x13`, `OP_AICHFILEHASHREQ 0x14`, `OP_AICHFILEHASHRES 0x15`) ; construction/streaming de l'arbre de Merkle SHA-1 eMule (`mule-format/src/aich/recovery.rs`) avec la double granularité exacte (nœuds > 1 part découpés par `PARTSIZE`, ≤ 1 part par `EMBLOCKSIZE`, blocs feuilles alignés par part : 52 × 184 320 + 143 360 octets) ; consensus de confiance `CAICHUntrustedHash` (≥ 10 IP uniques masquées `0x00F0FFFF`, ≥ 92 %) ; récupération de part corrompue au niveau bloc 180 Ko via données de récupération (ident + hash) ; service des deux opcodes en upload.
- **Publication des partages au serveur (`OP_OFFERFILES 0x15`)** : envoi après `OP_IDCHANGE` avec sémantique `CreateOfferedFilePacket` — marqueurs de complétion `0xFBFBFBFB`/`0xFBFB` sous compression serveur, HighID vs LowID, filtrage > 4 Gio sauf `SRV_TCPFLG_LARGEFILES` avec tags `FT_FILESIZE`/`FT_FILESIZE_HI`, mapping de type de fichier eMule (`Audio`/`Video`/`Image`/`Doc`/`Pro`), liste vide = keep-alive, plafond 200 fichiers.
- **Callback LowID** : `OP_CALLBACKREQUEST` (`0x1C`, `u32` nu), décodage `OP_CALLBACKREQUESTED` (`0x35`, `IP+port` + forme étendue crypt/userhash) et `OP_CALLBACK_FAIL` (`0x36`) ; rétention des sources LowID (`lowid:<client_id>`) au lieu de rejet ; registre de callbacks en attente (TTL 5 min) ; adoption des connexions entrantes matchant un callback comme sessions de téléchargement (`PeerSessionHandle._permit: Option<SocketPermit>`) ; garde eMule : pas de callback si nous sommes LowID.
- **Écriture `.part.met` compatible eMule** : `PartMetFile::write_to`/`save` au format exact `CPartFile::SavePartFile` (version `0xE0`/`0xE2` largefile, hashset, tags `FT_FILENAME`/`FT_FILESIZE`/`FT_TRANSFERRED`/`FT_AICH_HASH` base32, gaps en tags nommés `\x09<i>`/`\x0A<i>`) ; écriture atomique `.tmp` + rename ; sauvegardes à l'ajout, à chaque part complétée/évincée et à la pause ; suppression à la finalisation ; **fix reader** : résolution de l'FT_* effectif depuis le premier octet du nom de tag (compat gaps nommés eMule).
- **Kad — rôle de nœud de stockage** : `kad_udp/storage.rs`, store borné et expirant (TTL 5 h, 5 000 clés, 50 entrées/clé, 20 000 entrées) ; `PublishSourceReq` validé (IP émetteur + port TCP requis) et persisté, `SearchSourceReq`/`SearchKeywordReq` servis depuis le stockage avec pagination ; filtrage des entrées keyword malformées ; **republication périodique** des partages (source + mots-clés) vers les 20 contacts les plus proches dès que la table de routage est non vide puis toutes les 5 h (`KADEMLIAREPUBLISHTIMES`).
- **Fin de transfert propre** : traitement `OP_CANCELTRANSFER` (`0x56`) entrant (libération immédiate du slot upload) et `OP_OUTOFPARTS` (`0x57`) entrant (fin de session gracieuse) ; émission `OP_OUTOFPARTS` quand plus aucun bloc n'est à demander (`SendOutOfPartReqsAndPacket`) et `OP_CANCELTRANSFER` sur annulation en cours de transfert.
- **Upload compressé (`OP_COMPRESSEDPART 0x40`)** : service de blocs zlib (`Z_BEST_SPEED`, chunks ≤ 10 240 octets, format `hash16 + u32 start + u32 taille_zlib` par paquet `OP_EMULEPROT`) quand le pair annonce `nDataCompVer` (MiscOptions1 bits 20-23) ; fallback `SENDINGPART` si non rentable ; exclusions `ShouldCompressBasedOnFilename` (`zip/rar/7z/cbz/cbr/ogm/ace`, blocs ≤ 31 o) ; `has_compression_capability` lit désormais MiscOptions1 en plus d'`OP_EMULEINFO`.
- **Multi-slots upload** : `UploadManager` supporte jusqu'à `MAX_ACTIVE_UPLOADS` (10) slots concurrents (`active_uploads: HashMap`) avec promotion de la file pondérée par crédits à chaque libération ; `max_active` configurable ; `uploads.list` retourne tous les slots actifs.
- **Socket client-UDP (eD2K UDP)** : canal `PR_EMULE` partagé avec la socket Kad (multiplexage eMule) ; `OP_REASKFILEPING`/`OP_REASKACK`/`OP_FILENOTFOUND`/`OP_QUEUEFULL` (`0x90`–`0x93`) — inbound : réponse avec rang de file (`GetWaitingClientByIP_UDP`, part-status selon `m_byUDPVer` du demandeur), `OP_FILENOTFOUND`, `OP_QUEUEFULL` si file quasi pleine, silence sinon ; outbound : keep-alive ~21 min vers les sources `Queued` ayant annoncé un port UDP (`CT_EMULE_UDPPORTS` capturé au handshake), dedup par `(IP, hash)` ; rang appris via `OP_REASKACK` appliqué à la session après validation des reasks en attente (`UDPPacketPending`).
- **Scheduler global de bande passante** : les boucles de transfert download pacem les octets reçus via `PolicyRateLimiters.download_bucket` (brut et zlib, sessions sortantes et callback) ; l'upload existant acquiert désormais le bucket global par quanta de ≤10 240 octets (fairness inter-slots type `UploadBandwidthThrottler`) ; limites configurables via l'API `network` existante.
- **Serveur — liste, UDP et failover** : `OP_SERVERLIST` (`0x32`) décodé et fusionné dans `ed2k_servers` (préférence 0, entrées `ip:port` dédupliquées par upsert) ; nouveau `ServerUdpService` sur socket UDP dédiée (bind éphémère, comme `UDPSocket` d'eMule) avec codec `server_udp` — `OP_GLOBGETSOURCES2` (`0x94`) émis vers le serveur connecté (port UDP = TCP+4) et `OP_GLOBFOUNDSOURCES` (`0x9B`)/`OP_GLOBSERVSTATRES` (`0x97`) décodés, sources persistées comme `PeerSource` ; failover automatique dans le `SwarmCoordinator` — reconnexion vers un autre serveur `allowed` de la liste (préférence max, exclusion du serveur tombé, backoff 60 s global + 5 min par adresse) quand la connexion serveur tombe.
- **Republication `OP_OFFERFILES` périodique** : la liste des partages est réémise toutes les 30 min tant que la session serveur est active (capture du dernier `OP_IDCHANGE` pour `client_id`/`tcp_flags`) — les fichiers devenus partageables en cours de session sont publiés sans reconnexion ; un paquet `OP_OFFERFILES` vide sert de keep-alive (sémantique `KeepConnectionAlive` d'eMule).
- **Recherche UDP globale** (`OP_GLOBSEARCHREQ2`/`GLOBSEARCHRES`) : `servers.search` émet désormais la requête en TCP *et* en UDP (`0x92`, payload = expression de recherche réutilisée verbatim comme `SearchResultsWnd.cpp`) vers le serveur connecté + jusqu'à 9 serveurs `allowed` de la liste (port UDP = TCP+4) ; `OP_GLOBSEARCHRES` est décodé via le codec `OP_SEARCHRESULT` et émis comme notification `server.searchResult` identique au chemin TCP.
- **USS (Upload Speed Sense)** : `services/uss.rs` — limite d'upload dynamique pilotée par la latence, loi de contrôle fidèle à `LastCommonRouteFinder` (`soll_ping` tolérance % ×4 ou 200 ms, `hping` sur médiane glissante, dividers 1000 rampés sur 60 s, cadence `max(125 ms, 6400/upload)`, rebaseline après 60 échecs) ; sonde RTT par TCP connect vers le serveur connecté ou `pingHost` configuré (les raw sockets ICMP d'eMule exigeraient des droits admin) ; `accept_new_client` partagé avec `UploadManager` (`set_uss_gate`) — les nouveaux clients restent en file quand la latence dépasse la tolérance ; config persistée (`uss_config`), exposée et modifiable via `limits.get`/`limits.set` (`uss{...}`).
- **Identité upload par userhash** : la file d'attente et les slots upload sont désormais clés par `uh:<userhash>` (annonce `OP_HELLO`) au lieu de `ip:port` — un pair reconnecté depuis une autre IP conserve son rang, et des clients derrière une même IP/NAT restent des identités distinctes (dédoublonnage par identité comme `CUploadQueue`) ; `release_slot` accepte la clé *ou* l'adresse `ip:port` (compat RPC `uploads.cancel`), `waiting_rank_by_ip` (réponses `OP_REASKACK`) fonctionne toujours via le `client_address` stocké ; les crédits restent indexés par pubkey SUI.
- **Test de joignabilité UDP Kad (`KADEMLIA2_FIREWALLUDP` / `OP_FWCHECKUDPREQ`)** : sémantique `CUDPFirewallTester` — le daemon demande jusqu'à 2 contacts Kad vérifiés (version ≥ 6, port TCP connu, IP distinctes) via une connexion TCP eD2K (`OP_HELLO` annonçant `KadVersion=6` dans MiscOptions2, exigence `> KADEMLIA_VERSION5_48a`), envoie `OP_FWCHECKUDPREQ` (`0xA7`, PR_EMULE : `u16 intern` + `u16 extern` + `u32 key`), puis corrèle les datagrammes `0x62` (`u8 erreur` + `u16 port testé`) par IP candidate ; un paquet valide (`error=0`, port = intern/extern annoncé) → état **Open** (port externe NAT détecté → `use_extern`), tous les candidats en échec → **Firewalled**, aucun candidat → **Unverified**, budget global ~6 min ; paquets non sollicités ignorés (règle `bRequested` d'eMule) ; côté serveur, le dispatcher inbound répond aux `OP_FWCHECKUDPREQ` en émettant `FIREWALLUDP` vers le port interne puis externe du demandeur ; état exposé dans `kad.status` (`udpFirewallStatus`, `udpFirewallVerified`, `externUdpPort`, `effectiveUdpPort`) ; le `KadVersion` annoncé dans le hello normal passe de 2 à 6 (le daemon sert désormais le protocole de test) ; tests : codec 0x62, tracker (livraison/non-sollicité/IP erronée/port effectif), e2e loopback mock peer → `Open`.
- **Déchiffrement des réponses `FIREWALLUDP` (compatibilité WAN réelle)** : les pairs eMule chiffrent le datagramme de réponse avec le schéma `EncryptSendClient` d'`EncryptedDatagramSocket` — `RC4(MD5(receiverKey:u32 ‖ randKeyPart:u16))` à partir de l'octet 3, magic `0x395F2EC1`, header `[proto][rand:u16][magic:u32][pad][recvKey:u32][sendKey:u32]` — ce qui produisait des drops « invalid protocol byte » sur le WAN ; la boucle de réception `kad_udp` tente désormais le déchiffrement (clé = le sender key envoyé dans chaque `OP_FWCHECKUDPREQ`, conservée par le tracker) avant de rejeter un premier octet inconnu, et les paquets déchiffrés ré-intègrent le chemin de décodage normal ; la clé de vérification UDP est générée par IP cible selon `GetUDPVerifyKey` (MD5 de la clé Kad locale ‖ IP, XOR des mots, non nulle) ; tests e2e : le mock pair répond chiffré → `Open`.
- **Persistance du storage Kad** : les entrées indexées acceptées (`PublishSourceReq`/`PublishKeywordReq`) sont miroir-écrites dans `kad_sources_index`/`kad_keywords_index` (TTL 5 h) ; au démarrage de la socket Kad, les lignes expirées sont purgées puis le store mémoire est réhydraté via `list_all_kad_indexed_sources`/`list_all_kad_keyword_entries` en préservant le TTL restant — le rôle de nœud de stockage survit aux redémarrages.
- **Buddy callback Kad (LowID via relais Kad)** : implémentation des trois rôles eMule (`CClientList::GetBuddy` / `IncomingBuddy` / `TryToConnect CCS_KADCALLBACK`) —
  - *Relais* : `KADEMLIA_FINDBUDDY_REQ` (`0x51`) accepté uniquement si le nœud est vérifié non-firewallé (état `FIREWALLUDP` Open) et sans buddy actif → `KADEMLIA_FINDBUDDY_RES` (`0x5A`, `tcp_port` + options) ; enregistrement pending (TTL 120 s) ; adoption de la connexion TCP du demandeur comme *lien buddy* (`serve_buddy_link`) avec keepalive `OP_BUDDYPING`→`OP_BUDDYPONG` ; `KADEMLIA_CALLBACK_REQ` (`0x52`) relayé sur ce lien comme `OP_CALLBACK` (`0x99`, `check16+file16+ip4+port2`) ; `OP_REASKCALLBACKUDP` (`0x94`, socket client-UDP) relayé comme `OP_REASKCALLBACKTCP`.
  - *Demandeur* : `TAG_BUDDYHASH` (`0xF8`) extrait des réponses `SearchSourceRes` et persisté (`PeerSource.buddy_id`, colonne SQLite `buddy_id`) ; les sources Kad LowID (`tcp_port=0` + buddy) sont conservées sous l'adresse `kadcb:<ip>` ; le `SwarmCoordinator` résout le buddy dans la table de routage et émet `KADEMLIA_CALLBACK_REQ` ; les callbacks entrants sont adoptés par IP distante (`pending_kad_callbacks`, TTL 5 min) en session de téléchargement.
  - *Rappel serveur* : `OP_CALLBACKREQUESTED` entrant déclenche désormais une connexion sortante (`connect_back`) vers l'IP/port du demandeur — le handshake eD2K complet puis le service des requêtes d'upload, au lieu d'un simple log.
  - Tests : codec `0x51`/`0x52`/`0x5A` (roundtrip + tronqué), acceptation/rejet `FINDBUDDY_REQ` (non vérifié → refus silencieux), enregistrement pending, drop `CALLBACK_REQ` sans lien buddy.
- **Décompression des paquets serveur `OP_PACKEDPROT` (`0xD4`)** : les serveurs eD2K compressent fréquemment `OP_SEARCHRESULT`/`OP_FOUNDSOURCES` en zlib — le décodeur les rejetait (`CompressedProtocolUnsupported`), ce qui faisait apparaître ces serveurs comme muets et laissait la recherche UI en attente indéfinie. TCP : la session serveur décompresse le payload avant dispatch (`Packet::UnPackPacket` d'eMule, cap strict de 250 Ko, opcode préservé, proto restauré en `PR_ED2K`) ; UDP : `decode_server_udp` décompresse les datagrammes `0xD4` avec la même borne (`take(250_001)`). Les paquets mal formés/tronqués/trop gros sont loggés et ignorés sans casser la boucle de lecture. Tests : roundtrip zlib, corruption/troncature, cap anti-décompression-bomb (300 Ko → rejet, 250 Ko → accepté).
- **Connexion serveur obfusquée (DH768 + RC4, eMule `ECS_PENDING_SERVER`)** : `ObfuscatedStream::connect_server_outgoing` implémente le handshake client↔serveur dédié (distinct du handshake pair-à-pair par userhash) — envoi `[marker][G^A (96o)][padlen][pad]` en clair, réception de la clé publique DH du serveur, clés `RC4(MD5(S‖34))` send / `RC4(MD5(S‖203))` recv avec drop-1024, puis échange chiffré `[MAGICVALUE_SYNC][supported][requested][padlen][pad]` ↔ `[MAGICVALUE_SYNC][method][padlen][pad]`. Le `ServerSessionManager` extrait `ST_TCPPORTOBFUSCATION`/`ST_UDPPORTOBFUSCATION` (`0x97`/`0x98`) de `OP_SERVERIDENT` (persisté par adresse entre reconnections) et suit la sémantique `CServerSocket::ConnectTo` : tentative obfusquée sur le port dédié (ou le port normal si aucun n'est annoncé, une seule fois — `SetTriedCrypt`), fallback plaintext automatique sur échec du handshake, logs explicites du mode utilisé. Le login `OP_LOGINREQUEST` n'est envoyé qu'après la fin du handshake ; les helpers de la session sont généralisés au transport `ObfuscatedStream<TcpStream>` (transparent en plaintext). Configurable via `set_obfuscation_preference` (activé par défaut, padding configurable). Tests : handshake DH loopback complet (échange chiffré bidirectionnel), rejet propre d'une réponse non chiffrée, unitaires de décompression.
- **Recherche serveur : fin du blocage indéfini de l'UI** — un serveur acceptant la requête sans jamais répondre laissait `_isSearching` actif à vie (reset uniquement sur événement `server.searchResult` ou erreur RPC) ; un timeout de 30 s relâche désormais l'état avec un statut « terminée sans réponse », sans empêcher les résultats tardifs d'alimenter la liste.
- **Bootstrap Kad depuis les pairs entrants** : le seeding depuis les clients connectés n'existait que côté connexions sortantes ; les pairs **entrants** annonçant un port Kad (`CT_EMULE_UDPPORTS`) déclenchent désormais aussi `maybe_bootstrap_from_peer` (throttle 1×/min, table < 50 contacts).

### Ajouté & Corrigé — Secure User Identification (SUI)

- **Migration du `cryptkey.dat` legacy** : l'ancien blob binaire de 80 octets (identité factice des versions précédentes) est détecté (`LegacyKeyLoad::LegacyBlob`), quarantiné puis remplacé par une vraie clé RSA-384 avec log `WARN` explicite — au lieu de désactiver SUI définitivement. Un fichier de 80 octets *texte* reste traité comme corruption (`Invalid`, pas de régénération).
- **Interop réseau réel — ordre des paquets SUI** : les clients eMule réels envoient fréquemment `OP_SECIDENTSTATE (state=2)` avant leur `OP_PUBLICKEY`. Le challenge est désormais mis en attente et traité dès réception de la clé (outbound `OutboundSui` + inbound dispatcher), au lieu d'être jeté.
- **Signataire inbound** : `OP_SECIDENTSTATE` entrant est maintenant géré — un pair qui se connecte à nous et nous challenge reçoit notre `OP_PUBLICKEY` (si `state=2`) puis `OP_SIGNATURE` V1. `InboundListenerService` reçoit `IdentityService` via `set_identity`.
- **Observabilité SUI** : compteurs partagés `SuiStats` (challenges émis, signatures envoyées/vérifiées/rejetées, anti-flood, paquets malformés) exposés en lecture seule via RPC `identity.status` sous `sui`.

## [0.1.8] - 2026-09-19

Release de performance et de fidélité protocolaire eMule : agrégation multi-pairs des débits de téléchargement, gestion conforme des files d'attente distantes (Queue Ranking) et préservation des quotas d'essaim.

### Ajouté & Optimisé
- **Agrégation Multi-Pairs & Stabilisation du Débit (`transfer.rs`)** :
  - Calcul et synchronisation en temps réel du débit cumulé de téléchargement sur l'ensemble des sessions actives pour un même fichier (`sync_transfer_rates`).
  - Suppression des artefacts d'affichage à 0 B/s : `RateResetGuard` recalcule le débit restant lors de la déconnexion d'un pair au lieu d'écraser la vitesse globale à zéro.
  - Journalisation de télémétrie `[DL_PROGRESS]` périodique claire affichant le débit global cumulé et la contribution par pair actif.
- **Gestion Conforme des Files d'Attente Distantes (`peer_session.rs`)** :
  - Détection appropriée de la déconnexion en file d'attente (`Disconnected in queue`) : lorsque les pairs eMule ferment le socket TCP pour préserver leurs ressources tout en maintenant notre rang (`OP_QUEUERANKING`), la source reste en statut `Allowed`.
  - Application du délai de réinterrogation conforme eMule (10 à 15 minutes) au lieu d'une re-connexion immédiate agressive à 60s, évitant les pénalités et bannissements de file.
- **Protection du Plafond de Quota de l'Essaim (`swarm_coordinator/strategy.rs`, `types.rs`)** :
  - Intégration des sources en file d'attente (`Candidate`, `Tried`, `Allowed`) dans le calcul des sources viables, empêchant l'effondrement du plafond d'allocation water-filling.
- **Inbound Listener & Upload Queue (`dispatcher.rs`, `upload_queue.rs`)** :
  - Traitement complet des requêtes de téléversement entrantes (`OP_STARTUPLOADREQ`, `OP_ACCEPTUPLOADREQ`) et libération dynamique des slots d'envoi.
  - Logs de diagnostic stratégiques (`[UPLOAD_REQUEST]`, `[UPLOAD_SLOT_GRANTED]`, `[UPLOAD_RELEASED]`).
- **Cartographie Globale du Dépôt (`INDEX.md`)** :
  - Inventaire exhaustif et documentation de tous les fichiers versionnés du projet eMuleRust.

---

## [0.1.3-lab] - 2026-09-15

Release de consolidation majeure intégrant l'implémentation complète du protocole eD2K réel (Tier 3.1 à Tier 3.6), l'inspection détaillée et la Part Map (Phase 8.1), l'historique et statistiques locales avec exports caviardés (Phase 8.2), les catégories locales et la hiérarchie de destination à 4 niveaux (Phase 8.3), l'import défensif de fichiers `.part.met` eMule avec rapport d'audit (Phase 8.4), ainsi que le packaging multi-saveurs Windows et la migration de schéma SQLite (Phase 8.5).

> **Formulation d'attestation d'interopérabilité & confinement :**  
> *« eMuleRust est une application de laboratoire locale. Les transferts eD2K, la reprise, la vérification par parties et la finalisation ont été validés contre la référence eMule 0.50a dans un environnement strictement confiné à 127.0.0.1. Aucun serveur eD2K public, nœud Kad, peer externe, trafic UDP ni compression de protocole n’est activé. »*

### Ajouté
- **Protocole eD2K Loopback & Qualification eMule 0.50a (Tier 3.1 – 3.6)** :
  - Handshake complet eD2K et négociation d'extensions eMule (`ET_MOD_VERSION`, `ET_VERSION`, `ET_EXT_FEATURES`).
  - Déclaration de disponibilité de fichier (`FILE_STATUS_REQ` et `FILE_STATUS` avec bitfield par bloc et source complète).
  - Séquençage et planification bornée en blocs de 9,28 Mo décomposés en sous-plages de requêtes `REQUESTPARTS`.
  - Écriture en flux continu via `SENDINGPART` dans un fichier temporaire `.part`.
  - Contrôle d'intégrité cryptographique MD4 par bloc et calcul de l'empreinte racine eD2K globale.
  - Gestion du résidu de frontière (ex. 9 728 001 octets) et promotion atomique vers `downloads/`.
  - Reprise résiliente après arrêt brutal à ~50% avec persistance SQLite et reprise intégrale.
- **Inspection Détaillée & Part Map Interactive (Phase 8.1)** :
  - `DownloadDetailDialog` : identifiants complets, lien canonique `ed2k://`, métriques de transfert en direct.
  - Visualisation Part Map en blocs colorés avec tooltips interactifs et offsets exacts.
  - Export de diagnostics caviardés sans aucun jeton `auth.token`.
  - Modification de destination cible avec sonde d'écriture (`write-probe`).
- **Historique Local & Statistiques d'Activité (Phase 8.2)** :
  - Table SQLite `download_history` immuable avec index temporels et de types d'événements.
  - Tableau de bord `HistoryView` avec agrégats sur 4 fenêtres (Total, 7 jours, 24h, Session courante).
  - Politique de rétention configurable (7, 30, 90 jours ou conservation indéfinie) avec élagage automatique.
  - Exports caviardés JSON / CSV sans fuite de chemins privés.
- **Catégories Locales & Résolution de Destination à 4 Niveaux (Phase 8.3)** :
  - Table SQLite `download_categories` et association `category_id` dans `downloads`.
  - Invariant absolu de non-destruction : la suppression d'une catégorie réassocie les téléchargements à `category_id = NULL` sans effacer aucun fichier.
  - Résolution de destination hiérarchique : tâche > catégorie > global SQLite > bundle.
- **Import Défensif de Fichiers `.part.met` (Phase 8.4)** :
  - Décodage des formats binaires eMule standard (`0xE0`), grands fichiers (`0xE1`) et nouveaux formats (`0xE2`).
  - Inversion algorithmique des gaps pour calculer les plages valides réellement acquises.
  - Hachage cryptographique MD4 par bloc avec invalidation et ré-évaluation en trou des blocs corrompus.
  - Confinement strict sous `data/temp/<id>.part` et initialisation impérative en statut `Paused` (`sourceCount: 0`).
  - Rapport d'audit lisible avec bouton de copie dans le presse-papier et bouton de clôture.
- **Packaging Windows Multi-Saveurs & Migration SQLite (Phase 8.5)** :
  - Saveur `standard` épurée des outils de laboratoire et saveur `qa-loopback` instrumentée.
  - Migration de schéma SQLite idempotente et non destructive depuis Phase 7 certifiée par test unitaire.
  - Portabilité et mise à niveau sans perte des dossiers `data/` et `downloads/`.

---

## [0.1.2-lab] - 2026-09-14

Release de consolidation intégrant la séparation étanche `data/` vs `downloads/`, la persistance de la suppression SQLite, la sélection personnalisée du dossier de téléchargement, l'enrichissement des diagnostics, et le pipeline QA E2E automatisé Phase 7.3.

> **Formulation d'attestation d'interopérabilité :**  
> *« eMuleRust est une application de laboratoire locale. La compatibilité eD2K est validée sur corpus statique et pair de référence émulé en loopback fermé. Aucun serveur eD2K public, nœud Kad ou peer externe n’est contacté. »*

### Ajouté
- **Séparation Étanche de l'État Interne (`data/`) et des Fichiers Finalisés (`downloads/`)** :
  - `data/` contient exclusivement l'état interne : SQLite WAL (`mule.db`), session locale (`session.json`), jeton secret (`auth.token`), fichiers partiels éparses (`.part`).
  - `downloads/` héberge les fichiers finalisés et validés, avec création automatique si nécessaire.
  - En mode portable, le dossier par défaut de téléchargement est résolu vers `<bundleRoot>/downloads` distinct de `data/`.
- **Sélection Personnalisée du Dossier de Destination (Browse...)** :
  - Intégration du sélecteur de dossier natif dans la modale d'ajout de lien eD2K (`AddDownloadDialog`).
  - Prise en compte du chemin cible personnalisé dans la méthode RPC `downloads.addEd2kLink`.
- **Persistance de la Suppression de Téléchargement dans SQLite** :
  - La méthode RPC `downloads.remove` exécute un `DELETE FROM downloads WHERE id = ?` persisté dans SQLite.
  - Suppression physique automatique des fichiers d'état partiels associés (`.part` et `.part.met`).
  - Garantie de non-réapparition de la tâche après arrêt et redémarrage du démon (`test_downloads_remove_persistence_across_daemon_restart`).
- **Garantie d'Inviolabilité des Téléchargements lors du Nettoyage Lab** :
  - La méthode `lab.cleanupTestData` ne supprime que `<workdir>/fixtures/` et les fichiers incomplets temporaires ; les fichiers finalisés dans `downloads/` ne sont jamais altérés.
- **Ergonomie UI & Diagnostics Approfondis** :
  - Affichage sous forme de badges (`Chip`) des capacités actives annoncées par le démon (`downloads`, `download-events`, `lab-tools`).
  - Abréviation ergonomique des identifiants UUIDv4 (`#abc1...`) dans le tableau des téléchargements.
  - DiagnosticDialog enrichi avec masquage systématique des chemins sensibles et préservation du zéro-secret.
- **Phase 7.3 : Pipeline E2E QA Automatisé (`scripts/test_qa_loopback_e2e.ps1`)** :
  - Validation hermétique en environnement temporaire stérile hors-dépôt.
  - Couverture E2E complète : fixture nominale (1 Mo), fixture à la frontière de part (9 728 001 octets), arrêt brutal/reprise de téléchargement avec conservation des blocs déjà validés, et contrôle cryptographique final.

---

## [0.1.1-lab] - 2026-09-14

Release Candidate consolidée pour le mode portable (Phase 7.1) et les outils de laboratoire durcis (Phase 7.2).

### Ajouté
- **Mode Portable Hybride & Cascade Déterministe** : Résolution unifiée `--workdir` > `EMULERUST_WORKDIR` > `portable.flag` (`data/`) > `%LOCALAPPDATA%\eMuleRust` (avec alias déprécié `MULE_DATA_DIR`).
- **Double Verrouillage du Mode Lab** : Les fonctionnalités de laboratoire (`LabGuideDialog`, injection d'eD2K synthétique, nettoyage sécurisé) requièrent à la fois le mode portable et la feature compilée `lab-tools`.
- **Taxonomie JSON-RPC 2.0 Étendue** : Formalisation des codes `-32000` (AuthFailed), `-32001` (ProtocolMismatch), `-32002` (ForbiddenInInstalledMode), `-32003` (UnsafeCleanupTarget), `-32004` (FixtureDownloadMismatch), `-32005` (LoopbackPeerUnavailable), `-32010` (BusinessError).
- **Garantie Zéro Secret dans les Diagnostics** : Redaction systématique des jetons d'authentification (`auth.token`) et des clés privées dans `DiagnosticDialog`.
- **Politique PowerShell Durcie** : Transition stricte vers `-ExecutionPolicy RemoteSigned` (éradication de `-ExecutionPolicy Bypass`).

### Modifié
- **Épuration des Capacités Standard** : Suppression de `local-simulation` du build par défaut (`default = []`). Seules `["downloads", "download-events"]` sont annoncées par défaut.
- **Transparence de l'Inventaire du Bundle** : Transition de 22 à 21 fichiers dans `SHA256SUMS.txt` suite à l'adoption exclusive par Flutter Desktop de la version binaire `AssetManifest.bin` (remplaçant le format JSON redondant).

---

## [0.1.0-lab] - 2026-09-13

Première release interne de laboratoire pour **eMuleRust** (client moderne eD2K offline/loopback).
Cette version fige l'état de validation d'interopérabilité locale, le pipeline de stockage, le protocole borné et la suite d'outils de test.

### Résumé des Niveaux d'Attestation
- **Tier 1 (Corpus statique)** : **100 % Validé** — Décodage/encodage canonique et déterministe sur fixtures d'octets figées (`ed2k_corpus_tests.rs`).
- **Tier 2 (Pair de référence émulé)** : **100 % Validé** — Handshake nominal, négociation de capacités, requêtes/réponses de statut et transfert de blocs complets et résiduels contre un pair de référence local simulé (`ed2k_reference_interop_tests.rs`).
- **Tier 3 (Binaire externe épinglé)** : **Harnais disponible & audité** — Harnais d'orchestration hermétique prêt (`tier3_loopback.rs`), test live ignoré par défaut tant qu'un binaire eMule/aMule vérifié et épinglé n'est pas fourni.

> **Formulation d'attestation d'interopérabilité :**
> *« Compatibilité eD2K expérimentale validée sur corpus statique et pair de référence émulé, en environnement loopback fermé. Le harnais Tier 3 pour un binaire eMule/aMule externe épinglé est disponible, mais son test live reste ignoré par défaut tant qu’un binaire vérifié n’est pas fourni. »*

### Éléments Volontairement Absents
- **Zéro connexion réseau publique** : toutes les sockets sont rigoureusement confinées à `127.0.0.1` (loopback strict).
- **Zéro protocole Kad / Kademlia / DHT** : aucune requête DHT active ni nœud de routage Kad.
- **Zéro connexion serveur eD2K** : aucun échange TCP/UDP vers des serveurs d'indexation publics.
- **Zéro upload / seeding** : aucune publication ou envoi spontané de fragments à des tiers.
- **Zéro fuite d'identité hôte** : anonymisation stricte de toutes les traces wire et rapports CI.

---

### Ajouts & Nouveautés

#### Sécurité Visuelle Unicode & Normalisation
- Intégration de `unicode-normalization` pour l'application canonique de la forme Unicode NFC sur les chaînes manipulées.
- Fonction [`sanitize_unicode_for_display`](file:///d:/Projet/eMuleRust/crates/mule-protocol/src/ed2k/reference.rs) : conservation des valeurs brutes pour la comparaison octet par octet et échappement sécurisé en `\u{XXXX}` des caractères trompeurs pour les rapports CI et terminaux :
  - Overrides et isolats bidirectionnels (`U+202A`..=`U+202E`, `U+2066`..=`U+2069`, `U+061C`, `U+200E`, `U+200F`).
  - Caractères invisibles et espaces de largeur nulle (`U+200B`..=`U+200D`, `U+2060`, `U+FEFF`, `U+00AD`).
  - Caractères de contrôle non graphiques (`c.is_control()`).
- Détection et rejet automatique dans `TraceRecord::is_clean()` de toute trame contenant des bidi overrides ou des caractères invisibles non échappés.

#### Propriété Formelle de Non-Mutation
- Structure [`SessionSnapshot`](file:///d:/Projet/eMuleRust/crates/mule-protocol/src/ed2k/session.rs) et méthode `Ed2kSession::snapshot()` pour figer l'état protocolaire complet d'une session.
- Compteur de diagnostics isolé `unhandled_packets_count`.
- Garantie formelle et testée : le traitement d'une trame `Raw` (`0xC5 PR_EMULE`, opcode ambigu pour le profil actif, ou opcode inconnu `0xFE`) n'altère en aucun cas l'état de session, le tracker de statut (`status_tracker`), le tracker de blocs (`block_tracker`), la planification de téléchargement, la persistance SQLite, ou les compteurs de progression.

#### Schéma de Traces Normalisées Machine-Readable
- Enregistrements de traces normalisées avec métadonnées d'attestation au niveau racine :
  - `schemaVersion: 1`
  - `testTier: 2` (ou `3` pour Tier 3)
  - `referenceKind: "emulated-reference-peer"` (ou `"external-reference-binary"`)
  - `compatibilityProfile: "Emule050a"` | `"Amule233"` | `"LegacyEdonkey"` | `"UnknownStrict"`
  - `protocolByte`, `opcode`, `direction`, `payloadLength`, `outcome`.
- Écriture sécurisée des artefacts CI via `DivergenceReport::write_ci_report_secure` dans des répertoires temporaires isolés (`tempfile::tempdir()`), avec échec pré-vol empêchant toute création de fichier si une trace est compromise.

#### Profils de Compatibilité eD2K Explicites
- Définition de l'enum `Ed2kCompatibilityProfile` :
  - `Emule050a` : opcodes documentés eMule 0.50a (`0x47` RequestParts, `0x4F` SetReqFileId).
  - `Amule233` : profil isolé pour variantes aMule.
  - `LegacyEdonkey` : opcodes historiques eDonkey 2000 (`0x4B` RequestParts, `0x49` FileStatusReq).
  - `UnknownStrict` : rejet immédiat de toute ambiguïté (`AmbiguousOpcodeForProfile`).
- Matrice de décodage multi-profil et gestion des sentinelles wire (`(0,0)` et `(u32::MAX, u32::MAX)`).

#### Pipeline de Stockage & Démon Local
- Moteur de téléchargement par intervalles disjoints (`RangeSet`) avec allocation éparse (`part_file.rs`).
- Support des blocs résiduels (ex: fichier de 9 728 001 octets composé d'une part de 9 728 000 octets et d'un résidu de 1 octet).
- Démon local supervisé `mule-daemon` avec persistance SQLite WAL, verrouillage exclusif `fs4::FileExt::try_lock()` sur `mule.lock`, et jeton secret CSPRNG à 64 caractères hexadécimaux (`auth.token`).
- Superviseur Flutter Desktop avec découverte automatique de session (`session.json`), reconnexion par backoff exponentiel et gestion de course au démarrage.

---

### Commandes de Validation (100% Hors-Ligne)

```powershell
# 1. Vérification statique et compilation
cargo check --workspace --all-targets --all-features

# 2. Linter sans avertissement
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 3. Suite complète des 204 tests unitaires et d'intégration Rust (1 test Tier 3 live ignoré par défaut)
cargo test --workspace --all-features

# 4. Analyse statique Flutter Desktop
cd apps/desktop
flutter analyze

# 5. Suite des 31 tests Flutter Desktop
flutter test
```

### Décompte des Tests du Projet
- **Tests Rust** : **204 passés**, 1 ignoré (Tier 3 live), 0 échec.
- **Tests Flutter** : **31 passés**, 0 échec.
- **Total général actif** : **235 tests validés à 100 %**.
