import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import '../../../../services/daemon_supervisor.dart';
import 'package:ui_kit/ui_kit.dart';
import '../view_models/downloads_view_model.dart';
import 'category_edit_dialog.dart';
import 'diagnostic_dialog.dart';
import 'import_part_met_dialog.dart';
import 'settings_dialog.dart';

class DownloadsSidebar extends StatelessWidget {
  final DownloadsViewModel viewModel;
  final bool isCompact;

  const DownloadsSidebar({
    super.key,
    required this.viewModel,
    this.isCompact = false,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      width: isCompact ? 68 : 260,
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
            padding: EdgeInsets.symmetric(
              horizontal: isCompact ? 10 : 20,
              vertical: 14,
            ),
            decoration: const BoxDecoration(
              border: Border(
                bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
              ),
            ),
            child: Row(
              mainAxisAlignment: isCompact ? MainAxisAlignment.center : MainAxisAlignment.start,
              children: [
                Tooltip(
                  message: '${AppStrings.appName} - ${AppStrings.appTagline}',
                  child: Container(
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
                ),
                if (!isCompact) ...[
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
                            color: AppColors.textMuted,
                          ),
                          overflow: TextOverflow.ellipsis,
                        ),
                      ],
                    ),
                  ),
                ],
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
                  if (!isCompact)
                    const Padding(
                      padding: EdgeInsets.symmetric(horizontal: 20, vertical: 4),
                      child: Text(
                        AppStrings.sectionDownloads,
                        style: TextStyle(
                          fontSize: 11,
                          fontWeight: FontWeight.w600,
                          color: AppColors.textFaint,
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
                  if (!isCompact) ...[
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
                              color: AppColors.textFaint,
                              letterSpacing: 1.0,
                            ),
                          ),
                          IconButton(
                            icon: const Icon(Icons.add_rounded, size: 16, color: AppColors.textSoft),
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
                      color: AppColors.textSecondary,
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
                      color: AppColors.textFaint,
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
                  ],

                  // Section Title
                  if (!isCompact)
                    const Padding(
                      padding: EdgeInsets.symmetric(horizontal: 20, vertical: 4),
                      child: Text(
                        'AUDIT & ACTIVITY',
                        style: TextStyle(
                          fontSize: 11,
                          fontWeight: FontWeight.w600,
                          color: AppColors.textFaint,
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

                  // Serveurs eD2K Nav Item
                  _buildNavItem(
                    title: 'Serveurs eD2K',
                    icon: Icons.dns_rounded,
                    isSelected: viewModel.currentSection == AppNavSection.servers,
                    onTap: () => viewModel.setSection(AppNavSection.servers),
                    count: viewModel.servers.isNotEmpty ? viewModel.servers.length : null,
                  ),

                  // Recherche eD2K Nav Item
                  _buildNavItem(
                    title: 'Recherche eD2K',
                    icon: Icons.search_rounded,
                    isSelected: viewModel.currentSection == AppNavSection.search,
                    onTap: () => viewModel.setSection(AppNavSection.search),
                    count: viewModel.searchResults.isNotEmpty ? viewModel.searchResults.length : null,
                  ),

                  // Kad DHT Nav Item (Phase 12.0a)
                  _buildNavItem(
                    title: 'Kad DHT',
                    icon: Icons.hub_rounded,
                    isSelected: viewModel.currentSection == AppNavSection.kad,
                    onTap: () => viewModel.setSection(AppNavSection.kad),
                    count: viewModel.kadStatus.totalContacts > 0 ? viewModel.kadStatus.totalContacts : null,
                  ),

                  const SizedBox(height: 4),

                  // Settings Button
                  Padding(
                    padding: EdgeInsets.symmetric(horizontal: isCompact ? 6 : 12),
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
                        child: Tooltip(
                          message: isCompact ? 'Settings' : '',
                          child: Padding(
                            padding: EdgeInsets.symmetric(horizontal: isCompact ? 8 : 12, vertical: 6),
                            child: Row(
                              mainAxisAlignment: isCompact ? MainAxisAlignment.center : MainAxisAlignment.start,
                              children: [
                                const Icon(Icons.settings_outlined, size: 20, color: AppColors.textSecondary),
                                if (!isCompact) ...[
                                  const SizedBox(width: 12),
                                  const Expanded(
                                    child: Text(
                                      'Settings',
                                      style: TextStyle(
                                        fontSize: 14,
                                        fontWeight: FontWeight.normal,
                                        color: AppColors.textSecondary,
                                      ),
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                ],
                              ],
                            ),
                          ),
                        ),
                      ),
                    ),
                  ),
                  const SizedBox(height: 4),

                  // Import .part.met Button
                  Padding(
                    padding: EdgeInsets.symmetric(horizontal: isCompact ? 6 : 12),
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
                        child: Tooltip(
                          message: isCompact ? 'Import .part.met' : '',
                          child: Padding(
                            padding: EdgeInsets.symmetric(horizontal: isCompact ? 8 : 12, vertical: 6),
                            child: Row(
                              mainAxisAlignment: isCompact ? MainAxisAlignment.center : MainAxisAlignment.start,
                              children: [
                                const Icon(Icons.file_download_outlined, size: 20, color: AppColors.accentTeal),
                                if (!isCompact) ...[
                                  const SizedBox(width: 12),
                                  const Expanded(
                                    child: Text(
                                      'Import .part.met',
                                      style: TextStyle(
                                        fontSize: 14,
                                        fontWeight: FontWeight.normal,
                                        color: AppColors.textSecondary,
                                      ),
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                ],
                              ],
                            ),
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
            padding: EdgeInsets.all(isCompact ? 8 : 16),
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
                child: Tooltip(
                  message: isCompact
                      ? 'Daemon Status: ${viewModel.supervisor != null ? viewModel.supervisorState.label : viewModel.connectionState.label}\nClick to view Diagnostics'
                      : '',
                  child: Container(
                    padding: EdgeInsets.all(isCompact ? 10 : 14),
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(color: AppTheme.darkBorder, width: 0.5),
                    ),
                    child: isCompact
                        ? Center(
                            child: Container(
                              width: 12,
                              height: 12,
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
                          )
                        : Column(
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
                                  const Icon(Icons.info_outline, size: 14, color: AppColors.textFaint),
                                ],
                              ),
                              if (viewModel.daemonInfo != null) ...[
                                const SizedBox(height: 6),
                                Text(
                                  'Daemon v${viewModel.daemonInfo!.version} (proto v${viewModel.daemonInfo!.protocolVersion})',
                                  style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
                                ),
                              ],
                            ],
                          ),
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
    final content = Padding(
      padding: EdgeInsets.symmetric(horizontal: isCompact ? 6 : 12, vertical: 2),
      child: Material(
        color: isSelected
            ? AppTheme.primary.withValues(alpha: 0.15)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Padding(
            padding: EdgeInsets.symmetric(horizontal: isCompact ? 8 : 12, vertical: 6),
            child: Row(
              mainAxisAlignment: isCompact ? MainAxisAlignment.center : MainAxisAlignment.start,
              children: [
                Stack(
                  clipBehavior: Clip.none,
                  children: [
                    Icon(
                      icon,
                      size: 20,
                      color: isSelected ? AppTheme.primary : AppColors.textSecondary,
                    ),
                    if (isCompact && count != null && count > 0)
                      Positioned(
                        top: -4,
                        right: -6,
                        child: Container(
                          padding: const EdgeInsets.all(2),
                          decoration: BoxDecoration(
                            color: AppTheme.primary,
                            shape: BoxShape.circle,
                          ),
                          constraints: const BoxConstraints(minWidth: 10, minHeight: 10),
                        ),
                      ),
                  ],
                ),
                if (!isCompact) ...[
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(
                      title,
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight:
                            isSelected ? FontWeight.w600 : FontWeight.normal,
                        color: isSelected ? AppColors.textPrimary : AppColors.textSecondary,
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
                            : AppColors.fillSubtle,
                        borderRadius: BorderRadius.circular(10),
                      ),
                      child: Text(
                        '$count',
                        style: TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.w500,
                          color: isSelected ? AppColors.textPrimary : AppColors.textSoft,
                        ),
                      ),
                    ),
                ],
              ],
            ),
          ),
        ),
      ),
    );

    if (isCompact) {
      return Tooltip(
        message: count != null ? '$title ($count)' : title,
        preferBelow: false,
        child: content,
      );
    }
    return content;
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
      padding: EdgeInsets.symmetric(horizontal: isCompact ? 6 : 12, vertical: 2),
      child: Material(
        color: isSelected
            ? AppTheme.primary.withValues(alpha: 0.15)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: () => viewModel.setFilter(filter),
          child: Padding(
            padding: EdgeInsets.symmetric(horizontal: isCompact ? 8 : 12, vertical: 6),
            child: Row(
              mainAxisAlignment: isCompact ? MainAxisAlignment.center : MainAxisAlignment.start,
              children: [
                Stack(
                  clipBehavior: Clip.none,
                  children: [
                    Icon(
                      icon,
                      size: 20,
                      color: isSelected
                          ? AppTheme.primary
                          : (color ?? AppColors.textSecondary),
                    ),
                    if (isCompact && count > 0)
                      Positioned(
                        top: -4,
                        right: -6,
                        child: Container(
                          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                          decoration: BoxDecoration(
                            color: color ?? AppTheme.primary,
                            borderRadius: BorderRadius.circular(6),
                          ),
                          child: Text(
                            '$count',
                            style: const TextStyle(fontSize: 8, fontWeight: FontWeight.bold, color: AppColors.textPrimary),
                          ),
                        ),
                      ),
                  ],
                ),
                if (!isCompact) ...[
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(
                      filter.label,
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight:
                            isSelected ? FontWeight.w600 : FontWeight.normal,
                        color: isSelected ? AppColors.textPrimary : AppColors.textSecondary,
                      ),
                    ),
                  ),
                  Container(
                    padding:
                        const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                    decoration: BoxDecoration(
                      color: isSelected
                          ? AppTheme.primary.withValues(alpha: 0.3)
                          : AppColors.fillSubtle,
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: Text(
                      '$count',
                      style: TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.w500,
                        color: isSelected ? AppColors.textPrimary : AppColors.textSoft,
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );

    final wrapped = isCompact
        ? Tooltip(
            message: '${filter.label} ($count)',
            preferBelow: false,
            child: item,
          )
        : item;

    if (isDimmed) {
      return Opacity(opacity: 0.55, child: wrapped);
    }
    return wrapped;
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
                      color: isSelected ? AppColors.textPrimary : AppColors.textSecondary,
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
                        : AppColors.fillSubtle,
                    borderRadius: BorderRadius.circular(10),
                  ),
                  child: Text(
                    '$count',
                    style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.w500,
                      color: isSelected ? AppColors.textPrimary : AppColors.textSoft,
                    ),
                  ),
                ),
                if (category != null) ...[
                  const SizedBox(width: 4),
                  PopupMenuButton<String>(
                    icon: const Icon(Icons.more_vert_rounded, size: 14, color: AppColors.textFaint),
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
          style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Cancel', style: TextStyle(color: AppColors.textSoft)),
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
