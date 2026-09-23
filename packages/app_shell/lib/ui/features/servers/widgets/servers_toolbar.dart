// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// Servers section toolbar: refresh, server.met import and update-from-URL.
/// Icon buttons below 1150 px, labelled buttons otherwise.
class ServersToolbar extends StatelessWidget {
  final VoidCallback onRefresh;
  final VoidCallback onImportMet;
  final VoidCallback onUpdateFromUrl;

  const ServersToolbar({
    super.key,
    required this.onRefresh,
    required this.onImportMet,
    required this.onUpdateFromUrl,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isCompact = constraints.maxWidth < 1150;
          return Row(
            children: [
              const Icon(
                Icons.dns_rounded,
                color: AppTheme.primary,
                size: 22,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Text(context.l10n.gestionLocaleDesServeursEd2k,
                  style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.bold,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 12),
              if (isCompact) ...[
                IconButton(
                  tooltip: context.l10n.actualiser,
                  icon: const Icon(Icons.refresh_rounded, size: 18),
                  onPressed: onRefresh,
                ),
                IconButton(
                  tooltip: context.l10n.importerServerMet,
                  icon: const Icon(
                    Icons.file_upload_outlined,
                    size: 18,
                    color: AppTheme.primary,
                  ),
                  onPressed: onImportMet,
                ),
                IconButton(
                  tooltip: context.l10n.mettreAJourDepuisUrl,
                  icon: const Icon(
                    Icons.cloud_download_outlined,
                    size: 18,
                    color: AppColors.accentTeal,
                  ),
                  onPressed: onUpdateFromUrl,
                ),
              ] else ...[
                OutlinedButton.icon(
                  icon: const Icon(Icons.refresh_rounded, size: 16),
                  label: Text(context.l10n.actualiser),
                  onPressed: onRefresh,
                ),
                const SizedBox(width: 10),
                ElevatedButton.icon(
                  icon: const Icon(Icons.file_upload_outlined, size: 16),
                  label: Text(context.l10n.importerServerMet),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppTheme.primary,
                    foregroundColor: AppColors.textPrimary,
                  ),
                  onPressed: onImportMet,
                ),
                const SizedBox(width: 10),
                FilledButton.icon(
                  icon: const Icon(Icons.cloud_download_outlined, size: 16),
                  label: Text(context.l10n.mettreAJourDepuisUrl),
                  style: FilledButton.styleFrom(
                    backgroundColor: AppColors.labShade700,
                    foregroundColor: AppColors.textPrimary,
                  ),
                  onPressed: onUpdateFromUrl,
                ),
              ],
            ],
          );
        },
      ),
    );
  }
}
