// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Sidebar category row: color dot (or icon), name, count pill and an
/// optional edit/delete popup menu on user categories.
class SidebarCategoryItem extends StatelessWidget {
  final String title;
  final Color color;
  final int count;
  final bool isSelected;
  final VoidCallback onTap;
  final IconData? icon;
  final CategoryItem? category;
  final void Function(CategoryItem category) onEdit;
  final void Function(CategoryItem category) onDelete;

  const SidebarCategoryItem({
    super.key,
    required this.title,
    required this.color,
    required this.count,
    required this.isSelected,
    required this.onTap,
    this.icon,
    this.category,
    required this.onEdit,
    required this.onDelete,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 2),
      child: Material(
        color: isSelected
            ? AppTheme.primary.withValues(alpha: 0.15)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: 12,
              vertical: 5,
            ),
            child: Row(
              children: [
                if (icon != null)
                  Icon(icon, size: 16, color: color)
                else
                  Container(
                    width: 12,
                    height: 12,
                    decoration: BoxDecoration(
                      color: color,
                      shape: BoxShape.circle,
                    ),
                  ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    title,
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: isSelected
                          ? FontWeight.w600
                          : FontWeight.normal,
                      color: isSelected
                          ? AppColors.textPrimary
                          : AppColors.textSecondary,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 7,
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
                      fontSize: 11,
                      fontWeight: FontWeight.w500,
                      color: isSelected
                          ? AppColors.textPrimary
                          : AppColors.textSoft,
                    ),
                  ),
                ),
                if (category != null) ...[
                  const SizedBox(width: 4),
                  PopupMenuButton<String>(
                    icon: const Icon(
                      Icons.more_vert_rounded,
                      size: 14,
                      color: AppColors.textFaint,
                    ),
                    padding: EdgeInsets.zero,
                    constraints: BoxConstraints(
                      minWidth: 20,
                      minHeight: 20,
                    ),
                    tooltip: context.l10n.categoryOptions,
                    itemBuilder: (ctx) => [
                      PopupMenuItem(
                        value: 'edit',
                        child: Row(
                          children: [
                            Icon(Icons.edit_outlined, size: 16),
                            SizedBox(width: 8),
                            Text(context.l10n.editCategory),
                          ],
                        ),
                      ),
                      PopupMenuItem(
                        value: 'delete',
                        child: Row(
                          children: [
                            Icon(
                              Icons.delete_outline,
                              size: 16,
                              color: AppTheme.statusFailed,
                            ),
                            SizedBox(width: 8),
                            Text(context.l10n.deleteCategory,
                              style: TextStyle(
                                color: AppTheme.statusFailed,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                    onSelected: (action) {
                      if (action == 'edit') {
                        onEdit(category!);
                      } else if (action == 'delete') {
                        onDelete(category!);
                      }
                    },
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}
