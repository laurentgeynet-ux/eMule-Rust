import 'package:flutter/material.dart';

import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../downloads/view_models/downloads_view_model.dart';
import '../../downloads/widgets/network_policy_dialog.dart';

class ServersView extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const ServersView({super.key, required this.viewModel});

  @override
  State<ServersView> createState() => _ServersViewState();
}

class _ServersViewState extends State<ServersView> {
  String _searchQuery = '';
  int _sortColumnIndex = 1; // Default sort by Server Name
  bool _sortAscending = true;

  @override
  Widget build(BuildContext context) {
    final vm = widget.viewModel;
    var servers = vm.servers.where((s) {
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _buildToolbar(context, vm),
        if (hasServerConnectionCap) _buildSessionBar(context, vm),
        if (vm.errorMessage != null && vm.errorMessage!.isNotEmpty)
          _buildErrorBanner(context, vm),
        _buildSafetyBanner(context, vm, hasServerConnectionCap),
        _buildFilterBar(context, vm),
        const Divider(color: AppTheme.darkBorder, height: 1),
        Expanded(
          child: vm.isServersLoading && servers.isEmpty
              ? const Center(child: CircularProgressIndicator())
              : servers.isEmpty
                  ? _buildEmptyState(context, vm)
                  : _buildServersTable(context, vm, servers,
                      hasServerConnectionCap: hasServerConnectionCap),
        ),
      ],
    );
  }

  // ── Toolbar ────────────────────────────────────────────────────────────────

