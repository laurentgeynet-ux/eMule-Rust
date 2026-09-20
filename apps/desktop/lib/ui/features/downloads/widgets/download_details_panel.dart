import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/downloads_view_model.dart';
import 'download_detail_dialog.dart';

/// Collapsible summary panel shown at the bottom of the downloads workspace
/// when a download is selected. Offers a shortcut to the full detail dialog.
class DownloadDetailsPanel extends StatelessWidget {
  final DownloadItem item;
  final DownloadsViewModel viewModel;
  final bool isSmall;

  const DownloadDetailsPanel({
    super.key,
    required this.item,
    required this.viewModel,
    this.isSmall = false,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: AppSpacing.pagePadding,
      decoration: AppDecorations.footerBar,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.info_outline_rounded, size: 18, color: AppColors.accent),
              const SizedBox(width: AppSpacing.sm),
              Expanded(
                child: Text(
                  'Details: ${item.fileName}',
                  style: const TextStyle(fontSize: 14, fontWeight: FontWeight.bold),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              if (isSmall)
                IconButton(
                  icon: const Icon(Icons.open_in_new_rounded, size: 16),
                  tooltip: 'Inspect Full Details',
                  style: IconButton.styleFrom(foregroundColor: AppColors.accent),
                  onPressed: () => _openDetails(context),
                )
              else
                OutlinedButton.icon(
                  icon: const Icon(Icons.open_in_new_rounded, size: 14),
                  label: const Text('Inspect Full Details', style: TextStyle(fontSize: 12)),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppColors.accent,
                    side: const BorderSide(color: AppColors.accent, width: 0.8),
                    padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                  ),
                  onPressed: () => _openDetails(context),
                ),
              const SizedBox(width: AppSpacing.sm),
              IconButton(
                icon: const Icon(Icons.close, size: 16, color: AppColors.textMuted),
                onPressed: () => viewModel.selectDownload(null),
              ),
            ],
          ),
          const SizedBox(height: AppSpacing.sm),
          Wrap(
            spacing: AppSpacing.xxxl,
            runSpacing: AppSpacing.md,
            children: [
              _Metric(label: 'Download ID', value: item.id),
              _Metric(label: 'Total Size', value: Formatters.formatBytes(item.totalBytes)),
              _Metric(label: 'Verified Bytes', value: Formatters.formatBytes(item.verifiedBytes)),
              _Metric(label: 'Status', value: item.status.label),
              _Metric(label: 'Sources', value: '${item.sourceCount}'),
            ],
          ),
        ],
      ),
    );
  }

  void _openDetails(BuildContext context) {
    showDialog(
      context: context,
      builder: (ctx) => DownloadDetailDialog(
        downloadId: item.id,
        viewModel: viewModel,
      ),
    );
  }
}

class _Metric extends StatelessWidget {
  final String label;
  final String value;

  const _Metric({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: AppTextStyles.metricLabel),
        const SizedBox(height: AppSpacing.xxs),
        Text(value, style: AppTextStyles.metricValue),
      ],
    );
  }
}
