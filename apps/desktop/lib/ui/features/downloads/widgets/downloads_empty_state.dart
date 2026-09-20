import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/downloads_view_model.dart';
import 'add_link_dialog.dart';
import 'import_part_met_dialog.dart';
import 'lab_guide_dialog.dart';

/// Empty state of the downloads list: icon, title, subtitle and the
/// primary actions (add link, import .part.met, guided lab test).
class DownloadsEmptyState extends StatelessWidget {
  final DownloadsViewModel viewModel;

  const DownloadsEmptyState({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
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
          const Text(
            AppStrings.emptyStateTitle,
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.bold,
              color: AppColors.textSecondary,
            ),
          ),
          const SizedBox(height: AppSpacing.sm),
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 32),
            child: Text(
              AppStrings.emptyStateSubtitle,
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
                label: const Text(AppStrings.btnAddEd2kLink),
                style: FilledButton.styleFrom(backgroundColor: AppColors.accent),
                onPressed: () {
                  showDialog(
                    context: context,
                    builder: (ctx) => AddLinkDialog(viewModel: viewModel),
                  );
                },
              ),
              OutlinedButton.icon(
                key: const Key('empty_import_part_met_btn'),
                icon: const Icon(Icons.file_download_outlined, size: 16),
                label: const Text('Import .part.met'),
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppColors.textSecondary,
                  side: const BorderSide(color: AppColors.border),
                ),
                onPressed: () {
                  showDialog(
                    context: context,
                    builder: (ctx) => ImportPartMetDialog(viewModel: viewModel),
                  );
                },
              ),
              if (viewModel.isLabModeAvailable)
                OutlinedButton.icon(
                  icon: const Icon(Icons.biotech, size: 16),
                  label: const Text('Guided Local Test'),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.textSecondary,
                    side: BorderSide(color: AppColors.lab.withValues(alpha: 0.5)),
                  ),
                  onPressed: () {
                    showDialog(
                      context: context,
                      builder: (ctx) => LabGuideDialog(viewModel: viewModel),
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