  Widget _buildToolbar(BuildContext context, DownloadsViewModel vm) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isCompact = constraints.maxWidth < 1150;
          return Row(
            children: [
              const Icon(Icons.dns_rounded, color: AppTheme.primary, size: 22),
              const SizedBox(width: 10),
              const Expanded(
                child: Text(
                  'Gestion Locale des Serveurs eD2K',
                  style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.bold,
                    color: Colors.white,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 12),
              if (isCompact) ...[
                IconButton(
                  tooltip: 'Actualiser',
                  icon: const Icon(Icons.refresh_rounded, size: 18),
                  onPressed: () => vm.fetchServers(),
                ),
                IconButton(
                  tooltip: 'Importer server.met',
                  icon: const Icon(Icons.file_upload_outlined, size: 18, color: AppTheme.primary),
                  onPressed: () => _handleImportMet(context, vm),
                ),
                IconButton(
                  tooltip: 'Mettre à jour depuis URL',
                  icon: const Icon(Icons.cloud_download_outlined, size: 18, color: Colors.tealAccent),
                  onPressed: () => _handleUpdateFromUrl(context, vm),
                ),
              ] else ...[
                OutlinedButton.icon(
                  icon: const Icon(Icons.refresh_rounded, size: 16),
                  label: const Text('Actualiser'),
                  onPressed: () => vm.fetchServers(),
                ),
                const SizedBox(width: 10),
                ElevatedButton.icon(
                  icon: const Icon(Icons.file_upload_outlined, size: 16),
                  label: const Text('Importer server.met'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppTheme.primary,
                    foregroundColor: Colors.white,
                  ),
                  onPressed: () => _handleImportMet(context, vm),
                ),
                const SizedBox(width: 10),
                FilledButton.icon(
                  icon: const Icon(Icons.cloud_download_outlined, size: 16),
                  label: const Text('Mettre à jour depuis URL'),
                  style: FilledButton.styleFrom(
                    backgroundColor: Colors.teal[700],
                    foregroundColor: Colors.white,
                  ),
                  onPressed: () => _handleUpdateFromUrl(context, vm),
                ),
              ],
            ],
          );
        },
      ),
    );
  }

  // ── Session Bar ────────────────────────────────────────────────────────────

  Widget _buildSessionBar(BuildContext context, DownloadsViewModel vm) {
    final session = vm.serverSession;
    final status = session.status;
    final isConnected = status == ServerConnectionStatus.connected;
    final isConnecting = status == ServerConnectionStatus.connecting ||
        vm.isServerConnecting;
    final isDisconnected = status == ServerConnectionStatus.disconnected;

    Color barBg;
    Color barBorder;
    Color statusColor;
    if (isConnected) {
      barBg = Colors.green.withAlpha(20);
      barBorder = Colors.greenAccent.withAlpha(80);
      statusColor = Colors.greenAccent;
    } else if (isConnecting) {
      barBg = Colors.amber.withAlpha(20);
      barBorder = Colors.amberAccent.withAlpha(80);
      statusColor = Colors.amberAccent;
    } else {
      barBg = Colors.white.withAlpha(8);
      barBorder = AppTheme.darkBorder;
      statusColor = Colors.white38;
    }

    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: barBg,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: barBorder),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isNarrow = constraints.maxWidth < 750;

          final statusIndicator = Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 8,
                height: 8,
                margin: const EdgeInsets.only(right: 8),
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: statusColor,
                ),
              ),
              Text(
                status.toDisplayString(),
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: statusColor,
                ),
              ),
              if (isConnected) ...[
                const SizedBox(width: 8),
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
                  decoration: BoxDecoration(
                    color: session.isHighId
                        ? Colors.greenAccent.withAlpha(40)
                        : Colors.orangeAccent.withAlpha(40),
                    borderRadius: BorderRadius.circular(4),
                    border: Border.all(
                      color: session.isHighId
                          ? Colors.greenAccent.withAlpha(100)
                          : Colors.orangeAccent.withAlpha(100),
                    ),
                  ),
                  child: Text(
                    session.isHighId ? 'HighID' : 'LowID',
                    style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.bold,
                      color: session.isHighId
                          ? Colors.greenAccent
                          : Colors.orangeAccent,
                    ),
                  ),
                ),
              ],
            ],
          );

          final serverTitle = session.serverName != null && session.serverName!.isNotEmpty
              ? Text(
                  session.serverName!,
                  style: const TextStyle(
                    fontSize: 13,
                    color: Colors.white70,
                    fontWeight: FontWeight.w500,
                  ),
                  overflow: TextOverflow.ellipsis,
                )
              : !isDisconnected
                  ? Text(
                      session.serverAddress ?? '',
                      style: const TextStyle(
                          fontSize: 12,
                          color: Colors.white54,
                          fontFamily: 'monospace'),
                      overflow: TextOverflow.ellipsis,
                    )
                  : const Text(
                      'Aucun serveur connecté',
                      style: TextStyle(fontSize: 12, color: Colors.white38),
                    );

          final userMetrics = isConnected && session.currentUsers != null
              ? Text(
                  '${_fmt(session.currentUsers!)} utilisateurs • ${_fmt(session.currentFiles ?? 0)} fichiers',
                  style: const TextStyle(fontSize: 11, color: Colors.white54),
                )
              : null;

          final actionBtn = !isDisconnected
              ? TextButton.icon(
                  icon: Icon(
                    isConnecting ? Icons.close_rounded : Icons.power_settings_new,
                    size: 16,
                    color: Colors.redAccent,
                  ),
                  label: Text(
                    isConnecting ? 'Annuler' : 'Déconnecter',
                    style: const TextStyle(color: Colors.redAccent, fontSize: 12),
                  ),
                  style: TextButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                        horizontal: 10, vertical: 6),
                    minimumSize: Size.zero,
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                  ),
                  onPressed: () => vm.disconnectServer(),
                )
              : null;

          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (isNarrow) ...[
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Expanded(child: statusIndicator),
                    ?actionBtn,
                  ],
                ),
                const SizedBox(height: 6),
                Row(
                  children: [
                    Expanded(child: serverTitle),
                  ],
                ),
                if (userMetrics != null) ...[
                  const SizedBox(height: 4),
                  userMetrics,
                ],
              ] else ...[
                Row(
                  children: [
                    statusIndicator,
                    const SizedBox(width: 10),
                    Expanded(child: serverTitle),
                    if (userMetrics != null) ...[
                      const SizedBox(width: 12),
                      userMetrics,
                    ],
                    if (actionBtn != null) ...[
                      const SizedBox(width: 12),
                      actionBtn,
                    ],
                  ],
                ),
              ],
              if (isConnected &&
                  session.welcomeMessage != null &&
                  session.welcomeMessage!.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    session.welcomeMessage!,
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                        fontSize: 11.5, color: Colors.white54, height: 1.4),
                  ),
                ),
            ],
          );
        },
      ),
    );
  }

  String _fmt(int v) {
    if (v >= 1000000) return '${(v / 1000000).toStringAsFixed(1)}M';
    if (v >= 1000) return '${(v / 1000).toStringAsFixed(1)}k';
    return '$v';
  }

  // ── Error Banner ───────────────────────────────────────────────────────────

  Widget _buildErrorBanner(BuildContext context, DownloadsViewModel vm) {
    final error = vm.errorMessage ?? '';
    final isPolicyError = error.toLowerCase().contains('policy') ||
        error.toLowerCase().contains('networkprofile') ||
        error.toLowerCase().contains('réseau');

    return Container(
      margin: const EdgeInsets.fromLTRB(20, 10, 20, 0),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: AppTheme.statusFailed.withAlpha(30),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.statusFailed.withAlpha(120)),
      ),
      child: Row(
        children: [
          const Icon(Icons.error_outline_rounded,
              color: AppTheme.statusFailed, size: 20),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  error,
                  style: const TextStyle(
                      fontSize: 12,
                      color: Colors.white,
                      fontWeight: FontWeight.w500),
                ),
                if (isPolicyError) ...[
                  const SizedBox(height: 2),
                  const Text(
                    'Par défaut, Emule-Sion est confiné en boucle locale (LocalLab). Basculez sur PublicEd2kPreview pour autoriser les serveurs publics.',
                    style: TextStyle(fontSize: 11, color: Colors.white70),
                  ),
                ],
              ],
            ),
          ),
          if (isPolicyError) ...[
            const SizedBox(width: 10),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                backgroundColor: AppTheme.primary,
                foregroundColor: Colors.white,
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                minimumSize: Size.zero,
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              ),
              onPressed: () {
                showDialog(
                  context: context,
                  builder: (ctx) => NetworkPolicyDialog(viewModel: vm),
                );
              },
              child: const Text('Modifier le profil', style: TextStyle(fontSize: 11)),
            ),
          ],
        ],
      ),
    );
  }

  // ── Safety Banner ──────────────────────────────────────────────────────────

  Widget _buildSafetyBanner(
      BuildContext context, DownloadsViewModel vm, bool hasConnectionCap) {
    final text = hasConnectionCap
        ? 'Connexions eD2K disponibles. Vérifiez votre profil réseau (LocalLab vs PublicEd2kPreview) avant de vous connecter. Les adresses dans votre filtre IP sont automatiquement signalées.'
        : 'Avertissement Réseau : Importer ou gérer une liste de serveurs n\'établit aucune connexion réseau sortante. Les adresses figurant dans votre filtre IP actif sont automatiquement signalées.';

    return Container(
      margin: const EdgeInsets.fromLTRB(20, 14, 20, 4),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Colors.blue.withAlpha(25),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: Colors.blue.withAlpha(80)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isNarrow = constraints.maxWidth < 650;

          final securityBtn = OutlinedButton.icon(
            style: OutlinedButton.styleFrom(
              side: BorderSide(color: Colors.blueAccent.withAlpha(120)),
              foregroundColor: Colors.blueAccent,
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            icon: const Icon(Icons.security_rounded, size: 14),
            label: const Text('Sécurité / Profil', style: TextStyle(fontSize: 11)),
            onPressed: () {
              showDialog(
                context: context,
                builder: (ctx) => NetworkPolicyDialog(viewModel: vm),
              );
            },
          );

          if (isNarrow) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(Icons.shield_outlined, color: Colors.blueAccent, size: 20),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Text(
                        text,
                        style: const TextStyle(fontSize: 12, color: Colors.white70, height: 1.3),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Align(
                  alignment: Alignment.centerRight,
                  child: securityBtn,
                ),
              ],
            );
          }

          return Row(
            children: [
              const Icon(Icons.shield_outlined, color: Colors.blueAccent, size: 20),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  text,
                  style: const TextStyle(fontSize: 12.5, color: Colors.white70, height: 1.3),
                ),
              ),
              const SizedBox(width: 10),
              securityBtn,
            ],
          );
        },
      ),
    );
  }

  // ── Filter Bar ─────────────────────────────────────────────────────────────

  Widget _buildFilterBar(BuildContext context, DownloadsViewModel vm) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              decoration: InputDecoration(
                hintText: 'Rechercher un serveur (nom, IP, port)...',
                prefixIcon: const Icon(Icons.search, size: 18),
                contentPadding:
                    const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(8),
                  borderSide: const BorderSide(color: AppTheme.darkBorder),
                ),
                isDense: true,
              ),
              onChanged: (val) => setState(() => _searchQuery = val),
            ),
          ),
          const SizedBox(width: 16),
          if (vm.servers.isNotEmpty)
            TextButton.icon(
              icon: const Icon(Icons.delete_sweep_outlined,
                  color: Colors.redAccent, size: 16),
              label: const Text('Tout effacer',
                  style: TextStyle(color: Colors.redAccent)),
              onPressed: () => _confirmClearServers(context, vm),
            ),
        ],
      ),
    );
  }

  // ── Empty State ────────────────────────────────────────────────────────────

  Widget _buildEmptyState(BuildContext context, DownloadsViewModel vm) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.dns_outlined, size: 64, color: Colors.white.withAlpha(40)),
          const SizedBox(height: 16),
          const Text(
            'Aucun serveur eD2K enregistre',
            style: TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.w600,
                color: Colors.white70),
          ),
          const SizedBox(height: 8),
          const Text(
            'Importez un fichier server.met local pour inspecter et gerer vos serveurs.',
            style: TextStyle(fontSize: 13, color: Colors.white38),
          ),
          const SizedBox(height: 20),
          ElevatedButton.icon(
            icon: const Icon(Icons.file_upload_outlined),
            label: const Text('Selectionner server.met'),
            onPressed: () => _handleImportMet(context, vm),
          ),
        ],
      ),
    );
  }

  // ── Servers Table ──────────────────────────────────────────────────────────

  Widget _buildServersTable(
    BuildContext context,
    DownloadsViewModel vm,
    List<Ed2kServerModel> servers, {
    required bool hasServerConnectionCap,
  }) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final isMobile = constraints.maxWidth < 650;
        final isMedium = constraints.maxWidth >= 650 && constraints.maxWidth < 1000;

        if (isMobile) {
          return _buildServerCardsList(
            context,
            vm,
            servers,
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
                    ? (_sortColumnIndex == 4 ? 3 : _sortColumnIndex == 2 ? 2 : 1)
                    : _sortColumnIndex,
                sortAscending: _sortAscending,
                headingRowHeight: 40,
                dataRowMinHeight: 44,
                dataRowMaxHeight: 52,
                horizontalMargin: 16,
                columnSpacing: isMedium ? 14 : 18,
                columns: isMedium
                    ? [
                        const DataColumn(
                            label: Text('Etat / Filtre',
                                style: TextStyle(fontWeight: FontWeight.bold))),
                        DataColumn(
                          label: const Text('Nom du Serveur',
                              style: TextStyle(fontWeight: FontWeight.bold)),
                          onSort: (columnIndex, ascending) {
                            setState(() {
                              _sortColumnIndex = 1;
                              _sortAscending = ascending;
                            });
                          },
                        ),
                        DataColumn(
                          label: const Text('IP : Port',
                              style: TextStyle(fontWeight: FontWeight.bold)),
                          onSort: (columnIndex, ascending) {
                            setState(() {
                              _sortColumnIndex = 2;
                              _sortAscending = ascending;
                            });
                          },
                        ),
                        DataColumn(
                          label: const Text('Ping',
                              style: TextStyle(fontWeight: FontWeight.bold)),
                          numeric: true,
                          onSort: (columnIndex, ascending) {
                            setState(() {
                              _sortColumnIndex = 4;
                              _sortAscending = ascending;
                            });
                          },
                        ),
                        const DataColumn(
                            label: Text('Actions',
                                style: TextStyle(fontWeight: FontWeight.bold))),
                      ]
                    : [
                        const DataColumn(
                            label: Text('Etat / Filtre',
                                style: TextStyle(fontWeight: FontWeight.bold))),
                        DataColumn(
                          label: const Text('Nom du Serveur',
                              style: TextStyle(fontWeight: FontWeight.bold)),
                          onSort: (columnIndex, ascending) {
                            setState(() {
                              _sortColumnIndex = columnIndex;
                              _sortAscending = ascending;
                            });
                          },
                        ),
                        DataColumn(
                          label: const Text('IP : Port',
                              style: TextStyle(fontWeight: FontWeight.bold)),
                          onSort: (columnIndex, ascending) {
                            setState(() {
                              _sortColumnIndex = columnIndex;
                              _sortAscending = ascending;
                            });
                          },
                        ),
                        const DataColumn(
                            label: Text('Description',
                                style: TextStyle(fontWeight: FontWeight.bold))),
                        DataColumn(
                          label: const Text('Ping',
                              style: TextStyle(fontWeight: FontWeight.bold)),
                          numeric: true,
                          onSort: (columnIndex, ascending) {
                            setState(() {
                              _sortColumnIndex = columnIndex;
                              _sortAscending = ascending;
                            });
                          },
                        ),
                        DataColumn(
                          label: const Text('Preference',
                              style: TextStyle(fontWeight: FontWeight.bold)),
                          numeric: true,
                          onSort: (columnIndex, ascending) {
                            setState(() {
                              _sortColumnIndex = columnIndex;
                              _sortAscending = ascending;
                            });
                          },
                        ),
                        const DataColumn(
                            label: Text('Version',
                                style: TextStyle(fontWeight: FontWeight.bold))),
                        const DataColumn(
                            label: Text('Actions',
                                style: TextStyle(fontWeight: FontWeight.bold))),
                      ],
                rows: servers
                    .map((s) => _buildServerRow(
                          context,
                          vm,
                          s,
                          hasServerConnectionCap: hasServerConnectionCap,
                          isMedium: isMedium,
                        ))
                    .toList(),
              ),
            ),
          ),
        );
      },
    );
  }

  Widget _buildServerCardsList(
    BuildContext context,
    DownloadsViewModel vm,
    List<Ed2kServerModel> servers, {
    required bool hasServerConnectionCap,
  }) {
    return ListView.separated(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      itemCount: servers.length,
      separatorBuilder: (_, _) => const SizedBox(height: 10),
      itemBuilder: (context, index) {
        final server = servers[index];
        final isBlocked = server.filterStatus == ServerFilterStatus.blockedByFilter;
        final isDisabled = server.filterStatus == ServerFilterStatus.disabled;
        final isAllowed = server.filterStatus == ServerFilterStatus.allowed && !isDisabled;

        final session = vm.serverSession;
        final isActiveSession = session.serverAddress == server.address &&
            session.status != ServerConnectionStatus.disconnected;

        Widget statusBadge;
        if (isBlocked) {
          statusBadge = Container(
            padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
            decoration: BoxDecoration(
              color: Colors.red.withAlpha(40),
              borderRadius: BorderRadius.circular(4),
              border: Border.all(color: Colors.redAccent.withAlpha(100)),
            ),
            child: const Text('Bloque par IP Filter',
                style: TextStyle(
                    color: Colors.redAccent,
                    fontSize: 10.5,
                    fontWeight: FontWeight.bold)),
          );
        } else if (isDisabled) {
          statusBadge = Container(
            padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
            decoration: BoxDecoration(
              color: Colors.grey.withAlpha(40),
              borderRadius: BorderRadius.circular(4),
              border: Border.all(color: Colors.grey.withAlpha(100)),
            ),
            child: const Text('Desactive',
                style: TextStyle(color: Colors.white60, fontSize: 10.5)),
          );
        } else {
          statusBadge = Container(
            padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 2),
            decoration: BoxDecoration(
              color: Colors.green.withAlpha(40),
              borderRadius: BorderRadius.circular(4),
              border: Border.all(color: Colors.greenAccent.withAlpha(100)),
            ),
            child: const Text('Autorise',
                style: TextStyle(
                    color: Colors.greenAccent,
                    fontSize: 10.5,
                    fontWeight: FontWeight.bold)),
          );
        }

        return Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: AppTheme.darkSurface,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(
              color: isActiveSession
                  ? Colors.greenAccent.withAlpha(100)
                  : AppTheme.darkBorder,
            ),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  statusBadge,
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      server.name.isNotEmpty ? server.name : 'Sans nom',
                      style: const TextStyle(
                        fontWeight: FontWeight.bold,
                        fontSize: 13.5,
                        color: Colors.white,
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  if (server.ping > 0) ...[
                    const SizedBox(width: 6),
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                      decoration: BoxDecoration(
                        color: Colors.white.withAlpha(10),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        '${server.ping} ms',
                        style: const TextStyle(
                          fontSize: 11,
                          color: Colors.white70,
                          fontWeight: FontWeight.w500,
                        ),
                      ),
                    ),
                  ],
                ],
              ),
              const SizedBox(height: 6),
              Row(
                children: [
                  const Icon(Icons.lan_outlined, size: 13, color: Colors.white38),
                  const SizedBox(width: 5),
                  Text(
                    server.address,
                    style: const TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 11.5,
                      color: Colors.white70,
                    ),
                  ),
                  if (server.version != null && server.version!.isNotEmpty) ...[
                    const SizedBox(width: 10),
                    Text(
                      'v${server.version}',
                      style: const TextStyle(fontSize: 11, color: Colors.white38),
                    ),
                  ],
                ],
              ),
              if (server.description.isNotEmpty) ...[
                const SizedBox(height: 4),
                Text(
                  server.description,
                  style: const TextStyle(fontSize: 11, color: Colors.white54),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ],
              const SizedBox(height: 8),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  if (hasServerConnectionCap && isAllowed) ...[
                    if (isActiveSession) ...[
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                        decoration: BoxDecoration(
                          color: Colors.greenAccent.withAlpha(25),
                          borderRadius: BorderRadius.circular(4),
                          border: Border.all(color: Colors.greenAccent.withAlpha(80)),
                        ),
                        child: Text(
                          session.status.toDisplayString(),
                          style: const TextStyle(color: Colors.greenAccent, fontSize: 11),
                        ),
                      ),
                    ] else ...[
                      ElevatedButton.icon(
                        icon: const Icon(Icons.power_rounded, size: 13),
                        label: const Text('Se connecter', style: TextStyle(fontSize: 11)),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: AppTheme.primary,
                          foregroundColor: Colors.white,
                          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
                          visualDensity: VisualDensity.compact,
                        ),
                        onPressed: (!vm.isServerConnecting &&
                                session.status == ServerConnectionStatus.disconnected)
                            ? () => vm.connectServer(server.address)
                            : null,
                      ),
                    ],
                    const SizedBox(width: 8),
                  ],
                  IconButton(
                    icon: Icon(
                      isDisabled ? Icons.play_arrow_rounded : Icons.pause_rounded,
                      size: 18,
                      color: isDisabled ? Colors.greenAccent : Colors.amberAccent,
                    ),
                    tooltip: isDisabled ? 'Activer' : 'Desactiver',
                    onPressed: () => vm.setServerEnabled(server.address, isDisabled),
                  ),
                  IconButton(
                    icon: const Icon(Icons.delete_outline, size: 18, color: Colors.white54),
                    tooltip: 'Supprimer',
                    onPressed: () => vm.deleteServer(server.address),
                  ),
                ],
              ),
            ],
          ),
        );
      },
    );
  }

  DataRow _buildServerRow(
    BuildContext context,
    DownloadsViewModel vm,
    Ed2kServerModel server, {
    required bool hasServerConnectionCap,
    bool isMedium = false,
  }) {
    final isBlocked = server.filterStatus == ServerFilterStatus.blockedByFilter;
    final isDisabled = server.filterStatus == ServerFilterStatus.disabled;
    final isAllowed =
        server.filterStatus == ServerFilterStatus.allowed && !isDisabled;

    final session = vm.serverSession;
    final isActiveSession = session.serverAddress == server.address &&
        session.status != ServerConnectionStatus.disconnected;

    Widget statusBadge;
    if (isBlocked) {
      statusBadge = Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
        decoration: BoxDecoration(
          color: Colors.red.withAlpha(40),
          borderRadius: BorderRadius.circular(4),
          border: Border.all(color: Colors.redAccent.withAlpha(100)),
        ),
        child: const Text('Bloque par IP Filter',
            style: TextStyle(
                color: Colors.redAccent,
                fontSize: 11,
                fontWeight: FontWeight.bold)),
      );
    } else if (isDisabled) {
      statusBadge = Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
        decoration: BoxDecoration(
          color: Colors.grey.withAlpha(40),
          borderRadius: BorderRadius.circular(4),
          border: Border.all(color: Colors.grey.withAlpha(100)),
        ),
        child: const Text('Desactive',
            style: TextStyle(color: Colors.white60, fontSize: 11)),
      );
    } else {
      statusBadge = Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
        decoration: BoxDecoration(
          color: Colors.green.withAlpha(40),
          borderRadius: BorderRadius.circular(4),
          border: Border.all(color: Colors.greenAccent.withAlpha(100)),
        ),
        child: const Text('Autorise',
            style: TextStyle(
                color: Colors.greenAccent,
                fontSize: 11,
                fontWeight: FontWeight.bold)),
      );
    }

    Widget? connectBtn;
    if (hasServerConnectionCap && isAllowed) {
      if (isActiveSession) {
        connectBtn = Container(
          padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 3),
          decoration: BoxDecoration(
            color: Colors.greenAccent.withAlpha(25),
            borderRadius: BorderRadius.circular(4),
            border: Border.all(color: Colors.greenAccent.withAlpha(80)),
          ),
          child: Text(
            session.status.toDisplayString(),
            style: const TextStyle(color: Colors.greenAccent, fontSize: 11),
          ),
        );
      } else {
        final canConnect =
            session.status == ServerConnectionStatus.disconnected &&
                !vm.isServerConnecting;
        connectBtn = SizedBox(
          height: 30,
          child: ElevatedButton.icon(
            icon: const Icon(Icons.power_rounded, size: 14),
            label:
                const Text('Se connecter', style: TextStyle(fontSize: 12)),
            style: ElevatedButton.styleFrom(
              backgroundColor: AppTheme.primary.withAlpha(200),
              foregroundColor: Colors.white,
              padding:
                  const EdgeInsets.symmetric(horizontal: 10, vertical: 0),
              minimumSize: Size.zero,
              tapTargetSize: MaterialTapTargetSize.shrinkWrap,
            ),
            onPressed:
                canConnect ? () => vm.connectServer(server.address) : null,
          ),
        );
      }
    }

    final actionsCell = DataCell(Row(
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
            color: isDisabled ? Colors.greenAccent : Colors.amberAccent,
          ),
          tooltip: isDisabled ? 'Activer' : 'Desactiver',
          onPressed: () =>
              vm.setServerEnabled(server.address, isDisabled),
        ),
        IconButton(
          icon: const Icon(Icons.delete_outline,
              size: 18, color: Colors.white54),
          tooltip: 'Supprimer',
          onPressed: () => vm.deleteServer(server.address),
        ),
      ],
    ));

    if (isMedium) {
      return DataRow(
        cells: [
          DataCell(statusBadge),
          DataCell(ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 220),
            child: Text(
              server.name.isNotEmpty ? server.name : 'Sans nom',
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontWeight: FontWeight.w500),
            ),
          )),
          DataCell(Text(server.address,
              style: const TextStyle(fontFamily: 'monospace'))),
          DataCell(Text(server.ping > 0 ? '${server.ping} ms' : '-')),
          actionsCell,
        ],
      );
    }

    return DataRow(
      cells: [
        DataCell(statusBadge),
        DataCell(ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 240),
          child: Text(
            server.name.isNotEmpty ? server.name : 'Sans nom',
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(fontWeight: FontWeight.w500),
          ),
        )),
        DataCell(Text(server.address,
            style: const TextStyle(fontFamily: 'monospace'))),
        DataCell(ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 220),
          child: Text(
            server.description.isNotEmpty ? server.description : '-',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(color: Colors.white60),
          ),
        )),
        DataCell(Text(server.ping > 0 ? '${server.ping} ms' : '-')),
        DataCell(Text('${server.preference}')),
        DataCell(Text(server.version ?? '-')),
        actionsCell,
      ],
    );
  }

  // ── Import Dialog ──────────────────────────────────────────────────────────

  Future<void> _handleImportMet(
      BuildContext context, DownloadsViewModel vm) async {
    final pathController = TextEditingController();

    final path = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Row(
          children: [
            Icon(Icons.file_upload_outlined, color: AppTheme.primary),
            SizedBox(width: 8),
            Text('Importer un fichier server.met local'),
          ],
        ),
        content: SizedBox(
          width: 500,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Indiquez le chemin absolu vers votre fichier server.met local :',
                style: TextStyle(fontSize: 13, color: Colors.white70),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: pathController,
                autofocus: true,
                decoration: const InputDecoration(
                  labelText: 'Chemin du fichier server.met',
                  hintText: 'C:\\path\\to\\server.met ou /path/to/server.met',
                  border: OutlineInputBorder(),
                  prefixIcon: Icon(Icons.description_outlined, size: 18),
                ),
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(null),
            child: const Text('Annuler'),
          ),
          ElevatedButton.icon(
            icon: const Icon(Icons.preview_rounded, size: 16),
            label: const Text('Inspecter'),
            onPressed: () {
              final text = pathController.text.trim();
              if (text.isNotEmpty) {
                Navigator.of(ctx).pop(text);
              }
            },
          ),
        ],
      ),
    );

    if (path == null || path.isEmpty) return;

    if (!context.mounted) return;
    final report = await vm.inspectServerMet(path);
    if (report == null) return;

    if (!context.mounted) return;
    final shouldImport = await showDialog<bool>(
      context: context,
      builder: (ctx) => _buildInspectDialog(ctx, report),
    );

    if (shouldImport == true) {
      await vm.importServerMet(path);
    }
  }

  Widget _buildInspectDialog(
      BuildContext context, ServerInspectReport report) {
    return AlertDialog(
      title: const Row(
        children: [
          Icon(Icons.preview_rounded, color: AppTheme.primary),
          SizedBox(width: 8),
          Text('Apercu du fichier server.met'),
        ],
      ),
      content: SizedBox(
        width: 600,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text('Fichier : ${report.filePath}',
                style:
                    const TextStyle(fontSize: 12, color: Colors.white54)),
            const SizedBox(height: 14),
            Row(
              children: [
                _buildSummaryStat(
                    'Total', '${report.totalCount}', Colors.white),
                const SizedBox(width: 12),
                _buildSummaryStat(
                    'Autorises', '${report.allowedCount}', Colors.greenAccent),
                const SizedBox(width: 12),
                _buildSummaryStat('Bloques (Filtre IP)',
                    '${report.blockedCount}', Colors.redAccent),
              ],
            ),
            const SizedBox(height: 16),
            const Text(
              'Les serveurs marques en rouge correspondent a des plages de votre filtre IP local. Ils seront importes avec le statut bloque.',
              style: TextStyle(fontSize: 12, color: Colors.white70),
            ),
            const SizedBox(height: 12),
            Container(
              height: 200,
              decoration: BoxDecoration(
                border: Border.all(color: AppTheme.darkBorder),
                borderRadius: BorderRadius.circular(6),
              ),
              child: ListView.separated(
                itemCount: report.servers.length,
                separatorBuilder: (_, _) =>
                    const Divider(height: 1, color: AppTheme.darkBorder),
                itemBuilder: (ctx, idx) {
                  final s = report.servers[idx];
                  final isBlocked =
                      s.filterStatus == ServerFilterStatus.blockedByFilter;
                  return ListTile(
                    dense: true,
                    title: Text(s.name,
                        style:
                            const TextStyle(fontWeight: FontWeight.w600)),
                    subtitle: Text(s.address,
                        style: const TextStyle(
                            fontFamily: 'monospace', fontSize: 11)),
                    trailing: isBlocked
                        ? const Text('Bloque',
                            style: TextStyle(
                                color: Colors.redAccent,
                                fontWeight: FontWeight.bold,
                                fontSize: 11))
                        : const Text('Autorise',
                            style: TextStyle(
                                color: Colors.greenAccent, fontSize: 11)),
                  );
                },
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: const Text('Annuler'),
        ),
        ElevatedButton.icon(
          icon: const Icon(Icons.download_done_rounded, size: 16),
          label: const Text('Confirmer importation'),
          onPressed: () => Navigator.of(context).pop(true),
        ),
      ],
    );
  }

  Widget _buildSummaryStat(String label, String value, Color color) {
    return Expanded(
      child: Container(
        padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 10),
        decoration: BoxDecoration(
          color: color.withAlpha(20),
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: color.withAlpha(60)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(label, style: TextStyle(fontSize: 10.5, color: color)),
            const SizedBox(height: 2),
            Text(value,
                style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.bold,
                    color: color)),
          ],
        ),
      ),
    );
  }

  // ── Clear Dialog ───────────────────────────────────────────────────────────

  Future<void> _confirmClearServers(
      BuildContext context, DownloadsViewModel vm) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Effacer tous les serveurs ?'),
        content: const Text(
          'Cette operation supprime la liste des serveurs enregistres dans votre base locale.',
        ),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: const Text('Annuler')),
          ElevatedButton(
            style:
                ElevatedButton.styleFrom(backgroundColor: Colors.redAccent),
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Effacer'),
          ),
        ],
      ),
    );

    if (confirmed == true) {
      await vm.clearServers();
    }
  }

  // ── Update From URL Dialog ─────────────────────────────────────────────────

  Future<void> _handleUpdateFromUrl(
      BuildContext context, DownloadsViewModel vm) async {
    final urlController = TextEditingController(
      text: 'http://upd.emule-security.org/server.met',
    );

    final url = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Row(
          children: [
            Icon(Icons.cloud_download_outlined, color: Colors.tealAccent),
            SizedBox(width: 8),
            Text('Mettre à jour depuis une URL'),
          ],
        ),
        content: SizedBox(
          width: 520,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Indiquez l\'adresse HTTP ou HTTPS d\'un fichier server.met distant pour mettre à jour automatiquement votre annuaire de serveurs :',
                style: TextStyle(fontSize: 13, color: Colors.white70),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: urlController,
                autofocus: true,
                style: const TextStyle(fontSize: 13),
                decoration: const InputDecoration(
                  labelText: 'URL du server.met',
                  hintText: 'http://upd.emule-security.org/server.met',
                  border: OutlineInputBorder(),
                  prefixIcon: Icon(Icons.link_rounded, size: 18),
                ),
              ),
              const SizedBox(height: 10),
              const Text(
                'Note : Les adresses IP de réseaux privés (LAN/RFC1918) et loopback sont rejetées par sécurité (anti-SSRF).',
                style: TextStyle(fontSize: 11, color: Colors.white38),
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(null),
            child: const Text('Annuler'),
          ),
          FilledButton.icon(
            icon: const Icon(Icons.download_rounded, size: 16),
            label: const Text('Télécharger et Mettre à jour'),
            style: FilledButton.styleFrom(
              backgroundColor: Colors.teal[700],
              foregroundColor: Colors.white,
            ),
            onPressed: () {
              final text = urlController.text.trim();
              if (text.isNotEmpty) {
                Navigator.of(ctx).pop(text);
              }
            },
          ),
        ],
      ),
    );

    if (url == null || url.isEmpty) return;

    if (!context.mounted) return;
    await vm.updateServersFromUrl(url);
  }
}

