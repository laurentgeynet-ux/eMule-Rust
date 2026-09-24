# Rapport d'analyse de duplication — workspace Rust

Date : 2026-09-24. Analyseur : `scripts/find_duplicates.py` (symboles homonymes,
corps de fonctions normalisés, blocs clones inter-fichiers ≥ 8 lignes nettoyées).
Périmètre : `crates/**` src + tests — 330 fichiers, ~106 200 lignes.

## Classification

- **D1** clone exact → fusionner dans la crate propriétaire
- **D2** quasi-clone → généraliser / paramétrer
- **D3** duplication sémantique → unifier ou documenter la distinction
- **D4** code mort → supprimer
- **D5** faux positif / répétition légitime → conserver (documenté)

## D1 — Clones exacts (code de production)

| Doublon | Emplacements | Action |
| :--- | :--- | :--- |
| `send_msg` + `read_msg` (~30 l. chacun, identiques) | `mule-daemon/src/lab/downloader.rs`, `lab/source_service.rs` | Extraire vers `lab/io.rs` (pub(crate)) |
| Liste `capabilities` (~24 l., verbatim) | `rpc/handlers/settings.rs` + `rpc/methods.rs` | `const CAPABILITIES` unique dans `methods.rs` |
| `name()` (recherche `TAG_SPECIAL_NAME` dans tags) | `ed2k/server/search.rs` + `ed2k/server/types.rs` (mule-protocol) | Helper partagé dans `tags.rs` |
| `open()` (File::open + read_from) | `met/part_met.rs` + `met/server_met.rs` (mule-format) | Helper `open_read` dans `met/` |
| `count_kad_*` ×4, `purge_expired_*` ×3, `record_credit_*` ×2 | `mule-core/src/persistence/sqlite/kad.rs`, `credits.rs` | Helpers privés `count_rows` / `purge_expired` / `record_credit` |
| Accesseurs `misc_options1/2` ×8 | `mule-protocol/src/ed2k/peer_info.rs` | Helpers privés `misc1_nibble` / `misc2_flag` |

## D1 — Clones exacts (tests)

| Doublon | Occurrences | Action |
| :--- | :--- | :--- |
| `make_req` | ×11 fichiers `mule-daemon/tests/` | `tests/common/` |
| `setup_test_state` (3 variantes) | ×12 fichiers `mule-daemon/tests/` | `tests/common/` (variantes paramétrées) |
| `spawn_test_daemon` (3 variantes) | ×7 fichiers `mule-daemon/tests/` + `mule-mcp/tests/` | `tests/common/` |
| `RunningDaemon` (+ `start`, `Drop`) | ×3 fichiers `mule-daemon/tests/` | `tests/common/` |
| `send_json` / `recv_response_by_id` | ×2-3 fichiers | `tests/common/` |
| `sample_client_tags` ×3, `sample_file_hash` ×2, `hash16/hash20`, `read_msg` | `mule-protocol/tests/` | Étendre `tests/common/` existant |

## D3 — Duplication sémantique

| Cas | Diagnostic | Décision |
| :--- | :--- | :--- |
| `TagValue` ×2 | `mule-format::met` (fichiers .met : `Bsob`, `Hash16`, `Uint64`) vs `mule-protocol::ed2k` (filaire : `UInt8/16/32/64`, `Hash`). Domaines distincts, variantes distinctes — **non unifiable** sans perdre la discrimination filaire. | Conserver les deux ; renommer `met::TagValue` → `MetTagValue` pour lever l'homonymie |
| `PeerSource` ×2 | `mule-format::ed2k` (parsing de lien `host:port[:hash]`) vs `mule-core::model` (record persistant). Rôles différents. | Renommer le type format → `LinkSource` |
| `JsonRpcError` ×2 | `mule-mcp::protocol` (code i64, `#[allow(dead_code)]`) duplique `mule-daemon::error` (code i32, complet). `mule-mcp` dépend déjà de `mule-daemon`. | Compléter `error.rs` (`parse_error`, `invalid_request`), supprimer le type local mcp |
| `AppState` ×2 | `mule-daemon::state` vs `mule-mcp::main` — rôles distincts | Renommer celui de mcp → `McpState` |
| Schémas JSON des outils MCP | Fragments `json!({...})` quasi identiques répétés dans `tools/kad.rs`, `servers.rs`, `system.rs`, `downloads/schemas.rs`, `shares/schemas.rs` | Fragments mutualisés (`tools/schema_fragments.rs`) |
| `emit_event` (peer vs server session) | Même squelette, variante d'événement différente | D5 — corps 4 lignes, généralisation non rentable |
| `handle_*_status` ×4 | Wrappers `snapshot().await → to_value` | D5 — handlers RPC explicites, une ligne de logique |

## D4 — Code mort

- `#[allow(dead_code)]` sur `mule-mcp::protocol::JsonRpcError` : partiellement inutilisé → résorbé par la déduplication ci-dessus.
- Revue `clippy` `dead_code` après consolidation.

## D5 — Répétitions légitimes conservées

- `test_fixture_*` (mule-format / mule-protocol) : vecteurs de test explicites — la lisibilité prime.
- `internal_error`/`invalid_request`/`invalid_params` (mcp) : constructeurs triviaux — résorbés via `JsonRpcError`.
- Homonymes de méthodes sur types différents (`update_status`, `set_enabled`, `stop`, `snapshot`…) : API parallèles, pas de code dupliqué.
- `mod tests`, `mod types` : idiomatique Rust.
- `to_hex` ×4 : méthodes inhérentes sur 4 types de hash distincts ; seul `kad_id.rs` réimplémente la boucle au lieu de `hex::encode` → aligner.

## Ordre d'exécution des lots

1. **Lot 1** — `mule-daemon/tests/common/` (plus gros gisement, ~30 doublons)
2. **Lot 2** — types : `JsonRpcError` (mcp→daemon), `PeerSource`→`LinkSource`, `TagValue`→`MetTagValue`, `AppState`→`McpState`
3. **Lot 3** — utilitaires src : `lab/io.rs`, `CAPABILITIES`, helpers `tags.rs`/`met`, helpers sqlite, helpers `peer_info`, `to_hex`, fragments de schémas MCP
4. **Lot 4** — extension `mule-protocol/tests/common/`, reliquats, vérification `clippy`/`dead_code`
5. **Garde-fou** — `scripts/test_duplicates.ps1` + branchement `verify_all.ps1` + note AGENTS.md

Vérification après chaque lot : `cargo check --workspace --all-targets --all-features`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
`cargo test --workspace --all-features` (hors-ligne).
