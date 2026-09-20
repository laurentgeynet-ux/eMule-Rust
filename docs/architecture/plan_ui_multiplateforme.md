# Plan UI/UX multiplateforme — Emule-Sion

Objectif : passer de l'UI de test actuelle à une application sérieuse
**Windows / Linux / Android / iOS / Web**, toutes pilotées par le même
daemon Rust via JSON-RPC/WebSocket (`rpc_protocol.md`).

Principe directeur : **le daemon est le produit, les apps sont des
clients interchangeables** partageant `packages/mule_client`.

---

## Phase 0 — Fondation (✅ fait)

- [x] `packages/mule_client` extrait : interface `MuleDaemonClient`,
      `RpcDaemonClient` (WS), `FakeMuleDaemonClient`, modèles, `RpcDto`.
- [x] Protocole documenté : `docs/architecture/rpc_protocol.md`.

## Phase 1 — Consolidation de `mule_client`

But : faire du package une brique fiable avant de construire dessus.

| Tâche | Détail | Statut |
|---|---|---|
| Reconnexion auto | `RpcDaemonClient` : backoff exponentiel, re-`system.hello`, resync après reconnexion (opt-in `autoReconnect`, desktop déjà couvert par `DaemonSupervisor`) | ✅ |
| Ping/périodicité | `keepAliveInterval` : `system.status` périodique, échec → close → reconnect | ✅ |
| `events.subscribe` réel | Param `topics` filtré par session dans `handle_socket` (`downloads`/`peers`/`servers`/`search`) | ✅ |
| `Lagged` broadcast | `RecvError::Lagged` → notification `event.snapshotRequired`, gérée côté client via `_resync()` | ✅ |
| `DaemonEvent` | Refait : source unique du format wire (`method`, `topic`, `send`) ; tous les émetteurs l'utilisent | ✅ |
| Tests du package | `test/data/*` déplacés vers `packages/mule_client/test` | ✅ |

Critère de sortie : le client survit à une déconnexion daemon sans
intervention UI ; aucun event perdu silencieusement.

## Phase 2 — Design system (`packages/ui_kit`)

But : un seul langage visuel pour les 4 plateformes + web.

- [x] Créer `packages/ui_kit` (Flutter package, dépend de flutter/material)
- [x] Y déplacer `ui/core/theme.dart` (`AppTheme`), `formatters.dart`, `strings.dart`
- [x] Y déplacer `PartAvailabilityBar` ; à suivre : `DownloadRow`, pills de statut
- [ ] **Design tokens** : couleurs, espacements, rayons, typographies en
      constantes (`AppColors`, `AppSpacing`, `AppRadius`) — fini les
      valeurs magiques dispersées
- [ ] **Thème Material 3** : `ColorScheme.fromSeed` + déclinaison dark
      (actuel) et light (à créer) ; `ThemeMode` configurable
- [ ] **Typographie** : échelle fixe (11/12/13/14/16/20/24) ; monospace
      pour hash/ID via `fontFamily`
- [ ] **Composants canoniques** :
  - `StatusPill` (Downloading/Completed/Paused/Failed + couleurs)
  - `MetricTile` (débit ↑↓, sources)
  - `EmptyState`, `ErrorState`, `LoadingShimmer`
  - `SectionCard` (cartes du detail dialog)
  - `ResponsiveScaffold` (sidebar → rail → bottom nav selon largeur)

Critère de sortie : `apps/desktop` n'importe plus aucune couleur/typo
en dur ; tout passe par `ui_kit`.

## Phase 3 — Navigation & structure d'app

| Tâche | Détail |
|---|---|
| `go_router` | Routes : `/downloads`, `/downloads/:id`, `/search`, `/servers`, `/kad`, `/history`, `/settings`. Deep links prêts pour le web |
| Shell adaptatif | `ResponsiveScaffold` : sidebar (≥1100px) / NavigationRail (700–1100) / NavigationBar bottom (<700) |
| `url_strategy` | `usePathUrlStrategy()` pour le web (URLs sans `#`) |
| ed2k:// links | Enregistrer le handler de protocole (Windows registry, Linux .desktop) + `app_links` pour mobile/web |

## Phase 4 — Migration Riverpod (state management)

Remplacer `ChangeNotifier` + `ListenableBuilder` par Riverpod, **un
feature à la fois**, en gardant les tests verts.

