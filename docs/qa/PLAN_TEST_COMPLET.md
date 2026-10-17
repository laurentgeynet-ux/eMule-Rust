# Plan de Test Complet — eMuleRust v0.2.0-RC2

Plan de validation en 5 phases, du banc automatisé à l'interconnexion
réseau réel. Chaque phase est un **gate** : on ne passe à la suivante
que si la précédente est verte.

| Phase | Périmètre | Automatisable | État |
| :--- | :--- | :--- | :--- |
| **A — Build & intégrité** | Compilation, lint, tests unitaires/intégration, packaging | 100 % | ⏳ en cours |
| **B — Banc fonctionnel** | Toutes les fonctions du daemon en loopback | ~90 % (RPC scriptables) | ⏳ |
| **C — Interop binaires réels (Tier 3)** | eMule 0.50a, eMule 0.72a, aMule 3.1.0 en loopback fermé | Harnais complet | ✅ **7/7 × 3 références** |
| **D — Réseau réel** | eD2K public + Kad | Manuel supervisé | ⏳ autorisé (ADR 0004) |
| **E — Go/No-Go** | Checklist finale avant push/release | Checklist | ⏳ |

---

## Phase A — Build & Intégrité (gate 1)

| # | Test | Commande | Critère de réussite |
| :--- | :--- | :--- | :--- |
| A.1 | Lint workspace | `cargo check --workspace --all-targets --all-features` | 0 erreur |
| A.2 | Clippy strict | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 warning |
| A.3 | Suite Rust complète | `cargo test --workspace --all-features` | 100 % pass, hors-ligne |
| A.4 | Flutter | `cd apps/desktop; flutter analyze; flutter test` | 0 issue, tests verts |
| A.5 | Anti-duplication + tout-en-un | `scripts\verify_all.ps1` | Exit 0 |
| A.6 | Build release | `scripts\build_windows_release.ps1` puis flavor `qa-loopback` | `eMuleRust.exe` + ZIP + SHA256SUMS + SBOM produits |
| A.7 | Smoke test packagé | `scripts\test_packaged_bundle.ps1` (standard) et `-Flavor qa-loopback` | PASSED, daemon exit 0, `lab.*` absent du standard |
| A.8 | Pipeline E2E loopback | `scripts\test_qa_loopback_e2e.ps1` | PASSED |
| A.9 | Matrice RC | `scripts\test_rc_matrix.ps1` | Toutes lignes vertes |

> ✅ État au 17/10 : A.1–A.4, A.6–A.7 (standard) déjà validés sur commit `7a8a3d7`.
> Reste : A.5, A.6 (qa-loopback), A.7 (qa-loopback), A.8, A.9.

---

## Phase B — Banc Fonctionnel (gate 2)

Tout se pilote via WebSocket JSON-RPC loopback — scriptable via
`Invoke-WsRpc` (voir `test_qa_loopback_e2e.ps1`).

### B.1 Identité & Secure User Identification

| # | Cas | Vérification |
| :--- | :--- | :--- |
| B.1.1 | Génération RSA-384 | `cryptkey.dat` créé au 1er boot, pubkey MD4 stable |
| B.1.2 | Handshake SUI entre 2 instances locales | Signature vérifiée des deux côtés, `sui=true` |
| B.1.3 | Persistance crédits | `credits.list`/`credits.stats` cohérents après redémarrage |
| B.1.4 | Import identité eMule | `identity.importEmuleIdentity` sur un `cryptkey.dat`/`preferences.dat` exporté de eMule 0.72a |
| B.1.5 | Import clients.met | `credits.importClientsMet` sur le `clients.met` de eMule |

### B.2 Cycle de vie des téléchargements

