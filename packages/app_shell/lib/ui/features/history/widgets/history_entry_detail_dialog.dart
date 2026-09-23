import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// Read-only detail dialog for a single history entry.
void showHistoryEntryDetailDialog(BuildContext context, HistoryEntry entry) {
  showDialog(
    context: context,
    builder: (ctx) => AlertDialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      title: Row(
        children: [
          const Icon(
            Icons.history_rounded,
            color: AppTheme.primary,
            size: 20,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text(context.l10n.eventDetail(entry.fileName),
              style: const TextStyle(
                fontSize: 16,
                color: AppColors.textPrimary,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 500),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _row(ctx.l10n.eventType, entry.eventType),
            _row(ctx.l10n.statusLabel, entry.status),
            _row(ctx.l10n.totalSize, Formatters.formatBytes(entry.totalBytes)),
            _row(
              ctx.l10n.transferred,
              Formatters.formatBytes(entry.transferredBytes),
            ),
            _row(ctx.l10n.duration, Formatters.formatDuration(entry.durationSecs)),
            _row(ctx.l10n.attempts, '${entry.connectionAttempts}'),
            _row(ctx.l10n.resumes, '${entry.resumeCount}'),
            _row(ctx.l10n.verificationResult, entry.verificationResult),
            _row(ctx.l10n.ed2kHash, entry.ed2kHash),
            _row(ctx.l10n.sanitizedPath, '[DOWNLOAD_DIR]/${entry.fileName}'),
            _row(ctx.l10n.timestamp, entry.timestamp.toIso8601String()),
            if (entry.errorMessage != null)
              _row(ctx.l10n.errorMessage, entry.errorMessage!, isError: true),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: Text(context.l10n.close),
        ),
      ],
    ),
  );
}

Widget _row(String label, String value, {bool isError = false}) {
  return Padding(
    padding: const EdgeInsets.symmetric(vertical: 4),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 140,
          child: Text('$label:',
            style: const TextStyle(
              fontSize: 12,
              color: AppColors.textMuted,
              fontWeight: FontWeight.w500,
            ),
          ),
        ),
        Expanded(
          child: Text(
            value,
            style: TextStyle(
              fontSize: 12,
              color: isError
                  ? AppTheme.statusFailed
                  : AppColors.textPrimary,
              fontFamily: label.contains('Hash') || label.contains('Path')
                  ? 'monospace'
                  : null,
            ),
          ),
        ),
      ],
    ),
  );
}
