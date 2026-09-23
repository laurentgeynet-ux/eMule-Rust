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
        Flexible(
          child: Text(context.l10n.filterByEvent,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
          ),
        ),
        SizedBox(width: 10),
        Flexible(
          flex: 2,
          child: Container(
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
                isExpanded: true,
                style: TextStyle(
                  fontSize: 13,
                  color: AppColors.textPrimary,
                ),
                items: [
                  DropdownMenuItem(
                    value: null,
                    child: Text(
                      context.l10n.allEvents,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  DropdownMenuItem(
                    value: 'completed',
                    child: Text(
                      context.l10n.completed2,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  DropdownMenuItem(
                    value: 'failed',
                    child: Text(
                      context.l10n.failed,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  DropdownMenuItem(
                    value: 'resumed',
                    child: Text(
                      context.l10n.resumed,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  DropdownMenuItem(
                    value: 'verified_pending_promotion',
                    child: Text(
                      context.l10n.verifying,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  DropdownMenuItem(
                    value: 'removed',
                    child: Text(
                      context.l10n.removed,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
                onChanged: (val) => controller.setEventTypeFilter(val),
              ),
            ),
          ),
        ),
        const Spacer(),
        const SizedBox(width: 10),
        Text(context.l10n.recordS('${controller.entries.length}'),
          style: const TextStyle(fontSize: 12, color: AppColors.textFaint),
        ),
      ],
    );
  }
}
