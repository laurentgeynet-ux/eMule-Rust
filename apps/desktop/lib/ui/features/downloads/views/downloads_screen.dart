import 'package:flutter/material.dart';
import '../../../../domain/models/download_item.dart';
import '../../../core/formatters.dart';
import '../../../core/theme.dart';
import '../view_models/downloads_view_model.dart';
import '../widgets/add_link_dialog.dart';
import '../widgets/debug_simulation_bar.dart';
import '../widgets/download_row.dart';
import '../widgets/downloads_sidebar.dart';

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
                    // Top Navigation / Actions Bar
                    _buildTopBar(context),

                    // Error banner if any
                    if (viewModel.errorMessage != null)
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                        color: AppTheme.statusFailed.withValues(alpha: 0.2),
                        child: Row(
                          children: [
                            const Icon(Icons.error_outline, color: AppTheme.statusFailed, size: 18),
                            const SizedBox(width: 8),
                            Expanded(
                              child: Text(
                                viewModel.errorMessage!,
                                style: const TextStyle(color: AppTheme.statusFailed, fontSize: 13),
                              ),
                            ),
                          ],
                        ),
                      ),

                    // Notification banner if any
                    if (viewModel.statusNotification != null)
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                        color: AppTheme.primary.withValues(alpha: 0.15),
                        child: Row(
                          children: [
                            const Icon(Icons.info_outline, color: AppTheme.primary, size: 18),
                            const SizedBox(width: 8),
                            Expanded(
                              child: Text(
                                viewModel.statusNotification!,
                                style: const TextStyle(color: Colors.white, fontSize: 13),
                              ),
                            ),
                          ],
                        ),
                      ),

                    // Main Content: Downloads Table or Empty State
                    Expanded(
                      child: _buildDownloadsArea(context),
                    ),

                    // Selected item details panel (collapsible)
                    if (viewModel.selectedDownload != null)
                      _buildDetailsPanel(context, viewModel.selectedDownload!),

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
      padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 14),
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
            style: const TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
          ),
          const SizedBox(width: 24),

          // Search Field
          Expanded(
            child: SizedBox(
              height: 38,
              child: TextField(
                onChanged: viewModel.setSearchQuery,
                style: const TextStyle(fontSize: 13),
                decoration: InputDecoration(
                  hintText: 'Search downloads by filename...',
                  hintStyle: const TextStyle(fontSize: 13, color: Colors.white38),
                  prefixIcon: const Icon(Icons.search, size: 18, color: Colors.white38),
                  contentPadding: const EdgeInsets.symmetric(vertical: 0, horizontal: 12),
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

          const SizedBox(width: 20),

          // Global Download Speed Badge
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
            decoration: BoxDecoration(
              color: AppTheme.darkCard,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: AppTheme.darkBorder, width: 0.5),
            ),
            child: Row(
              children: [
                const Icon(Icons.arrow_downward_rounded, size: 16, color: AppTheme.statusDownloading),
                const SizedBox(width: 6),
                Text(
                  Formatters.formatSpeed(viewModel.totalDownloadSpeed),
                  style: const TextStyle(
                    fontSize: 13,
                    fontWeight: FontWeight.bold,
                    color: AppTheme.statusDownloading,
                  ),
                ),
              ],
            ),
          ),

          const SizedBox(width: 12),

          // Debug Tools Toggle
          IconButton(
            icon: Icon(
              Icons.bug_report_rounded,
              color: viewModel.isDebugBarVisible ? Colors.amberAccent : Colors.white54,
              size: 20,
            ),
            tooltip: 'Toggle Simulation & Debug Tools',
            onPressed: viewModel.toggleDebugBar,
          ),

          // Refresh Button
          IconButton(
            icon: const Icon(Icons.refresh_rounded, color: Colors.white54, size: 20),
            tooltip: 'Refresh Downloads',
            onPressed: viewModel.refresh,
          ),

          const SizedBox(width: 8),

          // Add Link Button
          FilledButton.icon(
            icon: const Icon(Icons.add_rounded, size: 18),
            label: const Text('Add Link'),
            style: FilledButton.styleFrom(
              backgroundColor: AppTheme.primary,
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
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
              padding: const EdgeInsets.all(20),
              decoration: BoxDecoration(
                color: AppTheme.darkSurface,
                shape: BoxShape.circle,
                border: Border.all(color: AppTheme.darkBorder, width: 0.5),
              ),
              child: const Icon(Icons.inbox_rounded, size: 48, color: Colors.white30),
            ),
            const SizedBox(height: 16),
            const Text(
              'No downloads in this view',
              style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold, color: Colors.white70),
            ),
            const SizedBox(height: 6),
            const Text(
              'Click "+ Add Link" to paste an eD2K link and begin downloading.',
              style: TextStyle(fontSize: 13, color: Colors.white38),
            ),
            const SizedBox(height: 16),
            FilledButton.icon(
              icon: const Icon(Icons.add_rounded, size: 16),
              label: const Text('Add eD2K Link'),
              style: FilledButton.styleFrom(backgroundColor: AppTheme.primary),
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
}
