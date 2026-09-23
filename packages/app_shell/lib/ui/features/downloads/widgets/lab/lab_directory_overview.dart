// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Header strip showing where internal daemon data vs completed downloads
/// live on disk, with a shortcut to open the downloads folder.
class LabDirectoryOverview extends StatelessWidget {
  final String workdir;
  final String downloadsDir;
  final VoidCallback onOpenFolder;

  const LabDirectoryOverview({
    super.key,
    required this.workdir,
    required this.downloadsDir,
    required this.onOpenFolder,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(
                Icons.folder_outlined,
                color: AppTheme.primary,
                size: 16,
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(context.l10n.storageDataInternalStateVs,
                  style: TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.bold,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 8),
              OutlinedButton.icon(
                icon: const Icon(Icons.folder_open, size: 13),
                label: Text(context.l10n.openFolder,
                  style: TextStyle(fontSize: 11),
                ),
                style: OutlinedButton.styleFrom(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  side: const BorderSide(color: AppTheme.darkBorder),
                  visualDensity: VisualDensity.compact,
                ),
                onPressed: onOpenFolder,
              ),
            ],
          ),
          const SizedBox(height: 4),
          Text(context.l10n.internalDataDbTempPart(workdir),
            style: const TextStyle(
              fontSize: 10,
              fontFamily: 'monospace',
              color: AppColors.textSoft,
            ),
            overflow: TextOverflow.ellipsis,
          ),
          Text(context.l10n.completedDownloads(downloadsDir),
            style: const TextStyle(
              fontSize: 10,
              fontFamily: 'monospace',
              color: AppTheme.primary,
            ),
            overflow: TextOverflow.ellipsis,
          ),
        ],
      ),
    );
  }
}