| Étape | Détail |
|---|---|
| Bootstrap | `ProviderScope` dans `main.dart` ; provider `muleDaemonClientProvider` (fake vs rpc via `dart-define`, même logique qu'actuellement) |
| Connexion | `connectionProvider` : `StreamProvider<DaemonConnectionState>` ; `daemonInfoProvider` |
| Downloads | `downloadsProvider` : `AsyncNotifier` consommant `downloads.list` + `event.downloadUpdated` (le plus gros : `downloads_view_model.dart` 1779 lignes → découper en providers par sous-domaine) |
| Recherche | `searchProvider` + `searchResultsProvider` |
| Servers/Kad | `serversProvider`, `kadProvider` |
| Settings | `settingsProvider` |

Règle : les widgets ne parlent **jamais** au client directement —
toujours via providers. Les view models actuels deviennent des
`Notifier` fins déléguant à `mule_client`.

## Phase 5 — Internationalisation

- [ ] `flutter gen-l10n` : `l10n/app_fr.arb` + `app_en.arb`
- [ ] Externaliser tous les strings hardcodés (`ui/core/strings.dart` →
      arb) : "Rechercher", tooltips, labels de statut, dialogs
- [ ] `MaterialApp(localizationsDelegates, supportedLocales)` + choix de
      langue dans settings

## Phase 6 — Desktop (Windows + Linux) — app de référence

C'est l'app la plus aboutie ; la finir en premier.

| Tâche | Package | Détail |
|---|---|---|
| Fenêtre | `window_manager` | taille min (900×600), mémoriser géométrie, titre dynamique |
| Tray | `tray_manager` | icône, menu (Ouvrir/Quit.), "fermer → tray" optionnel |
| Démon local | existant `DaemonSupervisor` | spawn/arrêt du daemon, afficher état dans sidebar (déjà fait : "Daemon Connected") |
| Fichiers | `file_picker` | import `.met`, `.part.met`, choix dossier destination |
| Notifs | `flutter_local_notifications` | "Téléchargement terminé" |
| Graphes | `fl_chart` | courbe débit up/down temps réel (dashboard eMule) |
| Intégrité | — | badge version protocole/daemon (déjà : "v0.1.9 (proto v1)") + `incompatible_version_dialog` sur `-32001` |

Linux : vérifier `tray_manager` + AppIndicator ; empaquetage
(`flutter_distributor` → .deb/.AppImage) en phase release.

## Phase 7 — Mobile (Android + iOS)

Contrainte majeure : **le daemon ne tourne pas sur le téléphone**
(background iOS quasi impossible, batterie). L'app mobile pilote un
daemon **distant** (PC/serveur de l'utilisateur).

| Tâche | Détail |
|---|---|
| `apps/mobile` | `flutter create`, dep `mule_client` + `ui_kit` |
| Écran de connexion | liste de daemons sauvegardés (`shared_preferences`/`flutter_secure_storage` pour le token), scan LAN optionnel |
| UI adaptée | `ResponsiveScaffold` gère déjà la taille ; bottom nav ; actions par swipe/long-press |
| Pull-to-refresh | `RefreshIndicator` sur listes |
| Sécurité | `wss://` obligatoire côté daemon + token ; jamais de daemon embarqué en v1 |

## Phase 8 — Web (`apps/web`)

Pilote le daemon à distance depuis un navigateur.

| Tâche | Détail |
|---|---|
| `apps/web` | `flutter create --platforms web`, deps `mule_client` + `ui_kit` |
| TLS | le navigateur exige `wss://` en contexte HTTPS → reverse proxy (nginx/caddy) devant axum, ou `rustls` dans le daemon |
| Auth | page login → `system.hello` token ; session token en `flutter_secure_storage` (web : localStorage chiffré limité, préférer mémoire) |
| CORS | axum `CorsLayer` restreint à l'origine du frontend |
| Perf | `events.subscribe` avec topics (Phase 1) pour limiter la bande passante ; canvaskit vs skwasm selon perf mesurée |

## Phase 9 — Qualité & pipeline

- [ ] CI : `dart analyze` + `flutter test` (desktop) + `cargo test` +
      `cargo clippy` sur chaque push
- [ ] Golden tests (`flutter_test` golden) pour `ui_kit`
- [ ] Tests d'intégration : `live_daemon_e2e_test.dart` déjà présent —
      l'étendre aux parcours clés (add link → download → complete)
- [ ] `dart doc`/`dartdoc` sur `mule_client` (doc de l'interface)
- [ ] Versioning synchronisé : version app = `daemonVersion` attendu ;
      refus de connexion sur `-32001` (déjà géré par
      `incompatible_version_dialog.dart`)

## Phase 10 — Release

| Cible | Livrable |
|---|---|
| Windows | `flutter build windows` → MSIX/installateur (innosetup) |
| Linux | `flutter_distributor` → .deb / .AppImage / snap |
| Android | `flutter build apk` / appbundle → Play Store (ou sideload) |
| iOS | `flutter build ipa` → TestFlight/App Store (nécessite macOS) |
| Web | `flutter build web` → hébergement statique + reverse proxy daemon |

---

## Découpage des packages (état cible)

```
packages/
├── mule_client/     ✅ client RPC + fake + modèles + DTO
├── ui_kit/          🔲 design tokens, thème, composants
└── (futur) mule_l10n/ si les strings doivent être partagées

apps/
├── desktop/         Windows + Linux (app de référence)
├── mobile/          🔲 Android + iOS (pilote distant)
└── web/             🔲 pilote distant navigateur
```

## Risques / points d'attention

1. **iOS background** : aucune chance de faire tourner un client P2P en
   arrière-plan → mobile = télécommande du daemon, assumé dès le départ.
2. **`wss://` + auth** : prérequis avant toute exposition du daemon hors
   loopback ; documenté dans `rpc_protocol.md` §8.
3. **Events haute fréquence** : `downloadUpdated` peut spammer ; le
   filtrage/throttling serveur (Phase 1) est un prérequis pour web/mobile.
4. **Tests UI sensibles à la largeur** : les overflow à 980px corrigés
   montrent que les breakpoints doivent être testés sur les 3 tailles du
   `ResponsiveScaffold`.
5. **Ne pas réécrire l'UI en parallèle** : migrer feature par feature,
   garder `flutter test` vert à chaque étape.
