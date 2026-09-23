// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Green preview card shown once the pasted ed2k link parses correctly.
class AddLinkPreviewCard extends StatelessWidget {
  final String fileName;
  final int sizeBytes;
  final String hash;

  const AddLinkPreviewCard({
    super.key,
    required this.fileName,
    required this.sizeBytes,
    required this.hash,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: AppTheme.primary.withValues(alpha: 0.5),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(
                Icons.check_circle_outline,
                color: AppTheme.statusCompleted,
                size: 16,
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  fileName,
                  style: const TextStyle(
                    fontWeight: FontWeight.bold,
                    fontSize: 13,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              Text(
                Formatters.formatBytes(sizeBytes),
                style: const TextStyle(
                  color: AppTheme.statusDownloading,
                  fontWeight: FontWeight.w600,
                  fontSize: 12,
                ),
              ),
            ],
          ),
          const SizedBox(height: 4),
          Text(context.l10n.hash(hash),
            style: const TextStyle(
              fontSize: 11,
              color: AppColors.textMuted,
              fontFamily: 'monospace',
            ),
          ),
        ],
      ),
    );
  }
}
