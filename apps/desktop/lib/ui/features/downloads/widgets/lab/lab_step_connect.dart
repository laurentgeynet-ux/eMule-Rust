import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'lab_step_card.dart';

/// Step 4: pick a queued download and connect it to the loopback source.
class LabStepConnect extends StatelessWidget {
  final List<DownloadItem> downloads;
  final String? selectedDownloadId;
  final ValueChanged<String?> onDownloadChanged;
  final String? activePeer;
  final LabFixtureInfo? fixture;
  final bool isConnecting;
  final VoidCallback onConnect;
  final String downloadsDir;
  final VoidCallback onOpenFolder;

  const LabStepConnect({
    super.key,
    required this.downloads,
    required this.selectedDownloadId,
    required this.onDownloadChanged,
    required this.activePeer,
    required this.fixture,
    required this.isConnecting,
    required this.onConnect,
    required this.downloadsDir,
    required this.onOpenFolder,
  });

  @override
  Widget build(BuildContext context) {
    if (downloads.isEmpty) {
      return const LabStepCard(
        stepNumber: 4,
        title: 'Connect Download to Loopback Source',
        description:
            'Instructs the client to connect to 127.0.0.1 source to transfer chunks.',
        content: Text(
          'No downloads in queue. Add an eD2K link in Step 2.',
          style: TextStyle(fontSize: 12, color: AppColors.textFaint),
        ),
      );
    }

    final selectedDownload = downloads.firstWhere(
      (d) => d.id == (selectedDownloadId ?? downloads.first.id),
      orElse: () => downloads.first,
    );

    final isPeerRunning = activePeer != null;
    final isCompleted =
        selectedDownload.status == DownloadStatus.completed;
    final isDownloading =
        selectedDownload.status == DownloadStatus.downloading;

    final fixture = this.fixture;
    final hasSizeMismatch = fixture != null &&
        selectedDownload.totalBytes != fixture.sizeBytes;
    final canConnect = isPeerRunning && !isCompleted && !isConnecting;

    return LabStepCard(
      stepNumber: 4,
      title: 'Connect Download to Loopback Source',
      description:
          'Instructs the client to connect to 127.0.0.1 source to transfer chunks.',
      isCompleted: isCompleted,
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          DropdownButtonFormField<String>(
            initialValue: selectedDownload.id,
            decoration: const InputDecoration(
              labelText: 'Target Download Task',
              isDense: true,
              contentPadding:
                  EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            ),
            items: downloads
                .map(
                  (d) => DropdownMenuItem<String>(
                    value: d.id,
                    child: Text(
                      '${d.fileName} (${Formatters.formatPercent(d.progress)})',
                      style: const TextStyle(fontSize: 12),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                )
                .toList(),
            onChanged: onDownloadChanged,
          ),
          if (hasSizeMismatch) ...[
            const SizedBox(height: 8),
            Container(
              padding: const EdgeInsets.symmetric(
                horizontal: 10,
                vertical: 6,
              ),
              decoration: BoxDecoration(
                color: AppColors.warning.withValues(alpha: 0.15),
                borderRadius: BorderRadius.circular(6),
                border: Border.all(
                  color: AppColors.warning.withValues(alpha: 0.4),
                ),
              ),
              child: Row(
                children: [
                  const Icon(
                    Icons.warning_amber_rounded,
                    color: AppColors.accentAmber,
                    size: 16,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Warning: Download size (${Formatters.formatBytes(selectedDownload.totalBytes)}) '
                      'does not match fixture (${Formatters.formatBytes(fixture.sizeBytes)}).',
                      style: const TextStyle(
                        fontSize: 11,
                        color: AppColors.accentAmber,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
          const SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.symmetric(
              horizontal: 12,
              vertical: 8,
            ),
            decoration: BoxDecoration(
              color: AppTheme.darkSurface,
              borderRadius: BorderRadius.circular(6),
              border: Border.all(
                color: AppTheme.darkBorder,
                width: 0.5,
              ),
            ),
            child: Row(
              children: [
                Icon(
                  isCompleted
                      ? Icons.check_circle
                      : (isDownloading
                          ? Icons.sync
                          : Icons.info_outline),
                  size: 16,
                  color: isCompleted
                      ? AppTheme.statusCompleted
                      : (isDownloading
                          ? AppTheme.primary
                          : AppColors.textMuted),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    isCompleted
                        ? 'Download completed and verified (${Formatters.formatBytes(selectedDownload.totalBytes)})'
                        : (isDownloading
                            ? 'Transferring: ${Formatters.formatBytes(selectedDownload.receivedBytes)} / ${Formatters.formatBytes(selectedDownload.totalBytes)} (${Formatters.formatSpeed(selectedDownload.downloadRateBps)})'
                            : 'State: ${selectedDownload.status.name.toUpperCase()} — Ready to connect to loopback peer.'),
                    style: TextStyle(
                      fontSize: 11,
                      color: isCompleted
                          ? AppTheme.statusCompleted
                          : AppColors.textSecondary,
                      fontWeight: isCompleted || isDownloading
                          ? FontWeight.bold
                          : FontWeight.normal,
                    ),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              FilledButton.icon(
                icon: isConnecting
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(
                          strokeWidth: 2,
                          color: AppColors.textPrimary,
                        ),
                      )
                    : const Icon(Icons.cable, size: 16),
                label: Text(
                  isCompleted ? 'Already Completed' : 'Connect to Source',
                ),
                onPressed: canConnect ? onConnect : null,
              ),
              if (!isPeerRunning) ...[
                const SizedBox(width: 12),
                const Expanded(
                  child: Text(
                    'Start loopback peer in Step 3 before connecting.',
                    style: TextStyle(
                      fontSize: 11,
                      color: AppColors.accentAmber,
                    ),
                  ),
                ),
              ],
            ],
          ),
          if (isCompleted) ...[
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.symmetric(
                horizontal: 12,
                vertical: 8,
              ),
              decoration: BoxDecoration(
                color: AppTheme.darkCard,
                borderRadius: BorderRadius.circular(6),
                border: Border.all(
                  color: AppTheme.statusCompleted.withValues(alpha: 0.4),
                ),
              ),
              child: Row(
                children: [
                  const Icon(
                    Icons.verified,
                    color: AppTheme.statusCompleted,
                    size: 16,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Final file: $downloadsDir\\${selectedDownload.fileName}',
                      style: const TextStyle(
                        fontSize: 11,
                        fontFamily: 'monospace',
                        color: AppColors.textPrimary,
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton.icon(
                    icon: const Icon(Icons.folder_open, size: 14),
                    label: const Text(
                      'Open Folder',
                      style: TextStyle(fontSize: 11),
                    ),
                    style: OutlinedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 6,
                      ),
                      side: const BorderSide(color: AppTheme.darkBorder),
                      visualDensity: VisualDensity.compact,
                    ),
                    onPressed: onOpenFolder,
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}
