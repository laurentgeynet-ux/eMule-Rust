import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../downloads/view_models/downloads_view_model.dart';

class HistoryView extends StatelessWidget {
  final DownloadsViewModel viewModel;

  const HistoryView({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: viewModel,
      builder: (context, _) {
        final stats = viewModel.historyStats;
        final entries = viewModel.historyEntries;
        final isLoading = viewModel.isHistoryLoading;

        return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        // Top Toolbar
        _buildToolbar(context),

        // Statistics Dashboard
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
          child: _buildStatsDashboard(context, stats),
        ),

        const Divider(color: AppTheme.darkBorder, height: 1),

        // Filter and Action Bar
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
          child: _buildFilterBar(context),
        ),

        // Event List or Empty State
        Expanded(
          child: isLoading && entries.isEmpty
              ? const Center(child: CircularProgressIndicator())
              : entries.isEmpty
                  ? _buildEmptyState(context)
                  : _buildHistoryTable(context, entries),
        ),
      ],
        );
      },
    );
  }

  Widget _buildToolbar(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isCompact = constraints.maxWidth < 680;

          return Row(
            children: [
              const Icon(Icons.history_rounded, color: AppTheme.primary, size: 22),
              const SizedBox(width: 10),
              const Expanded(
                child: Text(
                  'History & Local Statistics',
                  style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.bold,
                    color: Colors.white,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 8),

              // Export Button
              PopupMenuButton<String>(
                tooltip: 'Export history report',
                color: AppTheme.darkCard,
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(8),
                  side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
                ),
                onSelected: (format) async {
                  final content = await viewModel.exportHistory(format);
                  if (context.mounted) {
                    _showExportDialog(context, format, content);
                  }
                },
                itemBuilder: (ctx) => [
                  const PopupMenuItem(
                    value: 'json',
                    child: Row(
                      children: [
                        Icon(Icons.data_object_rounded, size: 18, color: Colors.white70),
                        SizedBox(width: 8),
                        Flexible(
                          child: Text(
                            'Export as JSON (Sanitized)',
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                  ),
                  const PopupMenuItem(
                    value: 'csv',
                    child: Row(
                      children: [
                        Icon(Icons.table_chart_outlined, size: 18, color: Colors.white70),
                        SizedBox(width: 8),
                        Flexible(
                          child: Text(
                            'Export as CSV (Sanitized)',
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
                child: Container(
                  padding: EdgeInsets.symmetric(
                    horizontal: isCompact ? 8 : 12,
                    vertical: 6,
                  ),
                  decoration: BoxDecoration(
                    color: Colors.white12,
                    borderRadius: BorderRadius.circular(6),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      const Icon(Icons.file_download_outlined, size: 16, color: Colors.white70),
                      if (!isCompact) ...[
                        const SizedBox(width: 6),
                        const Text(
                          'Export Report',
                          style: TextStyle(fontSize: 13, color: Colors.white70),
                        ),
                        const SizedBox(width: 4),
                        const Icon(Icons.arrow_drop_down, size: 18, color: Colors.white70),
                      ],
                    ],
                  ),
                ),
              ),
              const SizedBox(width: 8),

              // Clear History Button
              if (isCompact)
                IconButton(
                  style: IconButton.styleFrom(
                    foregroundColor: AppTheme.statusFailed,
                  ),
                  icon: const Icon(Icons.delete_sweep_outlined, size: 18),
                  tooltip: 'Clear History',
                  onPressed: () => _confirmClearHistory(context),
                )
              else
                OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(
                    foregroundColor: AppTheme.statusFailed,
                    side: BorderSide(color: AppTheme.statusFailed.withValues(alpha: 0.5)),
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                  ),
                  icon: const Icon(Icons.delete_sweep_outlined, size: 16),
                  label: const Text('Clear History'),
                  onPressed: () => _confirmClearHistory(context),
                ),
              const SizedBox(width: 6),

              // Refresh Button
              IconButton(
                icon: const Icon(Icons.refresh_rounded, size: 20, color: Colors.white70),
                tooltip: 'Refresh',
                onPressed: () => viewModel.fetchHistory(),
              ),
            ],
          );
        },
      ),
    );
  }

  Widget _buildStatsDashboard(BuildContext context, HistoryStats stats) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final cardWidth = (constraints.maxWidth - 36) / 4;
        return Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            _buildStatCard(
              width: cardWidth.clamp(200, 350),
              title: 'Total Completed',
              mainValue: '${stats.totalCompletedCount} files',
              subtitle: Formatters.formatBytes(stats.totalCompletedBytes),
              icon: Icons.check_circle_outline_rounded,
              color: AppTheme.statusCompleted,
            ),
            _buildStatCard(
              width: cardWidth.clamp(200, 350),
              title: 'Activity Windows',
              mainValue: '${stats.sessionCompletedCount} session (${Formatters.formatBytes(stats.sessionCompletedBytes)})',
              subtitle: '24h: ${stats.last24hCompletedCount} | 7d: ${stats.last7dCompletedCount}',
              icon: Icons.access_time_rounded,
              color: Colors.cyanAccent,
            ),
            _buildStatCard(
              width: cardWidth.clamp(200, 350),
              title: 'Resumes & Retries',
              mainValue: '${stats.totalResumesCount} resumes',
              subtitle: '${stats.totalFailedCount} failures logged',
              icon: Icons.replay_rounded,
              color: AppTheme.statusDownloading,
            ),
            _buildStatCard(
              width: cardWidth.clamp(200, 350),
              title: 'Integrity Health',
              mainValue: '${stats.totalIntegrityFailures} hash mismatch',
              subtitle: stats.totalIntegrityFailures == 0 ? '100% hash verified' : 'Integrity errors detected',
              icon: Icons.verified_user_outlined,
              color: stats.totalIntegrityFailures == 0 ? Colors.tealAccent : AppTheme.statusFailed,
            ),
          ],
        );
      },
    );
  }

  Widget _buildStatCard({
    required double width,
    required String title,
    required String mainValue,
    required String subtitle,
    required IconData icon,
    required Color color,
  }) {
    return Container(
      width: width,
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            padding: const EdgeInsets.all(8),
            decoration: BoxDecoration(
              color: color.withValues(alpha: 0.15),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Icon(icon, color: color, size: 22),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: const TextStyle(fontSize: 12, color: Colors.white54, fontWeight: FontWeight.w500),
                ),
                const SizedBox(height: 4),
                Text(
                  mainValue,
                  style: const TextStyle(fontSize: 15, fontWeight: FontWeight.bold, color: Colors.white),
                  overflow: TextOverflow.ellipsis,
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: const TextStyle(fontSize: 11, color: Colors.white38),
                  overflow: TextOverflow.ellipsis,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildFilterBar(BuildContext context) {
    final currentEventFilter = viewModel.historyEventTypeFilter;

    return Row(
      children: [
        const Text(
          'Filter by Event:',
          style: TextStyle(fontSize: 13, color: Colors.white70),
        ),
        const SizedBox(width: 10),
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 10),
          decoration: BoxDecoration(
            color: AppTheme.darkCard,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: AppTheme.darkBorder, width: 0.5),
          ),
          child: DropdownButtonHideUnderline(
            child: DropdownButton<String?>(
              value: currentEventFilter,
              dropdownColor: AppTheme.darkCard,
              style: const TextStyle(fontSize: 13, color: Colors.white),
              items: const [
                DropdownMenuItem(value: null, child: Text('All Events')),
                DropdownMenuItem(value: 'completed', child: Text('Completed')),
                DropdownMenuItem(value: 'failed', child: Text('Failed')),
                DropdownMenuItem(value: 'resumed', child: Text('Resumed')),
                DropdownMenuItem(value: 'verified_pending_promotion', child: Text('Verifying')),
                DropdownMenuItem(value: 'removed', child: Text('Removed')),
              ],
              onChanged: (val) {
                viewModel.setHistoryEventTypeFilter(val);
              },
            ),
          ),
        ),
        const Spacer(),
        Text(
          '${viewModel.historyEntries.length} record(s)',
          style: const TextStyle(fontSize: 12, color: Colors.white38),
        ),
      ],
    );
  }

  Widget _buildEmptyState(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.history_toggle_off_rounded, size: 64, color: Colors.white.withValues(alpha: 0.15)),
          const SizedBox(height: 16),
          const Text(
            'No download history recorded yet',
            style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600, color: Colors.white70),
          ),
          const SizedBox(height: 6),
          const Text(
            'Completed, resumed, failed, and removed transfer events will appear here.',
            style: TextStyle(fontSize: 13, color: Colors.white38),
          ),
        ],
      ),
    );
  }

  Widget _buildHistoryTable(BuildContext context, List<HistoryEntry> entries) {
    return ListView.separated(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 8),
      itemCount: entries.length,
      separatorBuilder: (_, _) => const SizedBox(height: 6),
      itemBuilder: (context, index) {
        final entry = entries[index];
        return _buildHistoryEntryRow(context, entry);
      },
    );
  }

  Widget _buildHistoryEntryRow(BuildContext context, HistoryEntry entry) {
    final color = _getEventColor(entry.eventType);

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isCompact = constraints.maxWidth < 700;

          if (isCompact) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                      decoration: BoxDecoration(
                        color: color.withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(4),
                        border: Border.all(color: color.withValues(alpha: 0.4), width: 0.5),
                      ),
                      child: Text(
                        entry.eventType.toUpperCase(),
                        style: TextStyle(fontSize: 10, fontWeight: FontWeight.bold, color: color),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        entry.fileName,
                        style: const TextStyle(fontSize: 13, fontWeight: FontWeight.w600, color: Colors.white),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      _formatTimestamp(entry.timestamp),
                      style: const TextStyle(fontSize: 11, color: Colors.white54),
                    ),
                    IconButton(
                      icon: const Icon(Icons.info_outline_rounded, size: 16, color: Colors.white54),
                      padding: EdgeInsets.zero,
                      constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
                      tooltip: 'View details',
                      onPressed: () => _showEntryDetails(context, entry),
                    ),
                  ],
                ),
                const SizedBox(height: 6),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(
                      '${Formatters.formatBytes(entry.transferredBytes)} of ${Formatters.formatBytes(entry.totalBytes)}',
                      style: const TextStyle(fontSize: 11, color: Colors.white70),
                    ),
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(
                          Formatters.formatDuration(entry.durationSecs),
                          style: const TextStyle(fontSize: 11, color: Colors.white54),
                        ),
                        const SizedBox(width: 8),
                        Container(
                          padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
                          decoration: BoxDecoration(
                            color: _getVerifColor(entry.verificationResult).withValues(alpha: 0.15),
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: Text(
                            entry.verificationResult,
                            style: TextStyle(
                              fontSize: 10,
                              color: _getVerifColor(entry.verificationResult),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ],
            );
          }

          return Row(
            children: [
              // Event Type Pill
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                decoration: BoxDecoration(
                  color: color.withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(6),
                  border: Border.all(color: color.withValues(alpha: 0.4), width: 0.5),
                ),
                child: Text(
                  entry.eventType.toUpperCase(),
                  style: TextStyle(fontSize: 11, fontWeight: FontWeight.bold, color: color),
                ),
              ),
              const SizedBox(width: 16),

              // File Name & ED2K Hash
              Expanded(
                flex: 4,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      entry.fileName,
                      style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w600, color: Colors.white),
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 2),
                    Text(
                      'Hash: ${entry.ed2kHash}',
                      style: const TextStyle(fontSize: 11, color: Colors.white38, fontFamily: 'monospace'),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ),
              ),

              // Transferred / Total Size
              Expanded(
                flex: 2,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      Formatters.formatBytes(entry.transferredBytes),
                      style: const TextStyle(fontSize: 13, color: Colors.white),
                    ),
                    Text(
                      'of ${Formatters.formatBytes(entry.totalBytes)}',
                      style: const TextStyle(fontSize: 11, color: Colors.white38),
                    ),
                  ],
                ),
              ),

              // Duration & Attempts
              Expanded(
                flex: 2,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      Formatters.formatDuration(entry.durationSecs),
                      style: const TextStyle(fontSize: 13, color: Colors.white),
                    ),
                    Text(
                      '${entry.resumeCount} res. / ${entry.connectionAttempts} att.',
                      style: const TextStyle(fontSize: 11, color: Colors.white38),
                    ),
                  ],
                ),
              ),

              // Verification badge
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
                decoration: BoxDecoration(
                  color: _getVerifColor(entry.verificationResult).withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  entry.verificationResult,
                  style: TextStyle(
                    fontSize: 11,
                    color: _getVerifColor(entry.verificationResult),
                  ),
                ),
              ),
              const SizedBox(width: 16),

              // Timestamp
              Text(
                _formatTimestamp(entry.timestamp),
                style: const TextStyle(fontSize: 12, color: Colors.white54),
              ),
              const SizedBox(width: 8),

              // Details Action
              IconButton(
                icon: const Icon(Icons.info_outline_rounded, size: 18, color: Colors.white54),
                tooltip: 'View details',
                onPressed: () => _showEntryDetails(context, entry),
              ),
            ],
          );
        },
      ),
    );
  }

  Color _getEventColor(String eventType) {
    switch (eventType.toLowerCase()) {
      case 'completed':
        return AppTheme.statusCompleted;
      case 'failed':
        return AppTheme.statusFailed;
      case 'resumed':
        return AppTheme.statusDownloading;
      case 'verified_pending_promotion':
        return Colors.purpleAccent;
      case 'removed':
        return Colors.orangeAccent;
      default:
        return Colors.white70;
    }
  }

  Color _getVerifColor(String verif) {
    switch (verif.toLowerCase()) {
      case 'verified_ok':
        return Colors.tealAccent;
      case 'hash_mismatch':
        return AppTheme.statusFailed;
      default:
        return Colors.white54;
    }
  }

  String _formatTimestamp(DateTime dt) {
    final y = dt.year.toString().padLeft(4, '0');
    final m = dt.month.toString().padLeft(2, '0');
    final d = dt.day.toString().padLeft(2, '0');
    final h = dt.hour.toString().padLeft(2, '0');
    final min = dt.minute.toString().padLeft(2, '0');
    return '$y-$m-$d $h:$min';
  }

  void _showEntryDetails(BuildContext context, HistoryEntry entry) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
        title: Row(
          children: [
            const Icon(Icons.history_rounded, color: AppTheme.primary, size: 20),
            const SizedBox(width: 10),
            Expanded(
              child: Text(
                'Event Detail: ${entry.fileName}',
                style: const TextStyle(fontSize: 16, color: Colors.white),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
        content: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 500),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              _buildDetailRow('Event Type', entry.eventType),
              _buildDetailRow('Status', entry.status),
              _buildDetailRow('Total Size', Formatters.formatBytes(entry.totalBytes)),
              _buildDetailRow('Transferred', Formatters.formatBytes(entry.transferredBytes)),
              _buildDetailRow('Duration', Formatters.formatDuration(entry.durationSecs)),
              _buildDetailRow('Attempts', '${entry.connectionAttempts}'),
              _buildDetailRow('Resumes', '${entry.resumeCount}'),
              _buildDetailRow('Verification Result', entry.verificationResult),
              _buildDetailRow('eD2K Hash', entry.ed2kHash),
              _buildDetailRow('Sanitized Path', '[DOWNLOAD_DIR]/${entry.fileName}'),
              _buildDetailRow('Timestamp', entry.timestamp.toIso8601String()),
              if (entry.errorMessage != null)
                _buildDetailRow('Error Message', entry.errorMessage!, isError: true),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }

  Widget _buildDetailRow(String label, String value, {bool isError = false}) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 140,
            child: Text(
              '$label:',
              style: const TextStyle(fontSize: 12, color: Colors.white54, fontWeight: FontWeight.w500),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: TextStyle(
                fontSize: 12,
                color: isError ? AppTheme.statusFailed : Colors.white,
                fontFamily: label.contains('Hash') || label.contains('Path') ? 'monospace' : null,
              ),
            ),
          ),
        ],
      ),
    );
  }

  void _showExportDialog(BuildContext context, String format, String content) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
        title: Row(
          children: [
            const Icon(Icons.file_download_outlined, color: AppTheme.primary, size: 20),
            const SizedBox(width: 10),
            Text(
              'Export History (${format.toUpperCase()})',
              style: const TextStyle(fontSize: 16, color: Colors.white),
            ),
          ],
        ),
        content: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 600, maxHeight: 400),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Report is sanitized: all local usernames and filesystem paths are redacted.',
                style: TextStyle(fontSize: 12, color: Colors.tealAccent),
              ),
              const SizedBox(height: 10),
              Expanded(
                child: Container(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: AppTheme.darkBackground,
                    borderRadius: BorderRadius.circular(6),
                    border: Border.all(color: AppTheme.darkBorder, width: 0.5),
                  ),
                  child: SingleChildScrollView(
                    child: SelectableText(
                      content,
                      style: const TextStyle(fontSize: 11, fontFamily: 'monospace', color: Colors.white70),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
        actions: [
          OutlinedButton.icon(
            icon: const Icon(Icons.copy_rounded, size: 16),
            label: const Text('Copy to Clipboard'),
            onPressed: () {
              Clipboard.setData(ClipboardData(text: content));
              Navigator.of(ctx).pop();
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('Export copied to clipboard')),
              );
            },
          ),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }

  void _confirmClearHistory(BuildContext context) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
        title: const Row(
          children: [
            Icon(Icons.warning_amber_rounded, color: AppTheme.statusFailed, size: 22),
            SizedBox(width: 10),
            Text('Clear Download History?'),
          ],
        ),
        content: const Text(
          'Are you sure you want to permanently delete all download history and statistics records?\n\n'
          'This action is irreversible.',
          style: TextStyle(fontSize: 13, color: Colors.white70, height: 1.4),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            style: FilledButton.styleFrom(backgroundColor: AppTheme.statusFailed),
            onPressed: () async {
              Navigator.of(ctx).pop();
              await viewModel.clearHistory();
            },
            child: const Text('Clear All History'),
          ),
        ],
      ),
    );
  }
}
