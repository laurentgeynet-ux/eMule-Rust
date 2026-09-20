# Protocole RPC du daemon — JSON-RPC 2.0 sur WebSocket

Document de référence du contrat entre `mule-daemon` et ses clients
(Flutter desktop, mobile, web, `mule-cli`, MCP).

> Source de vérité du code : `crates/mule-daemon/src/rpc/`
> (`server.rs`, `methods.rs`, `types.rs`, `handlers/*`).

---

## 1. Transport

| Élément | Valeur |
|---|---|
| Endpoint | `ws://<host>:<port>/rpc` |
| Health check | `GET /health` → `{"status":"healthy","daemonVersion":"x.y.z"}` |
| Framing | un message texte WebSocket = une requête **ou** une réponse **ou** une notification JSON |
| Encodage | UTF-8 JSON |

Chaque requête entrante est dispatchée dans une tâche `tokio::spawn` :
les réponses peuvent arriver **dans n'importe quel ordre** → le client
doit corréler via `id`. Un canal `mpsc` de capacité 128 serialise les
messages sortants (réponses + events) ; un client lent est déconnecté
par backpressure.

## 2. Enveloppes (JSON-RPC 2.0)

```jsonc
// Requête
{"jsonrpc":"2.0","id":1,"method":"downloads.list","params":{}}

// Réponse succès
{"jsonrpc":"2.0","id":1,"result":{...}}

// Réponse erreur
{"jsonrpc":"2.0","id":1,"error":{"code":-32010,"message":"...","data":{...}}}

// Notification serveur (event, pas d'id)
{"jsonrpc":"2.0","method":"<event>","params":{...}}
```

- Une requête **sans `id`** est traitée comme une notification : aucune
  réponse n'est émise (`methods.rs` : `let req_id = req.id?`).
- `id` est un `serde_json::Value` : entier, string ou null acceptés.

## 3. Handshake et authentification

Premier appel obligatoire :

```jsonc
{"jsonrpc":"2.0","id":0,"method":"system.hello","params":{
  "protocolVersion": 1,
  "token": "<auth_token si configuré>"
}}
```

Réponse :

```jsonc
{"jsonrpc":"2.0","id":0,"result":{
  "daemonVersion": "0.1.9",
  "protocolVersion": 1,
  "instanceId": "<uuid>",
  "sessionId": "<uuid>",
  "capabilities": ["downloads","download-events","servers","kad-contacts", ...],
  "workdirMode": "...",
  "workdirPath": "...",
  "defaultDownloadDir": "..."
}}
```

Règles implémentées (`handle_hello`) :
- `protocolVersion != PROTOCOL_VERSION` → erreur `-32001` (protocol mismatch).
- Si `auth_token` est configuré côté daemon et `token` absent/erroné → `-32000`.
- Si `auth_token` est **vide**, la session est authentifiée d'office
  (`AtomicBool::new(auth_token.is_empty())`).
- Toute méthode autre que `system.hello` avant auth → `-32000`.
- Les events broadcast ne sont envoyés qu'aux sessions authentifiées.

`events.subscribe` n'est honoré que pour les sessions authentifiées
(intercepté dans `handle_socket` ; avant auth → `-32000`). Voir §6 pour
la sémantique de souscription par topics.

## 4. Codes d'erreur

