# AGENTS.md — Règles pour agents IA

## Vue d'ensemble

**eMule-Rust** est une réimplémentation moderne des protocoles P2P **eD2K** et **Kad DHT (Kademlia)** :
- Moteur en **Rust** (workspace Cargo, Tokio async, codecs filaires bornés)
- Interface **Flutter Desktop** (Windows, Clean Architecture MVVM)
- Serveur **MCP** exposant ~87 outils pour agents IA

Licence : GPL-2.0-or-later. Plateforme cible : Windows 10/11 x64.

## Cartographie du dépôt

| Chemin | Rôle |
| :--- | :--- |
| `crates/mule-protocol` | Codecs filaires eD2K/Kad, opcodes, cadrage. **Ne pas modifier sans vecteur de test ni spec `docs/ed2k/`** |
| `crates/mule-daemon` | Services Tokio : `kad`, `kad_udp`, `peer_session`, `inbound_listener`, swarm allocator |
| `crates/mule-core` | Domaine métier, moteur de stockage éparse |
| `crates/mule-crypto` | MD4, hashes eD2K, identifiants |
| `crates/mule-filter` | Filtrage IP (`ipfilter.dat`) |
| `crates/mule-network-policy` | Pipeline anti-SSRF, isolation loopback, kill switch |
| `crates/mule-mcp` | Serveur MCP (~87 outils) |
| `crates/mule-cli` | CLI de pilotage |
| `crates/mule-format` | Formats de fichiers (`.met`, `.dat`…) |
| `packages/app_shell` | UI Flutter principale (MVVM, `lib/ui/features/…`) |
| `packages/mule_client`, `packages/ui_kit` | Client Dart et kit UI |
| `apps/desktop`, `apps/web` | Applications Flutter |
| `docs/` | Documentation : `docs/ed2k/` (specs + ADRs), `docs/walkthroughs/`, `docs/qa/`, `docs/INDEX.md` (cartographie) |
| `scripts/` | Scripts PowerShell de build/validation |

## Commandes de validation

Exécuter avant de considérer un changement terminé :

```powershell
# Rust — lint et tests (100% hors-ligne)
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# Flutter — analyse et tests
cd apps/desktop
flutter analyze
flutter test

# Validation complète
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\verify_all.ps1
```

## Règles critiques

