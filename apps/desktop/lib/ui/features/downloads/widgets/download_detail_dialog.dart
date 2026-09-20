import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/downloads_view_model.dart';
import 'detail/download_detail_action_bar.dart';
import 'detail/download_detail_header.dart';
import 'detail/download_diagnostics_card.dart';
import 'detail/download_identity_card.dart';
import 'detail/download_metrics_card.dart';
import 'detail/download_part_map_card.dart';
import 'detail/download_paths_card.dart';
import 'detail/download_remove_dialog.dart';
import 'detail/download_sources_card.dart';

/// Download detail dialog — composition only. Every section is a dedicated
/// widget under `widgets/detail/`; domain calls go through the view-model
/// facade (or its sub-controllers) until providers reach this dialog.
class DownloadDetailDialog extends StatefulWidget {
  final String downloadId;
  final DownloadsViewModel viewModel;

  const DownloadDetailDialog({
    super.key,
    required this.downloadId,
    required this.viewModel,
  });

  @override
  State<DownloadDetailDialog> createState() => _DownloadDetailDialogState();
}

class _DownloadDetailDialogState extends State<DownloadDetailDialog> {
  @override
  void initState() {
    super.initState();
    widget.viewModel.fetchDownloadDetail(widget.downloadId);
    final item = widget.viewModel.filteredDownloads
        .where((d) => d.id == widget.downloadId)
        .firstOrNull;
    if (item?.ed2kHash != null && item!.ed2kHash!.isNotEmpty) {
      widget.viewModel.loadSourcesForHash(item.ed2kHash!);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.viewModel,
      builder: (context, _) {
        final item =
            widget.viewModel.filteredDownloads
                .where((d) => d.id == widget.downloadId)
                .firstOrNull ??
            widget.viewModel.selectedDownload;

        if (item == null) {
          return AlertDialog(
            title: const Text('Download Not Found'),
            content: const Text(
              'The requested download is no longer available.',
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(context).pop(),
                child: const Text('Close'),
              ),
            ],
          );
        }

        final vm = widget.viewModel;
        final statusColor = AppTheme.getStatusColor(item.status);

        return Dialog(
          backgroundColor: AppTheme.darkSurface,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
            side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
          ),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 860, maxHeight: 780),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                DownloadDetailHeader(item: item, statusColor: statusColor),
                Expanded(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.all(20),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        DownloadIdentityCard(
                          item: item,
                          categories: vm.categories,
                          onCategoryChanged: (newCatId) =>
                              vm.setDownloadCategory(item.id, newCatId),
                        ),
                        const SizedBox(height: 16),
                        DownloadMetricsCard(
                          item: item,
                          statusColor: statusColor,
                        ),
                        const SizedBox(height: 16),
                        DownloadSourcesCard(
                          item: item,
                          controller: vm.peerSources,
                          onQueryServerSources: (hash, size) => vm
                              .queryServerSources(hash, fileSize: size),
                          onQueryKadSources: (hash, size) => vm
                              .searchKadSources(hash, fileSize: size),
                        ),
                        const SizedBox(height: 16),
                        DownloadPartMapCard(item: item),
                        const SizedBox(height: 16),
                        DownloadPathsCard(
                          item: item,
                          onChangeDestination: (newDir) =>
                              vm.changeDownloadDestination(item.id, newDir),
                        ),
                        const SizedBox(height: 16),
                        DownloadDiagnosticsCard(
                          item: item,
                          diagnosticBuilder:
                              vm.getRedactedDownloadDiagnostic,
                        ),
                      ],
                    ),
                  ),
                ),
                DownloadDetailActionBar(
                  item: item,
                  onPause: () => vm.pauseDownload(item.id),
                  onResume: () => vm.resumeDownload(item.id),
                  onRetry: () => vm.retryDownloadConnection(item.id),
                  onOpenCompletedFile: () => vm.openCompletedFile(item),
                  onRemove: () => showDownloadRemoveDialog(
                    context,
                    item,
                    onConfirm: ({required deleteFiles}) => vm.removeDownload(
                      item.id,
                      deleteFiles: deleteFiles,
                    ),
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
