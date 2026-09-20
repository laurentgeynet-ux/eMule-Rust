import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../view_models/downloads_view_model.dart';

class DownloadDetailDialog extends StatefulWidget {
  final String downloadId;
  final DownloadsViewModel viewModel;

  const DownloadDetailDialog({
    super.key,
    required this.downloadId,
    required this.viewModel,
  });

  @override
  State<DownloadDetailDialog> createState() => _DownloadDetailDialogState();
}

class _DownloadDetailDialogState extends State<DownloadDetailDialog> {
  final TextEditingController _manualSourceController = TextEditingController();

  @override
  void initState() {
    super.initState();
    widget.viewModel.fetchDownloadDetail(widget.downloadId);
    final item = widget.viewModel.filteredDownloads
        .where((d) => d.id == widget.downloadId)
        .firstOrNull;
    if (item?.ed2kHash != null && item!.ed2kHash!.isNotEmpty) {
      widget.viewModel.loadSourcesForHash(item.ed2kHash!);
    }
  }

  @override
  void dispose() {
    _manualSourceController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.viewModel,
      builder: (context, _) {
        final item =
            widget.viewModel.filteredDownloads
                .where((d) => d.id == widget.downloadId)
                .firstOrNull ??
            widget.viewModel.selectedDownload;

        if (item == null) {
          return AlertDialog(
            title: const Text('Download Not Found'),
            content: const Text(
              'The requested download is no longer available.',
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(context).pop(),
                child: const Text('Close'),
              ),
            ],
          );
        }

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
                // Header
                _buildHeader(context, item, statusColor),

                // Scrollable Content
                Expanded(
                  child: SingleChildScrollView(
                    padding: const EdgeInsets.all(20),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        // Metadata & Identity Card
                        _buildIdentityCard(context, item),
                        const SizedBox(height: 16),

                        // Transfer Metrics & Progress Card
                        _buildProgressAndMetricsCard(
                          context,
                          item,
                          statusColor,
                        ),
                        const SizedBox(height: 16),

                        // eD2K Peer Sources & Controlled WAN Session Card
                        _buildSourcesCard(context, item),
                        const SizedBox(height: 16),

                        // eD2K Part Map Card
                        _buildPartMapCard(context, item),
                        const SizedBox(height: 16),

                        // Paths & Destination Card
                        _buildPathsCard(context, item),
                        const SizedBox(height: 16),

                        // Diagnostics Card
                        _buildDiagnosticsCard(context, item),
                      ],
                    ),
                  ),
                ),

                // Action Bar (Separated from technical info)
                _buildActionBar(context, item),
              ],
            ),
          ),
        );
      },
    );
  }

  Widget _buildHeader(
    BuildContext context,
    DownloadItem item,
    Color statusColor,
  ) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 16),
      decoration: const BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.vertical(top: Radius.circular(12)),
        border: Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: Row(
        children: [
          Container(
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: statusColor.withValues(alpha: 0.15),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Icon(
              Icons.insert_drive_file_outlined,
              color: statusColor,
              size: 22,
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  item.fileName,
                  style: const TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.bold,
                    color: Colors.white,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 3),
                Row(
                  children: [
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 8,
                        vertical: 2,
                      ),
                      decoration: BoxDecoration(
                        color: statusColor.withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(10),
                        border: Border.all(
                          color: statusColor.withValues(alpha: 0.3),
                          width: 0.8,
                        ),
                      ),
                      child: Text(
                        item.status.label,
                        style: TextStyle(
                          fontSize: 11,
                          fontWeight: FontWeight.w600,
                          color: statusColor,
                        ),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Text(
                      '${Formatters.formatBytes(item.receivedBytes)} of ${Formatters.formatBytes(item.totalBytes)} (${Formatters.formatPercent(item.progress)})',
                      style: const TextStyle(
                        fontSize: 12,
                        color: Colors.white60,
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
          IconButton(
            icon: const Icon(Icons.close, size: 20, color: Colors.white54),
            onPressed: () => Navigator.of(context).pop(),
          ),
        ],
      ),
    );
  }

  Widget _buildIdentityCard(BuildContext context, DownloadItem item) {
    return _buildSectionCard(
      title: 'Identity & eD2K Links',
      icon: Icons.fingerprint_rounded,
      child: Column(
        children: [
          _buildCopyRow(
            context,
            label: 'Download UUID',
            value: item.id,
            monospace: true,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          _buildCopyRow(
            context,
            label: 'eD2K Hash (MD4)',
            value: item.ed2kHash ?? 'Pending calculation...',
            monospace: true,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          _buildCopyRow(
            context,
            label: 'Canonical eD2K Link',
            value:
                item.ed2kLink ??
                'ed2k://|file|${item.fileName}|${item.totalBytes}|${item.ed2kHash ?? ""}|/',
            monospace: true,
            maxLines: 2,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          Row(
            children: [
              const SizedBox(
                width: 140,
                child: Text(
                  'Category',
                  style: TextStyle(fontSize: 12, color: Colors.white54),
                ),
              ),
              Expanded(
                child: DropdownButtonHideUnderline(
                  child: DropdownButton<String?>(
                    value: item.categoryId,
                    isDense: true,
                    dropdownColor: AppTheme.darkCard,
                    items: [
                      const DropdownMenuItem<String?>(
                        value: null,
                        child: Text(
                          'Sans catégorie (None)',
                          style: TextStyle(color: Colors.white70, fontSize: 13),
                        ),
                      ),
                      ...widget.viewModel.categories.map(
                        (cat) => DropdownMenuItem<String?>(
                          value: cat.id,
                          child: Row(
                            children: [
                              Container(
                                width: 10,
                                height: 10,
                                decoration: BoxDecoration(
                                  color: cat.parsedColor,
                                  shape: BoxShape.circle,
                                ),
                              ),
                              const SizedBox(width: 8),
                              Text(
                                cat.name,
                                style: const TextStyle(
                                  color: Colors.white,
                                  fontSize: 13,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ],
                    onChanged: (newCatId) {
                      widget.viewModel.setDownloadCategory(item.id, newCatId);
                    },
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildProgressAndMetricsCard(
    BuildContext context,
    DownloadItem item,
    Color statusColor,
  ) {
    return _buildSectionCard(
      title: 'Transfer Metrics & Speed',
      icon: Icons.speed_rounded,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          ClipRRect(
            borderRadius: BorderRadius.circular(6),
            child: LinearProgressIndicator(
              value: item.progress,
              minHeight: 10,
              backgroundColor: Colors.white12,
              valueColor: AlwaysStoppedAnimation<Color>(statusColor),
            ),
          ),
          const SizedBox(height: 14),
          Wrap(
            spacing: 24,
            runSpacing: 12,
            children: [
              _buildMetricTile(
                'Received',
                Formatters.formatBytes(item.receivedBytes),
              ),
              _buildMetricTile(
                'Remaining',
                Formatters.formatBytes(item.remainingBytes),
              ),
              _buildMetricTile(
                'Verified',
                Formatters.formatBytes(item.verifiedBytes),
              ),
              _buildMetricTile(
                'Current Rate',
                Formatters.formatSpeed(item.downloadRateBps),
              ),
              _buildMetricTile(
                'Average Rate',
                Formatters.formatSpeed(item.averageRateBps),
              ),
              _buildMetricTile(
                'ETA',
                item.etaSeconds != null
                    ? Formatters.formatEta(
                        item.remainingBytes,
                        item.downloadRateBps,
                      )
                    : 'N/A',
              ),
              _buildMetricTile('Sources', '${item.sourceCount} loopback'),
              _buildMetricTile('Attempts', '${item.connectionAttempts}'),
            ],
          ),
          if (item.lastError != null && item.lastError!.isNotEmpty) ...[
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: AppTheme.statusFailed.withValues(alpha: 0.15),
                borderRadius: BorderRadius.circular(6),
                border: Border.all(
                  color: AppTheme.statusFailed.withValues(alpha: 0.3),
                ),
              ),
              child: Row(
                children: [
                  const Icon(
                    Icons.warning_amber_rounded,
                    color: AppTheme.statusFailed,
                    size: 16,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Last Error: ${item.lastError}',
                      style: const TextStyle(
                        fontSize: 12,
                        color: AppTheme.statusFailed,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildPartMapCard(BuildContext context, DownloadItem item) {
    final parts = item.parts;
    final verifiedCount = parts.where((p) => p.isVerified).length;
    final completedCount = parts.where((p) => p.state == 'completed').length;
    final downloadingCount = parts
        .where((p) => p.state == 'downloading')
        .length;
    final missingCount = parts.where((p) => p.state == 'missing').length;
    final unavailableCount =
        parts.where((p) => p.state == 'missing' && p.availableSourceCount == 0).length;

    return _buildSectionCard(
      title: 'eD2K Part Map (${parts.length} parts of 9.28 MB)',
      icon: Icons.grid_view_rounded,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (parts.isEmpty)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 8),
              child: Text(
                'Part breakdown is initializing...',
                style: TextStyle(fontSize: 12, color: Colors.white54),
              ),
            )
          else ...[
            // Grid of part blocks
            LayoutBuilder(
              builder: (context, constraints) {
                const double blockWidth = 32.0;
                const double blockSpacing = 4.0;

                return Wrap(
                  spacing: blockSpacing,
                  runSpacing: blockSpacing,
                  children: parts.map((part) {
                    final color = _getPartColor(part);
                    return Tooltip(
                      message:
                          'Part #${part.partIndex + 1}\n'
                          'Range: ${Formatters.formatBytes(part.startOffset)} - ${Formatters.formatBytes(part.endOffset)}\n'
                          'Size: ${Formatters.formatBytes(part.size)}\n'
                          'Received: ${Formatters.formatBytes(part.receivedBytes)}\n'
                          'Status: ${part.isVerified ? "Verified (Hash valid)" : part.state}\n'
                          'Sources: ${part.availableSourceCount}',
                      child: Container(
                        width: blockWidth,
                        height: 24,
                        decoration: BoxDecoration(
                          color: color,
                          borderRadius: BorderRadius.circular(3),
                          border: Border.all(
                            color: Colors.white.withValues(alpha: 0.15),
                            width: 0.5,
                          ),
                        ),
                        alignment: Alignment.center,
                        child: Text(
                          '${part.partIndex + 1}',
                          style: TextStyle(
                            fontSize: 9,
                            fontWeight: FontWeight.bold,
                            color: part.state == 'missing'
                                ? Colors.white38
                                : Colors.white,
                          ),
                        ),
                      ),
                    );
                  }).toList(),
                );
              },
            ),

            const SizedBox(height: 14),

            // Legend
            Wrap(
              spacing: 16,
              runSpacing: 8,
              children: [
                _buildLegendItem(
                  color: Colors.tealAccent,
                  label: 'Verified ($verifiedCount)',
                ),
                _buildLegendItem(
                  color: Colors.blueAccent,
                  label: 'Completed ($completedCount)',
                ),
                _buildLegendItem(
                  color: Colors.amberAccent,
                  label: 'Downloading ($downloadingCount)',
                ),
                _buildLegendItem(
                  color: Colors.white24,
                  label: 'Missing ($missingCount)',
                ),
                _buildLegendItem(
                  color: Colors.redAccent,
                  label: 'No source ($unavailableCount)',
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildPathsCard(BuildContext context, DownloadItem item) {
    return _buildSectionCard(
      title: 'Storage & Target Destination',
      icon: Icons.folder_open_rounded,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _buildCopyRow(
            context,
            label: 'Temporary Part Path (Confined)',
            value: item.tempPath ?? 'data/temp/${item.id}.part',
            monospace: true,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          Row(
            children: [
              Expanded(
                child: _buildCopyRow(
                  context,
                  label: 'Final Target Destination',
                  value: item.targetPath ?? 'Pending destination...',
                  monospace: true,
                ),
              ),
              const SizedBox(width: 12),
              OutlinedButton.icon(
                icon: const Icon(Icons.edit_location_alt_outlined, size: 16),
                label: const Text('Change Destination'),
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppTheme.primary,
                  side: const BorderSide(color: AppTheme.primary),
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 8,
                  ),
                ),
                onPressed: () => _promptChangeDestination(context, item),
              ),
            ],
          ),
          const SizedBox(height: 8),
          const Text(
            'Note: Changing destination redirects where the completed file is assembled. '
            'Ongoing .part chunks remain safely confined to data/temp/ until final verification.',
            style: TextStyle(
              fontSize: 11,
              color: Colors.white38,
              fontStyle: FontStyle.italic,
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildDiagnosticsCard(BuildContext context, DownloadItem item) {
    return _buildSectionCard(
      title: 'Diagnostics & Redacted Export',
      icon: Icons.bug_report_outlined,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          const Expanded(
            child: Text(
              'Export sanitized diagnostics (zero auth token secrets, loopback 127.0.0.1 confinement verification, redacted user directories).',
              style: TextStyle(fontSize: 12, color: Colors.white60),
            ),
          ),
          const SizedBox(width: 16),
          FilledButton.icon(
            icon: const Icon(Icons.copy_all_rounded, size: 16),
            label: const Text('Copy Redacted Diagnostic'),
            style: FilledButton.styleFrom(
              backgroundColor: AppTheme.darkCard,
              foregroundColor: Colors.white,
              side: const BorderSide(color: AppTheme.darkBorder),
            ),
            onPressed: () {
              final diag = widget.viewModel.getRedactedDownloadDiagnostic(item);
              Clipboard.setData(ClipboardData(text: diag));
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(
                  content: Text(
                    'Download diagnostic copied to clipboard (sanitized: no secrets)',
                  ),
                  duration: Duration(seconds: 2),
                ),
              );
            },
          ),
        ],
      ),
    );
  }

  Widget _buildActionBar(BuildContext context, DownloadItem item) {
    final isCompleted = item.status.isCompleted;

    return Container(
      padding: const EdgeInsets.all(16),
      decoration: const BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.vertical(bottom: Radius.circular(12)),
        border: Border(top: BorderSide(color: AppTheme.darkBorder, width: 0.5)),
      ),
      child: Wrap(
        spacing: 10,
        runSpacing: 10,
        alignment: WrapAlignment.spaceBetween,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Wrap(
            spacing: 10,
            runSpacing: 10,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              // Pause / Resume
              if (item.status.isPaused)
                FilledButton.icon(
                  icon: const Icon(Icons.play_arrow_rounded, size: 18),
                  label: const Text('Resume'),
                  style: FilledButton.styleFrom(
                    backgroundColor: AppTheme.statusCompleted,
                  ),
                  onPressed: () => widget.viewModel.resumeDownload(item.id),
                )
              else if (item.status.isActive)
                FilledButton.icon(
                  icon: const Icon(Icons.pause_rounded, size: 18),
                  label: const Text('Pause'),
                  style: FilledButton.styleFrom(
                    backgroundColor: AppTheme.statusPaused,
                  ),
                  onPressed: () => widget.viewModel.pauseDownload(item.id),
                ),

              // Retry Loopback Connection
              OutlinedButton.icon(
                icon: const Icon(Icons.sync_rounded, size: 16),
                label: const Text('Retry Loopback Source'),
                style: OutlinedButton.styleFrom(
                  foregroundColor: Colors.white70,
                  side: const BorderSide(color: AppTheme.darkBorder),
                ),
                onPressed: () =>
                    widget.viewModel.retryDownloadConnection(item.id),
              ),

              // Open Completed File
              Tooltip(
                message: isCompleted
                    ? 'Open completed file in file explorer'
                    : 'Available once download and verification finish',
                child: FilledButton.icon(
                  icon: const Icon(Icons.folder_open_rounded, size: 16),
                  label: const Text('Open Completed File'),
                  style: FilledButton.styleFrom(
                    backgroundColor: isCompleted
                        ? AppTheme.primary
                        : Colors.white12,
                    foregroundColor: isCompleted
                        ? Colors.white
                        : Colors.white38,
                  ),
                  onPressed: isCompleted
                      ? () => widget.viewModel.openCompletedFile(item)
                      : null,
                ),
              ),
            ],
          ),

          // Destructive Action: Remove
          OutlinedButton.icon(
            icon: const Icon(
              Icons.delete_outline_rounded,
              size: 16,
              color: AppTheme.statusFailed,
            ),
            label: const Text(
              'Remove...',
              style: TextStyle(color: AppTheme.statusFailed),
            ),
            style: OutlinedButton.styleFrom(
              side: BorderSide(
                color: AppTheme.statusFailed.withValues(alpha: 0.5),
              ),
            ),
            onPressed: () => _showRemoveConfirmation(context, item),
          ),
        ],
      ),
    );
  }

  void _promptChangeDestination(BuildContext context, DownloadItem item) {
    final target = item.targetPath ?? 'downloads';
    final controller = TextEditingController(
      text: target.contains('/') || target.contains(r'\')
          ? (target.replaceAll(r'\', '/').split('/')..removeLast()).join('/')
          : target,
    );

    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Change Target Destination'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Select target destination folder for "${item.fileName}":'),
            const SizedBox(height: 12),
            TextField(
              controller: controller,
              decoration: const InputDecoration(
                labelText: 'Target Directory Path',
                hintText: 'e.g. downloads or C:\\TargetDir',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 8),
            const Text(
              'The folder must exist and be writable. Active .part data remains in data/temp/ until completed.',
              style: TextStyle(fontSize: 11, color: Colors.white54),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            style: FilledButton.styleFrom(backgroundColor: AppTheme.primary),
            onPressed: () async {
              final newDir = controller.text.trim();
              if (newDir.isNotEmpty) {
                final success = await widget.viewModel
                    .changeDownloadDestination(item.id, newDir);
                if (ctx.mounted && success) {
                  Navigator.of(ctx).pop();
                }
              }
            },
            child: const Text('Save Destination'),
          ),
        ],
      ),
    );
  }

  void _showRemoveConfirmation(BuildContext context, DownloadItem item) {
    bool deleteFilesOnDisk = true;

    showDialog(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, setDialogState) => AlertDialog(
          title: const Text('Remove Download?'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Remove "${item.fileName}" from the downloads manager?'),
              const SizedBox(height: 12),
              Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: AppTheme.darkSurface,
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: AppTheme.darkBorder),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: const [
                    Text(
                      '• The download entry in SQLite will be permanently deleted.\n'
                      '• Temporary files (.part and .part.met) in data/temp/ will be removed if selected below.\n'
                      '• The final completed file in destination (if completed) will NOT be deleted.',
                      style: TextStyle(
                        fontSize: 12,
                        color: Colors.white70,
                        height: 1.4,
                      ),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 12),
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                dense: true,
                title: const Text(
                  'Delete temporary .part file from data/temp/',
                  style: TextStyle(fontSize: 13),
                ),
                value: deleteFilesOnDisk,
                onChanged: (val) {
                  setDialogState(() {
                    deleteFilesOnDisk = val ?? true;
                  });
                },
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              style: FilledButton.styleFrom(
                backgroundColor: AppTheme.statusFailed,
              ),
              onPressed: () {
                widget.viewModel.removeDownload(
                  item.id,
                  deleteFiles: deleteFilesOnDisk,
                );
                Navigator.of(ctx).pop(); // pop confirm dialog
                Navigator.of(context).pop(); // pop detail dialog
              },
              child: const Text('Confirm Remove'),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildSourcesCard(BuildContext context, DownloadItem item) {
    final hash = item.ed2kHash ?? '';
    final sources = widget.viewModel.getSourcesForHash(hash);
    final session = widget.viewModel.peerSession;
    final isConnectedToThis =
        session.fileHash == hash &&
        session.status != PeerConnectionStatus.disconnected;

    return _buildSectionCard(
      title: 'SOURCES ED2K & SESSION PEER',
      icon: Icons.hub_outlined,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Active Peer Session Banner
          if (isConnectedToThis) ...[
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: AppTheme.primary.withValues(alpha: 0.12),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(
                  color: AppTheme.primary.withValues(alpha: 0.3),
                ),
              ),
              child: Row(
                children: [
                  const Icon(
                    Icons.wifi_tethering,
                    color: AppTheme.primary,
                    size: 20,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Row(
                          children: [
                            Text(
                              'Session active : ${session.peerAddress ?? ""}',
                              style: const TextStyle(
                                fontSize: 12,
                                fontWeight: FontWeight.bold,
                                color: Colors.white,
                              ),
                            ),
                            const SizedBox(width: 8),
                            Container(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 6,
                                vertical: 1,
                              ),
                              decoration: BoxDecoration(
                                color: Colors.tealAccent.withValues(alpha: 0.2),
                                borderRadius: BorderRadius.circular(4),
                              ),
                              child: Text(
                                session.status.toDisplayString(),
                                style: const TextStyle(
                                  fontSize: 10,
                                  fontWeight: FontWeight.w600,
                                  color: Colors.tealAccent,
                                ),
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 2),
                        Text(
                          'Reçu : ${Formatters.formatBytes(session.transferredBytes)} (${Formatters.formatSpeed(session.transferRateBps)})',
                          style: const TextStyle(
                            fontSize: 11,
                            color: Colors.white70,
                          ),
                        ),
                      ],
                    ),
                  ),
                  OutlinedButton.icon(
                    style: OutlinedButton.styleFrom(
                      foregroundColor: Colors.redAccent,
                      side: const BorderSide(
                        color: Colors.redAccent,
                        width: 0.8,
                      ),
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 6,
                      ),
                    ),
                    icon: const Icon(Icons.link_off, size: 14),
                    label: const Text(
                      'Déconnecter',
                      style: TextStyle(fontSize: 11),
                    ),
                    onPressed: () => widget.viewModel.disconnectPeerSource(),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 12),
          ],

          // Toolbar: Add manual source & Refresh / Ask server
          Row(
            children: [
              Expanded(
                child: SizedBox(
                  height: 32,
                  child: TextField(
                    controller: _manualSourceController,
                    style: const TextStyle(fontSize: 12, color: Colors.white),
                    decoration: InputDecoration(
                      hintText: 'Ajouter source IP:port (ex: 127.0.0.1:39735)',
                      hintStyle: const TextStyle(
                        fontSize: 11,
                        color: Colors.white38,
                      ),
                      contentPadding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 8,
                      ),
                      filled: true,
                      fillColor: AppTheme.darkSurface,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(6),
                        borderSide: const BorderSide(
                          color: AppTheme.darkBorder,
                        ),
                      ),
                      enabledBorder: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(6),
                        borderSide: const BorderSide(
                          color: AppTheme.darkBorder,
                          width: 0.5,
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 8),
              SizedBox(
                height: 32,
                child: ElevatedButton.icon(
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppTheme.primary,
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                  ),
                  icon: const Icon(Icons.add, size: 14, color: Colors.white),
                  label: const Text(
                    'Ajouter',
                    style: TextStyle(fontSize: 11, color: Colors.white),
                  ),
                  onPressed: () async {
                    final addr = _manualSourceController.text.trim();
                    if (addr.isNotEmpty && hash.isNotEmpty) {
                      final ok = await widget.viewModel.addManualSource(
                        hash,
                        addr,
                      );
                      if (ok) {
                        _manualSourceController.clear();
                      }
                    }
                  },
                ),
              ),
              const SizedBox(width: 8),
              SizedBox(
                height: 32,
                child: OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(
                    side: const BorderSide(color: AppTheme.darkBorder),
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                  ),
                  icon: const Icon(
                    Icons.search,
                    size: 14,
                    color: Colors.white70,
                  ),
                  label: const Text(
                    'Demander au serveur',
                    style: TextStyle(fontSize: 11, color: Colors.white70),
                  ),
                  onPressed: hash.isNotEmpty
                      ? () => widget.viewModel.queryServerSources(
                          hash,
                          fileSize: item.totalBytes,
                        )
                      : null,
                ),
              ),
              const SizedBox(width: 8),
              SizedBox(
                height: 32,
                child: OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(
                    side: const BorderSide(color: Colors.amberAccent),
                    foregroundColor: Colors.amberAccent,
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                  ),
                  icon: const Icon(
                    Icons.hub_rounded,
                    size: 14,
                    color: Colors.amberAccent,
                  ),
                  label: const Text(
                    'Demander au réseau Kad',
                    style: TextStyle(fontSize: 11, color: Colors.amberAccent),
                  ),
                  onPressed: hash.isNotEmpty
                      ? () => widget.viewModel.searchKadSources(
                          hash,
                          fileSize: item.totalBytes,
                        )
                      : null,
                ),
              ),
              const SizedBox(width: 6),
              IconButton(
                icon: const Icon(
                  Icons.refresh,
                  size: 16,
                  color: Colors.white54,
                ),
                tooltip: 'Actualiser la liste',
                onPressed: hash.isNotEmpty
                    ? () => widget.viewModel.loadSourcesForHash(hash)
                    : null,
              ),
            ],
          ),
          const SizedBox(height: 12),

          // Sources Table / List
          if (sources.isEmpty)
            Container(
              width: double.infinity,
              padding: const EdgeInsets.symmetric(vertical: 16),
              alignment: Alignment.center,
              child: Text(
                widget.viewModel.isSourcesLoading
                    ? 'Chargement des sources...'
                    : 'Aucune source enregistrée pour ce fichier.',
                style: const TextStyle(fontSize: 12, color: Colors.white38),
              ),
            )
          else
            Container(
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(6),
                border: Border.all(color: AppTheme.darkBorder, width: 0.5),
              ),
              child: ListView.separated(
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                itemCount: sources.length,
                separatorBuilder: (context, index) =>
                    const Divider(height: 1, color: AppTheme.darkBorder),
                itemBuilder: (context, index) {
                  final src = sources[index];
                  final isThisConnected =
                      session.peerAddress == src.address &&
                      session.status != PeerConnectionStatus.disconnected;

                  final nowMs = DateTime.now().millisecondsSinceEpoch;
                  final isInCooldown =
                      src.cooldownUntilUnixMs != null &&
                      src.cooldownUntilUnixMs! > nowMs;
                  final cooldownSecondsRemaining = isInCooldown
                      ? ((src.cooldownUntilUnixMs! - nowMs) / 1000).ceil()
                      : 0;

                  return Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 8,
                    ),
                    child: Row(
                      children: [
                        Icon(
                          src.status == PeerSourceStatus.filtered
                              ? Icons.block
                              : src.status == PeerSourceStatus.rejected
                              ? Icons.gpp_bad
                              : src.status == PeerSourceStatus.failed
                              ? Icons.error_outline
                              : src.status == PeerSourceStatus.connected
                              ? Icons.wifi_tethering
                              : Icons.computer,
                          size: 16,
                          color: src.status == PeerSourceStatus.filtered
                              ? Colors.redAccent
                              : src.status == PeerSourceStatus.rejected
                              ? Colors.orangeAccent
                              : src.status == PeerSourceStatus.failed
                              ? Colors.redAccent
                              : src.status == PeerSourceStatus.connected
                              ? Colors.tealAccent
                              : Colors.white54,
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Row(
                                children: [
                                  Text(
                                    src.address,
                                    style: const TextStyle(
                                      fontSize: 12,
                                      fontWeight: FontWeight.w600,
                                      color: Colors.white,
                                      fontFamily: 'monospace',
                                    ),
                                  ),
                                  const SizedBox(width: 8),
                                  _buildSourceStatusBadge(src.status),
                                  if (isInCooldown) ...[
                                    const SizedBox(width: 6),
                                    Container(
                                      padding: const EdgeInsets.symmetric(
                                        horizontal: 5,
                                        vertical: 1,
                                      ),
                                      decoration: BoxDecoration(
                                        color: Colors.amber.withValues(
                                          alpha: 0.2,
                                        ),
                                        borderRadius: BorderRadius.circular(4),
                                        border: Border.all(
                                          color: Colors.amber.withValues(
                                            alpha: 0.4,
                                          ),
                                          width: 0.5,
                                        ),
                                      ),
                                      child: Text(
                                        'Cooldown ${cooldownSecondsRemaining}s',
                                        style: const TextStyle(
                                          fontSize: 9,
                                          fontWeight: FontWeight.bold,
                                          color: Colors.amberAccent,
                                        ),
                                      ),
                                    ),
                                  ],
                                  const SizedBox(width: 6),
                                  Text(
                                    '(${src.origin.toApiString()})',
                                    style: const TextStyle(
                                      fontSize: 10,
                                      color: Colors.white38,
                                    ),
                                  ),
                                ],
                              ),
                              if (src.lastError != null &&
                                  src.lastError!.isNotEmpty)
                                Text(
                                  src.lastError!,
                                  style: const TextStyle(
                                    fontSize: 10,
                                    color: Colors.redAccent,
                                  ),
                                ),
                            ],
                          ),
                        ),
                        if (src.status != PeerSourceStatus.filtered) ...[
                          if (isThisConnected)
                            OutlinedButton(
                              style: OutlinedButton.styleFrom(
                                foregroundColor: Colors.redAccent,
                                side: const BorderSide(
                                  color: Colors.redAccent,
                                  width: 0.8,
                                ),
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 8,
                                  vertical: 4,
                                ),
                                minimumSize: Size.zero,
                                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                              ),
                              onPressed: () => widget.viewModel
                                  .disconnectPeerSource(address: src.address),
                              child: const Text(
                                'Déconnecter',
                                style: TextStyle(fontSize: 10),
                              ),
                            )
                          else if (isInCooldown)
                            OutlinedButton(
                              style: OutlinedButton.styleFrom(
                                foregroundColor: Colors.amberAccent,
                                side: const BorderSide(
                                  color: Colors.amberAccent,
                                  width: 0.8,
                                ),
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 8,
                                  vertical: 4,
                                ),
                                minimumSize: Size.zero,
                                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                              ),
                              onPressed: widget.viewModel.isPeerConnecting
                                  ? null
                                  : () => widget.viewModel.connectPeerSource(
                                      hash,
                                      src.address,
                                      force: true,
                                    ),
                              child: const Text(
                                'Forcer',
                                style: TextStyle(fontSize: 10),
                              ),
                            )
                          else
                            ElevatedButton(
                              style: ElevatedButton.styleFrom(
                                backgroundColor: AppTheme.primary,
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 10,
                                  vertical: 4,
                                ),
                                minimumSize: Size.zero,
                                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                              ),
                              onPressed: widget.viewModel.isPeerConnecting
                                  ? null
                                  : () => widget.viewModel.connectPeerSource(
                                      hash,
                                      src.address,
                                    ),
                              child: Text(
                                src.status == PeerSourceStatus.failed
                                    ? 'Réessayer'
                                    : 'Connecter',
                                style: const TextStyle(
                                  fontSize: 10,
                                  color: Colors.white,
                                ),
                              ),
                            ),
                        ],
                      ],
                    ),
                  );
                },
              ),
            ),
        ],
      ),
    );
  }

  Widget _buildSourceStatusBadge(PeerSourceStatus status) {
    Color bg;
    Color fg;
    String text;

    switch (status) {
      case PeerSourceStatus.allowed:
        bg = Colors.greenAccent.withValues(alpha: 0.15);
        fg = Colors.greenAccent;
        text = 'Autorisé';
        break;
      case PeerSourceStatus.filtered:
        bg = Colors.redAccent.withValues(alpha: 0.15);
        fg = Colors.redAccent;
        text = 'Filtré IP';
        break;
      case PeerSourceStatus.rejected:
        bg = Colors.orangeAccent.withValues(alpha: 0.15);
        fg = Colors.orangeAccent;
        text = 'Rejeté profil';
        break;
      case PeerSourceStatus.connected:
        bg = Colors.tealAccent.withValues(alpha: 0.15);
        fg = Colors.tealAccent;
        text = 'Connecté';
        break;
      case PeerSourceStatus.tried:
        bg = Colors.blueAccent.withValues(alpha: 0.15);
        fg = Colors.blueAccent;
        text = 'Essayé';
        break;
      case PeerSourceStatus.failed:
        bg = Colors.red.withValues(alpha: 0.15);
        fg = Colors.red;
        text = 'Échec';
        break;
      case PeerSourceStatus.candidate:
        bg = Colors.white12;
        fg = Colors.white70;
        text = 'Candidat';
        break;
    }

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        text,
        style: TextStyle(fontSize: 9, fontWeight: FontWeight.bold, color: fg),
      ),
    );
  }

  Widget _buildSectionCard({
    required String title,
    required IconData icon,
    required Widget child,
  }) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(icon, size: 16, color: AppTheme.primary),
              const SizedBox(width: 8),
              Text(
                title,
                style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.bold,
                  color: Colors.white,
                  letterSpacing: 0.3,
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          child,
        ],
      ),
    );
  }

  Widget _buildMetricTile(String label, String value) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: const TextStyle(fontSize: 11, color: Colors.white38),
        ),
        const SizedBox(height: 3),
        Text(
          value,
          style: const TextStyle(
            fontSize: 13,
            fontWeight: FontWeight.bold,
            color: Colors.white,
          ),
        ),
      ],
    );
  }

  Widget _buildCopyRow(
    BuildContext context, {
    required String label,
    required String value,
    bool monospace = false,
    int maxLines = 1,
  }) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SizedBox(
          width: 170,
          child: Text(
            label,
            style: const TextStyle(fontSize: 12, color: Colors.white54),
          ),
        ),
        Expanded(
          child: SelectableText(
            value,
            maxLines: maxLines,
            style: TextStyle(
              fontSize: 12,
              color: Colors.white,
              fontFamily: monospace ? 'monospace' : null,
            ),
          ),
        ),
        const SizedBox(width: 8),
        InkWell(
          borderRadius: BorderRadius.circular(4),
          onTap: () {
            Clipboard.setData(ClipboardData(text: value));
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(
                content: Text('$label copied to clipboard'),
                duration: const Duration(seconds: 1),
              ),
            );
          },
          child: const Padding(
            padding: EdgeInsets.all(4.0),
            child: Icon(Icons.copy_rounded, size: 14, color: Colors.white54),
          ),
        ),
      ],
    );
  }

  Widget _buildLegendItem({required Color color, required String label}) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Container(
          width: 12,
          height: 12,
          decoration: BoxDecoration(
            color: color,
            borderRadius: BorderRadius.circular(2),
          ),
        ),
        const SizedBox(width: 6),
        Text(
          label,
          style: const TextStyle(fontSize: 11, color: Colors.white70),
        ),
      ],
    );
  }

  Color _getPartColor(DownloadPartInfo part) => partColorFor(part);
}
