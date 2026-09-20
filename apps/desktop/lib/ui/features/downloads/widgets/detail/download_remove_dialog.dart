import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Destructive confirmation: remove a download entry, optionally deleting
/// the .part/.part.met temporary files (the completed file is kept).
void showDownloadRemoveDialog(
  BuildContext context,
  DownloadItem item, {
  required void Function({required bool deleteFiles}) onConfirm,
}) {
  bool deleteFilesOnDisk = true;

  showDialog(
    context: context,
    builder: (ctx) => StatefulBuilder(
      builder: (ctx, setDialogState) => AlertDialog(
        title: const Text('Remove Download?'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Remove "${item.fileName}" from the downloads manager?'),
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: AppTheme.darkSurface,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: AppTheme.darkBorder),
              ),
              child: const Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '• The download entry in SQLite will be permanently deleted.\n'
                    '• Temporary files (.part and .part.met) in data/temp/ will be removed if selected below.\n'
                    '• The final completed file in destination (if completed) will NOT be deleted.',
                    style: TextStyle(
                      fontSize: 12,
                      color: AppColors.textSecondary,
                      height: 1.4,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 12),
            CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: const Text(
                'Delete temporary .part file from data/temp/',
                style: TextStyle(fontSize: 13),
              ),
              value: deleteFilesOnDisk,
              onChanged: (val) {
                setDialogState(() {
                  deleteFilesOnDisk = val ?? true;
                });
              },
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: AppTheme.statusFailed,
            ),
            onPressed: () {
              onConfirm(deleteFiles: deleteFilesOnDisk);
              Navigator.of(ctx).pop(); // pop confirm dialog
              Navigator.of(context).pop(); // pop detail dialog
            },
            child: const Text('Confirm Remove'),
          ),
        ],
      ),
    ),
  );
}
