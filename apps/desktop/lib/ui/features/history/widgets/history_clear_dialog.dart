import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/history_controller.dart';

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
      title: const Row(
        children: [
          Icon(
            Icons.warning_amber_rounded,
            color: AppTheme.statusFailed,
            size: 22,
          ),
          SizedBox(width: 10),
          Text('Clear Download History?'),
        ],
      ),
      content: const Text(
        'Are you sure you want to permanently delete all download history and statistics records?\n\n'
        'This action is irreversible.',
        style: TextStyle(
          fontSize: 13,
          color: AppColors.textSecondary,
          height: 1.4,
        ),
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
          onPressed: () async {
            Navigator.of(ctx).pop();
            await controller.clear();
          },
          child: const Text('Clear All History'),
        ),
      ],
    ),
  );
}
