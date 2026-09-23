// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../app/app_controller.dart';
import '../../../../../l10n/l10n.dart';

/// Sidebar filter row (All / Active / Paused / Completed / Failed) with
/// count pill; dimmed when the count is zero and not selected.
class SidebarFilterItem extends StatelessWidget {
  final DownloadFilter filter;
  final IconData icon;
  final int count;
  final Color? color;
  final bool isSelected;
  final bool isCompact;
  final VoidCallback onTap;

  const SidebarFilterItem({
    super.key,
    required this.filter,
    required this.icon,
    required this.count,
    this.color,
    required this.isSelected,
    required this.onTap,
    this.isCompact = false,
  });

  @override
  Widget build(BuildContext context) {
    final isDimmed = count == 0 && !isSelected;

    final item = Padding(
      padding: EdgeInsets.symmetric(
        horizontal: isCompact ? 6 : 12,
        vertical: 2,
      ),
      child: Material(
        color: isSelected
            ? AppTheme.primary.withValues(alpha: 0.15)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Padding(
            padding: EdgeInsets.symmetric(
              horizontal: isCompact ? 8 : 12,
              vertical: 6,
            ),
            child: Row(
              mainAxisAlignment: isCompact
                  ? MainAxisAlignment.center
                  : MainAxisAlignment.start,
              children: [
                Stack(
                  clipBehavior: Clip.none,
                  children: [
                    Icon(
                      icon,
                      size: 20,
                      color: isSelected
                          ? AppTheme.primary
                          : (color ?? AppColors.textSecondary),
                    ),
                    if (isCompact && count > 0)
                      Positioned(
                        top: -4,
                        right: -6,
                        child: Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 4,
                            vertical: 1,
                          ),
                          decoration: BoxDecoration(
                            color: color ?? AppTheme.primary,
                            borderRadius: BorderRadius.circular(6),
                          ),
                          child: Text('$count',
                            style: const TextStyle(
                              fontSize: 8,
                              fontWeight: FontWeight.bold,
                              color: AppColors.textPrimary,
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
                if (!isCompact) ...[
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(
                      filter.labelOf(context),
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight: isSelected
                            ? FontWeight.w600
                            : FontWeight.normal,
                        color: isSelected
                            ? AppColors.textPrimary
                            : AppColors.textSecondary,
                      ),
                    ),
                  ),
                  Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 2,
                    ),
                    decoration: BoxDecoration(
                      color: isSelected
                          ? AppTheme.primary.withValues(alpha: 0.3)
                          : AppColors.fillSubtle,
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Text('$count',
                      style: TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w500,
                        color: isSelected
                            ? AppColors.textPrimary
                            : AppColors.textSoft,
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );

    final wrapped = isCompact
        ? Tooltip(
            message: '${filter.labelOf(context)} ($count)',
            preferBelow: false,
            child: item,
          )
        : item;

    if (isDimmed) {
      return Opacity(opacity: 0.55, child: wrapped);
    }
    return wrapped;
  }
}
