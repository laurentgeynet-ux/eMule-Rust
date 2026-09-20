import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

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
            child: Text(
              'Event Detail: ${entry.fileName}',
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
            _row('Event Type', entry.eventType),
            _row('Status', entry.status),
            _row('Total Size', Formatters.formatBytes(entry.totalBytes)),
            _row(
              'Transferred',
              Formatters.formatBytes(entry.transferredBytes),
            ),
            _row('Duration', Formatters.formatDuration(entry.durationSecs)),
            _row('Attempts', '${entry.connectionAttempts}'),
            _row('Resumes', '${entry.resumeCount}'),
            _row('Verification Result', entry.verificationResult),
            _row('eD2K Hash', entry.ed2kHash),
            _row('Sanitized Path', '[DOWNLOAD_DIR]/${entry.fileName}'),
            _row('Timestamp', entry.timestamp.toIso8601String()),
            if (entry.errorMessage != null)
              _row('Error Message', entry.errorMessage!, isError: true),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: const Text('Close'),
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
          child: Text(
            '$label:',
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
