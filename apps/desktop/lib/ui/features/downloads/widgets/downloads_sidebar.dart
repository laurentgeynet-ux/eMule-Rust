import 'package:flutter/material.dart';
import '../../../../domain/models/daemon_info.dart';
import '../../../../services/daemon_supervisor.dart';
import '../../../core/strings.dart';
import '../../../core/theme.dart';
import '../view_models/downloads_view_model.dart';

class DownloadsSidebar extends StatelessWidget {
  final DownloadsViewModel viewModel;

  const DownloadsSidebar({super.key, required this.viewModel});

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 260,
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(
          right: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          // App Brand Header
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 24),
            decoration: const BoxDecoration(
              border: Border(
                bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
              ),
            ),
            child: Row(
              children: [
                Container(
                  padding: const EdgeInsets.all(8),
                  decoration: BoxDecoration(
                    color: AppTheme.primary.withValues(alpha: 0.2),
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: const Icon(
                    Icons.cloud_download_rounded,
                    color: AppTheme.primary,
                    size: 26,
                  ),
                ),
                const Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        AppStrings.appName,
                        style: TextStyle(
                          fontSize: 18,
                          fontWeight: FontWeight.bold,
                          letterSpacing: 0.5,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                      Text(
                        AppStrings.appTagline,
                        style: TextStyle(
                          fontSize: 12,
                          color: Colors.white54,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),

          const SizedBox(height: 16),

          // Section Title
          const Padding(
            padding: EdgeInsets.symmetric(horizontal: 20, vertical: 6),
            child: Text(
              AppStrings.sectionDownloads,
              style: TextStyle(
                fontSize: 11,
                fontWeight: FontWeight.w600,
                color: Colors.white38,
                letterSpacing: 1.0,
              ),
            ),
          ),

          // Filter Navigation Items
          _buildFilterItem(
            filter: DownloadFilter.all,
            icon: Icons.all_inbox_rounded,
            count: viewModel.countAll,
          ),
          _buildFilterItem(
            filter: DownloadFilter.active,
            icon: Icons.downloading_rounded,
            count: viewModel.countActive,
            color: AppTheme.statusDownloading,
          ),
          _buildFilterItem(
            filter: DownloadFilter.paused,
            icon: Icons.pause_circle_outline_rounded,
            count: viewModel.countPaused,
            color: AppTheme.statusPaused,
          ),
          _buildFilterItem(
            filter: DownloadFilter.completed,
            icon: Icons.check_circle_outline_rounded,
            count: viewModel.countCompleted,
            color: AppTheme.statusCompleted,
          ),
          _buildFilterItem(
            filter: DownloadFilter.failed,
            icon: Icons.error_outline_rounded,
            count: viewModel.countFailed,
            color: AppTheme.statusFailed,
          ),

          const Spacer(),

          // Daemon Connection Status Card
          Container(
            margin: const EdgeInsets.all(16),
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              color: AppTheme.darkCard,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: AppTheme.darkBorder, width: 0.5),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Container(
                      width: 10,
                      height: 10,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: viewModel.supervisor != null
                            ? _getSupervisorColor(viewModel.supervisorState)
                            : _getConnectionColor(viewModel.connectionState),
                        boxShadow: [
                          BoxShadow(
                            color: (viewModel.supervisor != null
                                    ? _getSupervisorColor(viewModel.supervisorState)
                                    : _getConnectionColor(viewModel.connectionState))
                                .withValues(alpha: 0.5),
                            blurRadius: 6,
                            spreadRadius: 1,
                          )
                        ],
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        viewModel.supervisor != null
                            ? viewModel.supervisorState.label
                            : viewModel.connectionState.label,
                        style: const TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
                if (viewModel.daemonInfo != null) ...[
                  const SizedBox(height: 6),
                  Text(
                    'Daemon v${viewModel.daemonInfo!.version} (proto v${viewModel.daemonInfo!.protocolVersion})',
                    style: const TextStyle(fontSize: 11, color: Colors.white54),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildFilterItem({
    required DownloadFilter filter,
    required IconData icon,
    required int count,
    Color? color,
  }) {
    final isSelected = viewModel.currentFilter == filter;
    final isDimmed = count == 0 && !isSelected;

    final item = Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 2),
      child: Material(
        color: isSelected
            ? AppTheme.primary.withValues(alpha: 0.15)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: () => viewModel.setFilter(filter),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            child: Row(
              children: [
                Icon(
                  icon,
                  size: 20,
                  color: isSelected
                      ? AppTheme.primary
                      : (color ?? Colors.white70),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    filter.label,
                    style: TextStyle(
                      fontSize: 14,
                      fontWeight:
                          isSelected ? FontWeight.w600 : FontWeight.normal,
                      color: isSelected ? Colors.white : Colors.white70,
                    ),
                  ),
                ),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(
                    color: isSelected
                        ? AppTheme.primary.withValues(alpha: 0.3)
                        : Colors.white10,
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Text(
                    '$count',
                    style: TextStyle(
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                      color: isSelected ? Colors.white : Colors.white60,
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );

    if (isDimmed) {
      return Opacity(opacity: 0.55, child: item);
    }
    return item;
  }

  Color _getConnectionColor(DaemonConnectionState state) {
    switch (state) {
      case DaemonConnectionState.connected:
        return AppTheme.statusCompleted;
      case DaemonConnectionState.connecting:
        return AppTheme.statusPaused;
      case DaemonConnectionState.error:
        return AppTheme.statusFailed;
      case DaemonConnectionState.disconnected:
        return AppTheme.statusQueued;
    }
  }

  Color _getSupervisorColor(SupervisorState state) {
    switch (state) {
      case SupervisorState.connected:
        return AppTheme.statusCompleted;
      case SupervisorState.starting:
      case SupervisorState.discovering:
      case SupervisorState.reconnecting:
        return AppTheme.statusPaused;
      case SupervisorState.protocolMismatch:
      case SupervisorState.authError:
      case SupervisorState.error:
        return AppTheme.statusFailed;
      case SupervisorState.idle:
        return AppTheme.statusQueued;
    }
  }
}
