// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import 'detail/download_detail_action_bar.dart';
import 'detail/download_detail_header.dart';
import 'detail/download_diagnostics_card.dart';
import 'detail/download_identity_card.dart';
import 'detail/download_metrics_card.dart';
import 'detail/download_part_map_card.dart';
import 'detail/download_paths_card.dart';
import 'detail/download_remove_dialog.dart';
import 'detail/download_sources_card.dart';
import '../../../../l10n/l10n.dart';

/// Download detail dialog — composition only. Every section is a dedicated
/// widget under `widgets/detail/`; domain calls go through the view-model
/// facade (or its sub-controllers) until providers reach this dialog.
class DownloadDetailDialog extends ConsumerStatefulWidget {
  final String downloadId;
    const DownloadDetailDialog({
    super.key,
    required this.downloadId,
    });

  @override
  ConsumerState<DownloadDetailDialog> createState() => _DownloadDetailDialogState();
}

class _DownloadDetailDialogState extends ConsumerState<DownloadDetailDialog> {
  @override
  void initState() {
    super.initState();
    Future.microtask(() {
      if (!mounted) return;
      ref.read(downloadsStoreControllerProvider).fetchDetail(widget.downloadId);
      final item = ref.read(filteredDownloadsProvider)
          .where((d) => d.id == widget.downloadId)
          .firstOrNull;
      if (item?.ed2kHash != null && item!.ed2kHash!.isNotEmpty) {
        ref.read(peerSourcesControllerProvider)
            .loadSourcesForHash(item.ed2kHash!);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final item =
        ref.watch(filteredDownloadsProvider)
            .where((d) => d.id == widget.downloadId)
            .firstOrNull ??
        ref.watch(selectedDownloadProvider);

    if (item == null) {
          return AlertDialog(
            title: Text(context.l10n.downloadNotFound),
            content: Text(context.l10n.theRequestedDownloadIsNo,
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(context).pop(),
                child: Text(context.l10n.close),
              ),
            ],
          );
        }

    final store = ref.read(downloadsStoreControllerProvider);
    final categoriesCtl = ref.read(categoriesControllerProvider);
    final categories = ref.watch(categoriesControllerProvider).categories;
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
                          categories: categories,
                          onCategoryChanged: (newCatId) =>
                              categoriesCtl.assignDownload(item.id, newCatId),
                        ),
                        const SizedBox(height: 16),
                        DownloadMetricsCard(
                          item: item,
                          statusColor: statusColor,
                        ),
                        const SizedBox(height: 16),
                        DownloadSourcesCard(
                          item: item,
                          controller: ref.watch(peerSourcesControllerProvider),
                          onQueryServerSources: (hash, size) => ref
                              .read(searchControllerProvider)
                              .querySources(hash, fileSize: size),
                          onQueryKadSources: (hash, size) => ref
                              .read(kadControllerProvider)
                              .searchSources(hash, fileSize: size),
                        ),
                        const SizedBox(height: 16),
                        DownloadPartMapCard(item: item),
                        const SizedBox(height: 16),
                        DownloadPathsCard(
                          item: item,
                          onChangeDestination: (newDir) =>
                              store.changeDestination(item.id, newDir),
                        ),
                        const SizedBox(height: 16),
                        DownloadDiagnosticsCard(
                          item: item,
                          diagnosticBuilder:
                              store.getRedactedDiagnostic,
                        ),
                      ],
                    ),
                  ),
                ),
                DownloadDetailActionBar(
                  item: item,
                  onPause: () => store.pause(item.id),
                  onResume: () => store.resume(item.id),
                  onRetry: () => store.retryConnection(item.id),
                  onOpenCompletedFile: () => store.openCompletedFile(item),
                  onRemove: () => showDownloadRemoveDialog(
                    context,
                    item,
                    onConfirm: ({required deleteFiles}) => store.remove(
                      item.id,
                      deleteFiles: deleteFiles,
                    ),
                  ),
                ),
              ],
            ),
          ),
        );
  }
}
