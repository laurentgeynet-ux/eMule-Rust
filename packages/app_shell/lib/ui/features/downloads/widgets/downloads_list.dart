import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:ui_kit/ui_kit.dart';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import 'download_row.dart';

/// Virtualized list of downloads (ListView.builder) with selection handling.
class DownloadsList extends ConsumerWidget {
    const DownloadsList({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final list = ref.watch(filteredDownloadsProvider);

    return ListView.builder(
      padding: AppSpacing.pagePadding,
      itemCount: list.length,
      itemBuilder: (context, index) {
        final item = list[index];
        final isSelected = ref.watch(downloadsStoreControllerProvider).selectedId == item.id;

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
              ref.read(downloadsStoreControllerProvider).select(isSelected ? null : item.id);
            }
          },
        );
      },
    );
  }
}
