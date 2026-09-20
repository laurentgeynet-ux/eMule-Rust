import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../app/app_controller.dart';
import 'category_edit_dialog.dart';
import 'diagnostic_dialog.dart';
import 'import_part_met_dialog.dart';
import 'settings_dialog.dart';
import 'sidebar/category_delete_dialog.dart';
import 'sidebar/sidebar_action_button.dart';
import 'sidebar/sidebar_brand_header.dart';
import 'sidebar/sidebar_category_item.dart';
import 'sidebar/sidebar_filter_item.dart';
import 'sidebar/sidebar_nav_item.dart';
import 'sidebar/sidebar_section_label.dart';
import 'sidebar/sidebar_status_card.dart';
import 'sidebar/sidebar_status_colors.dart';

/// Left navigation rail: brand header, download filters, categories,
/// section navigation, action shortcuts and the daemon status card.
class DownloadsSidebar extends StatelessWidget {
  final AppController viewModel;
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
          SidebarBrandHeader(isCompact: isCompact),
          Expanded(
            child: SingleChildScrollView(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const SizedBox(height: 8),
                  if (!isCompact)
                    const SidebarSectionLabel(
                      label: AppStrings.sectionDownloads,
                    ),
                  SidebarFilterItem(
                    filter: DownloadFilter.all,
                    icon: Icons.all_inbox_rounded,
                    count: viewModel.countAll,
                    isSelected: _isFilterSelected(DownloadFilter.all),
                    isCompact: isCompact,
                    onTap: () => viewModel.setFilter(DownloadFilter.all),
                  ),
                  SidebarFilterItem(
                    filter: DownloadFilter.active,
                    icon: Icons.downloading_rounded,
                    count: viewModel.countActive,
                    color: AppTheme.statusDownloading,
                    isSelected: _isFilterSelected(DownloadFilter.active),
                    isCompact: isCompact,
                    onTap: () => viewModel.setFilter(DownloadFilter.active),
                  ),
                  SidebarFilterItem(
                    filter: DownloadFilter.paused,
                    icon: Icons.pause_circle_outline_rounded,
                    count: viewModel.countPaused,
                    color: AppTheme.statusPaused,
                    isSelected: _isFilterSelected(DownloadFilter.paused),
                    isCompact: isCompact,
                    onTap: () => viewModel.setFilter(DownloadFilter.paused),
                  ),
                  SidebarFilterItem(
                    filter: DownloadFilter.completed,
                    icon: Icons.check_circle_outline_rounded,
                    count: viewModel.countCompleted,
                    color: AppTheme.statusCompleted,
                    isSelected: _isFilterSelected(DownloadFilter.completed),
                    isCompact: isCompact,
                    onTap: () => viewModel.setFilter(DownloadFilter.completed),
                  ),
                  SidebarFilterItem(
                    filter: DownloadFilter.failed,
                    icon: Icons.error_outline_rounded,
                    count: viewModel.countFailed,
                    color: AppTheme.statusFailed,
                    isSelected: _isFilterSelected(DownloadFilter.failed),
                    isCompact: isCompact,
                    onTap: () => viewModel.setFilter(DownloadFilter.failed),
                  ),
                  const SizedBox(height: 6),
                  const Divider(color: AppTheme.darkBorder, height: 1),
                  const SizedBox(height: 6),
                  if (!isCompact) ...[
                    _CategoriesHeader(onAdd: () => _showCategoryDialog(context)),
                    SidebarCategoryItem(
                      title: 'All Downloads',
                      color: AppColors.textSecondary,
                      icon: Icons.folder_outlined,
                      count: viewModel.countAll,
                      isSelected:
                          viewModel.currentSection ==
                                  AppNavSection.downloads &&
                              viewModel.selectedCategoryId == null,
                      onTap: () => _selectCategory(null),
                      onEdit: (_) {},
                      onDelete: (_) {},
                    ),
                    SidebarCategoryItem(
                      title: 'Sans catégorie',
                      color: AppColors.textFaint,
                      icon: Icons.label_off_outlined,
                      count: viewModel.uncategorizedCount,
                      isSelected:
                          viewModel.currentSection ==
                                  AppNavSection.downloads &&
                              viewModel.selectedCategoryId == 'uncategorized',
                      onTap: () => _selectCategory('uncategorized'),
                      onEdit: (_) {},
                      onDelete: (_) {},
                    ),
                    ...viewModel.categories.map(
                      (cat) => SidebarCategoryItem(
                        title: cat.name,
                        color: cat.parsedColor,
                        count: viewModel.categoryCounts[cat.id] ?? 0,
                        isSelected:
                            viewModel.currentSection ==
                                    AppNavSection.downloads &&
                                viewModel.selectedCategoryId == cat.id,
                        category: cat,
                        onTap: () => _selectCategory(cat.id),
                        onEdit: (category) =>
                            _showCategoryDialog(context, category),
                        onDelete: (category) => showCategoryDeleteDialog(
                          context,
                          category,
                          onDelete: viewModel.deleteCategory,
                        ),
                      ),
                    ),
                    const SizedBox(height: 6),
                    const Divider(color: AppTheme.darkBorder, height: 1),
                    const SizedBox(height: 6),
                  ],
                  if (!isCompact)
                    const SidebarSectionLabel(label: 'AUDIT & ACTIVITY'),
                  SidebarNavItem(
                    title: 'History & Statistics',
                    icon: Icons.history_rounded,
                    isSelected:
                        viewModel.currentSection == AppNavSection.history,
                    onTap: () => viewModel.setSection(AppNavSection.history),
                    count: viewModel.historyEntries.isNotEmpty
                        ? viewModel.historyEntries.length
                        : null,
                    isCompact: isCompact,
                  ),
                  SidebarNavItem(
                    title: 'Serveurs eD2K',
                    icon: Icons.dns_rounded,
                    isSelected:
                        viewModel.currentSection == AppNavSection.servers,
                    onTap: () => viewModel.setSection(AppNavSection.servers),
                    count: viewModel.servers.isNotEmpty
                        ? viewModel.servers.length
                        : null,
                    isCompact: isCompact,
                  ),
                  SidebarNavItem(
                    title: 'Recherche eD2K',
                    icon: Icons.search_rounded,
                    isSelected:
                        viewModel.currentSection == AppNavSection.search,
                    onTap: () => viewModel.setSection(AppNavSection.search),
                    count: viewModel.searchResults.isNotEmpty
                        ? viewModel.searchResults.length
                        : null,
                    isCompact: isCompact,
                  ),
                  SidebarNavItem(
                    title: 'Kad DHT',
                    icon: Icons.hub_rounded,
                    isSelected: viewModel.currentSection == AppNavSection.kad,
                    onTap: () => viewModel.setSection(AppNavSection.kad),
                    count: viewModel.kadStatus.totalContacts > 0
                        ? viewModel.kadStatus.totalContacts
                        : null,
                    isCompact: isCompact,
                  ),
                  const SizedBox(height: 4),
                  SidebarActionButton(
                    icon: Icons.settings_outlined,
                    label: 'Settings',
                    iconColor: AppColors.textSecondary,
                    isCompact: isCompact,
                    onTap: () => showDialog(
                      context: context,
                      builder: (ctx) => SettingsDialog(viewModel: viewModel),
                    ),
                  ),
                  const SizedBox(height: 4),
                  SidebarActionButton(
                    key: const Key('btn_import_part_met'),
                    icon: Icons.file_download_outlined,
                    label: 'Import .part.met',
                    iconColor: AppColors.accentTeal,
                    isCompact: isCompact,
                    onTap: () => showDialog(
                      context: context,
                      builder: (ctx) =>
                          ImportPartMetDialog(viewModel: viewModel),
                    ),
                  ),
                  const SizedBox(height: 8),
                ],
              ),
            ),
          ),
          SidebarStatusCard(
            isCompact: isCompact,
            statusColor: _statusColor,
            statusLabel: _statusLabel,
            tooltipLabel: _statusLabel,
            versionLine: viewModel.daemonInfo != null
                ? 'Daemon v${viewModel.daemonInfo!.version} '
                    '(proto v${viewModel.daemonInfo!.protocolVersion})'
                : null,
            onTap: () => showDialog(
              context: context,
              builder: (ctx) => DiagnosticDialog(viewModel: viewModel),
            ),
          ),
        ],
      ),
    );
  }

  bool _isFilterSelected(DownloadFilter filter) =>
      viewModel.currentSection == AppNavSection.downloads &&
      viewModel.currentFilter == filter;

  void _selectCategory(String? id) {
    viewModel.selectCategory(id);
    viewModel.setSection(AppNavSection.downloads);
  }

  void _showCategoryDialog(BuildContext context, [CategoryItem? category]) {
    showDialog(
      context: context,
      builder: (ctx) =>
          CategoryEditDialog(viewModel: viewModel, category: category),
    );
  }

  String get _statusLabel => viewModel.supervisor != null
      ? viewModel.supervisorState.label
      : viewModel.connectionState.label;

  Color get _statusColor => viewModel.supervisor != null
      ? supervisorStateColor(viewModel.supervisorState)
      : connectionStateColor(viewModel.connectionState);
}

class _CategoriesHeader extends StatelessWidget {
  final VoidCallback onAdd;

  const _CategoriesHeader({required this.onAdd});

  @override
  Widget build(BuildContext context) {
    return Padding(
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
            icon: const Icon(
              Icons.add_rounded,
              size: 16,
              color: AppColors.textSoft,
            ),
            tooltip: 'Add Category',
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
            onPressed: onAdd,
          ),
        ],
      ),
    );
  }
}
