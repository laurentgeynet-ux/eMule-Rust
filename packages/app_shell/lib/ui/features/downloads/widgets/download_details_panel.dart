// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import 'download_detail_dialog.dart';
import '../../../../l10n/enum_labels.dart';
import '../../../../l10n/l10n.dart';

/// Collapsible summary panel shown at the bottom of the downloads workspace
/// when a download is selected. Offers a shortcut to the full detail dialog.
class DownloadDetailsPanel extends ConsumerWidget {
  final DownloadItem item;
    final bool isSmall;

  const DownloadDetailsPanel({
    super.key,
    required this.item,
    this.isSmall = false,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Container(
      padding: AppSpacing.pagePadding,
      decoration: AppDecorations.footerBar,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.info_outline_rounded, size: 18, color: AppColors.accent),
              const SizedBox(width: AppSpacing.sm),
              Expanded(
                child: Text(context.l10n.details(item.fileName),
                  style: const TextStyle(fontSize: 14, fontWeight: FontWeight.bold),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              if (isSmall)
                IconButton(
                  icon: const Icon(Icons.open_in_new_rounded, size: 16),
                  tooltip: context.l10n.inspectFullDetails,
                  style: IconButton.styleFrom(foregroundColor: AppColors.accent),
                  onPressed: () => _openDetails(context),
                )
              else
                OutlinedButton.icon(
                  icon: const Icon(Icons.open_in_new_rounded, size: 14),
                  label: Text(context.l10n.inspectFullDetails, style: TextStyle(fontSize: 12)),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.accent,
                    side: const BorderSide(color: AppColors.accent, width: 0.8),
                    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                  ),
                  onPressed: () => _openDetails(context),
                ),
              const SizedBox(width: AppSpacing.sm),
              IconButton(
                icon: const Icon(Icons.close, size: 16, color: AppColors.textMuted),
                onPressed: () {
                  final router = GoRouter.maybeOf(context);
                  if (router != null) {
                    router.go('/downloads');
                  } else {
                    ref.read(downloadsStoreControllerProvider).select(null);
                  }
                },
              ),
            ],
          ),
          const SizedBox(height: AppSpacing.sm),
          Wrap(
            spacing: AppSpacing.xxxl,
            runSpacing: AppSpacing.md,
            children: [
              _Metric(label: context.l10n.downloadId, value: item.id),
              _Metric(label: context.l10n.totalSize, value: Formatters.formatBytes(item.totalBytes)),
              _Metric(label: context.l10n.verifiedBytes, value: Formatters.formatBytes(item.verifiedBytes)),
              _Metric(
                label: context.l10n.status,
                value: item.status.labelL10n(context.l10n),
              ),
              _Metric(label: context.l10n.sources, value: '${item.sourceCount}'),
            ],
          ),
        ],
      ),
    );
  }

  void _openDetails(BuildContext context) {
    showDialog(
      context: context,
      builder: (ctx) => DownloadDetailDialog(downloadId: item.id),
    );
  }
}

class _Metric extends StatelessWidget {
  final String label;
  final String value;

  const _Metric({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: AppTextStyles.metricLabel),
        const SizedBox(height: AppSpacing.xxs),
        Text(value, style: AppTextStyles.metricValue),
      ],
    );
  }
}
