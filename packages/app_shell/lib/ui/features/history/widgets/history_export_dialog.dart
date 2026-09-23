// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// Shows a sanitized history export (JSON/CSV) with copy-to-clipboard.
void showHistoryExportDialog(
  BuildContext context,
  String format,
  String content,
) {
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
            Icons.file_download_outlined,
            color: AppTheme.primary,
            size: 20,
          ),
          const SizedBox(width: 10),
          Text(context.l10n.exportHistory(format.toUpperCase()),
            style: const TextStyle(
              fontSize: 16,
              color: AppColors.textPrimary,
            ),
          ),
        ],
      ),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 600, maxHeight: 400),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(context.l10n.reportIsSanitizedAllLocal,
              style: TextStyle(fontSize: 12, color: AppColors.accentTeal),
            ),
            const SizedBox(height: 10),
            Expanded(
              child: Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: AppTheme.darkBackground,
                  borderRadius: BorderRadius.circular(6),
                  border: Border.all(
                    color: AppTheme.darkBorder,
                    width: 0.5,
                  ),
                ),
                child: SingleChildScrollView(
                  child: SelectableText(
                    content,
                    style: const TextStyle(
                      fontSize: 11,
                      fontFamily: 'monospace',
                      color: AppColors.textSecondary,
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
      actions: [
        OutlinedButton.icon(
          icon: const Icon(Icons.copy_rounded, size: 16),
          label: Text(context.l10n.copyToClipboard),
          onPressed: () {
            Clipboard.setData(ClipboardData(text: content));
            Navigator.of(ctx).pop();
          },
        ),
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: Text(context.l10n.close),
        ),
      ],
    ),
  );
}
