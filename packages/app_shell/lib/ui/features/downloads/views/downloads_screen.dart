import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import '../../../../app/shell_controller.dart';
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
import '../../shares/views/shares_view.dart';
import '../../search/views/search_screen.dart';
import '../../kad/views/kad_screen.dart';
import '../../uploads/views/uploads_view.dart';

/// Adaptive app shell: full sidebar on wide layouts, compact rail on medium,
/// drawer on narrow. Global banners and the debug bar wrap the routed content
/// ([child]) provided by the `go_router` ShellRoute. When no [child] is given
/// (standalone usage in tests), the content is derived from
/// [AppController.currentSection].
class DownloadsScreen extends ConsumerWidget {
  final Widget? child;

  const DownloadsScreen({super.key, this.child});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
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
                        child: const DownloadsSidebar(isCompact: false),
                      ),
                    )
                  : null,
              body: Row(
                children: [
                  // Left Navigation Sidebar (Hidden on small screen, compact on medium, full on large)
                  if (!isSmall)
                    DownloadsSidebar(isCompact: isMedium),

                  // Right Workspace Area
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        // Incompatible protocol banner if any
                        if (ref.watch(shellControllerProvider).protocolMismatchError != null)
                          IncompatibleVersionBanner(
                            errorMessage: ref.watch(shellControllerProvider).protocolMismatchError!,
                            onRetry: () => ref.watch(daemonSupervisorProvider)?.start(),
                          ),

                        // Status Notification banner if any
                        if (ref.watch(shellControllerProvider).statusNotification != null)
                          StatusNotificationBanner(
                            message: ref.watch(shellControllerProvider).statusNotification!,
                            onClose: ref.read(shellControllerProvider).clearStatusNotification,
                          ),

                        // Main Workspace Area (routed or VM-driven)
                        Expanded(
                          child: child ?? _sectionContent(ref),
                        ),

                        // Optional Debug Bar
                        if (ref.watch(shellControllerProvider).isDebugBarVisible)
                          const DebugSimulationBar(),
                      ],
                    ),
                  ),
                ],
              ),
            );
          },
        );
  }

  /// Legacy content switch kept for standalone usage (tests) where no router
  /// child is injected. In the routed app each GoRoute supplies its own view.
  Widget _sectionContent(WidgetRef ref) {
    return switch (ref.watch(shellControllerProvider).section) {
      AppNavSection.history => const HistoryView(),
      AppNavSection.servers => const ServersView(),
      AppNavSection.search => const SearchView(),
      AppNavSection.kad => const KadView(),
      AppNavSection.uploads => const UploadsView(),
      AppNavSection.shares => const SharesView(),
      AppNavSection.downloads => const DownloadsView(),
    };
  }
}

/// Downloads workspace: top action bar, downloads list, and the collapsible
/// details panel for the selected download. Rendered as the `/downloads`
/// route content inside [DownloadsScreen].
class DownloadsView extends ConsumerWidget {
  const DownloadsView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final isSmall =
        MediaQuery.sizeOf(context).width < AppBreakpoints.compact;
    final selected = ref.watch(selectedDownloadProvider);
    return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          DownloadsTopBar(isSmall: isSmall),
          Expanded(child: _buildContent(ref)),
          if (selected != null)
            DownloadDetailsPanel(item: selected, isSmall: isSmall),
        ],
    );
  }

  Widget _buildContent(WidgetRef ref) {
    final filtered = ref.watch(filteredDownloadsProvider);
    if (ref.watch(downloadsStoreControllerProvider).isLoading &&
        filtered.isEmpty) {
      return const Center(child: CircularProgressIndicator());
    }
    if (filtered.isEmpty) {
      return const DownloadsEmptyState();
    }
    return const DownloadsList();
  }
}
