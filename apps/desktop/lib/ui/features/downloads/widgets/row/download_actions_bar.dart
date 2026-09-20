import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Per-row action cluster: assign category, inspect details,
/// pause/resume, remove.
class DownloadActionsBar extends StatelessWidget {
  final DownloadItem item;
  final CategoryItem? category;
  final List<CategoryItem> categories;
  final bool compact;
  final ValueChanged<String?> onCategorySelected;
  final VoidCallback onDetails;
  final VoidCallback onPause;
  final VoidCallback onResume;
  final VoidCallback onRemove;

  const DownloadActionsBar({
    super.key,
    required this.item,
    required this.category,
    required this.categories,
    required this.onCategorySelected,
    required this.onDetails,
    required this.onPause,
    required this.onResume,
    required this.onRemove,
    this.compact = false,
  });

  @override
  Widget build(BuildContext context) {
    final category = this.category;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        PopupMenuButton<String?>(
          icon: Icon(
            category != null
                ? Icons.label_rounded
                : Icons.label_outline_rounded,
            color: category != null
                ? category.parsedColor
                : AppColors.textMuted,
            size: compact ? 16 : 18,
          ),
          tooltip: category != null
              ? 'Category: ${category.name}'
              : 'Assign Category',
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
          onSelected: onCategorySelected,
          itemBuilder: (ctx) => [
            const PopupMenuItem<String?>(
              value: null,
              child: Text('Sans catégorie (None)'),
            ),
            ...categories.map(
              (c) => PopupMenuItem<String?>(
                value: c.id,
                child: Row(
                  children: [
                    Container(
                      width: 10,
                      height: 10,
                      decoration: BoxDecoration(
                        color: c.parsedColor,
                        shape: BoxShape.circle,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Text(c.name),
                  ],
                ),
              ),
            ),
          ],
        ),
        IconButton(
          icon: Icon(
            Icons.info_outline_rounded,
            color: AppColors.textSecondary,
            size: compact ? 16 : 20,
          ),
          tooltip: 'Inspect Details & Part Map',
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
          onPressed: onDetails,
        ),
        if (item.status.isPaused)
          IconButton(
            icon: Icon(
              Icons.play_arrow_rounded,
              color: AppTheme.statusCompleted,
              size: compact ? 18 : 24,
            ),
            tooltip: 'Resume',
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
            onPressed: onResume,
          )
        else if (item.status.isActive)
          IconButton(
            icon: Icon(
              Icons.pause_rounded,
              color: AppTheme.statusPaused,
              size: compact ? 18 : 24,
            ),
            tooltip: 'Pause',
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
            onPressed: onPause,
          ),
        IconButton(
          icon: Icon(
            Icons.delete_outline_rounded,
            color: AppColors.textMuted,
            size: compact ? 16 : 20,
          ),
          tooltip: 'Remove',
          padding: EdgeInsets.zero,
          constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
          onPressed: onRemove,
        ),
      ],
    );
  }
}
