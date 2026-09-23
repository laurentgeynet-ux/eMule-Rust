import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Destructive confirmation: remove a download entry.
/// In-progress downloads always have their .part/.part.met temp files
/// deleted; for completed downloads, a checkbox offers to also delete the
/// shared finished file in the destination folder.
void showDownloadRemoveDialog(
  BuildContext context,
  DownloadItem item, {
  required void Function({required bool deleteFiles}) onConfirm,
}) {
  final isCompleted = item.status.isCompleted;
  bool deleteSharedFile = false;

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
            if (isCompleted) ...[
              const SizedBox(height: 12),
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                dense: true,
                title: Text(context.l10n.deleteSharedCompletedFile,
                  style: TextStyle(fontSize: 13),
                ),
                value: deleteSharedFile,
                onChanged: (val) {
                  setDialogState(() {
                    deleteSharedFile = val ?? false;
                  });
                },
              ),
            ],
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
              // En cours : fichiers temporaires toujours supprimés ;
              // terminé : le fichier partagé n'est supprimé que sur demande.
              onConfirm(
                deleteFiles: isCompleted ? deleteSharedFile : true,
              );
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
