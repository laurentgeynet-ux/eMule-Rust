# ui_kit

Design system partagé des applications Emule-Sion : tokens de thème,
formatters, chaînes et widgets réutilisables. Consommé par
`apps/desktop` et destiné à `apps/mobile` / `apps/web`.

## Contenu

| Export | Rôle |
|---|---|
| `AppTheme` | Couleurs brand/status (`primary`, `darkSurface`, `statusDownloading`…), `dark()` ThemeData, `getStatusColor(DownloadStatus)` |
| `Formatters` | `formatBytes`, `formatSpeed`, `formatEta`, `formatPercent` |
| `AppStrings` | Chaînes UI centralisées (EN) — base avant i18n `gen_l10n` |
| `PartAvailabilityBar` + `partColorFor` | Bandeau eMule par part : rouge = aucune source, bleu = rare→commun, teal = vérifié |

## Usage

```dart
import 'package:ui_kit/ui_kit.dart';

MaterialApp(theme: AppTheme.dark(), ...);
Text(Formatters.formatBytes(item.receivedBytes));
PartAvailabilityBar(parts: item.parts, totalBytes: item.totalBytes);
```

## Règles

- Aucune couleur/typo en dur dans les apps : passer par `AppTheme`.
- Les widgets du kit ne connaissent pas le `DaemonClient` — ils prennent
  des modèles `mule_client` en paramètres.
- Dépend de `mule_client` pour les types (`DownloadStatus`,
  `DownloadPartInfo`).
