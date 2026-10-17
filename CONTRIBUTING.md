# Contribuer à eMuleRust

Merci de l'intérêt porté au projet. Ce document décrit le flux de
contribution et les exigences de qualité.

## Flux de branches

| Branche | Rôle |
| :--- | :--- |
| `master` | Production — uniquement des merges validés depuis `Beta`, tagués `v*` |
| `Beta` | Pré-release / QA — release candidates |
| `Dev` | Intégration continue — cible par défaut des pull requests |
| `feature/*`, `Android`, `iptv` | Branches de travail — fusionnées dans `Dev` via PR |

Flux : `feature/*` → `Dev` → `Beta` (RC) → `master` (release taguée).

## Avant de soumettre une PR

Exécuter la suite de validation complète (100 % hors-ligne) :

```powershell
# Rust — lint et tests
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# Flutter — analyse et tests
cd apps/desktop
flutter analyze
flutter test

# Ou tout-en-un :
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\verify_all.ps1
```

Le workspace compile avec `-D warnings` : aucun warning n'est toléré.

## Règles non négociables

1. **Fidélité protocolaire** : la référence de vérité est le code source
   d'**eMule 0.72a**. Tout changement sur opcodes, cadrage ou comportement
   filaire doit être vérifié contre ces sources et `docs/ed2k/`, avec un
   vecteur de test. Consulter les ADRs avant de modifier `mule-protocol`.
2. **Tests hors-ligne uniquement** : aucun test ne doit émettre de trafic
   réseau sortant. Utiliser loopback (`test_qa_loopback_e2e.ps1` en référence).
3. **Sécurité** : ne jamais affaiblir le pipeline anti-SSRF, l'isolation
   loopback par défaut, le kill switch atomique ou la pseudonymisation des
   pairs (`mule-network-policy`). Jamais de release publique avec
   `-LabMode` / `--features lab-tools`.
4. **Dépendances** : vérifier `deny.toml` avant d'ajouter une crate ;
   préférer les dépendances du workspace (`[workspace.dependencies]`).

## Conventions

- Code et documentation en **français**.
- Erreurs Rust : `thiserror`, pas de `unwrap()` hors tests.
- Logging : `tracing` (jamais `println!` dans les crates).
- Async : Tokio ; pas de blocage synchrone dans les tâches async.
- Flutter : Clean Architecture MVVM ; widgets dans `packages/ui_kit`,
  features dans `app_shell/lib/ui/features/`.
- Messages de commit : style *conventional commits* en français
  (`feat(x):`, `fix(y):`, `chore:`, `docs:`…).

## Signaler un bug

Ouvrir une *issue* GitHub avec : version (`version.json` du bundle),
système, étapes de reproduction, logs pertinents. Pour les
vulnérabilités de sécurité, suivre [`SECURITY.md`](SECURITY.md).

## Licence

Toute contribution est licenciée sous **GPL-2.0-or-later**
(voir [`LICENSE`](LICENSE)).
