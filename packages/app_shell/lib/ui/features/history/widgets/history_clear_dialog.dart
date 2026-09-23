// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/history_controller.dart';
import '../../../../l10n/l10n.dart';

/// Destructive confirmation: wipe all download history and statistics.
void showHistoryClearDialog(
  BuildContext context,
  HistoryController controller,
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
          Icon(
            Icons.warning_amber_rounded,
            color: AppTheme.statusFailed,
            size: 22,
          ),
          SizedBox(width: 10),
          Text(context.l10n.clearDownloadHistory),
        ],
      ),
      content: Text(context.l10n.areYouSureYouWant,
        style: TextStyle(
          fontSize: 13,
          color: AppColors.textSecondary,
          height: 1.4,
        ),
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
          onPressed: () async {
            Navigator.of(ctx).pop();
            await controller.clear();
          },
          child: Text(context.l10n.clearAllHistory),
        ),
      ],
    ),
  );
}
