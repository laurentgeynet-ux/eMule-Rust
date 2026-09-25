// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'search_result_row.dart';
import '../../../../l10n/l10n.dart';

/// Virtualized results table with sortable columns (name, size, sources).
/// Minimum table width 950 px with horizontal scroll below that.
class SearchResultsTable extends StatelessWidget {
  final ScrollController scrollController;
  final List<SearchResultFileModel> results;
  final Map<String, DownloadItem> downloadsByHash;
  final int sortColumnIndex;
  final bool sortAscending;
  final void Function(int columnIndex, bool ascending) onSort;
  final Future<bool> Function(SearchResultFileModel file) onDownload;
  final VoidCallback onGoToDownloads;
  final void Function(SearchResultFileModel file) onQuerySources;
  final String? Function() lastError;

  const SearchResultsTable({
    super.key,
    required this.scrollController,
    required this.results,
    required this.downloadsByHash,
    required this.sortColumnIndex,
    required this.sortAscending,
    required this.onSort,
    required this.onDownload,
    required this.onGoToDownloads,
    required this.onQuerySources,
    required this.lastError,
  });

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        const minTableWidth = 950.0;
        final tableWidth = constraints.maxWidth < minTableWidth
            ? minTableWidth
            : constraints.maxWidth;

        return SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: SizedBox(
            width: tableWidth,
            child: Column(
              children: [
                _TableHeader(
                  sortColumnIndex: sortColumnIndex,
                  sortAscending: sortAscending,
                  onSort: onSort,
                ),
                const Divider(
                  color: AppTheme.darkBorder,
                  height: 1,
                  thickness: 0.5,
                ),
                Expanded(
                  child: Scrollbar(
                    controller: scrollController,
                    thumbVisibility: true,
                    child: ListView.builder(
                      controller: scrollController,
                      itemCount: results.length,
                      itemExtent: 44.0,
                      itemBuilder: (context, index) {
                        final file = results[index];
                        final existingDl =
                            downloadsByHash[file.fileHash.toLowerCase()];
                        return SearchResultRow(
                          file: file,
                          index: index,
                          existingDownload: existingDl,
                          onDownload: onDownload,
                          onGoToDownloads: onGoToDownloads,
                          onQuerySources: onQuerySources,
                          lastError: lastError,
                        );
                      },
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

class _TableHeader extends StatelessWidget {
  final int sortColumnIndex;
  final bool sortAscending;
  final void Function(int columnIndex, bool ascending) onSort;

  const _TableHeader({
    required this.sortColumnIndex,
    required this.sortAscending,
    required this.onSort,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 40,
      color: AppTheme.darkSurface,
      padding: const EdgeInsets.symmetric(horizontal: 16),
      child: Row(
        children: [
          _headerCell(label: context.l10n.nomDuFichier, columnIndex: 0, flex: 4),
          const SizedBox(width: 12),
          _headerCell(
            label: context.l10n.taille,
            columnIndex: 1,
            width: 95,
            numeric: true,
          ),
          const SizedBox(width: 12),
          _headerCell(
            label: context.l10n.sources,
            columnIndex: 2,
            width: 100,
            numeric: true,
          ),
          const SizedBox(width: 12),
          _headerCell(
            label: context.l10n.hachageEd2k,
            columnIndex: 3,
            width: 220,
            sortable: false,
          ),
          const SizedBox(width: 12),
          _headerCell(
            label: context.l10n.actions,
            columnIndex: 4,
            width: 330,
            sortable: false,
          ),
        ],
      ),
    );
  }

  Widget _headerCell({
    required String label,
    required int columnIndex,
    int flex = 0,
    double? width,
    bool numeric = false,
    bool sortable = true,
  }) {
    final isSorted = sortColumnIndex == columnIndex;
    final content = Row(
      mainAxisSize: MainAxisSize.min,
      mainAxisAlignment:
          numeric ? MainAxisAlignment.end : MainAxisAlignment.start,
      children: [
        Flexible(
          child: Text(
            label,
            style: TextStyle(
              fontWeight: FontWeight.bold,
              color: isSorted
                  ? AppColors.textPrimary
                  : AppColors.textSecondary,
              fontSize: 12.5,
            ),
            overflow: TextOverflow.ellipsis,
          ),
        ),
        if (sortable) ...[
          const SizedBox(width: 4),
          Icon(
            isSorted
                ? (sortAscending
                      ? Icons.arrow_upward_rounded
                      : Icons.arrow_downward_rounded)
                : Icons.unfold_more_rounded,
            size: 14,
            color: isSorted ? AppTheme.primary : AppColors.fillMedium,
          ),
        ],
      ],
    );

    final Widget cellWidget;
    if (sortable) {
      cellWidget = Align(
        alignment: numeric ? Alignment.centerRight : Alignment.centerLeft,
        child: InkWell(
          onTap: () {
            if (sortColumnIndex == columnIndex) {
              onSort(columnIndex, !sortAscending);
            } else {
              onSort(columnIndex, columnIndex == 0);
            }
          },
          borderRadius: BorderRadius.circular(4),
          child: Padding(
            padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 4),
            child: content,
          ),
        ),
      );
    } else {
      cellWidget = Align(
        alignment: numeric ? Alignment.centerRight : Alignment.centerLeft,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 4),
          child: content,
        ),
      );
    }

    if (flex > 0) {
      return Expanded(flex: flex, child: cellWidget);
    }
    return SizedBox(width: width, child: cellWidget);
  }
}
