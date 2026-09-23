// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'category_badge.dart';
import 'download_actions_bar.dart';
import 'download_file_icons.dart';
import 'download_priority_chip.dart';
import 'download_status_pill.dart';
import '../../../../../l10n/l10n.dart';

/// Compact (narrow) layout of a download row: stacked name + status +
/// progress + stats for widths below ~850px.
class DownloadRowCompact extends StatelessWidget {
  final DownloadItem item;
  final CategoryItem? category;
  final List<CategoryItem> categories;
  final ValueChanged<String?> onCategorySelected;
  final VoidCallback onDetails;
  final VoidCallback onRename;
  final VoidCallback onPause;
  final VoidCallback onResume;
  final VoidCallback onRemove;
  final ValueChanged<DownloadPriority>? onPrioritySelected;

  const DownloadRowCompact({
    super.key,
    required this.item,
    required this.category,
    required this.categories,
    required this.onCategorySelected,
    required this.onDetails,
    required this.onRename,
    required this.onPause,
    required this.onResume,
    required this.onRemove,
    this.onPrioritySelected,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Container(
              padding: const EdgeInsets.all(6),
              decoration: BoxDecoration(
                color: AppColors.textPrimary.withValues(alpha: 0.05),
                borderRadius: BorderRadius.circular(6),
              ),
              child: Icon(
                fileIconFor(item.fileName),
                color: AppColors.textSecondary,
                size: 18,
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    item.fileName,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w600,
                      color: AppColors.textPrimary,
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 2),
                  Wrap(
                    crossAxisAlignment: WrapCrossAlignment.center,
                    spacing: 8,
                    runSpacing: 2,
                    children: [
                      Text(context.l10n.id(abbreviateId(item.id)),
                        style: const TextStyle(
                          fontSize: 10,
                          color: AppColors.textFaint,
                          fontFamily: 'monospace',
                        ),
                      ),
                      if (category != null)
                        CategoryBadge(category: category!, compact: true),
                      DownloadPriorityChip(
                        value: item.priority,
                        compact: true,
                        onChanged: (p) => onPrioritySelected?.call(p),
                      ),
                    ],
                  ),
                ],
              ),
            ),
            const SizedBox(width: 6),
            DownloadStatusPill(status: item.status, compact: true),
            const SizedBox(width: 4),
            DownloadActionsBar(
              item: item,
              category: category,
              categories: categories,
              onCategorySelected: onCategorySelected,
              onDetails: onDetails,
              onRename: onRename,
              onPause: onPause,
              onResume: onResume,
              onRemove: onRemove,
              compact: true,
            ),
          ],
        ),
        const SizedBox(height: 10),
        PartAvailabilityBar(
          parts: item.parts,
          totalBytes: item.totalBytes,
          pendingRanges: item.pendingRanges,
          progress: item.progress,
          muted: item.status == DownloadStatus.paused ||
              item.status == DownloadStatus.failed,
          height: 12,
        ),
        const SizedBox(height: 6),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text('${Formatters.formatBytes(item.receivedBytes)} / ${Formatters.formatBytes(item.totalBytes)} (${Formatters.formatPercent(item.progress)})',
              style: const TextStyle(
                fontSize: 11,
                color: AppColors.textSecondary,
              ),
            ),
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  Formatters.formatSpeed(item.downloadRateBps),
                  style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                    color: item.downloadRateBps > 0
                        ? AppTheme.statusDownloading
                        : AppColors.textFaint,
                  ),
                ),
                const SizedBox(width: 8),
                Text(context.l10n.src('${item.sourceCount}'),
                  style: const TextStyle(
                    fontSize: 11,
                    color: AppColors.textFaint,
                  ),
                ),
              ],
            ),
          ],
        ),
      ],
    );
  }
}
