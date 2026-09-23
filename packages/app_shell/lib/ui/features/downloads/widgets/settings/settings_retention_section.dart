// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Section 2: history retention policy dropdown.
class SettingsRetentionSection extends StatelessWidget {
  final int retentionDays;
  final ValueChanged<int> onChanged;

  const SettingsRetentionSection({
    super.key,
    required this.retentionDays,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            color: AppTheme.primary.withValues(alpha: 0.15),
            borderRadius: BorderRadius.circular(6),
          ),
          child: const Icon(
            Icons.history_rounded,
            size: 18,
            color: AppTheme.primary,
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(context.l10n.historyRetentionPolicy,
                style: TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.bold,
                  color: AppColors.textPrimary,
                ),
              ),
              const SizedBox(height: 4),
              Text(context.l10n.historicalTransitionEventsOlderThan,
                style: TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                  height: 1.3,
                ),
              ),
              const SizedBox(height: 12),
              Wrap(
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: 8,
                runSpacing: 8,
                children: [
                  Text(context.l10n.keepHistoryFor,
                    style: TextStyle(
                      fontSize: 13,
                      color: AppColors.textSecondary,
                    ),
                  ),
                  Container(
                    padding: EdgeInsets.symmetric(horizontal: 10),
                    decoration: BoxDecoration(
                      color: AppTheme.darkCard,
                      borderRadius: BorderRadius.circular(6),
                      border: Border.all(
                        color: AppTheme.darkBorder,
                        width: 0.5,
                      ),
                    ),
                    child: DropdownButtonHideUnderline(
                      child: DropdownButton<int>(
                        value: retentionDays,
                        dropdownColor: AppTheme.darkCard,
                        style: TextStyle(
                          fontSize: 13,
                          color: AppColors.textPrimary,
                        ),
                        items: [
                          DropdownMenuItem(
                            value: 7,
                            child: Text(context.l10n.sevenDays),
                          ),
                          DropdownMenuItem(
                            value: 30,
                            child: Text(context.l10n.thirtyDaysDefault),
                          ),
                          DropdownMenuItem(
                            value: 90,
                            child: Text(context.l10n.ninetyDays),
                          ),
                          DropdownMenuItem(
                            value: 0,
                            child:
                                Text(context.l10n.indefinitelyNoPruning),
                          ),
                          // Valeur persistée hors des choix standards
                          // (ex. via un autre client/MCP) : item dédié
                          // pour éviter l'assertion DropdownButton.
                          if (!const {7, 30, 90, 0}
                              .contains(retentionDays))
                            DropdownMenuItem(
                              value: retentionDays,
                              child: Text(context.l10n
                                  .settingsRetentionDaysCustom(
                                      retentionDays)),
                            ),
                        ],
                        onChanged: (days) {
                          if (days != null) onChanged(days);
                        },
                      ),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}
