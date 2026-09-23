// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Quick remove confirmation used directly from the downloads list row
/// (the richer variant lives in detail/download_remove_dialog.dart).
///
/// In-progress downloads always have their .part/.part.met temp files
/// deleted — no checkbox. For completed downloads, a checkbox offers to
/// also delete the shared finished file in the destination folder.
void showDownloadRemoveConfirmDialog(
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
            Text(context.l10n.removeFromDownloads(item.fileName)),
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
              Navigator.of(ctx).pop();
            },
            child: Text(context.l10n.remove2),
          ),
        ],
      ),
    ),
  );
}
