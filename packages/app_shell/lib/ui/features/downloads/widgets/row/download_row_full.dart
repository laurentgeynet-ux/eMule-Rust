import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'category_badge.dart';
import 'download_actions_bar.dart';
import 'download_file_icons.dart';
import 'download_priority_chip.dart';
import 'download_status_pill.dart';
import '../../../../../l10n/l10n.dart';

/// Full (wide) layout of a download row: icon, name + id + category,
/// size + progress, percent, status pill, rate/sources and actions.
class DownloadRowFull extends StatelessWidget {
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

  const DownloadRowFull({
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
    final statusColor = AppTheme.getStatusColor(item.status);

    return Row(
      children: [
        Container(
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            color: AppColors.textPrimary.withValues(alpha: 0.05),
            borderRadius: BorderRadius.circular(6),
          ),
          child: Icon(
            fileIconFor(item.fileName),
            color: AppColors.textSecondary,
            size: 20,
          ),
        ),
        const SizedBox(width: 14),
        Expanded(
          flex: 4,
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
                runSpacing: 4,
                children: [
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Flexible(
                        child: Tooltip(
                          message: context.l10n.fullId(item.id),
                          preferBelow: false,
                          child: Text(context.l10n.id(abbreviateId(item.id)),
                            style: const TextStyle(
                              fontSize: 11,
                              color: AppColors.textFaint,
                              fontFamily: 'monospace',
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ),
                      const SizedBox(width: 4),
                      InkWell(
                        borderRadius: BorderRadius.circular(4),
                        onTap: () {
                          Clipboard.setData(
                            ClipboardData(text: item.id),
                          );
                        },
                        child: const Padding(
                          padding: EdgeInsets.all(2.0),
                          child: Icon(
                            Icons.copy_rounded,
                            size: 11,
                            color: AppColors.textFaint,
                          ),
                        ),
                      ),
                    ],
                  ),
                  if (category != null)
                    CategoryBadge(category: category!),
                  DownloadPriorityChip(
                    value: item.priority,
                    onChanged: (p) => onPrioritySelected?.call(p),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(width: 16),
        Expanded(
          flex: 3,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('${Formatters.formatBytes(item.receivedBytes)} / ${Formatters.formatBytes(item.totalBytes)}',
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textSecondary,
                ),
              ),
              const SizedBox(height: 4),
              ClipRRect(
                borderRadius: BorderRadius.circular(4),
                child: LinearProgressIndicator(
                  value: item.progress,
                  minHeight: 6,
                  backgroundColor: AppColors.fillLight,
                  valueColor: AlwaysStoppedAnimation<Color>(statusColor),
                ),
              ),
              const SizedBox(height: 3),
              PartAvailabilityBar(
                parts: item.parts,
                totalBytes: item.totalBytes,
                pendingRanges: item.pendingRanges,
                progress: item.progress,
                muted: item.status == DownloadStatus.paused ||
                    item.status == DownloadStatus.failed,
                height: 9,
              ),
            ],
          ),
        ),
        const SizedBox(width: 16),
        SizedBox(
          width: 55,
          child: Text(
            Formatters.formatPercent(item.progress),
            style: const TextStyle(
              fontSize: 13,
              fontWeight: FontWeight.w600,
              color: AppColors.textPrimary,
            ),
          ),
        ),
        DownloadStatusPill(status: item.status),
        const SizedBox(width: 16),
        SizedBox(
          width: 110,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(
                Formatters.formatSpeed(item.downloadRateBps),
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: item.downloadRateBps > 0
                      ? AppTheme.statusDownloading
                      : AppColors.textFaint,
                ),
              ),
              const SizedBox(height: 2),
              Text(context.l10n.sources2('${item.sourceCount}'),
                style: const TextStyle(
                  fontSize: 11,
                  color: AppColors.textMuted,
                ),
              ),
              // L'emplacement ETA est toujours réservé (visible ou non) :
              // les lignes « en cours » et « terminées » gardent la même
              // hauteur et le même alignement vertical.
              Visibility(
                visible: item.etaSeconds != null,
                maintainSize: true,
                maintainAnimation: true,
                maintainState: true,
                child: Text(
                  item.etaSeconds != null
                      ? '${context.l10n.eta}: ${Formatters.formatDuration(item.etaSeconds!)}'
                      : '',
                  style: const TextStyle(
                    fontSize: 11,
                    color: AppColors.textMuted,
                  ),
                ),
              ),
            ],
          ),
        ),
        const SizedBox(width: 16),
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
        ),
      ],
    );
  }
}
