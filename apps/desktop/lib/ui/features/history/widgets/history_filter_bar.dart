import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/history_controller.dart';

/// Event-type filter dropdown + record count.
class HistoryFilterBar extends StatelessWidget {
  final HistoryController controller;

  const HistoryFilterBar({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        const Text(
          'Filter by Event:',
          style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
        ),
        const SizedBox(width: 10),
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(
            color: AppTheme.darkCard,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: AppTheme.darkBorder, width: 0.5),
          ),
          child: DropdownButtonHideUnderline(
            child: DropdownButton<String?>(
              value: controller.eventTypeFilter,
              dropdownColor: AppTheme.darkCard,
              style: const TextStyle(
                fontSize: 13,
                color: AppColors.textPrimary,
              ),
              items: const [
                DropdownMenuItem(value: null, child: Text('All Events')),
                DropdownMenuItem(
                  value: 'completed',
                  child: Text('Completed'),
                ),
                DropdownMenuItem(value: 'failed', child: Text('Failed')),
                DropdownMenuItem(value: 'resumed', child: Text('Resumed')),
                DropdownMenuItem(
                  value: 'verified_pending_promotion',
                  child: Text('Verifying'),
                ),
                DropdownMenuItem(value: 'removed', child: Text('Removed')),
              ],
              onChanged: (val) => controller.setEventTypeFilter(val),
            ),
          ),
        ),
        const Spacer(),
        Text(
          '${controller.entries.length} record(s)',
          style: const TextStyle(fontSize: 12, color: AppColors.textFaint),
        ),
      ],
    );
  }
}
