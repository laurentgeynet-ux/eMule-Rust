// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import '../../../../l10n/l10n.dart';
import 'add_link_dialog.dart';
import 'import_part_met_dialog.dart';
import 'lab_guide_dialog.dart';

/// Empty state of the downloads list: icon, title, subtitle and the
/// primary actions (add link, import .part.met, guided lab test).
class DownloadsEmptyState extends ConsumerWidget {
    const DownloadsEmptyState({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            padding: const EdgeInsets.all(22),
            decoration: const BoxDecoration(
              color: AppColors.surface,
              shape: BoxShape.circle,
              border: Border.fromBorderSide(
                BorderSide(color: AppColors.border, width: 0.5),
              ),
            ),
            child: const Icon(
              Icons.cloud_download_outlined,
              size: 52,
              color: AppColors.iconFaint,
            ),
          ),
          const SizedBox(height: AppSpacing.lg),
          Text(
            context.l10n.emptyStateTitle,
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.bold,
              color: AppColors.textSecondary,
            ),
          ),
          const SizedBox(height: AppSpacing.sm),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 32),
            child: Text(
              context.l10n.emptyStateSubtitle,
              textAlign: TextAlign.center,
              style: TextStyle(fontSize: 13, color: AppColors.textFaint, height: 1.4),
            ),
          ),
          const SizedBox(height: AppSpacing.xl),
          Wrap(
            spacing: AppSpacing.md,
            runSpacing: AppSpacing.sm,
            alignment: WrapAlignment.center,
            children: [
              FilledButton.icon(
                icon: const Icon(Icons.add_rounded, size: 16),
                label: Text(context.l10n.btnAddEd2kLink),
                style: FilledButton.styleFrom(backgroundColor: AppColors.accent),
                onPressed: () {
                  showDialog(
                    context: context,
                    builder: (ctx) => const AddLinkDialog(),
                  );
                },
              ),
              OutlinedButton.icon(
                key: const Key('empty_import_part_met_btn'),
                icon: const Icon(Icons.file_download_outlined, size: 16),
                label: Text(context.l10n.actionImportPartMet),
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppColors.textSecondary,
                  side: const BorderSide(color: AppColors.border),
                ),
                onPressed: () {
                  showDialog(
                    context: context,
                    builder: (ctx) => const ImportPartMetDialog(),
                  );
                },
              ),
              if (ref.watch(isLabModeAvailableProvider))
                OutlinedButton.icon(
                  icon: const Icon(Icons.biotech, size: 16),
                  label: Text(context.l10n.actionGuidedLabTest),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.textSecondary,
                    side: BorderSide(color: AppColors.lab.withValues(alpha: 0.5)),
                  ),
                  onPressed: () {
                    showDialog(
                      context: context,
                      builder: (ctx) => const LabGuideDialog(),
                    );
                  },
                ),
            ],
          ),
        ],
      ),
    );
  }
}