1. **Fidélité protocolaire** : la référence de vérité est le code source d'**eMule 0.72a** (`D:\eMule0.72a-Sources\eMule0.72a-Sources\srchybrid`). Tout changement sur opcodes, cadrage ou comportement filaire doit être vérifié contre ces sources et `docs/ed2k/`. Consulter les ADRs avant de modifier `mule-protocol`.
2. **Tests hors-ligne uniquement** : aucun test ne doit émettre de trafic réseau sortant. Utiliser loopback (`test_qa_loopback_e2e.ps1` comme référence).
3. **Sécurité non négociable** : ne jamais affaiblir le pipeline anti-SSRF, l'isolation loopback par défaut, le kill switch atomique ou la pseudonymisation des pairs (`mule-network-policy`). Ne jamais produire de release publique avec `-LabMode` / `--features lab-tools` — `test_packaged_bundle.ps1` vérifie que le flavor `standard` n'expose aucune méthode `lab.*`.
4. **Windows-first** : instructions et scripts en PowerShell, pas de bash. Chemins avec `\` ou quoting approprié.
5. **Dépendances** : vérifier `deny.toml` avant d'ajouter une crate ; préférer les dépendances du workspace (`[workspace.dependencies]`).
6. **Aucune valeur en dur** : couleurs, dimensions et styles Flutter → design tokens (`packages/ui_kit/lib/src/tokens.dart` ou palette de feature `*_colors.dart` — vérifié par `apps/desktop/test/architecture/design_tokens_guard_test.dart`) ; réglages Rust (seuil, timeout, quota, taille, retry) → `DaemonConfig`, `ResourceBudgets`, `mule-protocol/src/limits.rs` ou `const` documentée en tête de fichier. Une valeur = une seule source de vérité.
7. **Un composant par fichier** : toute fonction, widget ou bloc logique qui peut vivre dans son propre fichier doit l'avoir — pas de fichier monolithique ni de littéral perdu dans la logique. À la création d'un fichier, appliquer les sections « Design tokens et maintenabilité UI » et « Constantes et réglages » ci-dessous.

## Conventions de code

- **Langue** : code et documentation en français (cohérent avec l'existant).
- **Erreurs Rust** : `thiserror` pour les erreurs typées, pas de `unwrap()` en dehors des tests.
- **Logging** : `tracing` (jamais `println!` dans les crates).
- **Async** : Tokio ; pas de blocage synchrone dans les tâches async.
- **Flutter** : Clean Architecture MVVM ; widgets dans `packages/ui_kit`, features dans `app_shell/lib/ui/features/`.
- **Warnings** : le workspace compile avec `-D warnings` ; aucun warning toléré.

## Design tokens et maintenabilité UI (Flutter)

### Aucune valeur en dur

Toute valeur visuelle (couleur, espacement, rayon, taille de texte, breakpoint, décoration) doit provenir des design tokens — jamais de littéral ad hoc dans une vue ou un widget.

| Besoin | Source obligatoire |
| :--- | :--- |
| Espacements, paddings, gaps | `AppSpacing.*` (`packages/ui_kit/lib/src/tokens.dart`) |
| Rayons de bordure | `AppRadius.*` |
| Couleurs | `AppColors.*` (sémantique) ou `AppTheme.*` (`src/theme.dart`) |
| Styles de texte | `AppTextStyles.*` |
| Breakpoints layout | `AppBreakpoints.*` |
| Décorations récurrentes | `AppDecorations.*` |

Si le token manque : l'ajouter dans `tokens.dart` (ou `theme.dart` pour la palette de base) avec un nom sémantique, puis l'utiliser. Ne jamais introduire de `Colors.*`, `Color(0x…)`, `EdgeInsets`, `BorderRadius` ou `TextStyle` littéraux dans `app_shell`.

```dart
// Interdit
Container(padding: const EdgeInsets.all(16), color: Colors.white12)

// Correct
Container(padding: AppSpacing.pagePadding, color: AppColors.fillLight)
```

Seuls fichiers autorisés à contenir des littéraux de couleur : `tokens.dart` et `theme.dart` (ui_kit), plus les palettes de feature nommées `*_colors.dart` (ex. `network_profile_colors.dart`). Cette règle est vérifiée par le test `apps/desktop/test/architecture/design_tokens_guard_test.dart` — ne pas le désactiver ; escape hatch ponctuel : `// design_tokens_guard: ignore` en fin de ligne.

### Clean Architecture MVVM

- Chaque feature vit dans `app_shell/lib/ui/features/<feature>/` avec :
  - `views/` : composition d'écran, aucune logique métier ni état.
  - `view_models/` : controllers/stores (état + logique de présentation).
  - `widgets/` : composants privés de la feature, regroupés en sous-dossiers par usage.
- Widget réutilisé par plusieurs features → le déplacer dans `packages/ui_kit`.
- Une `view` n'appelle jamais le daemon directement : toujours via un controller/provider (`app/providers.dart`).

### Anti-fichiers monolithiques

- Un fichier doit rester lisible en entier : toute fonction, widget ou bloc logique qui peut vivre dans son propre fichier doit l'avoir — pas de fichier fourre-tout. Un composant par fichier, nommé d'après sa responsabilité (cf. `downloads/widgets/detail/`, `add_link/`).
- Cible d'extraction selon le niveau : widget privé → `widgets/` (sous-dossier par usage), helper/formatage partagé → `ui_kit` (`formatters.dart`, `strings.dart`), utilitaire plateforme → `app_shell/lib/core/`.
- En modifiant un fichier existant trop long ou dupliqué, extraire les parties indépendantes vers leurs fichiers dédiés — la migration fait partie du changement, pas un refactor « plus tard ».

## Constantes et réglages (Rust)

### Aucun magic number dans la logique

