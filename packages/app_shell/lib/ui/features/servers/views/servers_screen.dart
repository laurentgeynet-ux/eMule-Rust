// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/providers.dart';
import '../../../../app/shell_controller.dart';
import '../widgets/server_met_import_dialog.dart';
import '../widgets/server_session_bar.dart';
import '../widgets/servers_clear_dialog.dart';
import '../widgets/servers_empty_state.dart';
import '../widgets/servers_error_banner.dart';
import '../widgets/servers_filter_bar.dart';
import '../widgets/servers_safety_banner.dart';
import '../widgets/servers_table.dart';
import '../widgets/servers_toolbar.dart';
import '../widgets/servers_update_url_dialog.dart';

/// eD2K servers section — composition only. Server state comes from
/// [serversControllerProvider]; `daemonInfo`/`errorMessage` are read from
/// the root view model until connection state gets its own provider.
class ServersView extends ConsumerStatefulWidget {
  const ServersView({super.key});

  @override
  ConsumerState<ServersView> createState() => _ServersViewState();
}

class _ServersViewState extends ConsumerState<ServersView> {
  String _searchQuery = '';
  int _sortColumnIndex = 1; // Default sort by Server Name
  bool _sortAscending = true;

  @override
  Widget build(BuildContext context) {
    final vm = ref.watch(appControllerProvider);
    final ctl = ref.watch(serversControllerProvider);

    var servers = ctl.servers.where((s) {
      if (_searchQuery.isEmpty) return true;
      final q = _searchQuery.toLowerCase();
      return s.name.toLowerCase().contains(q) ||
          s.ip.contains(q) ||
          s.description.toLowerCase().contains(q) ||
          s.address.contains(q);
    }).toList();

    // Apply sorting
    servers.sort((a, b) {
      int cmp = 0;
      switch (_sortColumnIndex) {
        case 1: // Nom
          cmp = a.name.toLowerCase().compareTo(b.name.toLowerCase());
          break;
        case 2: // IP / Port
          cmp = a.address.compareTo(b.address);
          break;
        case 4: // Ping
          cmp = a.ping.compareTo(b.ping);
          break;
        case 5: // Preference
          cmp = a.preference.compareTo(b.preference);
          break;
        default:
          cmp = a.name.toLowerCase().compareTo(b.name.toLowerCase());
      }
      return _sortAscending ? cmp : -cmp;
    });

    final hasServerConnectionCap =
        vm.daemonInfo?.capabilities.contains('server-connection') ?? false;
    final errorMessage = vm.errorMessage;

    void showPolicyDialog() {
      final router = GoRouter.maybeOf(context);
      if (router != null) {
        router.go('/settings?section=connection');
      } else {
        ref.read(shellControllerProvider).setSection(AppNavSection.settings);
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        ServersToolbar(
          onRefresh: ctl.fetch,
          onImportMet: () => showServerMetImportDialog(context, ctl),
          onUpdateFromUrl: () => showServersUpdateUrlDialog(context, ctl),
        ),
        if (hasServerConnectionCap) ServerSessionBar(controller: ctl),
        if (errorMessage != null && errorMessage.isNotEmpty)
          ServersErrorBanner(
            error: errorMessage,
            onEditProfile: showPolicyDialog,
          ),
        ServersSafetyBanner(
          hasConnectionCapability: hasServerConnectionCap,
          onShowPolicy: showPolicyDialog,
        ),
        ServersFilterBar(
          onQueryChanged: (val) => setState(() => _searchQuery = val),
          hasServers: ctl.servers.isNotEmpty,
          onClear: () => showServersClearDialog(context, ctl),
        ),
        const Divider(color: AppTheme.darkBorder, height: 1),
        Expanded(
          child: ctl.isLoading && servers.isEmpty
              ? const Center(child: CircularProgressIndicator())
              : servers.isEmpty
              ? ServersEmptyState(
                  onImport: () => showServerMetImportDialog(context, ctl),
                )
              : ServersTable(
                  controller: ctl,
                  servers: servers,
                  hasServerConnectionCap: hasServerConnectionCap,
                  sortColumnIndex: _sortColumnIndex,
                  sortAscending: _sortAscending,
                  onSort: (columnIndex, ascending) => setState(() {
                    _sortColumnIndex = columnIndex;
                    _sortAscending = ascending;
                  }),
                ),
        ),
      ],
    );
  }
}
