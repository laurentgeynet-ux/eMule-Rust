import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/servers_controller.dart';
import 'server_status_badge.dart';
import 'server_cards_list.dart';
import '../../../../l10n/l10n.dart';

/// Sortable servers data table (desktop / medium widths). Below 650 px the
/// layout switches to [ServerCardsList].
class ServersTable extends StatelessWidget {
  final ServersController controller;
  final List<Ed2kServerModel> servers;
  final bool hasServerConnectionCap;
  final int sortColumnIndex;
  final bool sortAscending;
  final void Function(int columnIndex, bool ascending) onSort;

  const ServersTable({
    super.key,
    required this.controller,
    required this.servers,
    required this.hasServerConnectionCap,
    required this.sortColumnIndex,
    required this.sortAscending,
    required this.onSort,
  });

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final isMobile = constraints.maxWidth < 650;
        final isMedium =
            constraints.maxWidth >= 650 && constraints.maxWidth < 1000;

        if (isMobile) {
          return ServerCardsList(
            controller: controller,
            servers: servers,
            hasServerConnectionCap: hasServerConnectionCap,
          );
        }

        return SingleChildScrollView(
          scrollDirection: Axis.vertical,
          child: SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: ConstrainedBox(
              constraints: BoxConstraints(minWidth: constraints.maxWidth),
              child: DataTable(
                sortColumnIndex: isMedium
                    ? (sortColumnIndex == 4
                          ? 3
                          : sortColumnIndex == 2
                          ? 2
                          : 1)
                    : sortColumnIndex,
                sortAscending: sortAscending,
                headingRowHeight: 40,
                dataRowMinHeight: 44,
                dataRowMaxHeight: 52,
                horizontalMargin: 16,
                columnSpacing: isMedium ? 14 : 18,
                columns: isMedium
                    ? [
                        DataColumn(
                          label: Text(context.l10n.etatFiltre,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                        ),
                        DataColumn(
                          label: Text(context.l10n.nomDuServeur,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                          onSort: (columnIndex, ascending) =>
                              onSort(1, ascending),
                        ),
                        DataColumn(
                          label: Text(context.l10n.ipPort,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                          onSort: (columnIndex, ascending) =>
                              onSort(2, ascending),
                        ),
                        DataColumn(
                          label: Text(context.l10n.ping,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                          numeric: true,
                          onSort: (columnIndex, ascending) =>
                              onSort(4, ascending),
                        ),
                        DataColumn(
                          label: Text(context.l10n.actions,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                        ),
                      ]
                    : [
                        DataColumn(
                          label: Text(context.l10n.etatFiltre,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                        ),
                        DataColumn(
                          label: Text(context.l10n.nomDuServeur,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                          onSort: (columnIndex, ascending) =>
                              onSort(columnIndex, ascending),
                        ),
                        DataColumn(
                          label: Text(context.l10n.ipPort,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                          onSort: (columnIndex, ascending) =>
                              onSort(columnIndex, ascending),
                        ),
                        DataColumn(
                          label: Text(context.l10n.description,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                        ),
                        DataColumn(
                          label: Text(context.l10n.ping,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                          numeric: true,
                          onSort: (columnIndex, ascending) =>
                              onSort(columnIndex, ascending),
                        ),
                        DataColumn(
                          label: Text(context.l10n.preference,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                          numeric: true,
                          onSort: (columnIndex, ascending) =>
                              onSort(columnIndex, ascending),
                        ),
                        DataColumn(
                          label: Text(context.l10n.version,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                        ),
                        DataColumn(
                          label: Text(context.l10n.actions,
                            style: TextStyle(fontWeight: FontWeight.bold),
                          ),
                        ),
                      ],
                rows: servers
                    .map(
                      (s) => _buildRow(
                        context,
                        s,
                        hasServerConnectionCap: hasServerConnectionCap,
                        isMedium: isMedium,
                      ),
                    )
                    .toList(),
              ),
            ),
          ),
        );
      },
    );
  }

  DataRow _buildRow(
    BuildContext context,
    Ed2kServerModel server, {
    required bool hasServerConnectionCap,
    bool isMedium = false,
  }) {
    final isDisabled = server.filterStatus == ServerFilterStatus.disabled;
    final isAllowed =
        server.filterStatus == ServerFilterStatus.allowed && !isDisabled;

    final session = controller.session;
    final isActiveSession =
        session.serverAddress == server.address &&
        session.status != ServerConnectionStatus.disconnected;

    Widget? connectBtn;
    if (hasServerConnectionCap && isAllowed) {
      if (isActiveSession) {
        connectBtn = Container(
          padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
          decoration: BoxDecoration(
            color: AppColors.positive.withAlpha(25),
            borderRadius: BorderRadius.circular(4),
            border: Border.all(color: AppColors.positive.withAlpha(80)),
          ),
          child: Text(
            session.status.toDisplayString(),
            style: const TextStyle(color: AppColors.positive, fontSize: 11),
          ),
        );
      } else {
        final canConnect =
            session.status == ServerConnectionStatus.disconnected &&
            !controller.isConnecting;
        connectBtn = SizedBox(
          height: 30,
          child: ElevatedButton.icon(
            icon: const Icon(Icons.power_rounded, size: 14),
            label: Text(context.l10n.seConnecter, style: TextStyle(fontSize: 12)),
            style: ElevatedButton.styleFrom(
              backgroundColor: AppTheme.primary.withAlpha(200),
              foregroundColor: AppColors.textPrimary,
              padding: const EdgeInsets.symmetric(
                horizontal: 10,
                vertical: 0,
              ),
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            onPressed: canConnect
                ? () => controller.connect(server.address)
                : null,
          ),
        );
      }
    }

    final actionsCell = DataCell(
      Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (connectBtn != null) ...[
            connectBtn,
            const SizedBox(width: 6),
          ],
          IconButton(
            icon: Icon(
              isDisabled ? Icons.play_arrow_rounded : Icons.pause_rounded,
              size: 18,
              color: isDisabled ? AppColors.positive : AppColors.accentAmber,
            ),
            tooltip:
                isDisabled ? context.l10n.serverEnable : context.l10n.serverDisable,
            onPressed: () =>
                controller.setEnabled(server.address, isDisabled),
          ),
          IconButton(
            icon: const Icon(
              Icons.delete_outline,
              size: 18,
              color: AppColors.textMuted,
            ),
            tooltip: context.l10n.supprimer,
            onPressed: () => controller.delete(server.address),
          ),
        ],
      ),
    );

    if (isMedium) {
      return DataRow(
        cells: [
          DataCell(
            ServerStatusBadge(filterStatus: server.filterStatus),
          ),
          DataCell(
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 220),
              child: Text(
                server.name.isNotEmpty ? server.name : context.l10n.serverUnnamed,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontWeight: FontWeight.w500),
              ),
            ),
          ),
          DataCell(
            Text(
              server.address,
              style: const TextStyle(fontFamily: 'monospace'),
            ),
          ),
          DataCell(Text(server.ping > 0 ? '${server.ping} ms' : '-')),
          actionsCell,
        ],
      );
    }

    return DataRow(
      cells: [
        DataCell(ServerStatusBadge(filterStatus: server.filterStatus)),
        DataCell(
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 240),
            child: Text(
              server.name.isNotEmpty ? server.name : context.l10n.serverUnnamed,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontWeight: FontWeight.w500),
            ),
          ),
        ),
        DataCell(
          Text(
            server.address,
            style: const TextStyle(fontFamily: 'monospace'),
          ),
        ),
        DataCell(
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 220),
            child: Text(
              server.description.isNotEmpty ? server.description : '-',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(color: AppColors.textSoft),
            ),
          ),
        ),
        DataCell(Text(server.ping > 0 ? '${server.ping} ms' : '-')),
        DataCell(Text('${server.preference}')),
        DataCell(Text(server.version ?? '-')),
        actionsCell,
      ],
    );
  }
}
