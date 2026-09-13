import 'package:flutter/material.dart';
import '../../../core/theme.dart';
import '../view_models/downloads_view_model.dart';

class DebugSimulationBar extends StatelessWidget {
  final DownloadsViewModel viewModel;

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
          const Icon(Icons.bug_report_outlined, size: 18, color: Colors.amberAccent),
          const SizedBox(width: 8),
          const Text(
            'Dev Simulation Tools:',
            style: TextStyle(fontSize: 12, fontWeight: FontWeight.bold, color: Colors.amberAccent),
          ),
          const SizedBox(width: 12),
          if (selected != null) ...[
            Text(
              'Selected: "${selected.fileName}"',
              style: const TextStyle(fontSize: 12, color: Colors.white70),
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
              child: const Text('Inject 256 KB Chunk'),
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
              child: const Text('Inject 1 MB Chunk'),
            ),
          ] else ...[
            const Text(
              'Click any download row above to select it for data injection tests.',
              style: TextStyle(fontSize: 12, color: Colors.white54, fontStyle: FontStyle.italic),
            ),
          ],
          const Spacer(),
          IconButton(
            icon: const Icon(Icons.close, size: 16, color: Colors.white54),
            tooltip: 'Close Debug Bar',
            onPressed: viewModel.toggleDebugBar,
          ),
        ],
      ),
    );
  }
}