| # | Cas | Vérification |
| :--- | :--- | :--- |
| B.2.1 | Ajout par lien `ed2k://` | Parsing strict + tags inconnus préservés |
| B.2.2 | Création `.part` sparse + `.part.met` | Layout conforme `CPartFile` (gaps nommés) |
| B.2.3 | Téléchargement loopback (qa flavor) | Parts complètes, progression exacte |
| B.2.4 | Vérification par part | Hash eD2K par part + root hash conformes `test-vectors.md` |
| B.2.5 | Corruption injectée → AICH | Preuve Merkle détecte le bloc 180 KiB fautif, re-download sélectif |
| B.2.6 | Pause / reprise / suppression | Transitions d'état + persistance SQLite |
| B.2.7 | Reprise après kill -9 du daemon | `save_active_part_mets` a figé l'état, reprise sans perte |
| B.2.8 | Finalisation | Fichier déplacé `downloads/`, hash final == source |

### B.3 Interop formats de fichiers (sans réseau)

| # | Cas | Vérification |
| :--- | :--- | :--- |
| B.3.1 | Import `.part.met` eMule 0.50a | `import_part_met` → reprise exacte des parts déjà présentes |
| B.3.2 | Idem eMule 0.72a | Rapport d'audit Phase 8.4 propre |
| B.3.3 | Idem aMule (format `.part.met` aMule) | Gaps/traductions correctes |
| B.3.4 | `known.met` / `emfriends.met` | Import sans erreur, données exploitées |
| B.3.5 | `nodes.dat` Kad | `kad.importNodesDat` → contacts routables |

### B.4 Partages & upload

| # | Cas | Vérification |
| :--- | :--- | :--- |
| B.4.1 | Hashage incoming share | `ED2K_PART_SIZE` boundaries respectées |
| B.4.2 | File d'attente upload | Slot alloué, queue ranking croissant |
| B.4.3 | USS (Upload Speed Sense) | ICMP/mesure latence → ajustement débit |
| B.4.4 | `upload-single-slot` | Débit par slot borné |

### B.5 Sources, serveurs, recherche

| # | Cas | Vérification |
| :--- | :--- | :--- |
| B.5.1 | Source Exchange v4 (SX1/SX2) | Sources propagées entre 2 daemons loopback |
| B.5.2 | A4AF (Ask For Another File) | Bascule de source sur meilleur fichier |
| B.5.3 | server.met / connexions serveur (loopback) | Fake server loopback : login, search, sources |
| B.5.4 | Recherche keywords | Indexation/réponse conforme `SEARCHTOLERANCE` |

### B.6 Réseau interne & sécurité

| # | Cas | Vérification |
| :--- | :--- | :--- |
| B.6.1 | Anti-SSRF pipeline | Rejets 127.x/privés/multicast sur les chemins interdits |
| B.6.2 | `ipfilter.dat` | Pair filtré refusé à l'hello ET au dial |
| B.6.3 | Kill switch | Bascule instantanée, zéro fuite |
| B.6.4 | Rate limits | Budgets `ResourceBudgets` respectés sous charge |
| B.6.5 | Pseudonymisation | userhash salé distinct par session |
| B.6.6 | Obfuscation | Handshake obfusqué + `obfuscationRequired` refuse le clair |

### B.7 Kad DHT (loopback)

| # | Cas | Vérification |
| :--- | :--- | :--- |
| B.7.1 | Bootstrap entre 2+ daemons | Contacts échangés, table routage remplie |
| B.7.2 | Lookup itératif | Convergence `SEARCHTOLERANCE` |
| B.7.3 | Publish/Search sources & keywords | Round-trip DHT complet |
| B.7.4 | `kad.listContacts` / `clearContacts` | Introspection exacte |
| B.7.5 | Firewall check tag | Tag `eMuleRust` annoncé |

### B.8 MCP, CLI, superviseur

| # | Cas | Vérification |
| :--- | :--- | :--- |
| B.8.1 | Contrat des 87 outils MCP | `mule-mcp` : chaque outil → JSON-RPC valide |
| B.8.2 | `mule-cli` | Chaque commande ↔ même résultat que RPC |
| B.8.3 | Assistant local | Profils d'outils, orchestrateur loopback |
| B.8.4 | Matrice superviseur | QA_RECEPTION_PROTOCOL §5 (port occupé, 2e instance, kill, accents, fermeture) |
| B.8.5 | Settings hot-reload | `settings.set` appliqué à chaud + persisté |

---

## Phase C — Interop Binaires Réels en Loopback (gate 3)