| Code | Constante | Sens |
|---|---|---|
| -32700 | `CODE_PARSE_ERROR` | JSON mal formé |
| -32600 | `CODE_INVALID_REQUEST` | Enveloppe invalide |
| -32601 | `CODE_METHOD_NOT_FOUND` | Méthode inconnue |
| -32602 | `CODE_INVALID_PARAMS` | Params invalides (`data` peut détailler) |
| -32603 | `CODE_INTERNAL_ERROR` | Erreur interne |
| -32000 | `CODE_AUTH_FAILED` | Auth échouée / pas encore authentifié |
| -32001 | `CODE_PROTOCOL_MISMATCH` | `protocolVersion` incompatible |
| -32002 | `CODE_FORBIDDEN_IN_INSTALLED_MODE` | Action interdite en mode installé |
| -32003 | `CODE_UNSAFE_CLEANUP_TARGET` | Cleanup vers un chemin non autorisé |
| -32004 | `CODE_FIXTURE_DOWNLOAD_MISMATCH` | Fixture de test invalide |
| -32005 | `CODE_LOOPBACK_PEER_UNAVAILABLE` | Peer loopback indisponible |
| -32010 | `CODE_BUSINESS_ERROR` | Erreur métier générique (NotFound, Core, Session…) |

`DaemonError` → `JsonRpcError` : `InvalidParams`→-32602,
`NotFound`/`Business`/`Core`/`Session`→-32010, `Io`/`Internal`→-32603.

## 5. Catalogue des méthodes

Namespaces stables (voir `methods.rs` pour la liste exhaustive maintenue) :

| Namespace | Méthodes représentatives |
|---|---|
| `system.*` | `hello`, `status`, `stop`/`shutdown` |
| `downloads.*` | `list`, `get`, `addEd2kLink`, `pause`, `resume`, `remove`, `setDestination`, `setCategory`, `getSources`, `inspectPartMet`, `importPartMet`, `retryConnection` |
| `servers.*` | `list`, `connect`, `disconnect`, `status`, `search`, `querySources`, `importMet`, `updateFromUrl`, `setEnabled`, `delete`, `clear` |
| `sources.*` | `list`, `addManual`, `connect`, `disconnect`, `clear`, `status`/`swarmStatus` |
| `kad.*` | `status`, `startUdp`, `stopUdp`, `bootstrap`, `findNode`, `searchSources`, `searchKeywords`, `searchNotes`, `publish*`, `listContacts`, `importNodesDat` |
| `network.*` | `getPolicy`, `setProfile`, `enable`, `disconnectAll`, `getAuditLog`, `setBudgets` |
| `settings.*` / `limits.*` | `get`, `set`, `setRetentionDays`, `resetDownloadDir` |
| `shares.*` | `list`, `add`, `remove`, `setEnabled` |
| `uploads.*` / `listener.*` | `list`, `status`, `cancel` ; `start`, `stop`, `status` |
| `identity.*` / `credits.*` | `status`, `rotate` ; `list`, `stats` |
| `upnp.*` | `status`, `mapPorts`, `unmapPorts`, `setEnabled` |
| `filter.*` | `getStatus`, `importFile`, `setEnabled`, `clear` |
| `history.*` / `categories.*` | `list`, `stats`, `clear`, `export` ; CRUD + `reorder` |
| `events.*` | `subscribe` (filtrage par topics, voir §6) |
| `lab.*` / `ed2k.*` / `debug.*` | feature-gated (`lab-loopback`, `ed2k-loopback`, `debug-rpc`) — **ne pas documenter comme API publique** |

Conventions de nommage en vigueur : `namespace.verbe` en camelCase
(`downloads.addEd2kLink`). Garder cette convention pour toute nouvelle
méthode ; les alias existants (`downloads.retry`, `swarm.status`…) sont
maintenus pour compat.

## 6. Events (notifications serveur)

Les events passent par `state.event_tx` (broadcast tokio de `String`)
puis sont forwardés comme notifications JSON-RPC aux sessions
authentifiées. `DaemonEvent` (`services/events.rs`) est la source unique
du format wire : chaque variante connaît sa `method`, son `topic` de
souscription et sérialise son payload via `to_wire_json()` ; tous les
émetteurs passent par `DaemonEvent::send`. Format wire = méthode pointée :

