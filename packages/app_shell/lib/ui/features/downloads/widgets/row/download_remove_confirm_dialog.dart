import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

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
        title: Text(context.l10n.removeDownload),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(context.l10n.removeFromDownloads(fileName)),
            const SizedBox(height: 12),
            CheckboxListTile(
              contentPadding: EdgeInsets.zero,
              dense: true,
              title: Text(context.l10n.deleteDownloadedPartTempFiles,
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
            child: Text(context.l10n.cancel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: AppTheme.statusFailed,
            ),
            onPressed: () {
              onConfirm(deleteFiles: deleteFilesOnDisk);
              Navigator.of(ctx).pop();
            },
            child: Text(context.l10n.remove2),
          ),
        ],
      ),
    ),
  );
}
