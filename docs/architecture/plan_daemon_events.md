# Plan — Événements daemon & données temps réel

> Objectif : éliminer le polling et les valeurs simulées côté UI en exposant
> les événements manquants du daemon. Référence : `rpc_protocol.md` §6.
>
> Règle transverse : tout nouvel événement passe par `DaemonEvent`
> (`services/events.rs`) avec `method()` + `topic()` explicites, et le
> protocole est documenté dans `rpc_protocol.md`.

## Phase E1 — Uploads temps réel (rang 1) — ✅ livrée

### Daemon (Rust)

- [ ] `DaemonEvent::UploadSessionUpdated { sequence, session }`
      (`UploadSessionSnapshot` existe déjà dans `services/upload_queue.rs`)
  - méthode `upload.sessionUpdated`, topic `uploads`
- [ ] `DaemonEvent::UploadsUpdated { sequence, uploads, queue }`
      (résumé liste active + file) — méthode `upload.uploadsUpdated`
- [ ] Émettre depuis `UploadManager` : démarrage, progression périodique
      (1 s), fin, annulation, entrée/sortie de file
- [ ] Ajouter `upload_rate_bps` agrégé au snapshot d'état global
      (somme des `upload_rate_bps` actifs — calcul déjà fait dans
      `inbound_listener.rs:227`)

### mule_client (Dart)

- [ ] `Stream<UploadSessionModel> get uploadSessionEvents`
- [ ] `Stream<List<UploadSessionModel>> get uploadsEvents` (ou snapshot)
- [ ] Fake client : simuler une session upload périodique

### app_shell (UI)

- [ ] `UploadsController` (`view_models/uploads_controller.dart`) +
      `uploadsControllerProvider`
- [ ] `TransferRateTracker.uploadRate` alimenté par les events (remplace
      `() => 0` dans `AppController`)
- [ ] Vue "Uploads" (nouvelle section `AppNavSection.uploads` + route
      `/uploads`) : sessions actives (client, fichier, débit, état) + file
      d'attente avec rang/score
- [ ] Badge UL de la top bar branché sur le débit réel

## Phase E2 — Tick de stats globales (rang 2) — ✅ livrée

### Daemon

- [ ] `DaemonEvent::StatsTick { sequence, stats }` — méthode
      `event.statsTick`, topic `stats`, cadence 1–2 s
  - payload : `downloadBps`, `uploadBps`, `peersConnected`,
    `downloadsActive`, `serverConnected`, `kadContacts`,
    `kadUdpBound`
- [ ] Timer dans `state.rs` ou service dédié ; suppression si aucun
      abonné au topic `stats` (économie CPU)

### mule_client

- [ ] `Stream<DaemonStatsModel> get statsEvents` + modèle `DaemonStatsModel`
- [ ] Fake : sinusoïde/burst simulé

### app_shell

- [ ] `StatsController` exposant le dernier tick + `TransferRateTracker`
      alimenté par `statsEvents` quand disponible (fallback sampling local)
- [ ] Top bar : badges serveur/Kad rafraîchis par le tick
- [ ] Extension future : mini-dashboard d'accueil

## Phase E3 — Cycle de vie des downloads (rang 3) — ✅ livrée

### Daemon

- [ ] `DaemonEvent::DownloadAdded { sequence, download }`
      (`event.downloadAdded`, topic `downloads`)
- [ ] `DaemonEvent::DownloadRemoved { sequence, id }`
      (`event.downloadRemoved`)
- [ ] `DaemonEvent::DownloadCompleted { sequence, download }`
      (`event.downloadCompleted`) — transition status→completed
- [ ] `DaemonEvent::DownloadFailed { sequence, download, reason }`
      (`event.downloadFailed`)

### mule_client

- [ ] Streams typés correspondants ; `DownloadItem` inchangé (snapshot
      réutilisé dans les events)

### app_shell

- [ ] `downloads_store_controller` : appliquer Added/Removed sans diff
      de `listDownloads`
- [ ] `Completed`/`Failed` → notification tray (`tray_manager` message)
      sur desktop + snackbar/toast sur mobile/web
- [ ] Sidebar counts réactifs immédiats

## Phase E4 — Statuts pollés → événements (rang 4) — ✅ livrée

### Daemon

- [ ] `kad.statusChanged` (topic `kad`) — émis sur changement
      ID/contacts/buckets/état UDP
- [ ] `history.entryAdded` (topic `history`)
- [ ] `settings.changed` (topic `settings`)
- [ ] `network.policyChanged` (topic `network`) — profil, kill switch,
      budgets

### mule_client

- [ ] Streams : `kadStatusEvents`, `historyEvents`, `settingsEvents`,
      `networkPolicyEvents`

### app_shell

- [ ] `KadController` : supprimer le timer 3 s, écouter
      `kadStatusEvents` (+ fetch initial)
- [ ] `HistoryController` : insérer l'entrée sur `entryAdded`
- [ ] `SettingsController`/`NetworkPolicyController` : appliquer les
      events (multi-clients synchronisés)

## Phase E5 — Détail par pair (rang 5, optionnel)

- [ ] `download_rate_bps` par `PeerSessionSnapshot` (débit par source)
- [ ] Tableau des sources du détail download avec débit live
- [ ] `etaSeconds` dans `DownloadSnapshot` léger (évite l'appel detail)

## Ordre de mise en œuvre

E1 → E2 → E3 → E4 (E5 en bonus). Chaque phase = 3 commits :
daemon (Rust + tests), client Dart (modèles + streams + fake), UI
(controller + widgets + tests). Vérifications : `cargo test` daemon,
`flutter analyze` + `flutter test` (desktop), `flutter build web`.
`rpc_protocol.md` mis à jour à chaque phase (méthode, topic, payload).
