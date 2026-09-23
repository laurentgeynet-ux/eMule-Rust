// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Section 1: default download directory — path field + save, reset to
/// portable default and open folder shortcuts.
class SettingsDirectorySection extends StatelessWidget {
  final TextEditingController dirController;
  final bool isCustom;
  final bool isSaving;
  final VoidCallback onSave;
  final VoidCallback onReset;
  final VoidCallback onOpenFolder;

  /// Ouvre l'explorateur de dossiers natif et injecte le chemin choisi
  /// dans [dirController].
  final VoidCallback? onBrowse;

  /// Appelé sur chaque saisie utilisateur (dirty tracking — un champ en
  /// cours d'édition n'est pas écrasé par une resync externe).
  final ValueChanged<String>? onChanged;

  const SettingsDirectorySection({
    super.key,
    required this.dirController,
    required this.isCustom,
    required this.isSaving,
    required this.onSave,
    required this.onReset,
    required this.onOpenFolder,
    this.onBrowse,
    this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(context.l10n.globalDefaultDownloadDirectory,
          style: TextStyle(
            fontSize: 14,
            fontWeight: FontWeight.w600,
            color: AppColors.textPrimary,
          ),
        ),
        const SizedBox(height: 6),
        Text(context.l10n.thisPathIsUsedAs,
          style: TextStyle(
            fontSize: 12,
            color: AppColors.textSoft,
            height: 1.4,
          ),
        ),
        const SizedBox(height: 14),
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: TextField(
                controller: dirController,
                style: const TextStyle(fontSize: 13),
                onChanged: onChanged,
                onSubmitted: (_) => onSave(),
                decoration: InputDecoration(
                  isDense: true,
                  labelText: context.l10n.destinationDirectoryPath,
                  hintText: context.l10n.eGCDownloadsOr,
                  filled: true,
                  fillColor: AppTheme.darkCard,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(8),
                    borderSide:
                        const BorderSide(color: AppTheme.darkBorder),
                  ),
                  helperText: isCustom
                      ? context.l10n.settingsDirCustomHelper
                      : context.l10n.settingsDirPortableHelper,
                  helperStyle: TextStyle(
                    color: isCustom
                        ? AppColors.accentTeal
                        : AppColors.textFaint,
                    fontSize: 11,
                  ),
                ),
              ),
            ),
            const SizedBox(width: 6),
            IconButton(
              icon: const Icon(Icons.folder_open_rounded, size: 18),
              tooltip: context.l10n.browse,
              onPressed: isSaving ? null : onBrowse,
            ),
            FilledButton(
              style: FilledButton.styleFrom(
                backgroundColor: AppTheme.primary,
                padding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 14,
                ),
              ),
              onPressed: isSaving ? null : onSave,
              child: isSaving
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(
                        strokeWidth: 2,
                        color: AppColors.textPrimary,
                      ),
                    )
                  : Text(context.l10n.save),
            ),
          ],
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 12,
          runSpacing: 8,
          children: [
            OutlinedButton.icon(
              icon: const Icon(Icons.restart_alt_rounded, size: 16),
              label: Text(context.l10n.resetToPortableDefault),
              style: OutlinedButton.styleFrom(
                foregroundColor: AppColors.textSecondary,
                side: const BorderSide(color: AppTheme.darkBorder),
              ),
              onPressed: isSaving ? null : onReset,
            ),
            TextButton.icon(
              icon: const Icon(Icons.folder_open_rounded, size: 16),
              label: Text(context.l10n.openFolder),
              onPressed: onOpenFolder,
            ),
          ],
        ),
      ],
    );
  }
}
