# ui_kit

Design system partagé des applications eMuleRust : tokens de thème,
formatters, chaînes et widgets réutilisables. Consommé par
`apps/desktop` et destiné à `apps/mobile` / `apps/web`.

## Contenu

| Export | Rôle |
|---|---|
| `AppTheme` | Couleurs brand/status (`primary`, `darkSurface`, `statusDownloading`…), `dark()` ThemeData, `getStatusColor(DownloadStatus)` |
| `Formatters` | `formatBytes`, `formatSpeed`, `formatEta`, `formatPercent` |
| `AppStrings` | Chaînes UI centralisées (EN) — base avant i18n `gen_l10n` |
| `PartAvailabilityBar` + `partColorFor` | Bandeau eMule `DrawStatusBar` byte-précis, palette standard plate : bleu = reçu, rouge = aucune source, dégradé cyan→bleu eMule rare→commun, jaune = bloc en vol, vert = vérifié et strip de progression |

## Usage

```dart
import 'package:ui_kit/ui_kit.dart';

MaterialApp(theme: AppTheme.dark(), ...);
Text(Formatters.formatBytes(item.receivedBytes));
PartAvailabilityBar(
  parts: item.parts,
  totalBytes: item.totalBytes,
  pendingRanges: item.pendingRanges, // blocs en vol → ambre
  progress: item.progress,           // strip vert en haut
  muted: item.status == DownloadStatus.paused,
);
```

## Règles

- Aucune couleur/typo en dur dans les apps : passer par `AppTheme`.
- Les widgets du kit ne connaissent pas le `DaemonClient` — ils prennent
  des modèles `mule_client` en paramètres.
- Dépend de `mule_client` pour les types (`DownloadStatus`,
  `DownloadPartInfo`).