Toute valeur de réglage — seuil, timeout, quota, taille, intervalle, nombre de retries — doit être une `const` nommée ou un champ de config, jamais un littéral dans le corps d'une fonction. Un réglage a **une seule source de vérité** : si plusieurs modules en ont besoin, il vit dans le fichier partagé, pas en double.

| Type de valeur | Emplacement obligatoire |
| :--- | :--- |
| Réglage opérationnel du daemon (peers, ports, quotas, capacités, répertoires) | `DaemonConfig` (`crates/mule-daemon/src/config.rs`) ; si modifiable à chaud → clé dans `KNOWN_SETTINGS_KEYS` (`rpc/handlers/settings.rs`, ex. `minUploadKbs`) |
| Budgets et timeouts réseau (connexions, rate limit, buffers) | `ResourceBudgets` (`crates/mule-network-policy/src/budgets.rs`) |
| Bornes protocolaires / filaires (tailles de frame, tags, parts) | `mule-protocol/src/limits.rs` ou `ed2k/limits.rs` ; struct `*Limits` si la borne est injectable |
| Constante interne à un seul module | `const` documentée en tête de fichier (ex. `INFLIGHT_BLOCK_TTL_MS`) |

Tolérés : `0`, `1`, indices de boucle, littéraux évidents dans les tests et les formats d'affichage.

```rust
// Interdit — le « 10 » est introuvable sans lire tout le fichier
if upload_kbs < 10 { ... }

// Correct — le réglage vit à un seul endroit
/// Débit d'upload minimal par pair (Ko/s).
pub const MIN_UPLOAD_KBS: u32 = 10;
if upload_kbs < MIN_UPLOAD_KBS { ... }
```

Modifier un réglage ne doit jamais demander de fouiller la logique : on ouvre le fichier de constantes/config, on change la valeur, point.

## Anti-duplication (Rust)

Une fonctionnalité = **un seul propriétaire**. Avant d'écrire un helper ou un type, vérifier qu'il n'existe pas déjà :

| Besoin | Propriétaire unique |
| :--- | :--- |
| Helpers de tests d'intégration cross-crates (daemon RPC, état, JSON-RPC, WebSocket) | `crates/mule-test-support` (dev-dependency) |
| Fixtures de tests protocolaires (hashes, tags clients, clients loopback/nominaux) | `crates/mule-protocol/tests/common/` |
| Helpers de tests intra-crate | `tests/common/` du crate concerné |
| Erreurs JSON-RPC | `mule-daemon::error::JsonRpcError` |
| Liste des capacités RPC (`hello`/`status`) | `mule-daemon::rpc::capabilities` |
| I/O lab (`send_msg`/`read_msg`) | `mule-daemon::lab::io` (feature `lab-loopback`) |
| Schémas d'outils MCP (`obj_schema`, `prop`) | `mule-mcp::tools` |
| Lecture bornée de fichiers `.met` | `mule-format::met::reader` |
| Erreurs SQL/contexte DB du service downloads | `mule-daemon::services::downloads::db_err` |

Distinctions **légitimes** à ne pas fusionner (noms proches, sémantiques différentes) :
- `mule-protocol::ed2k::tags::TagValue` (tags filaires eD2K) ≠ `mule-format::met::MetTagValue` (tags de fichiers `.met`)
- `mule-core::PeerSource` (origine d'un pair dans le domaine) ≠ `mule-format::LinkSource` (source dans un lien eD2K)
- `mule-daemon::AppState` (état RPC du daemon) ≠ `mule-mcp::McpState` (état du serveur MCP)

Garde-fou : `scripts/test_duplicates.ps1` (intégré à `verify_all.ps1`) échoue si un nouveau clone exact apparaît hors de `scripts/dup_allowlist.txt`. Rapport détaillé : `docs/qa/rapport_doublons_rust.md`, régénéré via `python scripts/find_duplicates.py > target/dup-report.txt`.

## Documentation associée

- `docs/INDEX.md` — inventaire exhaustif des fichiers
- `docs/architecture/rapport_architecture_et_avancement.md` — état technique
- `docs/CHANGELOG.md` — historique des versions
