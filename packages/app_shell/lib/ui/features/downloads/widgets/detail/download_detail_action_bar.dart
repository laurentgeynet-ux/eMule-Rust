import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Bottom action bar of the detail dialog: pause/resume, retry loopback,
/// open completed file, and the destructive remove action.
class DownloadDetailActionBar extends StatelessWidget {
  final DownloadItem item;
  final VoidCallback onPause;
  final VoidCallback onResume;
  final VoidCallback onRetry;
  final VoidCallback onOpenCompletedFile;
  final VoidCallback onRemove;

  const DownloadDetailActionBar({
    super.key,
    required this.item,
    required this.onPause,
    required this.onResume,
    required this.onRetry,
    required this.onOpenCompletedFile,
    required this.onRemove,
  });

  @override
  Widget build(BuildContext context) {
    final isCompleted = item.status.isCompleted;

    return Container(
      padding: const EdgeInsets.all(16),
      decoration: const BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.vertical(bottom: Radius.circular(12)),
        border: Border(
          top: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: Wrap(
        spacing: 10,
        runSpacing: 10,
        alignment: WrapAlignment.spaceBetween,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Wrap(
            spacing: 10,
            runSpacing: 10,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              if (item.status.isPaused)
                FilledButton.icon(
                  icon: const Icon(Icons.play_arrow_rounded, size: 18),
                  label: Text(context.l10n.resume),
                  style: FilledButton.styleFrom(
                    backgroundColor: AppTheme.statusCompleted,
                  ),
                  onPressed: onResume,
                )
              else if (item.status.isActive)
                FilledButton.icon(
                  icon: const Icon(Icons.pause_rounded, size: 18),
                  label: Text(context.l10n.pause),
                  style: FilledButton.styleFrom(
                    backgroundColor: AppTheme.statusPaused,
                  ),
                  onPressed: onPause,
                ),
              OutlinedButton.icon(
                icon: const Icon(Icons.sync_rounded, size: 16),
                label: Text(context.l10n.retryLoopbackSource),
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppColors.textSecondary,
                  side: const BorderSide(color: AppTheme.darkBorder),
                ),
                onPressed: onRetry,
              ),
              Tooltip(
                message: isCompleted
                    ? context.l10n.openFileExplorerTooltip
                    : context.l10n.openFileDisabledTooltip,
                child: FilledButton.icon(
                  icon: const Icon(Icons.folder_open_rounded, size: 16),
                  label: Text(context.l10n.openCompletedFile),
                  style: FilledButton.styleFrom(
                    backgroundColor: isCompleted
                        ? AppTheme.primary
                        : AppColors.fillLight,
                    foregroundColor: isCompleted
                        ? AppColors.textPrimary
                        : AppColors.textFaint,
                  ),
                  onPressed: isCompleted ? onOpenCompletedFile : null,
                ),
              ),
            ],
          ),
          OutlinedButton.icon(
            icon: const Icon(
              Icons.delete_outline_rounded,
              size: 16,
              color: AppTheme.statusFailed,
            ),
            label: Text(context.l10n.remove,
              style: TextStyle(color: AppTheme.statusFailed),
            ),
            style: OutlinedButton.styleFrom(
              side: BorderSide(
                color: AppTheme.statusFailed.withValues(alpha: 0.5),
              ),
            ),
            onPressed: onRemove,
          ),
        ],
      ),
    );
  }
}
