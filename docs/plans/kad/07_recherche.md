# Étape 7 — Recherche fonctionnelle

> **Dépendances** : étape 5 (pagination et collecte glissante supposent le transport fiable).
> **7a touche `mule-protocol`** → règles strictes : spec `docs/ed2k/` + sources `KademliaUDPListener.cpp:883` (`CreateSearchExpressionTree`) + round-trip obligatoire.

## 7a — Encodeur d'expression restrictive (`mule-protocol`)

**État** : `search_expr.rs` est **décodeur seul** (`MAX_SEARCH_EXPR_DEPTH = 24`). Le flag restrictif existe sur le fil : `start_pos & 0x8000` + arbre sérialisé en suffixe (`decoder/search.rs:719-728`).

**À faire** :

1. `encode_search_expr(expr: &KadSearchExpression) -> Vec<u8>` dans `search_expr.rs` — miroir **exact** du décodeur (mêmes opcodes filaires, même ordre, `utf8_string`, num width).
2. `encoder.rs` : `encode_search_keyword_req_restrictive(keyword_hash, start_pos | 0x8000, expr_bytes)` ou paramètre optionnel sur l'existant.
3. **⚠️ Vérification obligatoire** avant d'écrire : layout exact de `CreateSearchExpressionTree` (`KademliaUDPListener.cpp:883`+) — ordre des opérandes, marqueurs `0x00/0x01/0x02`, tailles des nombres (2/4/8 octets selon valeur).
4. Tests : round-trip `decode(encode(x)) == x` sur toutes les formes de `KadSearchExpression` + vecteur binaire eMule si disponible dans `docs/ed2k/`.

## 7b — Multi-termes (`search_keywords.rs`)

**État** : `extract_kad_keywords(term).into_iter().next()` — un seul mot, l'arbre restrictif n'est utilisé qu'en entrée.

**À faire** :

- Construire le `KadSearchExpression` complet : `AND` des termes + nœuds `>=`/`<=` pour `min_size`/`max_size`/`extension` si présents.
- Sémantique eMule : **un `SEARCH_KEY_REQ` par terme-clé de l'arbre** (pas un pivot unique), chacun portant l'expression complète — vers les mêmes indexeurs convergés.
- Fusion des résultats : dédup par `file_hash` au fil de l'eau, union des entrées.

## 7c — Pagination

**État** : `send_search_keyword` accepte déjà `start_pos` ; `send_search_sources` hardcode `0` (`search_client.rs:85`).

**À faire** :

1. `send_search_sources(..., start_pos: u16)` — ajouter le paramètre, passer à `encode_search_source_req` à la place du `0`.
2. `search_sources_iterative` / `search_keywords_iterative` : par indexeur, si la page précédente était pleine (`>= SEARCH_PAGE_FULL = 300` ou le max wire d'un batch), requête `start_pos += returned` jusqu'à page non pleine ou borne `SEARCH_MAX_PAGES`/`SEARCH_MAX_RESULTS`.

Constantes dans le fichier de recherche concerné : `SEARCH_PAGE_FULL`, `SEARCH_MAX_PAGES`, `SEARCH_MAX_RESULTS`.

## 7d — Coalescing + `source_count`

**État** : `in_flight_searches: HashSet<String>` (`kad.rs:45`) — le duplicata reçoit `Ok(vec![])` au lieu du résultat réel (`search_sources.rs:24-54`).

**À faire** :

1. `HashSet<String>` → `HashMap<String, tokio::sync::broadcast::Sender<Result<Arc<Vec<..>>, KadTxError>>>` : premier appelant = leader, duplicatas s'abonnent et reçoivent **le même résultat**.
2. `publish_keyword` (`search_keywords.rs:230`) : `source_count` = nombre réel de sources connues du fichier partagé (interroger l'index/`DownloadService`), jamais le `1` constant.

## Tests

- `mule-protocol` : round-trip expression + vecteur binaire.
- Multi-termes : requête « a b » → 2 `SEARCH_KEY_REQ` émis avec arbre AND complet.
- Pagination : indexeur factice renvoyant 300 puis 50 → deux requêtes (`start_pos` 0 puis 300), arrêt après la page non pleine.
- Coalescing : deux appels simultanés même clé → **même** `Vec` reçu par les deux.

## Critères de fin

- [ ] Recherche « a b c » retourne des résultats AND multi-termes.
- [ ] Résultats au-delà de 300 par indexeur.
- [ ] Duplicata de recherche reçoit le vrai résultat.
- [ ] `cargo test -p mule-protocol --all-features` vert.
