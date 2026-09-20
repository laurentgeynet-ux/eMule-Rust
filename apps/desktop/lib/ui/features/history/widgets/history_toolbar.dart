import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/history_controller.dart';
import 'history_clear_dialog.dart';
import 'history_export_dialog.dart';

/// History section toolbar: title, export menu (JSON/CSV), clear-all and
/// refresh actions.
class HistoryToolbar extends StatelessWidget {
  final HistoryController controller;

  const HistoryToolbar({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isCompact = constraints.maxWidth < 680;

          return Row(
            children: [
              const Icon(
                Icons.history_rounded,
                color: AppTheme.primary,
                size: 22,
              ),
              const SizedBox(width: 10),
              const Expanded(
                child: Text(
                  'History & Local Statistics',
                  style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.bold,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 8),
              PopupMenuButton<String>(
                tooltip: 'Export history report',
                color: AppTheme.darkCard,
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(8),
                  side: const BorderSide(
                    color: AppTheme.darkBorder,
                    width: 0.5,
                  ),
                ),
                onSelected: (format) async {
                  final content = await controller.export(format);
                  if (context.mounted) {
                    showHistoryExportDialog(context, format, content);
                  }
                },
                itemBuilder: (ctx) => const [
                  PopupMenuItem(
                    value: 'json',
                    child: Row(
                      children: [
                        Icon(
                          Icons.data_object_rounded,
                          size: 18,
                          color: AppColors.textSecondary,
                        ),
                        SizedBox(width: 8),
                        Flexible(
                          child: Text(
                            'Export as JSON (Sanitized)',
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                  ),
                  PopupMenuItem(
                    value: 'csv',
                    child: Row(
                      children: [
                        Icon(
                          Icons.table_chart_outlined,
                          size: 18,
                          color: AppColors.textSecondary,
                        ),
                        SizedBox(width: 8),
                        Flexible(
                          child: Text(
                            'Export as CSV (Sanitized)',
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
                child: Container(
                  padding: EdgeInsets.symmetric(
                    horizontal: isCompact ? 8 : 12,
                    vertical: 6,
                  ),
                  decoration: BoxDecoration(
                    color: AppColors.fillLight,
                    borderRadius: BorderRadius.circular(6),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const Icon(
                        Icons.file_download_outlined,
                        size: 16,
                        color: AppColors.textSecondary,
                      ),
                      if (!isCompact) ...[
                        const SizedBox(width: 6),
                        const Text(
                          'Export Report',
                          style: TextStyle(
                            fontSize: 13,
                            color: AppColors.textSecondary,
                          ),
                        ),
                        const SizedBox(width: 4),
                        const Icon(
                          Icons.arrow_drop_down,
                          size: 18,
                          color: AppColors.textSecondary,
                        ),
                      ],
                    ],
                  ),
                ),
              ),
              const SizedBox(width: 8),
              if (isCompact)
                IconButton(
                  style: IconButton.styleFrom(
                    foregroundColor: AppTheme.statusFailed,
                  ),
                  icon: const Icon(Icons.delete_sweep_outlined, size: 18),
                  tooltip: 'Clear History',
                  onPressed: () =>
                      showHistoryClearDialog(context, controller),
                )
              else
                OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppTheme.statusFailed,
                    side: BorderSide(
                      color: AppTheme.statusFailed.withValues(alpha: 0.5),
                    ),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 8,
                    ),
                  ),
                  icon: const Icon(Icons.delete_sweep_outlined, size: 16),
                  label: const Text('Clear History'),
                  onPressed: () =>
                      showHistoryClearDialog(context, controller),
                ),
              const SizedBox(width: 6),
              IconButton(
                icon: const Icon(
                  Icons.refresh_rounded,
                  size: 20,
                  color: AppColors.textSecondary,
                ),
                tooltip: 'Refresh',
                onPressed: () => controller.fetch(),
              ),
            ],
          );
        },
      ),
    );
  }
}
