import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../app/app_controller.dart';
import 'download_detail_dialog.dart';
import 'row/download_remove_confirm_dialog.dart';
import 'row/download_row_compact.dart';
import 'row/download_row_full.dart';

/// Selectable download list row — switches between full and compact
/// layouts based on available width.
class DownloadRow extends StatelessWidget {
  final DownloadItem item;
  final AppController viewModel;
  final bool isSelected;
  final VoidCallback onTap;

  const DownloadRow({
    super.key,
    required this.item,
    required this.viewModel,
    required this.isSelected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final category = item.categoryId != null
        ? viewModel.categories
            .where((c) => c.id == item.categoryId)
            .firstOrNull
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
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: 16,
              vertical: 12,
            ),
            child: LayoutBuilder(
              builder: (context, constraints) {
                if (constraints.maxWidth < 850) {
                  return DownloadRowCompact(
                    item: item,
                    category: category,
                    categories: viewModel.categories,
                    onCategorySelected: (catId) =>
                        viewModel.setDownloadCategory(item.id, catId),
                    onDetails: () => _openDetailDialog(context),
                    onPause: () => viewModel.pauseDownload(item.id),
                    onResume: () => viewModel.resumeDownload(item.id),
                    onRemove: () => _confirmRemove(context),
                  );
                }
                return DownloadRowFull(
                  item: item,
                  category: category,
                  categories: viewModel.categories,
                  onCategorySelected: (catId) =>
                      viewModel.setDownloadCategory(item.id, catId),
                  onDetails: () => _openDetailDialog(context),
                  onPause: () => viewModel.pauseDownload(item.id),
                  onResume: () => viewModel.resumeDownload(item.id),
                  onRemove: () => _confirmRemove(context),
                );
              },
            ),
          ),
        ),
      ),
    );
  }

  void _openDetailDialog(BuildContext context) {
    showDialog(
      context: context,
      builder: (ctx) => DownloadDetailDialog(
        downloadId: item.id,
        viewModel: viewModel,
      ),
    );
  }

  void _confirmRemove(BuildContext context) {
    showDownloadRemoveConfirmDialog(
      context,
      item.fileName,
      onConfirm: ({required deleteFiles}) =>
          viewModel.removeDownload(item.id, deleteFiles: deleteFiles),
    );
  }
}
