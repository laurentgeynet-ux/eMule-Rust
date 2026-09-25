// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import '../../../../app/shell_controller.dart';
import '../../../../l10n/enum_labels.dart';
import '../../../../l10n/l10n.dart';
import '../../../../core/system_actions.dart';
import 'category_edit_dialog.dart';
import 'diagnostic_dialog.dart';
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
class DownloadsSidebar extends ConsumerWidget {
    final bool isCompact;

  const DownloadsSidebar({
    super.key,
    this.isCompact = false,
  });

  @override
  Widget build(BuildContext context, WidgetRef ref) {
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
                    SidebarSectionLabel(
                      label: context.l10n.sectionDownloads,
                    ),
                  SidebarFilterItem(
                    filter: DownloadFilter.all,
                    icon: Icons.all_inbox_rounded,
                    count: ref.watch(downloadsStoreControllerProvider).countAll,
                    isSelected: _isFilterSelected(ref, DownloadFilter.all),
                    isCompact: isCompact,
                    onTap: () => ref.read(shellControllerProvider).setFilter(DownloadFilter.all),
                  ),
                  SidebarFilterItem(
                    filter: DownloadFilter.active,
                    icon: Icons.downloading_rounded,
                    count: ref.watch(downloadsStoreControllerProvider).countActive,
                    color: AppTheme.statusDownloading,
                    isSelected: _isFilterSelected(ref, DownloadFilter.active),
                    isCompact: isCompact,
                    onTap: () => ref.read(shellControllerProvider).setFilter(DownloadFilter.active),
                  ),
                  SidebarFilterItem(
                    filter: DownloadFilter.paused,
                    icon: Icons.pause_circle_outline_rounded,
                    count: ref.watch(downloadsStoreControllerProvider).countPaused,
                    color: AppTheme.statusPaused,
                    isSelected: _isFilterSelected(ref, DownloadFilter.paused),
                    isCompact: isCompact,
                    onTap: () => ref.read(shellControllerProvider).setFilter(DownloadFilter.paused),
                  ),
                  SidebarFilterItem(
                    filter: DownloadFilter.completed,
                    icon: Icons.check_circle_outline_rounded,
                    count: ref.watch(downloadsStoreControllerProvider).countCompleted,
                    color: AppTheme.statusCompleted,
                    isSelected: _isFilterSelected(ref, DownloadFilter.completed),
                    isCompact: isCompact,
                    onTap: () => ref.read(shellControllerProvider).setFilter(DownloadFilter.completed),
                  ),
                  SidebarFilterItem(
                    filter: DownloadFilter.failed,
                    icon: Icons.error_outline_rounded,
                    count: ref.watch(downloadsStoreControllerProvider).countFailed,
                    color: AppTheme.statusFailed,
                    isSelected: _isFilterSelected(ref, DownloadFilter.failed),
                    isCompact: isCompact,
                    onTap: () => ref.read(shellControllerProvider).setFilter(DownloadFilter.failed),
                  ),
                  const SizedBox(height: 6),
                  const Divider(color: AppTheme.darkBorder, height: 1),
                  const SizedBox(height: 6),
                  if (!isCompact) ...[
                    _CategoriesHeader(onAdd: () => _showCategoryDialog(context)),
                    SidebarCategoryItem(
                      title: context.l10n.filterAll,
                      color: AppColors.textSecondary,
                      icon: Icons.folder_outlined,
                      count: ref.watch(downloadsStoreControllerProvider).countAll,
                      isSelected:
                          ref.watch(shellControllerProvider).section ==
                                  AppNavSection.downloads &&
                              ref.watch(categoriesControllerProvider).selectedId == null,
                      onTap: () => _selectCategory(ref, null),
                      onEdit: (_) {},
                      onDelete: (_) {},
                    ),
                    SidebarCategoryItem(
                      title: context.l10n.uncategorized,
                      color: AppColors.textFaint,
                      icon: Icons.label_off_outlined,
                      count: ref.watch(uncategorizedCountProvider),
                      isSelected:
                          ref.watch(shellControllerProvider).section ==
                                  AppNavSection.downloads &&
                              ref.watch(categoriesControllerProvider).selectedId == 'uncategorized',
                      onTap: () => _selectCategory(ref, 'uncategorized'),
                      onEdit: (_) {},
                      onDelete: (_) {},
                    ),
                    ...ref.watch(categoriesControllerProvider).categories.map(
                      (cat) => SidebarCategoryItem(
                        title: cat.name,
                        color: cat.parsedColor,
                        count: ref.watch(categoryCountsProvider)[cat.id] ?? 0,
                        isSelected:
                            ref.watch(shellControllerProvider).section ==
                                    AppNavSection.downloads &&
                                ref.watch(categoriesControllerProvider).selectedId == cat.id,
                        category: cat,
                        onTap: () => _selectCategory(ref, cat.id),
                        onEdit: (category) =>
                            _showCategoryDialog(context, category),
                        onDelete: (category) => showCategoryDeleteDialog(
                          context,
                          category,
                          onDelete: ref.read(categoriesControllerProvider).delete,
                        ),
                      ),
                    ),
                    const SizedBox(height: 6),
                    const Divider(color: AppTheme.darkBorder, height: 1),
                    const SizedBox(height: 6),
                  ],
                  SidebarNavItem(
                    title: context.l10n.navUploads,
                    icon: Icons.upload_rounded,
                    isSelected:
                        ref.watch(shellControllerProvider).section == AppNavSection.uploads,
                    onTap: () => ref.read(shellControllerProvider).setSection(AppNavSection.uploads),
                    count: ref.watch(uploadsControllerProvider).active.isNotEmpty
                        ? ref.watch(uploadsControllerProvider).active.length
                        : null,
                    isCompact: isCompact,
                  ),
                  SidebarNavItem(
                    title: context.l10n.navShares,
                    icon: Icons.folder_shared_outlined,
                    isSelected:
                        ref.watch(shellControllerProvider).section == AppNavSection.shares,
                    onTap: () => ref.read(shellControllerProvider).setSection(AppNavSection.shares),
                    count: ref.watch(sharesControllerProvider).files.isNotEmpty
                        ? ref.watch(sharesControllerProvider).files.length
                        : null,
                    isCompact: isCompact,
                  ),
                  if (!isCompact)
                    SidebarSectionLabel(
                      label: context.l10n.sectionAuditActivity,
                    ),
                  SidebarNavItem(
                    title: context.l10n.navHistoryStats,
                    icon: Icons.history_rounded,
                    isSelected:
                        ref.watch(shellControllerProvider).section == AppNavSection.history,
                    onTap: () => ref.read(shellControllerProvider).setSection(AppNavSection.history),
                    count: ref.watch(historyControllerProvider).entries.isNotEmpty
                        ? ref.watch(historyControllerProvider).entries.length
                        : null,
                    isCompact: isCompact,
                  ),
                  SidebarNavItem(
                    title: context.l10n.navServers,
                    icon: Icons.dns_rounded,
                    isSelected:
                        ref.watch(shellControllerProvider).section == AppNavSection.servers,
                    onTap: () => ref.read(shellControllerProvider).setSection(AppNavSection.servers),
                    count: ref.watch(serversControllerProvider).servers.isNotEmpty
                        ? ref.watch(serversControllerProvider).servers.length
                        : null,
                    isCompact: isCompact,
                  ),
                  SidebarNavItem(
                    title: context.l10n.navSearch,
                    icon: Icons.search_rounded,
                    isSelected:
                        ref.watch(shellControllerProvider).section == AppNavSection.search,
                    onTap: () => ref.read(shellControllerProvider).setSection(AppNavSection.search),
                    count: ref.watch(searchControllerProvider).results.isNotEmpty
                        ? ref.watch(searchControllerProvider).results.length
                        : null,
                    isCompact: isCompact,
                  ),
                  SidebarNavItem(
                    title: context.l10n.navKad,
                    icon: Icons.hub_rounded,
                    isSelected: ref.watch(shellControllerProvider).section == AppNavSection.kad,
                    onTap: () => ref.read(shellControllerProvider).setSection(AppNavSection.kad),
                    count: ref.watch(kadControllerProvider).status.totalContacts > 0
                        ? ref.watch(kadControllerProvider).status.totalContacts
                        : null,
                    isCompact: isCompact,
                  ),
                  const SizedBox(height: 4),
                  SidebarNavItem(
                    title: context.l10n.navAssistant,
                    icon: Icons.smart_toy_outlined,
                    isSelected:
                        ref.watch(shellControllerProvider).section ==
                            AppNavSection.assistant,
                    onTap: () => ref
                        .read(shellControllerProvider)
                        .setSection(AppNavSection.assistant),
                    isCompact: isCompact,
                  ),
                  SidebarNavItem(
                    title: context.l10n.actionSettings,
                    icon: Icons.settings_outlined,
                    isSelected:
                        ref.watch(shellControllerProvider).section ==
                            AppNavSection.settings,
                    onTap: () => ref
                        .read(shellControllerProvider)
                        .setSection(AppNavSection.settings),
                    isCompact: isCompact,
                  ),
                  const SizedBox(height: 8),
                ],
              ),
            ),
          ),
          SidebarStatusCard(
            isCompact: isCompact,
            statusColor: _statusColor(ref),
            statusLabel: _statusLabel(context, ref),
            tooltipLabel: _statusLabel(context, ref),
            networkBadges: _networkBadges(context, ref),
            serverLine: _serverLine(ref),
            versionLine: ref.watch(daemonInfoProvider) != null
                ? context.l10n.daemonVersionLine(
                    ref.watch(daemonInfoProvider)!.version,
                    ref.watch(daemonInfoProvider)!.protocolVersion.toString(),
                  )
                : null,
            onTap: () => showDialog(
              context: context,
              builder: (ctx) => const DiagnosticDialog(),
            ),
          ),
          if (systemActions.isSupported &&
              ref.watch(daemonSupervisorProvider) != null)
            SidebarActionButton(
              icon: Icons.open_in_new,
              label: context.l10n.webInterface,
              iconColor: AppColors.accentTeal,
              isCompact: isCompact,
              onTap: () => systemActions.openUrl(_webUiUrl(ref)),
            ),
          if (ref.watch(onChangeDaemonProvider) != null)
            SidebarActionButton(
              icon: Icons.link_off,
              label: context.l10n.changeDaemon,
              iconColor: AppColors.accentCyan,
              isCompact: isCompact,
              onTap: () => ref.read(onChangeDaemonProvider)!(),
            ),
        ],
      ),
    );
  }

  bool _isFilterSelected(WidgetRef ref, DownloadFilter filter) =>
      ref.watch(shellControllerProvider).section == AppNavSection.downloads &&
      ref.watch(shellControllerProvider).filter == filter;

  /// HTTP URL of the daemon-served web UI, derived from the session's
  /// WebSocket endpoint (`ws://host:port/rpc` → `http://host:port/`).
  String _webUiUrl(WidgetRef ref) {
    final endpoint =
        ref.watch(daemonSupervisorProvider)?.activeSession?.endpoint;
    final uri = Uri.tryParse(endpoint ?? '');
    if (uri == null || uri.host.isEmpty) return 'http://127.0.0.1:4660/';
    final scheme = uri.scheme == 'wss' ? 'https' : 'http';
    return '$scheme://${uri.host}:${uri.port}/';
  }

  void _selectCategory(WidgetRef ref, String? id) {
    ref.read(categoriesControllerProvider).select(id);
    ref.read(shellControllerProvider).setSection(AppNavSection.downloads);
  }

  void _showCategoryDialog(BuildContext context, [CategoryItem? category]) {
    showDialog(
      context: context,
      builder: (ctx) =>
          CategoryEditDialog(category: category),
    );
  }

  String _statusLabel(BuildContext context, WidgetRef ref) =>
      ref.watch(daemonSupervisorProvider) != null
          ? ref.watch(daemonSupervisorStateProvider).labelL10n(context.l10n)
          : ref.watch(daemonConnectionStateProvider).labelL10n(context.l10n);

  Color _statusColor(WidgetRef ref) => ref.watch(daemonSupervisorProvider) != null
      ? supervisorStateColor(ref.watch(daemonSupervisorStateProvider))
      : connectionStateColor(ref.watch(daemonConnectionStateProvider));

  /// Pastilles réseau eD2K/Kad sous la carte daemon : HighID vert,
  /// LowID orange, déconnecté rouge ; même code couleur pour Kad
  /// (connecté vert, amorçage orange, socket arrêté rouge).
  List<(String, Color)> _networkBadges(BuildContext context, WidgetRef ref) {
    final l10n = context.l10n;
    final session = ref.watch(serversControllerProvider).session;
    final kad = ref.watch(kadControllerProvider).status;
    final (String, Color) ed2k = switch (session.status) {
      ServerConnectionStatus.connected => session.isHighId
          ? (l10n.badgeEd2kHighId, AppColors.positive)
          : (l10n.badgeEd2kLowId, AppColors.accentOrange),
      ServerConnectionStatus.connecting =>
        (l10n.badgeEd2kConnecting, AppColors.accentOrange),
      ServerConnectionStatus.disconnecting =>
        (l10n.badgeEd2kDisconnecting, AppColors.accentOrange),
      ServerConnectionStatus.disconnected =>
        (l10n.badgeEd2kDisconnected, AppColors.danger),
    };
    final (String, Color) kadBadge = !kad.isUdpActive
        ? (l10n.badgeKadStopped, AppColors.danger)
        : kad.totalContacts > 0
            ? (l10n.badgeKadConnected, AppColors.positive)
            : (l10n.badgeKadBootstrapping, AppColors.accentOrange);
    return [ed2k, kadBadge];
  }

  /// Nom du serveur eD2K courant (adresse en repli) — null quand la
  /// session n'est pas connectée.
  String? _serverLine(WidgetRef ref) {
    final session = ref.watch(serversControllerProvider).session;
    if (session.status != ServerConnectionStatus.connected) return null;
    if (session.serverName != null && session.serverName!.isNotEmpty) {
      return session.serverName!;
    }
    return session.serverAddress;
  }
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
          Text(
            context.l10n.sectionCategories,
            style: const TextStyle(
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
            tooltip: context.l10n.tooltipAddCategory,
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
            onPressed: onAdd,
          ),
        ],
      ),
    );
  }
}
