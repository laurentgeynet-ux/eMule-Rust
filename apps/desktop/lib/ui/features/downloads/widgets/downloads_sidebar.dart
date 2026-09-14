import 'package:flutter/material.dart';
import '../../../../domain/models/category_item.dart';
import '../../../../domain/models/daemon_info.dart';
import '../../../../services/daemon_supervisor.dart';
import '../../../core/strings.dart';
import '../../../core/theme.dart';
import '../view_models/downloads_view_model.dart';
import 'category_edit_dialog.dart';
import 'diagnostic_dialog.dart';
import 'import_part_met_dialog.dart';
import 'settings_dialog.dart';

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
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
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
                const SizedBox(width: 12),
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

          // Scrollable middle section
          Expanded(
            child: SingleChildScrollView(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const SizedBox(height: 8),

                  // Section Title
                  const Padding(
                    padding: EdgeInsets.symmetric(horizontal: 20, vertical: 4),
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

                  const SizedBox(height: 6),
                  const Divider(color: AppTheme.darkBorder, height: 1),
                  const SizedBox(height: 6),

                  // Categories Section Header
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 2),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        const Text(
                          'CATEGORIES',
                          style: TextStyle(
                            fontSize: 11,
                            fontWeight: FontWeight.w600,
                            color: Colors.white38,
                            letterSpacing: 1.0,
                          ),
                        ),
                        IconButton(
                          icon: const Icon(Icons.add_rounded, size: 16, color: Colors.white60),
                          tooltip: 'Add Category',
                          padding: EdgeInsets.zero,
                          constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
                          onPressed: () {
                            showDialog(
                              context: context,
                              builder: (ctx) => CategoryEditDialog(viewModel: viewModel),
                            );
                          },
                        ),
                      ],
                    ),
                  ),

                  // Virtual: All Downloads
                  _buildCategoryItem(
                    context: context,
                    title: 'All Downloads',
                    color: Colors.white70,
                    icon: Icons.folder_outlined,
                    count: viewModel.countAll,
                    isSelected: viewModel.currentSection == AppNavSection.downloads &&
                        viewModel.selectedCategoryId == null,
                    onTap: () {
                      viewModel.selectCategory(null);
                      viewModel.setSection(AppNavSection.downloads);
                    },
                  ),

                  // Virtual: Uncategorized
                  _buildCategoryItem(
                    context: context,
                    title: 'Sans catégorie',
                    color: Colors.white38,
                    icon: Icons.label_off_outlined,
                    count: viewModel.uncategorizedCount,
                    isSelected: viewModel.currentSection == AppNavSection.downloads &&
                        viewModel.selectedCategoryId == 'uncategorized',
                    onTap: () {
                      viewModel.selectCategory('uncategorized');
                      viewModel.setSection(AppNavSection.downloads);
                    },
                  ),

                  // User Categories
                  ...viewModel.categories.map((cat) {
                    final isSelected = viewModel.currentSection == AppNavSection.downloads &&
                        viewModel.selectedCategoryId == cat.id;
                    final count = viewModel.categoryCounts[cat.id] ?? 0;
                    return _buildCategoryItem(
                      context: context,
                      title: cat.name,
                      color: cat.parsedColor,
                      count: count,
                      isSelected: isSelected,
                      category: cat,
                      onTap: () {
                        viewModel.selectCategory(cat.id);
                        viewModel.setSection(AppNavSection.downloads);
                      },
                    );
                  }),

                  const SizedBox(height: 6),
                  const Divider(color: AppTheme.darkBorder, height: 1),
                  const SizedBox(height: 6),

                  // Section Title
                  const Padding(
                    padding: EdgeInsets.symmetric(horizontal: 20, vertical: 4),
                    child: Text(
                      'AUDIT & ACTIVITY',
                      style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w600,
                        color: Colors.white38,
                        letterSpacing: 1.0,
                      ),
                    ),
                  ),

                  // History & Statistics Nav Item
                  _buildNavItem(
                    title: 'History & Statistics',
                    icon: Icons.history_rounded,
                    isSelected: viewModel.currentSection == AppNavSection.history,
                    onTap: () => viewModel.setSection(AppNavSection.history),
                    count: viewModel.historyEntries.isNotEmpty ? viewModel.historyEntries.length : null,
                  ),

                  const SizedBox(height: 4),

                  // Settings Button
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Material(
                      color: Colors.transparent,
                      borderRadius: BorderRadius.circular(8),
                      child: InkWell(
                        borderRadius: BorderRadius.circular(8),
                        onTap: () {
                          showDialog(
                            context: context,
                            builder: (ctx) => SettingsDialog(viewModel: viewModel),
                          );
                        },
                        child: Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                          child: Row(
                            children: const [
                              Icon(Icons.settings_outlined, size: 20, color: Colors.white70),
                              SizedBox(width: 12),
                              Expanded(
                                child: Text(
                                  'Settings',
                                  style: TextStyle(
                                    fontSize: 14,
                                    fontWeight: FontWeight.normal,
                                    color: Colors.white70,
                                  ),
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 4),

                  // Import .part.met Button
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    child: Material(
                      color: Colors.transparent,
                      borderRadius: BorderRadius.circular(8),
                      child: InkWell(
                        key: const Key('btn_import_part_met'),
                        borderRadius: BorderRadius.circular(8),
                        onTap: () {
                          showDialog(
                            context: context,
                            builder: (ctx) => ImportPartMetDialog(viewModel: viewModel),
                          );
                        },
                        child: Padding(
                          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                          child: Row(
                            children: const [
                              Icon(Icons.file_download_outlined, size: 20, color: Colors.tealAccent),
                              SizedBox(width: 12),
                              Expanded(
                                child: Text(
                                  'Import .part.met',
                                  style: TextStyle(
                                    fontSize: 14,
                                    fontWeight: FontWeight.normal,
                                    color: Colors.white70,
                                  ),
                                  overflow: TextOverflow.ellipsis,
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 8),
                ],
              ),
            ),
          ),

          // Daemon Connection Status Card (Clickable to view Diagnostics)
          Padding(
            padding: const EdgeInsets.all(16),
            child: Material(
              color: AppTheme.darkCard,
              borderRadius: BorderRadius.circular(8),
              child: InkWell(
                borderRadius: BorderRadius.circular(8),
                onTap: () {
                  showDialog(
                    context: context,
                    builder: (ctx) => DiagnosticDialog(viewModel: viewModel),
                  );
                },
                child: Container(
                  padding: const EdgeInsets.all(14),
                  decoration: BoxDecoration(
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
                          const Icon(Icons.info_outline, size: 14, color: Colors.white38),
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
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildNavItem({
    required String title,
    required IconData icon,
    required bool isSelected,
    required VoidCallback onTap,
    int? count,
  }) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 2),
      child: Material(
        color: isSelected
            ? AppTheme.primary.withValues(alpha: 0.15)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
            child: Row(
              children: [
                Icon(
                  icon,
                  size: 20,
                  color: isSelected ? AppTheme.primary : Colors.white70,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    title,
                    style: TextStyle(
                      fontSize: 14,
                      fontWeight:
                          isSelected ? FontWeight.w600 : FontWeight.normal,
                      color: isSelected ? Colors.white : Colors.white70,
                    ),
                  ),
                ),
                if (count != null)
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
  }

  Widget _buildFilterItem({
    required DownloadFilter filter,
    required IconData icon,
    required int count,
    Color? color,
  }) {
    final isSelected =
        viewModel.currentSection == AppNavSection.downloads &&
        viewModel.currentFilter == filter;
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
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
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

  Widget _buildCategoryItem({
    required BuildContext context,
    required String title,
    required Color color,
    required int count,
    required bool isSelected,
    required VoidCallback onTap,
    IconData? icon,
    CategoryItem? category,
  }) {
    final item = Padding(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 2),
      child: Material(
        color: isSelected
            ? AppTheme.primary.withValues(alpha: 0.15)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
            child: Row(
              children: [
                if (icon != null)
                  Icon(icon, size: 16, color: color)
                else
                  Container(
                    width: 12,
                    height: 12,
                    decoration: BoxDecoration(
                      color: color,
                      shape: BoxShape.circle,
                    ),
                  ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    title,
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight:
                          isSelected ? FontWeight.w600 : FontWeight.normal,
                      color: isSelected ? Colors.white : Colors.white70,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
                  decoration: BoxDecoration(
                    color: isSelected
                        ? AppTheme.primary.withValues(alpha: 0.3)
                        : Colors.white10,
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Text(
                    '$count',
                    style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w500,
                      color: isSelected ? Colors.white : Colors.white60,
                    ),
                  ),
                ),
                if (category != null) ...[
                  const SizedBox(width: 4),
                  PopupMenuButton<String>(
                    icon: const Icon(Icons.more_vert_rounded, size: 14, color: Colors.white38),
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(minWidth: 20, minHeight: 20),
                    tooltip: 'Category Options',
                    itemBuilder: (ctx) => [
                      const PopupMenuItem(
                        value: 'edit',
                        child: Row(
                          children: [
                            Icon(Icons.edit_outlined, size: 16),
                            SizedBox(width: 8),
                            Text('Edit Category'),
                          ],
                        ),
                      ),
                      const PopupMenuItem(
                        value: 'delete',
                        child: Row(
                          children: [
                            Icon(Icons.delete_outline, size: 16, color: AppTheme.statusFailed),
                            SizedBox(width: 8),
                            Text('Delete Category', style: TextStyle(color: AppTheme.statusFailed)),
                          ],
                        ),
                      ),
                    ],
                    onSelected: (action) {
                      if (action == 'edit') {
                        showDialog(
                          context: context,
                          builder: (ctx) => CategoryEditDialog(
                            viewModel: viewModel,
                            category: category,
                          ),
                        );
                      } else if (action == 'delete') {
                        _confirmDeleteCategory(context, category);
                      }
                    },
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );

    return item;
  }

  void _confirmDeleteCategory(BuildContext context, CategoryItem category) {
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
            Container(
              width: 12,
              height: 12,
              decoration: BoxDecoration(
                color: category.parsedColor,
                shape: BoxShape.circle,
              ),
            ),
            const SizedBox(width: 8),
            Text('Delete "${category.name}"?'),
          ],
        ),
        content: const Text(
          'Deleting this category will move all associated downloads to "Sans catégorie" (Uncategorized).\n\n'
          'No downloads or files will be deleted.',
          style: TextStyle(fontSize: 13, color: Colors.white70),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Cancel', style: TextStyle(color: Colors.white60)),
          ),
          FilledButton(
            style: FilledButton.styleFrom(backgroundColor: AppTheme.statusFailed),
            onPressed: () {
              Navigator.of(ctx).pop();
              viewModel.deleteCategory(category.id);
            },
            child: const Text('Delete Category'),
          ),
        ],
      ),
    );
  }
}
