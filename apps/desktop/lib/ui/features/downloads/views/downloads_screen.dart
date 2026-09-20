import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../view_models/downloads_view_model.dart';
import '../widgets/debug_simulation_bar.dart';
import '../widgets/download_details_panel.dart';
import '../widgets/downloads_empty_state.dart';
import '../widgets/downloads_list.dart';
import '../widgets/downloads_sidebar.dart';
import '../widgets/downloads_top_bar.dart';
import '../widgets/incompatible_version_dialog.dart';
import '../widgets/status_notification_banner.dart';
import '../../history/views/history_view.dart';
import '../../servers/views/servers_screen.dart';
import '../../search/views/search_screen.dart';
import '../../kad/views/kad_screen.dart';

/// Adaptive app shell: full sidebar on wide layouts, compact rail on medium,
/// drawer on narrow. Global banners and the debug bar wrap the routed content
/// ([child]) provided by the `go_router` ShellRoute. When no [child] is given
/// (standalone usage in tests), the content is derived from
/// [DownloadsViewModel.currentSection].
class DownloadsScreen extends StatelessWidget {
  final DownloadsViewModel viewModel;
  final Widget? child;

  const DownloadsScreen({super.key, required this.viewModel, this.child});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: viewModel,
      builder: (context, _) {
        return LayoutBuilder(
          builder: (context, constraints) {
            final isSmall = constraints.maxWidth < AppBreakpoints.compact;
            final isMedium = constraints.maxWidth >= AppBreakpoints.compact &&
                constraints.maxWidth < AppBreakpoints.medium;

            return Scaffold(
              backgroundColor: AppColors.background,
              drawer: isSmall
                  ? Drawer(
                      backgroundColor: AppColors.surface,
                      child: SafeArea(
                        child: DownloadsSidebar(viewModel: viewModel, isCompact: false),
                      ),
                    )
                  : null,
              body: Row(
                children: [
                  // Left Navigation Sidebar (Hidden on small screen, compact on medium, full on large)
                  if (!isSmall)
                    DownloadsSidebar(
                      viewModel: viewModel,
                      isCompact: isMedium,
                    ),

                  // Right Workspace Area
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        // Incompatible protocol banner if any
                        if (viewModel.protocolMismatchError != null)
                          IncompatibleVersionBanner(
                            errorMessage: viewModel.protocolMismatchError!,
                            onRetry: () => viewModel.supervisor?.start(),
                          ),

                        // Status Notification banner if any
                        if (viewModel.statusNotification != null)
                          StatusNotificationBanner(
                            message: viewModel.statusNotification!,
                            onClose: viewModel.clearStatusNotification,
                          ),

                        // Main Workspace Area (routed or VM-driven)
                        Expanded(
                          child: child ?? _sectionContent(),
                        ),

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
      },
    );
  }

  /// Legacy content switch kept for standalone usage (tests) where no router
  /// child is injected. In the routed app each GoRoute supplies its own view.
  Widget _sectionContent() {
    return switch (viewModel.currentSection) {
      AppNavSection.history => const HistoryView(),
      AppNavSection.servers => const ServersView(),
      AppNavSection.search => const SearchView(),
      AppNavSection.kad => const KadView(),
      AppNavSection.downloads => DownloadsView(viewModel: viewModel),
    };
  }
}

/// Downloads workspace: top action bar, downloads list, and the collapsible
/// details panel for the selected download. Rendered as the `/downloads`
/// route content inside [DownloadsScreen].
class DownloadsView extends StatelessWidget {
  final DownloadsViewModel viewModel;

  const DownloadsView({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
    final isSmall =
        MediaQuery.sizeOf(context).width < AppBreakpoints.compact;
    return ListenableBuilder(
      listenable: viewModel,
      builder: (context, _) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          DownloadsTopBar(viewModel: viewModel, isSmall: isSmall),
          Expanded(child: _buildContent()),
          if (viewModel.selectedDownload != null)
            DownloadDetailsPanel(
              item: viewModel.selectedDownload!,
              viewModel: viewModel,
              isSmall: isSmall,
            ),
        ],
      ),
    );
  }

  Widget _buildContent() {
    if (viewModel.isLoading && viewModel.filteredDownloads.isEmpty) {
      return const Center(child: CircularProgressIndicator());
    }
    if (viewModel.filteredDownloads.isEmpty) {
      return DownloadsEmptyState(viewModel: viewModel);
    }
    return DownloadsList(viewModel: viewModel);
  }
}
