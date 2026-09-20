# Conventions UI/UX — Clean Architecture

Règles appliquées à `apps/*` et `packages/ui_kit`. Objectif : maintenabilité
long terme et réutilisation desktop / mobile / web.

## Couches

```
apps/<app>/lib/
├── app/                    # router, shell, providers globaux
├── services/               # services applicatifs (DaemonSupervisor…)
└── ui/features/<feature>/
    ├── views/              # écrans (composition uniquement)
    ├── widgets/            # widgets du feature, 1 widget par fichier
    └── view_models/        # état & logique de présentation
packages/mule_client/       # domain models + interface + transports (WS, fake)
packages/ui_kit/            # design system : thème, tokens, strings, widgets canoniques
```

- Les vues ne contiennent que la **composition** : toute section non triviale
  (`_buildXxx`) devient un widget nommé dans `widgets/`, **un widget/classe
  par fichier**.
- Les widgets ne parlent jamais au transport directement : ils reçoivent un
  view model / des callbacks.
- Aucune logique métier dans `ui_kit` — il reste pur présentation.

## Tokens — pas de hardcodage

Dans les vues et widgets, **aucune valeur en dur** pour les couleurs, la
hiérarchie typographique, l'espacement courant et les breakpoints :

| Besoin | Token (`package:ui_kit`) |
|---|---|
| Couleurs sémantiques | `AppColors.*` (textPrimary/Secondary/Soft/Muted/Faint, iconFaint, fillSubtle/Light/Medium, accent*, success/warning/danger/info/positive, tint*, lab*) |
| Couleurs de statut download | `AppTheme.getStatusColor(status)`, `AppTheme.status*` |
| Espacements | `AppSpacing.xxs…xxxl`, `AppSpacing.pagePadding`, `toolbarPadding`, `sectionPadding` |
| Rayons | `AppRadius.sm/md/lg` (+`smAll/mdAll/lgAll`) |
| Text styles | `AppTextStyles.sectionTitle/toolbarLabel/badgeLabel/metricLabel/metricValue/bodySmall` |
| Breakpoints | `AppBreakpoints.compact` (750), `AppBreakpoints.medium` (1100) |
| Décorations récurrentes | `AppDecorations.headerBar`, `footerBar`, `chip()` |
| Textes utilisateur | `context.l10n.*` (gen_l10n — `lib/l10n/app_fr.arb` est
  le template, `app_en.arb` la traduction ; `AppStrings` est deprecated) |

Si une couleur/valeur manque : **ajouter un token** dans
`packages/ui_kit/lib/src/tokens.dart` plutôt qu'écrire la valeur en dur.

## Navigation

- Routes déclaratives `go_router` dans `lib/app/router.dart` — jamais
  `Navigator.push` pour les sections principales.
- Le `ShellController` (via `AppController`) est la source de vérité de la
  section courante ;
  la synchro URL se fait dans `router.dart` (VM→URL par listener,
  URL→VM par adoption de la route dans le shell).

## État

- P4 : l'ancien `DownloadsViewModel` est devenu `AppController`
  (`lib/app/app_controller.dart` — racine de composition + session daemon),
  découpé en **contrôleurs par sous-domaine** (1 classe/fichier sous
  `features/<domaine>/view_models/`), chacun exposé via un
  `ChangeNotifierProvider` dans `app/providers.dart`. Les contrôleurs
  reçoivent `onStatus`/`onError` (+ callbacks de rafraîchissement
  croisé) — ils ne connaissent pas le VM. Le VM reste l'orchestrateur :
  connexion/supervisor, section courante, bannières, façade de
  délégation pendant la transition. Disposal : le provider possède le
  contrôleur (le VM ne dispose pas les contrôleurs).
- Chaque vue de section possède son propre `ListenableBuilder`/`watch`
  (ne pas dépendre du rebuild du shell).

## Tests

- Toute modification UI doit garder `flutter test` vert (90+ tests).
- Les tests standalone peuvent monter `DownloadsScreen` sans routeur
  (le shell bascule alors sur `viewModel.currentSection`).
