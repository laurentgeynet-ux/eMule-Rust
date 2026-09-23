// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Destructive confirmation: delete a category — downloads move back to
/// "Sans catégorie", no files are deleted.
void showCategoryDeleteDialog(
  BuildContext context,
  CategoryItem category, {
  required void Function(String categoryId) onDelete,
}) {
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
          Container(
            width: 12,
            height: 12,
            decoration: BoxDecoration(
              color: category.parsedColor,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 8),
          Text(context.l10n.delete(category.name)),
        ],
      ),
      content: Text(context.l10n.deletingThisCategoryWillMove,
        style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: Text(context.l10n.cancel,
            style: TextStyle(color: AppColors.textSoft),
          ),
        ),
        FilledButton(
          style: FilledButton.styleFrom(
            backgroundColor: AppTheme.statusFailed,
          ),
          onPressed: () {
            Navigator.of(ctx).pop();
            onDelete(category.id);
          },
          child: Text(context.l10n.deleteCategory),
        ),
      ],
    ),
  );
}