| method | params | topic | Fréquence |
|---|---|---|---|
| `event.downloadUpdated` | `{sequence, download: DownloadSnapshot}` | `downloads` | haute (progression) |
| `event.peerSessionUpdated` | `{sequence, session: PeerSessionSnapshot}` | `peers` | moyenne |
| `server.sessionUpdated` | `{sequence, session: ServerSessionSnapshot}` | `servers` | faible |
| `upload.uploadsUpdated` | `{sequence, active: [UploadSessionSnapshot], queue: [UploadSessionSnapshot], uploadRateBps}` | `uploads` | par mutation + progression throttlée ~500 ms |
| `server.searchResult` | `{sequence, files: [...]}` | `search` | par recherche |
| `server.foundSources` | `{sequence, fileHash, sources}` | `search` | par recherche de sources |
| `upnp.statusChanged` | `UpnpStatus` | `network` | par action UPnP |
| `event.snapshotRequired` | `{skipped: u64}` | — (toujours forwardé) | rarissime (lag) |

Le champ `sequence` (u64 monotone partagé) permet au client de détecter
les trous et de resynchroniser via `*.list`/`*.status` — implémenté
dans `RpcDaemonClient` (gap → `listDownloads()` en arrière-plan).

**Souscription par topics** — `events.subscribe {topics: [...]}` est
intercepté dans `handle_socket` (filtre par session, pas par le daemon) :

- `topics` absent ou `null` → tout est forwardé (rétro-compat).
- `topics: []` ou liste → seuls les events dont le `topic` est dans la
  liste passent ; `event.snapshotRequired` (sans topic) passe toujours.
- La correspondance méthode → topic est définie dans
  `DaemonEvent::topic` (`services/events.rs`) ; les méthodes `Custom`
  non listées retombent sur le topic `other`.

**Resynchronisation** — quand une session consomme trop lentement et que
le broadcast buffer droppe des events (`RecvError::Lagged`), le daemon
émet `event.snapshotRequired {skipped}` : le client doit refaire un
snapshot complet (`downloads.list`, `servers.list`, …) plutôt que
compter sur le flux incrémental.

**À définir (roadmap)** :
- Throttling/échantillonnage côté serveur des events de progression
  (utile pour `apps/web` sur mobile data).

## 7. Versioning

- `PROTOCOL_VERSION` (u32, actuellement `1`) : incrémenté à chaque
  changement **cassant** (renommage, suppression, changement de type).
- Ajouts rétro-compatibles (nouvelle méthode, nouveau champ optionnel,
  nouvelle capability) : **sans** bump de version, annoncés via
  `capabilities` dans `system.hello` → le client feature-detecte.
- `daemonVersion` (semver du crate) : informationnel, ne pas parser
  pour la compat.

## 8. Sécurité / exposition réseau

- Le endpoint n'est pas TLS : pour `apps/web` en HTTPS il faudra `wss://`
  (terminaison TLS devant axum : reverse proxy, ou `rustls`).
- `auth_token` transmis en clair dans `system.hello` → acceptable en
  loopback/LAN, obligatoire derrière TLS si exposé.
- Prévoir une liste d'origines si le même port sert aussi des routes HTTP.
- Les handlers `lab.*`/`ed2k.*`/`debug.*` sont compilés sous features —
  s'assurer qu'aucun build de production ne les active.

## 9. Client Dart (`packages/mule_client`)

Le client partagé existe sous `packages/mule_client` (consommé par
`apps/desktop` via `path:`). Il expose `MuleDaemonClient` (interface
domaine), `RpcDaemonClient` (transport WS) et `FakeDaemonClient`.

Capacités implémentées : handshake `system.hello`, corrélation par `id`,
dispatch des notifications par `method`, gap-detection via `sequence`
avec resync, reconnexion automatique (backoff + re-`hello` + resync,
opt-in `autoReconnect`), keep-alive (`system.status` périodique →
close → reconnect), gestion de `event.snapshotRequired` → resync
complet, souscription `events.subscribe` par topics.

Widgets/UI partagés : `packages/ui_kit` (`AppTheme`, `Formatters`,
`PartAvailabilityBar`, …).
