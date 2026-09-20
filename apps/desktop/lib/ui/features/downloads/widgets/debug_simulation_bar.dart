import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../app/app_controller.dart';
import '../../../../l10n/l10n.dart';

class DebugSimulationBar extends StatelessWidget {
  final AppController viewModel;

  const DebugSimulationBar({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
    final selected = viewModel.selectedDownload;

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
                viewModel.injectSimulationChunk(selected.id, selected.receivedBytes, hexChunk);
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
                viewModel.injectSimulationChunk(selected.id, selected.receivedBytes, hexChunk);
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
            onPressed: viewModel.toggleDebugBar,
          ),
        ],
      ),
    );
  }
}
