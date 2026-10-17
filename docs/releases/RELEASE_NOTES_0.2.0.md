# Notes de Version — eMuleRust v0.2.0

**Date de publication :** 23 Septembre 2026
**Statut :** Release finale — alignement eMule 0.72a, interface web zéro-config, publication open source

---

## 1. Synthèse de la Version

La version **v0.2.0** est la release finale de la ligne 0.2.x. Elle
reprend l'intégralité du contenu de `v0.2.0-RC1` (voir
[RELEASE_NOTES_0.2.0-RC1.md](RELEASE_NOTES_0.2.0-RC1.md)) et y ajoute
l'interface web servie par le daemon, ainsi que la préparation du dépôt
pour publication publique.

Contenu principal hérité de la RC1 :

- Bridages internes levés et alignement comportemental **eMule 0.72a**
  (100 slots d'upload, pacing des connexions, soft-cap Source Exchange).
- **Amorçage Kad résilient** : file `bootstrap_candidates`, drain
  fidèle à `CKademlia::Process`, chargement auto de `nodes.dat`.
- **USS** par traceroute ICMP (*Last Common Route Finder*).
- Débits d'upload **réellement mesurés** (par slot et agrégés).
- Onglet Partages enrichi (catégories, recherche, stats d'upload),
  audit des 36 clés de réglages, localisation française exhaustive.

## 2. Nouveautés depuis la RC1

### Interface web servie par le daemon — connexion zéro-config

- **ServeDir embarqué** (`tower-http`) : le daemon sert le build
  Flutter web sur son port HTTP. Résolution du dossier :
  `--web-dir` / `MULE_WEB_DIR` > auto-détection `<exe>/../web`
  (bundle `bin/` → `web/`), `<exe>/web`, `../../apps/web/build/web`
  (dev cargo). `index.html` en fallback SPA.
- **Auth transparente** : `GET /session-config.js` émet
  `window.MULE_SESSION` (endpoint + token) — réponse limitée aux binds
  **loopback** pour ne jamais exposer `auth_token` sur une interface
  réseau.
- **App web** : connexion sans saisie quand servie par le daemon,
  param `?token=`, champ token optionnel, `autoReconnect` sur le
  client RPC.
- **Desktop** : bouton « Interface web » dans la sidebar ouvrant le
  navigateur sur l'endpoint de session.
- **Bundle** : `build_windows_release.ps1` compile `apps/web`
  (`-SkipWeb`, `-WebBaseHref`), copie les assets sous `web/`, génère
  `Interface Web.bat` (démarre le daemon si absent puis ouvre
  `http://127.0.0.1:4660/`) et ajoute `includesWebUi` à
  `version.json`.

### Palette assouplie de la barre de progression

La sémantique `DrawStatusBar` eMule est conservée avec des teintes
Tailwind déjà utilisées par `AppTheme` ; les constantes de couleurs de
parts sont publiques et partagées avec la carte part-map.

### Préparation à la publication open source

- `LICENSE` (GPL-2.0-or-later), `CONTRIBUTING.md`, `SECURITY.md`.
- `.gitignore` généralisé à toutes les apps Flutter
  (`**/build/`, `**/.dart_tool/`, artefacts Android, keystores).
- Configs `.agents/` dépersonnalisées (`*.example.json`).
- `DeployDir` de `build_windows_release.ps1` rendu opt-in.

---

## 3. Commandes de Validation

```powershell
# Vérification complète de compilation et des lints
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Suite de tests Rust (100% hors-ligne)
cargo test --workspace --all-features

# Validation de l'application Flutter Desktop
cd apps/desktop
flutter analyze
flutter test
```

---

## 4. Migrations

- **`shared_files.category_id`** : `ALTER TABLE` idempotent au
  démarrage — aucune action manuelle requise.
- **`nodes.dat`** : déposer le fichier dans `<data_dir>` suffit — il
  est chargé automatiquement au boot en candidats de bootstrap.

---

## 5. Notes de Sécurité

- Aucune méthode `lab.*` dans le flavor `standard` — le bundle packagé
  est vérifié par `test_packaged_bundle.ps1`.
- `session-config.js` (token de session web) n'est émis que sur les
  binds **loopback**.
- Pipeline anti-SSRF, isolation loopback, kill switch atomique et
  pseudonymisation des pairs (`mule-network-policy`) inchangés.
- Intégrité : `SHA256SUMS.txt` + `version.json` + SBOM CycloneDX
  générés à chaque build, vérifiés par le gate de cohérence 4-way.
