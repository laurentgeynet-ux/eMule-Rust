import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Empty history placeholder.
class HistoryEmptyState extends StatelessWidget {
  const HistoryEmptyState({super.key});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            Icons.history_toggle_off_rounded,
            size: 64,
            color: AppColors.textPrimary.withValues(alpha: 0.15),
          ),
          const SizedBox(height: 16),
          const Text(
            'No download history recorded yet',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
              color: AppColors.textSecondary,
            ),
          ),
          const SizedBox(height: 6),
          const Text(
            'Completed, resumed, failed, and removed transfer events will appear here.',
            style: TextStyle(fontSize: 13, color: AppColors.textFaint),
          ),
        ],
      ),
    );
  }
}
