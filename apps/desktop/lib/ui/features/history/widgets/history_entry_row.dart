import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'history_colors.dart';
import 'history_entry_detail_dialog.dart';

/// One history entry row — stacked layout below 700 px, columns above.
class HistoryEntryRow extends StatelessWidget {
  final HistoryEntry entry;

  const HistoryEntryRow({super.key, required this.entry});

  @override
  Widget build(BuildContext context) {
    final color = historyEventColor(entry.eventType);

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isCompact = constraints.maxWidth < 700;

          if (isCompact) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    _eventPill(color, compact: true),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        entry.fileName,
                        style: const TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: AppColors.textPrimary,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      formatHistoryTimestamp(entry.timestamp),
                      style: const TextStyle(
                        fontSize: 11,
                        color: AppColors.textMuted,
                      ),
                    ),
                    IconButton(
                      icon: const Icon(
                        Icons.info_outline_rounded,
                        size: 16,
                        color: AppColors.textMuted,
                      ),
                      padding: EdgeInsets.zero,
                      constraints: const BoxConstraints(
                        minWidth: 24,
                        minHeight: 24,
                      ),
                      tooltip: 'View details',
                      onPressed: () =>
                          showHistoryEntryDetailDialog(context, entry),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(
                      '${Formatters.formatBytes(entry.transferredBytes)} of '
                      '${Formatters.formatBytes(entry.totalBytes)}',
                      style: const TextStyle(
                        fontSize: 11,
                        color: AppColors.textSecondary,
                      ),
                    ),
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          Formatters.formatDuration(entry.durationSecs),
                          style: const TextStyle(
                            fontSize: 11,
                            color: AppColors.textMuted,
                          ),
                        ),
                        const SizedBox(width: 8),
                        _verifBadge(),
                      ],
                    ),
                  ],
                ),
              ],
            );
          }

          return Row(
            children: [
              _eventPill(color),
              const SizedBox(width: 16),
              Expanded(
                flex: 4,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      entry.fileName,
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                        color: AppColors.textPrimary,
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 2),
                    Text(
                      'Hash: ${entry.ed2kHash}',
                      style: const TextStyle(
                        fontSize: 11,
                        color: AppColors.textFaint,
                        fontFamily: 'monospace',
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ),
              ),
              Expanded(
                flex: 2,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      Formatters.formatBytes(entry.transferredBytes),
                      style: const TextStyle(
                        fontSize: 13,
                        color: AppColors.textPrimary,
                      ),
                    ),
                    Text(
                      'of ${Formatters.formatBytes(entry.totalBytes)}',
                      style: const TextStyle(
                        fontSize: 11,
                        color: AppColors.textFaint,
                      ),
                    ),
                  ],
                ),
              ),
              Expanded(
                flex: 2,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      Formatters.formatDuration(entry.durationSecs),
                      style: const TextStyle(
                        fontSize: 13,
                        color: AppColors.textPrimary,
                      ),
                    ),
                    Text(
                      '${entry.resumeCount} res. / '
                      '${entry.connectionAttempts} att.',
                      style: const TextStyle(
                        fontSize: 11,
                        color: AppColors.textFaint,
                      ),
                    ),
                  ],
                ),
              ),
              _verifBadge(),
              const SizedBox(width: 16),
              Text(
                formatHistoryTimestamp(entry.timestamp),
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                ),
              ),
              const SizedBox(width: 8),
              IconButton(
                icon: const Icon(
                  Icons.info_outline_rounded,
                  size: 18,
                  color: AppColors.textMuted,
                ),
                tooltip: 'View details',
                onPressed: () =>
                    showHistoryEntryDetailDialog(context, entry),
              ),
            ],
          );
        },
      ),
    );
  }

  Widget _eventPill(Color color, {bool compact = false}) {
    return Container(
      padding: EdgeInsets.symmetric(
        horizontal: compact ? 6 : 8,
        vertical: compact ? 2 : 4,
      ),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(compact ? 4 : 6),
        border: Border.all(color: color.withValues(alpha: 0.4), width: 0.5),
      ),
      child: Text(
        entry.eventType.toUpperCase(),
        style: TextStyle(
          fontSize: compact ? 10 : 11,
          fontWeight: FontWeight.bold,
          color: color,
        ),
      ),
    );
  }

  Widget _verifBadge() {
    final color = historyVerificationColor(entry.verificationResult);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        entry.verificationResult,
        style: TextStyle(fontSize: 11, color: color),
      ),
    );
  }
}
