import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

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
          Text('Delete "${category.name}"?'),
        ],
      ),
      content: const Text(
        'Deleting this category will move all associated downloads to "Sans catégorie" (Uncategorized).\n\n'
        'No downloads or files will be deleted.',
        style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: const Text(
            'Cancel',
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
          child: const Text('Delete Category'),
        ),
      ],
    ),
  );
}
