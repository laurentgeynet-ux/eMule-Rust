# mule_client

Client daemon partagé pour les applications Emule-Sion (desktop, mobile, web).

Contient le contrat d'interface `MuleDaemonClient`, le transport
`RpcDaemonClient` (JSON-RPC 2.0 sur WebSocket), le `FakeMuleDaemonClient`
pour les tests/dev, tous les modèles de domaine et les DTOs du protocole.

## Usage

```dart
import 'package:mule_client/mule_client.dart';

final client = RpcDaemonClient(
  autoReconnect: true,                      // retry auto (mobile/web)
  keepAliveInterval: Duration(seconds: 20), // ping system.status
);

await client.connect(
  url: Uri.parse('ws://127.0.0.1:4660/rpc'),
  token: '<auth.token>',
);

final downloads = await client.listDownloads();
client.downloadEvents.listen((e) => print('${e.download.fileName} ${e.sequence}'));
```

## Protocole

Voir `docs/architecture/rpc_protocol.md`. Résumé :

- Endpoint `ws://<host>:<port>/rpc`, JSON-RPC 2.0, corrélation par `id`
- Handshake `system.hello {protocolVersion, token}` → capabilities +
  `DaemonInfo` (version, `instanceId`, `sessionId`)
- Notifications : `event.downloadUpdated`, `event.peerSessionUpdated`,
  `server.sessionUpdated`, `server.searchResult`, `server.foundSources`,
  `event.snapshotRequired`
- Filtrage : `events.subscribe {topics: ["downloads","peers","servers","search"]}`

## Comportements transport

| Mécanisme | Détail |
|---|---|
| Gap detection | `params.sequence` monotone ; trou → `listDownloads()` en arrière-plan |
| `snapshotRequired` | Émis quand le broadcast buffer lag → resync complet |
| Reconnexion | `autoReconnect: true` → backoff 0.5s→10s, re-`hello`, resync. Laisser `false` quand un superviseur externe gère le cycle de vie (desktop `DaemonSupervisor`) |
| Keep-alive | `system.status` périodique ; échec → close → chemin de reconnexion |
| Timeout | 15 s par appel RPC |

## Dev

```bash
flutter pub get
dart analyze lib test
flutter test
```

Le `FakeMuleDaemonClient(enableAutoSimulation: true)` simule un daemon
complet en mémoire — utile pour les widget tests et les démos UI.
