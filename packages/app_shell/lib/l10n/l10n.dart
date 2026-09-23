// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/widgets.dart';

import '../app/app_controller.dart';
import 'generated/app_localizations.dart';

/// Convenience accessor: `context.l10n.someKey`.
extension L10nX on BuildContext {
  AppLocalizations get l10n => AppLocalizations.of(this);
}

/// Localized label for a [DownloadFilter] (the enum's `label` getter is the
/// English fallback kept for non-UI code).
extension DownloadFilterL10n on DownloadFilter {
  String labelOf(BuildContext context) => switch (this) {
        DownloadFilter.all => context.l10n.filterAll,
        DownloadFilter.active => context.l10n.filterActive,
        DownloadFilter.paused => context.l10n.filterPaused,
        DownloadFilter.completed => context.l10n.filterCompleted,
        DownloadFilter.failed => context.l10n.filterFailed,
      };
}
