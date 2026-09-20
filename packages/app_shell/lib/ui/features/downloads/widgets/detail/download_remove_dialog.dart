import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

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
        title: Text(context.l10n.removeDownload),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(context.l10n.removeFromTheDownloadsManager(item.fileName)),
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: AppTheme.darkSurface,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: AppTheme.darkBorder),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(context.l10n.theDownloadEntryInSqlite,
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
              title: Text(context.l10n.deleteTemporaryPartFileFrom,
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
              Navigator.of(ctx).pop(); // pop confirm dialog
              Navigator.of(context).pop(); // pop detail dialog
            },
            child: Text(context.l10n.confirmRemove),
          ),
        ],
      ),
    ),
  );
}
