# AGENTS.md — Règles pour agents IA

## Vue d'ensemble

**Emule-Sion** est une réimplémentation moderne des protocoles P2P **eD2K** et **Kad DHT (Kademlia)** :
- Moteur en **Rust** (workspace Cargo, Tokio async, codecs filaires bornés)
- Interface **Flutter Desktop** (Windows, Clean Architecture MVVM)
- Serveur **MCP** exposant ~80 outils pour agents IA

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
| `crates/mule-mcp` | Serveur MCP (~80 outils) |
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

## Conventions de code

- **Langue** : code et documentation en français (cohérent avec l'existant).
- **Erreurs Rust** : `thiserror` pour les erreurs typées, pas de `unwrap()` en dehors des tests.
- **Logging** : `tracing` (jamais `println!` dans les crates).
- **Async** : Tokio ; pas de blocage synchrone dans les tâches async.
- **Flutter** : Clean Architecture MVVM ; widgets dans `packages/ui_kit`, features dans `app_shell/lib/ui/features/`.
- **Warnings** : le workspace compile avec `-D warnings` ; aucun warning toléré.

## Documentation associée

- `docs/INDEX.md` — inventaire exhaustif des fichiers
- `docs/architecture/rapport_architecture_et_avancement.md` — état technique
- `docs/CHANGELOG.md` — historique des versions
