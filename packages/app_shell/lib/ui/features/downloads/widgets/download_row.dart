import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../app/providers.dart';
import 'download_detail_dialog.dart';
import 'download_sources_expansion.dart';
import 'row/download_remove_confirm_dialog.dart';
import 'row/rename_file_dialog.dart';
import 'row/download_row_compact.dart';
import 'row/download_row_full.dart';

/// Selectable download list row — switches between full and compact
/// layouts based on available width.
class DownloadRow extends ConsumerWidget {
  final DownloadItem item;
  final bool isSelected;
  final VoidCallback onTap;
  final VoidCallback? onDoubleTap;
  final bool expanded;

  const DownloadRow({
    super.key,
    required this.item,
    required this.isSelected,
    required this.onTap,
    this.onDoubleTap,
    this.expanded = false,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final categories = ref.watch(categoriesControllerProvider).categories;
    final downloads = ref.read(downloadsStoreControllerProvider);
    final categoriesCtl = ref.read(categoriesControllerProvider);
    final peerSourcesCtl = ref.read(peerSourcesControllerProvider);
    final category = item.categoryId != null
        ? categories.where((c) => c.id == item.categoryId).firstOrNull
        : null;

    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
        color: isSelected
            ? AppTheme.primary.withValues(alpha: 0.1)
            : AppTheme.darkSurface,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: isSelected ? AppTheme.primary : AppTheme.darkBorder,
          width: isSelected ? 1.5 : 0.5,
        ),
      ),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          onDoubleTap: onDoubleTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            child: Column(
              children: [
                LayoutBuilder(
                  builder: (context, constraints) {
                    if (constraints.maxWidth < 850) {
                      return DownloadRowCompact(
                        item: item,
                        category: category,
                        categories: categories,
                        onCategorySelected: (catId) =>
                            categoriesCtl.assignDownload(item.id, catId),
                        onDetails: () => _openDetailDialog(context, ref),
                        onRename: () => _confirmRename(context, ref),
                        onPause: () => downloads.pause(item.id),
                        onResume: () => downloads.resume(item.id),
                        onRemove: () => _confirmRemove(context, ref),
                        onPrioritySelected: (p) => peerSourcesCtl
                            .setDownloadPriority(item.id, p.name),
                      );
                    }
                    return DownloadRowFull(
                      item: item,
                      category: category,
                      categories: categories,
                      onCategorySelected: (catId) =>
                          categoriesCtl.assignDownload(item.id, catId),
                      onDetails: () => _openDetailDialog(context, ref),
                      onRename: () => _confirmRename(context, ref),
                      onPause: () => downloads.pause(item.id),
                      onResume: () => downloads.resume(item.id),
                      onRemove: () => _confirmRemove(context, ref),
                      onPrioritySelected: (p) => peerSourcesCtl
                          .setDownloadPriority(item.id, p.name),
                    );
                  },
                ),
                if (expanded) DownloadSourcesExpansion(item: item),
              ],
            ),
          ),
        ),
      ),
    );
  }

  void _openDetailDialog(BuildContext context, WidgetRef ref) {
    showDialog(
      context: context,
      builder: (ctx) => DownloadDetailDialog(downloadId: item.id),
    );
  }

  /// Renommage (en cours ou terminé) — le daemon met à jour le fichier
  /// disque et l'entrée de partage quand le download est complété.
  void _confirmRename(BuildContext context, WidgetRef ref) {
    showRenameFileDialog(
      context,
      currentName: item.fileName,
      onSubmit: (newName) => ref
          .read(downloadsStoreControllerProvider)
          .rename(item.id, newName),
    );
  }

  void _confirmRemove(BuildContext context, WidgetRef ref) {
    showDownloadRemoveConfirmDialog(
      context,
      item,
      onConfirm: ({required deleteFiles}) => ref
          .read(downloadsStoreControllerProvider)
          .remove(item.id, deleteFiles: deleteFiles),
    );
  }
}