Harnais existant : `crates/mule-protocol/tests/tier3_external_reference/`
(feature `ed2k-reference-loopback`, tests `#[ignore]`d, checklist 10
conditions — voir son README).

### C.0 Préparation commune (par binaire de référence)

1. **Épinglage cryptographique** — calculer et figer le SHA-256 :
   ```powershell
   Get-FileHash 'D:\eMule0.50a\emule.exe' -Algorithm SHA256
   Get-FileHash 'D:\eMule0.72a\eMule0.72a\emule.exe' -Algorithm SHA256
   Get-FileHash 'D:\aMule-3.1.0-Windows-x64\amule-portable-x64\bin\amule.exe' -Algorithm SHA256
   ```
2. Déclarer `reference_manifest.json` (projet, version, SHA-256, profil
   de compatibilité, limitations connues).
3. **Config hermétique** du client de référence : `EnableKad=0`,
   `AutoConnect=0`, `AutoUpdateServerList=0`, UPnP off, webserver off,
   dossier `config/` dédié copié dans le sandbox — jamais la config
   d'origine.
4. Ports éphémères loopback uniquement, `--test-threads=1`, timeouts ≤ 3 s.
5. Espace IP neutre : les deux pairs se voient comme `127.0.0.1`.

### C.1 Matrice par client

| Test | eMule 0.50a | eMule 0.72a | aMule 3.1.0 | Ce qui est prouvé |
| :--- | :-: | :-: | :-: | :--- |
| HELLO / HELLOANSWER | ✅ | ✅ | ✅ | Handshake, tags, versions mutuellement reconnues |
| `OP_HELLO` profil étendu | — | ✅ | ✅ | Extended protocol, features bits |
| Requête fichier + FileAnswer | ✅ | ✅ | ✅ | `OP_FILEREQANSNOFILE`/`OP_FILEREQANSWER` conformes |
| Envoi de parts eMule → eMuleRust | ✅ | ✅ | ✅ | Décodage `OP_SENDINGPART` réel |
| Envoi de parts eMuleRust → eMule | ✅ | ✅ | ✅ | L'UI eMule affiche la progression correcte |
| `OP_COMPRESSEDPART` | ✅ | ✅ | ✅ | zlib streaming interop |
| Queue ranking distant | ✅ | ✅ | ✅ | Position de file réaliste |
| AICH (preuve + requête) | — | ✅ | ✅ | Merkle interop sur 180 KiB |
| Obfuscation | — | ✅ | ✅ | Protocol encryption |
| Source Exchange v4 | — | ✅ | ✅ | SX1/SX2 vs implémentation réelle |
| `.part.met` reprise croisée | ✅ | ✅ | ✅ | Fichier partagé entre les deux mondes |

> ✅ **EXÉCUTÉ le 03/03** — les 7 tests live passent sur les trois
> références épinglées :
> | Binaire | SHA-256 (préfixe) | Profil | Résultat |
> | :--- | :--- | :--- | :--- |
> | `D:\eMule0.50a\emule.exe` | `06CDF814…` | `Emule050a` | 7/7 |
> | `D:\eMule0.72a\eMule0.72a\emule.exe` | `B7DD6276…` | `Emule072a` | 7/7 |
> | `D:\…\amule-portable-x64\bin\amuled.exe` | `6D3B1309…` | `Amule310` | 7/7 |
>
> Adaptations livrées pour aMule : `ReferenceKind` (binaire lancé en place
> — DLLs adjacentes — avec `-c <config_dir>`), `amule.conf` neutralisé +
> `shareddir.dat` (indexation du fixture), EC confiné à `127.0.0.1`,
> `taskkill` générique, assertions de version profil-dépendantes
> (aMule encode différemment → consigné en divergence « info »).
> Manifeste rempli : `tests/fixtures/ed2k/reference_manifest.json`.

