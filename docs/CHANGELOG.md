# Journal des Modifications — Emule-Sion

Toutes les modifications notables de ce projet sont documentées dans ce fichier.
Le format est basé sur [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), et ce projet adhère à [Semantic Versioning](https://semver.org/lang/fr/).

## [Unreleased]

- **Desktop : notifications natives de fin de téléchargement** :
  `DownloadNotifier` (`apps/desktop/lib/core/download_notifier.dart`)
  souscrit à `downloadLifecycleEvents` et affiche un toast OS via
  `local_notifier` (Windows/Linux/macOS) sur `completed` (nom du fichier)
  et `failed` (nom + raison). No-op sur les autres plateformes ou si le
  notifier OS est indisponible.

- **Web : bouton "Changer de daemon"** : nouveau provider
  `onChangeDaemonProvider` (null par défaut — desktop/mobile). La sidebar
  affiche une action `link_off` quand il est défini ; `EmuleSionApp`
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
  uniquement composition root (router, `EmuleSionApp`, tests).


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
  `EmuleSionApp` est partagé ; `apps/desktop/lib/main.dart` devient un
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
  - Runners **Android** et **iOS** générés (`com.emulesion.app`) :
    `AndroidManifest.xml` avec permission `INTERNET` + intent-filter
    `ed2k`, `Info.plist` avec `CFBundleURLSchemes` `ed2k`, nom d'app
    `Emule-Sion`. Les services desktop (`WindowSetup`, `TraySetup`) sont
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
  dans `EmuleSionApp` (`muleDaemonClientProvider`,
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
  - Inventaire exhaustif et documentation de tous les fichiers versionnés du projet Emule-Sion.

---

## [0.1.3-lab] - 2026-09-15

Release de consolidation majeure intégrant l'implémentation complète du protocole eD2K réel (Tier 3.1 à Tier 3.6), l'inspection détaillée et la Part Map (Phase 8.1), l'historique et statistiques locales avec exports caviardés (Phase 8.2), les catégories locales et la hiérarchie de destination à 4 niveaux (Phase 8.3), l'import défensif de fichiers `.part.met` eMule avec rapport d'audit (Phase 8.4), ainsi que le packaging multi-saveurs Windows et la migration de schéma SQLite (Phase 8.5).

> **Formulation d'attestation d'interopérabilité & confinement :**  
> *« Emule-Sion est une application de laboratoire locale. Les transferts eD2K, la reprise, la vérification par parties et la finalisation ont été validés contre la référence eMule 0.50a dans un environnement strictement confiné à 127.0.0.1. Aucun serveur eD2K public, nœud Kad, peer externe, trafic UDP ni compression de protocole n’est activé. »*

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
> *« Emule-Sion est une application de laboratoire locale. La compatibilité eD2K est validée sur corpus statique et pair de référence émulé en loopback fermé. Aucun serveur eD2K public, nœud Kad ou peer externe n’est contacté. »*

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
- **Mode Portable Hybride & Cascade Déterministe** : Résolution unifiée `--workdir` > `EMULE_SION_WORKDIR` > `portable.flag` (`data/`) > `%LOCALAPPDATA%\EmuleSion` (avec alias déprécié `MULE_DATA_DIR`).
- **Double Verrouillage du Mode Lab** : Les fonctionnalités de laboratoire (`LabGuideDialog`, injection d'eD2K synthétique, nettoyage sécurisé) requièrent à la fois le mode portable et la feature compilée `lab-tools`.
- **Taxonomie JSON-RPC 2.0 Étendue** : Formalisation des codes `-32000` (AuthFailed), `-32001` (ProtocolMismatch), `-32002` (ForbiddenInInstalledMode), `-32003` (UnsafeCleanupTarget), `-32004` (FixtureDownloadMismatch), `-32005` (LoopbackPeerUnavailable), `-32010` (BusinessError).
- **Garantie Zéro Secret dans les Diagnostics** : Redaction systématique des jetons d'authentification (`auth.token`) et des clés privées dans `DiagnosticDialog`.
- **Politique PowerShell Durcie** : Transition stricte vers `-ExecutionPolicy RemoteSigned` (éradication de `-ExecutionPolicy Bypass`).

### Modifié
- **Épuration des Capacités Standard** : Suppression de `local-simulation` du build par défaut (`default = []`). Seules `["downloads", "download-events"]` sont annoncées par défaut.
- **Transparence de l'Inventaire du Bundle** : Transition de 22 à 21 fichiers dans `SHA256SUMS.txt` suite à l'adoption exclusive par Flutter Desktop de la version binaire `AssetManifest.bin` (remplaçant le format JSON redondant).

---

## [0.1.0-lab] - 2026-09-13

Première release interne de laboratoire pour **Emule-Sion** (client moderne eD2K offline/loopback).
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
- Fonction [`sanitize_unicode_for_display`](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/ed2k/reference.rs) : conservation des valeurs brutes pour la comparaison octet par octet et échappement sécurisé en `\u{XXXX}` des caractères trompeurs pour les rapports CI et terminaux :
  - Overrides et isolats bidirectionnels (`U+202A`..=`U+202E`, `U+2066`..=`U+2069`, `U+061C`, `U+200E`, `U+200F`).
  - Caractères invisibles et espaces de largeur nulle (`U+200B`..=`U+200D`, `U+2060`, `U+FEFF`, `U+00AD`).
  - Caractères de contrôle non graphiques (`c.is_control()`).
- Détection et rejet automatique dans `TraceRecord::is_clean()` de toute trame contenant des bidi overrides ou des caractères invisibles non échappés.

#### Propriété Formelle de Non-Mutation
- Structure [`SessionSnapshot`](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/ed2k/session.rs) et méthode `Ed2kSession::snapshot()` pour figer l'état protocolaire complet d'une session.
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
