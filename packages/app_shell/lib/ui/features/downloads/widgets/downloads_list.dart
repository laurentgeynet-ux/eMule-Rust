// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:ui_kit/ui_kit.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../app/providers.dart';
import 'download_row.dart';

/// Virtualized list of downloads (ListView.builder) with selection handling.
/// Double-tapping a row expands it inline to show the eD2K/Kad source list,
/// like eMule's expandable download rows.
class DownloadsList extends ConsumerStatefulWidget {
  const DownloadsList({super.key});

  @override
  ConsumerState<DownloadsList> createState() => _DownloadsListState();
}

class _DownloadsListState extends ConsumerState<DownloadsList> {
  String? _expandedId;

  void _toggleExpanded(String id, String? ed2kHash) {
    setState(() {
      _expandedId = _expandedId == id ? null : id;
    });
    if (_expandedId == id && ed2kHash != null && ed2kHash.isNotEmpty) {
      ref.read(peerSourcesControllerProvider).loadSourcesForHash(ed2kHash);
    }
  }

  @override
  Widget build(BuildContext context) {
    final list = ref.watch(filteredDownloadsProvider);

    return ListView.builder(
      padding: AppSpacing.pagePadding,
      itemCount: list.length,
      itemBuilder: (context, index) {
        final item = list[index];
        final isSelected =
            ref.watch(downloadsStoreControllerProvider).selectedId == item.id;

        return DownloadRow(
          key: ValueKey(item.id),
          item: item,
          isSelected: isSelected,
          onTap: () {
            final router = GoRouter.maybeOf(context);
            if (router != null) {
              router.go(
                isSelected
                    ? '/downloads'
                    : '/downloads/${Uri.encodeComponent(item.id)}',
              );
            } else {
              ref
                  .read(downloadsStoreControllerProvider)
                  .select(isSelected ? null : item.id);
            }
          },
          onDoubleTap: () => _toggleExpanded(item.id, item.ed2kHash),
          expanded: _expandedId == item.id,
        );
      },
    );
  }
}
