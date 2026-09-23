# EMule-Rust — Desktop

Application Flutter (Windows/Linux) pilotant le daemon `mule-daemon`
via JSON-RPC/WebSocket. Client et design system vivent dans les packages
partagés `packages/mule_client` et `packages/ui_kit`.

## Lancer

```bash
flutter pub get
flutter run -d windows          # ou -d linux
```

Sans options, l'app démarre/supervise le daemon local
(`DaemonSupervisor` : découverte `session.json`, auth via `auth.token`,
spawn du binaire `mule-daemon` si absent, reconnexion backoff).

## Modes de développement

```bash
# UI avec daemon simulé en mémoire (aucun processus Rust requis)
flutter run -d windows --dart-define=USE_FAKE_DAEMON=true

# Activer la barre d'outils de simulation (injectRange, lab loopback)
flutter run -d windows --dart-define=ENABLE_DEV_TOOLS=true
```

## Structure

```
lib/
├── main.dart                        # bootstrap, choix fake/rpc, lifecycle
├── services/
│   ├── daemon_supervisor.dart       # spawn/découverte/auth/reconnect daemon
│   └── work_directory_resolver.dart # Portable vs Installed modes
└── ui/features/
    ├── downloads/                   # liste, sidebar, dialogs, view model
    ├── search/                      # recherche server/UDP global/Kad
    ├── servers/                     # gestion serveurs eD2K
    ├── kad/                         # statut DHT
    ├── history/                     # historique & statistiques
    └── settings/
```

Le code client RPC, les modèles de domaine et le fake sont dans
`packages/mule_client` ; thème/formatters/widgets réutilisables dans
`packages/ui_kit`. Ne pas réintroduire ces couches ici.

## Tests

```bash
flutter test            # 90+ tests : widget, view model, e2e daemon réel
dart analyze lib test
```

`test/integration/live_daemon_e2e_test.dart` compile et lance un vrai
daemon Rust en loopback.

## Layout adaptatif

Les écrans répondent à trois largeurs : sidebar complète (≥ 1100 px),
rail compact, layout empilé (< 750 px). Les `Row` à segments larges
sont scrollables horizontalement en mode compact.
