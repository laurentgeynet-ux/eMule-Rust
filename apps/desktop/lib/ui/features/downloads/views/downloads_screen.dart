import 'package:flutter/material.dart';
import '../../../../domain/models/download_item.dart';
import '../../../core/formatters.dart';
import '../../../core/strings.dart';
import '../../../core/theme.dart';
import '../view_models/downloads_view_model.dart';
import '../widgets/add_link_dialog.dart';
import '../widgets/debug_simulation_bar.dart';
import '../widgets/diagnostic_dialog.dart';
import '../widgets/download_detail_dialog.dart';
import '../widgets/download_row.dart';
import '../widgets/downloads_sidebar.dart';
import '../widgets/import_part_met_dialog.dart';
import '../widgets/incompatible_version_dialog.dart';
import '../widgets/lab_guide_dialog.dart';
import '../../history/views/history_view.dart';

class DownloadsScreen extends StatelessWidget {
  final DownloadsViewModel viewModel;

  const DownloadsScreen({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: viewModel,
      builder: (context, _) {
        return Scaffold(
          backgroundColor: AppTheme.darkBackground,
          body: Row(
            children: [
              // Left Navigation Sidebar
              DownloadsSidebar(viewModel: viewModel),

              // Right Workspace Area
              Expanded(
                child: Column(
                  children: [
                    // Incompatible protocol banner if any
                    if (viewModel.protocolMismatchError != null)
                      IncompatibleVersionBanner(
                        errorMessage: viewModel.protocolMismatchError!,
                        onRetry: () => viewModel.supervisor?.start(),
                      ),

                    // Status Notification banner if any
                    if (viewModel.statusNotification != null)
                      _buildStatusNotificationBanner(context, viewModel.statusNotification!),

                    // Main Workspace Area
                    if (viewModel.currentSection == AppNavSection.history) ...[
                      Expanded(
                        child: HistoryView(viewModel: viewModel),
                      ),
                    ] else ...[
                      // Top Navigation / Actions Bar
                      _buildTopBar(context),

                      // Main Content: Downloads Table or Empty State
                      Expanded(
                        child: _buildDownloadsArea(context),
                      ),

                      // Selected item details panel (collapsible)
                      if (viewModel.selectedDownload != null)
                        _buildDetailsPanel(context, viewModel.selectedDownload!),
                    ],

                    // Optional Debug Bar
                    if (viewModel.isDebugBarVisible)
                      DebugSimulationBar(viewModel: viewModel),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildTopBar(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: Row(
        children: [
          // Filter title
          Text(
            viewModel.currentFilter.label,
            style: const TextStyle(fontSize: 16, fontWeight: FontWeight.bold),
          ),
          const SizedBox(width: 16),

          // Search Field (Flexible responsive width)
          Expanded(
            child: SizedBox(
              height: 36,
              child: TextField(
                onChanged: viewModel.setSearchQuery,
                style: const TextStyle(fontSize: 13),
                decoration: InputDecoration(
                  hintText: AppStrings.searchPlaceholder,
                  hintStyle: const TextStyle(fontSize: 12, color: Colors.white38),
                  isDense: true,
                  prefixIcon: const Icon(Icons.search, size: 16, color: Colors.white38),
                  prefixIconConstraints: const BoxConstraints(minWidth: 30, minHeight: 30),
                  contentPadding: const EdgeInsets.symmetric(vertical: 8, horizontal: 8),
                  filled: true,
                  fillColor: AppTheme.darkCard,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(8),
                    borderSide: BorderSide.none,
                  ),
                ),
              ),
            ),
          ),

          const SizedBox(width: 10),

          // Mode Badge (Clickable for Diagnostics)
          Tooltip(
            message: viewModel.isLabModeAvailable
                ? '${AppStrings.tooltipLoopbackLabMode}\n\nClick to inspect system & workdir diagnostics.'
                : 'System Diagnostics & Confinement (127.0.0.1)\n\nClick to inspect system & workdir diagnostics.',
            preferBelow: true,
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
            decoration: BoxDecoration(
              color: const Color(0xF01E2A3A),
              borderRadius: BorderRadius.circular(8),
              border: Border.all(
                color: (viewModel.isLabModeAvailable ? Colors.teal : Colors.blueGrey).withValues(alpha: 0.4),
                width: 0.5,
              ),
            ),
            textStyle: const TextStyle(
              fontSize: 12,
              color: Colors.white70,
              height: 1.4,
            ),
            child: InkWell(
              borderRadius: BorderRadius.circular(8),
              onTap: () {
                showDialog(
                  context: context,
                  builder: (ctx) => DiagnosticDialog(viewModel: viewModel),
                );
              },
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 5),
                decoration: BoxDecoration(
                  color: (viewModel.isLabModeAvailable ? Colors.teal : Colors.blueGrey).withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(
                    color: (viewModel.isLabModeAvailable ? Colors.teal : Colors.blueGrey).withValues(alpha: 0.3),
                    width: 0.5,
                  ),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(
                      viewModel.isLabModeAvailable ? Icons.science_outlined : Icons.analytics_outlined,
                      size: 14,
                      color: viewModel.isLabModeAvailable ? Colors.tealAccent : Colors.white70,
                    ),
                    const SizedBox(width: 5),
                    Text(
                      viewModel.isLabModeAvailable ? AppStrings.loopbackLabMode : 'Diagnostics',
                      style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w500,
                        color: viewModel.isLabModeAvailable ? Colors.tealAccent : Colors.white70,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),

          // Guided Local Test Flow Button (Gated to Portable or Lab mode)
          if (viewModel.isLabModeAvailable)
            IconButton(
              constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
              padding: EdgeInsets.zero,
              visualDensity: VisualDensity.compact,
              icon: const Icon(Icons.biotech, color: Colors.tealAccent, size: 18),
              tooltip: 'Guided Local Test Flow',
              onPressed: () {
                showDialog(
                  context: context,
                  builder: (ctx) => LabGuideDialog(viewModel: viewModel),
                );
              },
            ),

          const SizedBox(width: 6),

          // Global Speed Badge: Download & Upload (↓ 0 B/s   ↑ 0 B/s)
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
            decoration: BoxDecoration(
              color: AppTheme.darkCard,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: AppTheme.darkBorder, width: 0.5),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                const Icon(Icons.arrow_downward_rounded, size: 13, color: AppTheme.statusDownloading),
                const SizedBox(width: 3),
                Text(
                  Formatters.formatSpeed(viewModel.totalDownloadSpeed),
                  style: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.bold,
                    color: AppTheme.statusDownloading,
                  ),
                ),
                const SizedBox(width: 8),
                const Icon(Icons.arrow_upward_rounded, size: 13, color: Colors.white38),
                const SizedBox(width: 3),
                Text(
                  Formatters.formatSpeed(viewModel.totalUploadSpeed),
                  style: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.bold,
                    color: Colors.white38,
                  ),
                ),
              ],
            ),
          ),

          // Dev Tools Gating: Only visible if enabled via --dart-define=ENABLE_DEV_TOOLS=true or fake client
          if (viewModel.isDevToolsAvailable) ...[
            const SizedBox(width: 8),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 3),
              decoration: BoxDecoration(
                color: Colors.amber.withValues(alpha: 0.2),
                borderRadius: BorderRadius.circular(6),
                border: Border.all(color: Colors.amber.withValues(alpha: 0.4), width: 0.5),
              ),
              child: const Text(
                AppStrings.devModeBadge,
                style: TextStyle(
                  fontSize: 10,
                  fontWeight: FontWeight.bold,
                  color: Colors.amberAccent,
                ),
              ),
            ),
            IconButton(
              constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
              padding: EdgeInsets.zero,
              visualDensity: VisualDensity.compact,
              icon: Icon(
                Icons.bug_report_rounded,
                color: viewModel.isDebugBarVisible ? Colors.amberAccent : Colors.white54,
                size: 18,
              ),
              tooltip: AppStrings.tooltipDevTools,
              onPressed: viewModel.toggleDebugBar,
            ),
          ],

          const SizedBox(width: 2),

          // Refresh Button
          IconButton(
            constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
            padding: EdgeInsets.zero,
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.refresh_rounded, color: Colors.white54, size: 18),
            tooltip: AppStrings.tooltipRefresh,
            onPressed: viewModel.refresh,
          ),



          const SizedBox(width: 6),

          // Add Link Button
          FilledButton.icon(
            icon: const Icon(Icons.add_rounded, size: 16),
            label: const Text(AppStrings.btnAddLink),
            style: FilledButton.styleFrom(
              backgroundColor: AppTheme.primary,
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
            ),
            onPressed: () {
              showDialog(
                context: context,
                builder: (ctx) => AddLinkDialog(viewModel: viewModel),
              );
            },
          ),
        ],
      ),
    );
  }

  Widget _buildDownloadsArea(BuildContext context) {
    final list = viewModel.filteredDownloads;

    if (viewModel.isLoading && list.isEmpty) {
      return const Center(child: CircularProgressIndicator());
    }

    if (list.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              padding: const EdgeInsets.all(22),
              decoration: BoxDecoration(
                color: AppTheme.darkSurface,
                shape: BoxShape.circle,
                border: Border.all(color: AppTheme.darkBorder, width: 0.5),
              ),
              child: const Icon(Icons.cloud_download_outlined, size: 52, color: Colors.white30),
            ),
            const SizedBox(height: 16),
            const Text(
              AppStrings.emptyStateTitle,
              style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold, color: Colors.white70),
            ),
            const SizedBox(height: 8),
            const Padding(
              padding: EdgeInsets.symmetric(horizontal: 32),
              child: Text(
                AppStrings.emptyStateSubtitle,
                textAlign: TextAlign.center,
                style: TextStyle(fontSize: 13, color: Colors.white38, height: 1.4),
              ),
            ),
            const SizedBox(height: 20),
            Wrap(
              spacing: 12,
              runSpacing: 8,
              alignment: WrapAlignment.center,
              children: [
                FilledButton.icon(
                  icon: const Icon(Icons.add_rounded, size: 16),
                  label: const Text(AppStrings.btnAddEd2kLink),
                  style: FilledButton.styleFrom(backgroundColor: AppTheme.primary),
                  onPressed: () {
                    showDialog(
                      context: context,
                      builder: (ctx) => AddLinkDialog(viewModel: viewModel),
                    );
                  },
                ),
                OutlinedButton.icon(
                  key: const Key('empty_import_part_met_btn'),
                  icon: const Icon(Icons.file_download_outlined, size: 16),
                  label: const Text('Import .part.met'),
                  style: OutlinedButton.styleFrom(
                    foregroundColor: Colors.white70,
                    side: const BorderSide(color: AppTheme.darkBorder),
                  ),
                  onPressed: () {
                    showDialog(
                      context: context,
                      builder: (ctx) => ImportPartMetDialog(viewModel: viewModel),
                    );
                  },
                ),
                if (viewModel.isLabModeAvailable)
                  OutlinedButton.icon(
                    icon: const Icon(Icons.biotech, size: 16),
                    label: const Text('Guided Local Test'),
                    style: OutlinedButton.styleFrom(
                      foregroundColor: Colors.tealAccent,
                      side: BorderSide(color: Colors.teal.withValues(alpha: 0.5)),
                    ),
                    onPressed: () {
                      showDialog(
                        context: context,
                        builder: (ctx) => LabGuideDialog(viewModel: viewModel),
                      );
                    },
                  ),
              ],
            ),
          ],
        ),
      );
    }

    return ListView.builder(
      padding: const EdgeInsets.all(16),
      itemCount: list.length,
      itemBuilder: (context, index) {
        final item = list[index];
        final isSelected = viewModel.selectedDownloadId == item.id;

        return DownloadRow(
          item: item,
          viewModel: viewModel,
          isSelected: isSelected,
          onTap: () {
            viewModel.selectDownload(isSelected ? null : item.id);
          },
        );
      },
    );
  }

  Widget _buildDetailsPanel(BuildContext context, DownloadItem item) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(
          top: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.info_outline_rounded, size: 18, color: AppTheme.primary),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  'Details: ${item.fileName}',
                  style: const TextStyle(fontSize: 14, fontWeight: FontWeight.bold),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              OutlinedButton.icon(
                icon: const Icon(Icons.open_in_new_rounded, size: 14),
                label: const Text('Inspect Full Details', style: TextStyle(fontSize: 12)),
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppTheme.primary,
                  side: const BorderSide(color: AppTheme.primary, width: 0.8),
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                ),
                onPressed: () {
                  showDialog(
                    context: context,
                    builder: (ctx) => DownloadDetailDialog(
                      downloadId: item.id,
                      viewModel: viewModel,
                    ),
                  );
                },
              ),
              const SizedBox(width: 8),
              IconButton(
                icon: const Icon(Icons.close, size: 16, color: Colors.white54),
                onPressed: () => viewModel.selectDownload(null),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 32,
            runSpacing: 12,
            children: [
              _detailMetric('Download ID', item.id),
              _detailMetric('Total Size', Formatters.formatBytes(item.totalBytes)),
              _detailMetric('Verified Bytes', Formatters.formatBytes(item.verifiedBytes)),
              _detailMetric('Status', item.status.label),
              _detailMetric('Sources', '${item.sourceCount}'),
            ],
          ),
        ],
      ),
    );
  }

  Widget _detailMetric(String label, String value) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: const TextStyle(fontSize: 11, color: Colors.white38)),
        const SizedBox(height: 2),
        Text(value, style: const TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white)),
      ],
    );
  }

  Widget _buildStatusNotificationBanner(BuildContext context, String message) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      decoration: BoxDecoration(
        color: Colors.teal.withValues(alpha: 0.15),
        border: const Border(bottom: BorderSide(color: Colors.teal, width: 0.5)),
      ),
      child: Row(
        children: [
          const Icon(Icons.check_circle_outline, size: 16, color: Colors.tealAccent),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              message,
              style: const TextStyle(fontSize: 12, color: Colors.tealAccent),
            ),
          ),
          InkWell(
            onTap: viewModel.clearStatusNotification,
            child: const Padding(
              padding: EdgeInsets.all(4),
              child: Icon(Icons.close, size: 14, color: Colors.tealAccent),
            ),
          ),
        ],
      ),
    );
  }
}
