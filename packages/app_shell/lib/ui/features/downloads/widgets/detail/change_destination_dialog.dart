// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Dialog: change the final destination directory of a download. The
/// .part data stays confined in data/temp/ until verification completes.
void showChangeDestinationDialog(
  BuildContext context,
  DownloadItem item, {
  required Future<bool> Function(String newDirectory) onSave,
}) {
  final target = item.targetPath ?? 'downloads';
  final controller = TextEditingController(
    text: target.contains('/') || target.contains(r'\')
        ? (target.replaceAll(r'\', '/').split('/')..removeLast()).join('/')
        : target,
  );

  showDialog(
    context: context,
    builder: (ctx) => AlertDialog(
      title: Text(context.l10n.changeTargetDestination),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(context.l10n.selectTargetDestinationFolderFor(item.fileName)),
          const SizedBox(height: 12),
          TextField(
            controller: controller,
            decoration: InputDecoration(
              labelText: context.l10n.targetDirectoryPath,
              hintText: context.l10n.eGDownloadsOrC,
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 8),
          Text(context.l10n.theFolderMustExistAnd,
            style: TextStyle(fontSize: 11, color: AppColors.textMuted),
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: Text(context.l10n.cancel),
        ),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: AppTheme.primary),
          onPressed: () async {
            final newDir = controller.text.trim();
            if (newDir.isNotEmpty) {
              final success = await onSave(newDir);
              if (ctx.mounted && success) {
                Navigator.of(ctx).pop();
              }
            }
          },
          child: Text(context.l10n.saveDestination),
        ),
      ],
    ),
  );
}
