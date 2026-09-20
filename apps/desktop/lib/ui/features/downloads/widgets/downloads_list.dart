import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/app_controller.dart';
import 'download_row.dart';

/// Virtualized list of downloads (ListView.builder) with selection handling.
class DownloadsList extends StatelessWidget {
  final AppController viewModel;

  const DownloadsList({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
    final list = viewModel.filteredDownloads;

    return ListView.builder(
      padding: AppSpacing.pagePadding,
      itemCount: list.length,
      itemBuilder: (context, index) {
        final item = list[index];
        final isSelected = viewModel.selectedDownloadId == item.id;

        return DownloadRow(
          key: ValueKey(item.id),
          item: item,
          viewModel: viewModel,
          isSelected: isSelected,
          onTap: () {
            viewModel.selectDownload(isSelected ? null : item.id);
          },
        );
      },
    );
  }
}