```powershell
# Exécution (après préparation du manifeste)
$env:EMULE_REFERENCE_BIN = "D:\eMule0.50a\emule.exe"
$env:EMULE_REFERENCE_SHA256 = "06CDF814387F627A4BD05A0C68211F715BFA952423E8E8A462E1F47C11A4D20E"
$env:EMULE_REFERENCE_PROFILE = "Emule050a"   # Emule072a | Amule310
cargo test -p mule-protocol --test tier3_loopback --features ed2k-reference-loopback -- --ignored --test-threads=1
```

---

## Phase D — Réseau Réel eD2K/Kad (gate 4)

> ✅ **Autorisé** : l'ADR `0004-public-ed2k-kad-production.md` remplace
> l'ADR 0002 (phase de construction terminée → production). Le profil
> `public-ed2k-kad-preview` devient le mode public supporté. La pureté
> de `mule-protocol` et le confinement par défaut restent en vigueur.
> UPnP : `upnp.setEnabled` explicite ; l'unmap à l'arrêt est borné à
> 500 ms (fix `d0ed8bf`).

| # | Étape | Vérification |
| :--- | :--- | :--- |
| D.1 | `server.met` récupéré d'une source fiable | Liste chargée hors-ligne depuis fichier |
| D.2 | Connexion 1 serveur public réputé | Login OK, **HighID** obtenu via UPnP (ou LowID documenté) |
| D.3 | Kad bootstrap | `kad.bootstrap` depuis `nodes.dat` eMule → contacts réels |
| D.4 | Recherche keyword réelle | Résultats reçus, sources listées |
| D.5 | Téléchargement réel | Contenu librement redistribuable uniquement (Linux ISO, corpus test eD2K, créations propres) |
| D.6 | Upload observé | Slots ouverts, progression vers pairs réels |
| D.7 | Soak 2–4 h | Mémoire stable, pas de fuite handles/sockets, logs sans erreur fatale |
| D.8 | Teardown | `system.stop` → unmap UPnP vérifié sur le routeur, zéro process résiduel |

---

## Phase E — Go/No-Go avant push/release

- [x] Phase A : 100 % verte (`verify_all.ps1`, clippy, tests workspace,
      Flutter 123/123, matrice RC 7/7, smoke test bundle standard)
- [x] Phase B : banc fonctionnel **134/134 PASS** (commit `42bd5ed`)
- [x] Phase C : interop prouvée sur les **3 références** — 7/7 vs
      eMule 0.50a, 7/7 vs eMule 0.72a, 7/7 vs aMule 3.1.0 (SHA-256
      épinglés dans `reference_manifest.json`)
- [x] Phase D partielle : `test_phase_d_public_network.ps1` **21/21 PASS**
      (import 16 serveurs réels, session eD2K établie en LowID,
      95 contacts Kad, recherches eD2K+Kad réelles, disconnectAll,
      UPnP mappé/unmap propre). **Reste non couvert** : D.5
      téléchargement réel, D.6 upload observé, D.7 soak 2–4 h —
      à juger au Go/No-Go (session utilisateur prolongée).
- [x] `version.json.gitDirty == false` sur le bundle final (build
      `42bd5ed`, SHA-256 `6b48d120…`)
- [ ] CHANGELOG à jour pour les fixes `d0ed8bf` / `7a8a3d7` / `63430e8`
      / `fe729ed` / `8b932a3`
- [ ] `git push -u origin master`

### Observation Phase D — LowID

La session serveur s'est établie en **LowID** (clientId < 0x01000000)
alors que UPnP rapporte `tcpMapped=true` sur la gateway. Hypothèses :
test de callback TCP du serveur exécuté avant la fin du mapping, ou
port externe ≠ interne. Comportement fonctionnellement acceptable pour
une RC (LowID fonctionne, dégradé), à documenter dans les notes de
release si confirmé.

## Notes

- L'attestation officielle actuelle (README Tier 3) dit : *« pas
  d'interopérabilité live revendiquée tant que Tier 3 n'a pas tourné
  contre un binaire épinglé »*. Le plan Phase C lève exactement cette
  réserve.
- Ne jamais réutiliser les `config/` d'origine des clients de référence
  (ils contiennent identités et historiques réels) — toujours des copies
  hermétiques.
- Tous les tests Phase A–C restent hors-ligne ; seule Phase D touche
  Internet.
