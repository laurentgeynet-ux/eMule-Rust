import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/history_controller.dart';
import '../../../../l10n/l10n.dart';

/// Event-type filter dropdown + record count.
class HistoryFilterBar extends StatelessWidget {
  final HistoryController controller;

  const HistoryFilterBar({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Text(context.l10n.filterByEvent,
          style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
        ),
        SizedBox(width: 10),
        Container(
          padding: EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(
            color: AppTheme.darkCard,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: AppTheme.darkBorder, width: 0.5),
          ),
          child: DropdownButtonHideUnderline(
            child: DropdownButton<String?>(
              value: controller.eventTypeFilter,
              dropdownColor: AppTheme.darkCard,
              style: TextStyle(
                fontSize: 13,
                color: AppColors.textPrimary,
              ),
              items: [
                DropdownMenuItem(value: null, child: Text(context.l10n.allEvents)),
                DropdownMenuItem(
                  value: 'completed',
                  child: Text(context.l10n.completed2),
                ),
                DropdownMenuItem(value: 'failed', child: Text(context.l10n.failed)),
                DropdownMenuItem(value: 'resumed', child: Text(context.l10n.resumed)),
                DropdownMenuItem(
                  value: 'verified_pending_promotion',
                  child: Text(context.l10n.verifying),
                ),
                DropdownMenuItem(value: 'removed', child: Text(context.l10n.removed)),
              ],
              onChanged: (val) => controller.setEventTypeFilter(val),
            ),
          ),
        ),
        const Spacer(),
        Text(context.l10n.recordS('${controller.entries.length}'),
          style: const TextStyle(fontSize: 12, color: AppColors.textFaint),
        ),
      ],
    );
  }
}
