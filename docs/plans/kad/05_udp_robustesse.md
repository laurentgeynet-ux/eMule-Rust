# Étape 5 — Robustesse UDP

> **Dépendances** : étape 4 (le calcul `Degraded` consomme la taxonomie d'erreurs créée ici).
> **Risque interop** : nul — tout est local (classification, retry sortant, matching, fenêtre de collecte).

## 5a. Taxonomie d'erreurs

`kad_udp/types.rs` :

```rust
#[derive(Debug, thiserror::Error)]
pub enum KadTxError {
    #[error("timeout")] Timeout,
    #[error("policy denied: {0}")] PolicyDenied(String),
    #[error("socket io: {0}")] SocketIo(String),
    #[error("malformed response")] Malformed,
    #[error("cancelled")] Cancelled,
}
```

Les `send_*` (`routing_client.rs`, `search_client.rs`, `publish_*`) retournent `Result<T, KadTxError>` au lieu de `String`. **Règle d'usage** dans les appelants (`lookup.rs`, `bootstrap_*`, verifier) :

| Erreur | Action sur le contact |
|---|---|
| `Timeout`, `Malformed` | `mark_failed` (le pair est fautif) |
| `SocketIo`, `PolicyDenied`, `Cancelled` | **rien** — faute locale, ne jamais pénaliser le pair |

## 5b. Fenêtre de collecte glissante

`search_client.rs:29-47` — `collect_search_batches` :

```rust
// Avant : timeout(SEARCH_RESP_IDLE_WINDOW, rx.recv()) — fixe
// Après : deadline globale + idle réarmé à chaque paquet valide
let deadline = Instant::now() + SEARCH_COLLECT_DEADLINE;
loop {
    match timeout_at(deadline.min(now + SEARCH_RESP_IDLE_WINDOW), rx.recv()).await {
        Ok(Some(Ok(msg))) if let Some(more) = extract(msg) => { acc.extend(more); /* idle réarmé par la boucle */ }
        ...
    }
}
```

Constantes : `SEARCH_RESP_IDLE_WINDOW = 350 ms` (inchangé, mais devient *glissant*), `SEARCH_COLLECT_DEADLINE = 2 s` (plafond absolu), caps `12` paquets / `300` entrées inchangés.

## 5c. Retry borné avec jitter

`routing_client.rs` — helper interne `send_with_retry(fut_factory)` : **une** re-tentative après `RETRY_DELAY + jitter` (`RETRY_JITTER_MS = 400`) uniquement sur `Timeout`, uniquement pour `ping` / `send_bootstrap` / `send_find_node`. **Jamais** sur `SEARCH_*`/`PUBLISH_*` (eMule ne répond pas quand il n'a rien → retry inutile ; amplification).

Attention : chaque tentative = nouvelle transaction (`tx_id`, `InFlightTxGuard`) — ne pas réutiliser le même `tx_id`.

## 5d. Matching de transactions durci

`kad_udp.rs` boucle de réception (~l.478-519) — aujourd'hui matching par IP seule :

1. Chercher d'abord les transactions avec `target == SocketAddr exact (ip, port)` + `expected_response` + `expected_target_hash` correspondants.
2. Fallback `(ip, opcode, hash)` seulement si **un seul** candidat (NAT change le port source — eMule tolère).
3. Sinon : drop + `tracing::warn!(kad.tx_ambiguous = ...)` — ne jamais router arbitrairement.

**Pas de nonce** : Kad2 ne transporte aucun nonce arbitraire — la corrélation reste `(ip, port, opcode, target_hash)`.

## Tests

- `KadTxError` : timeout factice → `Timeout` ; socket fermée → `SocketIo` ; appelant ne `mark_failed` pas sur `SocketIo`.
- Collecte : paquets à t=0, t=500 ms, t=900 ms → tous collectés (idle réarmé) ; silence >350 ms → fin ; plafond 2 s respecté.
- Matching : deux transactions même IP ports différents → chacune reçoit sa réponse ; deux transactions même (ip,port,opcode) → drop + warn.
- Retry : socket qui drop le 1er datagramme → succès au 2e ; pas de 3e tentative.

## Critères de fin

- [ ] Aucun `mark_failed` sur erreur locale.
- [ ] Paquets tardifs (<2 s) collectés ; idle court toujours possible pour recherche rapide.
- [ ] Collision de transactions observable en log, jamais routée au hasard.
