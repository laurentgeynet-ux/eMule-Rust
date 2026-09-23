// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import '../../../../l10n/l10n.dart';

class DebugSimulationBar extends ConsumerWidget {
    const DebugSimulationBar({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final selected = ref.watch(selectedDownloadProvider);

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        border: const Border(
          top: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: Row(
        children: [
          const Icon(Icons.bug_report_outlined, size: 18, color: AppColors.accentAmber),
          const SizedBox(width: 8),
          Text(context.l10n.devSimulationTools,
            style: TextStyle(fontSize: 12, fontWeight: FontWeight.bold, color: AppColors.accentAmber),
          ),
          const SizedBox(width: 12),
          if (selected != null) ...[
            Text(context.l10n.selected(selected.fileName),
              style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
            ),
            const SizedBox(width: 12),
            FilledButton.tonal(
              style: FilledButton.styleFrom(
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                textStyle: const TextStyle(fontSize: 11),
              ),
              onPressed: () {
                // Generate 256KB of dummy hex data (512 hex characters)
                final hexChunk = 'AA' * 256 * 1024;
                ref.read(labControllerProvider).injectSimulationChunk(selected.id, selected.receivedBytes, hexChunk);
              },
              child: Text(context.l10n.inject256KbChunk),
            ),
            const SizedBox(width: 8),
            FilledButton.tonal(
              style: FilledButton.styleFrom(
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                textStyle: const TextStyle(fontSize: 11),
              ),
              onPressed: () {
                final hexChunk = 'BB' * 1024 * 1024;
                ref.read(labControllerProvider).injectSimulationChunk(selected.id, selected.receivedBytes, hexChunk);
              },
              child: Text(context.l10n.inject1MbChunk),
            ),
            const Spacer(),
          ] else ...[
            Expanded(
              child: Text(context.l10n.clickAnyDownloadRowAbove,
                style: TextStyle(fontSize: 12, color: AppColors.textMuted, fontStyle: FontStyle.italic),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
          IconButton(
            icon: const Icon(Icons.close, size: 16, color: AppColors.textMuted),
            tooltip: context.l10n.closeDebugBar,
            onPressed: ref.read(shellControllerProvider).toggleDebugBar,
          ),
        ],
      ),
    );
  }
}
