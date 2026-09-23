// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/providers.dart';
import '../widgets/history_empty_state.dart';
import '../widgets/history_entry_row.dart';
import '../widgets/history_filter_bar.dart';
import '../widgets/history_stats_dashboard.dart';
import '../widgets/history_toolbar.dart';

/// History & statistics section — composition only. All state lives in
/// [historyControllerProvider]; widgets live under `widgets/`.
class HistoryView extends ConsumerWidget {
  const HistoryView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final history = ref.watch(historyControllerProvider);
    final entries = history.entries;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        HistoryToolbar(controller: history),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
          child: HistoryStatsDashboard(stats: history.stats),
        ),
        const Divider(color: AppTheme.darkBorder, height: 1),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
          child: HistoryFilterBar(controller: history),
        ),
        Expanded(
          child: history.isLoading && entries.isEmpty
              ? const Center(child: CircularProgressIndicator())
              : entries.isEmpty
              ? const HistoryEmptyState()
              : ListView.separated(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 20,
                    vertical: 8,
                  ),
                  itemCount: entries.length,
                  separatorBuilder: (_, _) => const SizedBox(height: 6),
                  itemBuilder: (context, index) =>
                      HistoryEntryRow(entry: entries[index]),
                ),
        ),
      ],
    );
  }
}
