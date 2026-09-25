// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../l10n/generated/app_localizations.dart';
import 'migration_source.dart';

/// Résultat d'un import individuel : résumé ou message d'erreur.
typedef MigrationRunResult = ({String? summary, String? error});

/// Une ligne « fichier détecté » de l'assistant de migration :
/// checkbox, icône, libellé + nom de fichier, puis état du run
/// (en cours / résumé / erreur).
class MigrationSourceTile extends StatelessWidget {
  const MigrationSourceTile({
    super.key,
    required this.source,
    required this.checked,
    required this.enabled,
    required this.onChanged,
    this.result,
    this.running = false,
  });

  final MigrationSource source;
  final bool checked;
  final bool enabled;
  final ValueChanged<bool> onChanged;
  final MigrationRunResult? result;
  final bool running;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final res = result;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: AppSpacing.xxs),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            height: 20,
            width: 20,
            child: Checkbox(
              value: checked,
              onChanged: enabled ? (v) => onChanged(v ?? false) : null,
            ),
          ),
          const SizedBox(width: AppSpacing.sm),
          Icon(source.icon, size: 16, color: AppColors.textMuted),
          const SizedBox(width: AppSpacing.sm),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  '${source.label(l10n)} — ${source.fileName}',
                  style: AppTextStyles.toolbarLabel.copyWith(
                    color: AppColors.textPrimary,
                  ),
                ),
                if (running)
                  const Padding(
                    padding: EdgeInsets.only(top: AppSpacing.xxs),
                    child: SizedBox(
                      height: 12,
                      width: 12,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                  )
                else if (res != null)
                  Padding(
                    padding: const EdgeInsets.only(top: AppSpacing.xxs),
                    child: Text(
                      res.error ?? res.summary ?? '',
                      style: AppTextStyles.badgeLabel.copyWith(
                        color: res.error != null
                            ? AppTheme.statusFailed
                            : AppColors.textMuted,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
