import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Quick remove confirmation used directly from the downloads list row
/// (the richer variant lives in detail/download_remove_dialog.dart).
void showDownloadRemoveConfirmDialog(
  BuildContext context,
  String fileName, {
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
            Text('Remove "$fileName" from downloads?'),
            const SizedBox(height: 12),
            CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: const Text(
                'Delete downloaded part/temp files from disk',
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
              Navigator.of(ctx).pop();
            },
            child: const Text('Remove'),
          ),
        ],
      ),
    ),
  );
}
