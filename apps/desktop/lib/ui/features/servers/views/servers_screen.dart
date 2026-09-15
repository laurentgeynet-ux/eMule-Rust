import 'package:flutter/material.dart';

import '../../../../domain/models/server_models.dart';
import '../../../core/theme.dart';
import '../../downloads/view_models/downloads_view_model.dart';

class ServersView extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const ServersView({super.key, required this.viewModel});

  @override
  State<ServersView> createState() => _ServersViewState();
}

class _ServersViewState extends State<ServersView> {
  String _searchQuery = '';

  @override
  Widget build(BuildContext context) {
    final vm = widget.viewModel;
    final servers = vm.servers.where((s) {
      if (_searchQuery.isEmpty) return true;
      final q = _searchQuery.toLowerCase();
      return s.name.toLowerCase().contains(q) ||
          s.ip.contains(q) ||
          s.description.toLowerCase().contains(q) ||
          s.address.contains(q);
    }).toList();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _buildToolbar(context, vm),
        _buildSafetyBanner(context),
        _buildFilterBar(context, vm),
        const Divider(color: AppTheme.darkBorder, height: 1),
        Expanded(
          child: vm.isServersLoading && servers.isEmpty
              ? const Center(child: CircularProgressIndicator())
              : servers.isEmpty
                  ? _buildEmptyState(context, vm)
                  : _buildServersTable(context, vm, servers),
        ),
      ],
    );
  }

  Widget _buildToolbar(BuildContext context, DownloadsViewModel vm) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5)),
      ),
      child: Row(
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
        ],
      ),
    );
  }

  Widget _buildSafetyBanner(BuildContext context) {
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 14, 20, 4),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Colors.blue.withAlpha(25),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: Colors.blue.withAlpha(80)),
      ),
      child: Row(
        children: [
          const Icon(Icons.shield_outlined, color: Colors.blueAccent, size: 20),
          const SizedBox(width: 12),
          const Expanded(
            child: Text(
              'Avertissement Réseau : Importer ou gérer une liste de serveurs n\'établit aucune connexion réseau sortante (Zéro socket WAN). Les adresses figurant dans votre filtre IP actif sont automatiquement signalées.',
              style: TextStyle(fontSize: 12.5, color: Colors.white70, height: 1.3),
            ),
          ),
        ],
      ),
    );
  }

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
                contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
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
              icon: const Icon(Icons.delete_sweep_outlined, color: Colors.redAccent, size: 16),
              label: const Text('Tout effacer', style: TextStyle(color: Colors.redAccent)),
              onPressed: () => _confirmClearServers(context, vm),
            ),
        ],
      ),
    );
  }

  Widget _buildEmptyState(BuildContext context, DownloadsViewModel vm) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(Icons.dns_outlined, size: 64, color: Colors.white.withAlpha(40)),
          const SizedBox(height: 16),
          const Text(
            'Aucun serveur eD2K enregistré',
            style: TextStyle(fontSize: 16, fontWeight: FontWeight.w600, color: Colors.white70),
          ),
          const SizedBox(height: 8),
          const Text(
            'Importez un fichier server.met local pour inspecter et gérer vos serveurs.',
            style: TextStyle(fontSize: 13, color: Colors.white38),
          ),
          const SizedBox(height: 20),
          ElevatedButton.icon(
            icon: const Icon(Icons.file_upload_outlined),
            label: const Text('Sélectionner server.met'),
            onPressed: () => _handleImportMet(context, vm),
          ),
        ],
      ),
    );
  }

  Widget _buildServersTable(BuildContext context, DownloadsViewModel vm, List<Ed2kServerModel> servers) {
    return SingleChildScrollView(
      scrollDirection: Axis.vertical,
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: DataTable(
          headingRowHeight: 40,
          dataRowMinHeight: 44,
          dataRowMaxHeight: 52,
          horizontalMargin: 16,
          columnSpacing: 18,
          columns: const [
            DataColumn(label: Text('État / Filtre', style: TextStyle(fontWeight: FontWeight.bold))),
            DataColumn(label: Text('Nom du Serveur', style: TextStyle(fontWeight: FontWeight.bold))),
            DataColumn(label: Text('IP : Port', style: TextStyle(fontWeight: FontWeight.bold))),
            DataColumn(label: Text('Description', style: TextStyle(fontWeight: FontWeight.bold))),
            DataColumn(label: Text('Ping', style: TextStyle(fontWeight: FontWeight.bold))),
            DataColumn(label: Text('Préférence', style: TextStyle(fontWeight: FontWeight.bold))),
            DataColumn(label: Text('Version', style: TextStyle(fontWeight: FontWeight.bold))),
            DataColumn(label: Text('Actions', style: TextStyle(fontWeight: FontWeight.bold))),
          ],
          rows: servers.map((s) => _buildServerRow(context, vm, s)).toList(),
        ),
      ),
    );
  }

  DataRow _buildServerRow(BuildContext context, DownloadsViewModel vm, Ed2kServerModel server) {
    final isBlocked = server.filterStatus == ServerFilterStatus.blockedByFilter;
    final isDisabled = server.filterStatus == ServerFilterStatus.disabled;

    Widget statusBadge;
    if (isBlocked) {
      statusBadge = Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
        decoration: BoxDecoration(
          color: Colors.red.withAlpha(40),
          borderRadius: BorderRadius.circular(4),
          border: Border.all(color: Colors.redAccent.withAlpha(100)),
        ),
        child: const Text('Bloqué par IP Filter', style: TextStyle(color: Colors.redAccent, fontSize: 11, fontWeight: FontWeight.bold)),
      );
    } else if (isDisabled) {
      statusBadge = Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
        decoration: BoxDecoration(
          color: Colors.grey.withAlpha(40),
          borderRadius: BorderRadius.circular(4),
          border: Border.all(color: Colors.grey.withAlpha(100)),
        ),
        child: const Text('Désactivé', style: TextStyle(color: Colors.white60, fontSize: 11)),
      );
    } else {
      statusBadge = Container(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
        decoration: BoxDecoration(
          color: Colors.green.withAlpha(40),
          borderRadius: BorderRadius.circular(4),
          border: Border.all(color: Colors.greenAccent.withAlpha(100)),
        ),
        child: const Text('Autorisé', style: TextStyle(color: Colors.greenAccent, fontSize: 11, fontWeight: FontWeight.bold)),
      );
    }

    return DataRow(
      cells: [
        DataCell(statusBadge),
        DataCell(Text(server.name.isNotEmpty ? server.name : 'Sans nom', style: const TextStyle(fontWeight: FontWeight.w500))),
        DataCell(Text(server.address, style: const TextStyle(fontFamily: 'monospace'))),
        DataCell(Text(
          server.description.isNotEmpty ? server.description : '-',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(color: Colors.white60),
        )),
        DataCell(Text(server.ping > 0 ? '${server.ping} ms' : '-')),
        DataCell(Text('${server.preference}')),
        DataCell(Text(server.version ?? '-')),
        DataCell(Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            IconButton(
              icon: Icon(
                isDisabled ? Icons.play_arrow_rounded : Icons.pause_rounded,
                size: 18,
                color: isDisabled ? Colors.greenAccent : Colors.amberAccent,
              ),
              tooltip: isDisabled ? 'Activer' : 'Désactiver',
              onPressed: () => vm.setServerEnabled(server.address, isDisabled),
            ),
            IconButton(
              icon: const Icon(Icons.delete_outline, size: 18, color: Colors.white54),
              tooltip: 'Supprimer',
              onPressed: () => vm.deleteServer(server.address),
            ),
          ],
        )),
      ],
    );
  }

  Future<void> _handleImportMet(BuildContext context, DownloadsViewModel vm) async {
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

    // 1. Inspect first without persisting
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

  Widget _buildInspectDialog(BuildContext context, ServerInspectReport report) {
    return AlertDialog(
      title: const Row(
        children: [
          Icon(Icons.preview_rounded, color: AppTheme.primary),
          SizedBox(width: 8),
          Text('Aperçu du fichier server.met'),
        ],
      ),
      content: SizedBox(
        width: 600,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text('Fichier : ${report.filePath}', style: const TextStyle(fontSize: 12, color: Colors.white54)),
            const SizedBox(height: 14),
            Row(
              children: [
                _buildSummaryStat('Total', '${report.totalCount}', Colors.white),
                const SizedBox(width: 12),
                _buildSummaryStat('Autorisés', '${report.allowedCount}', Colors.greenAccent),
                const SizedBox(width: 12),
                _buildSummaryStat('Bloqués (Filtre IP)', '${report.blockedCount}', Colors.redAccent),
              ],
            ),
            const SizedBox(height: 16),
            const Text(
              'Les serveurs marqués en rouge correspondent à des plages de votre filtre IP local (ipfilter.dat). Ils seront importés avec le statut bloqué.',
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
                separatorBuilder: (_, _) => const Divider(height: 1, color: AppTheme.darkBorder),
                itemBuilder: (ctx, idx) {
                  final s = report.servers[idx];
                  final isBlocked = s.filterStatus == ServerFilterStatus.blockedByFilter;
                  return ListTile(
                    dense: true,
                    title: Text(s.name, style: const TextStyle(fontWeight: FontWeight.w600)),
                    subtitle: Text(s.address, style: const TextStyle(fontFamily: 'monospace', fontSize: 11)),
                    trailing: isBlocked
                        ? const Text('Bloqué', style: TextStyle(color: Colors.redAccent, fontWeight: FontWeight.bold, fontSize: 11))
                        : const Text('Autorisé', style: TextStyle(color: Colors.greenAccent, fontSize: 11)),
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
          label: const Text('Confirmer l\'importation'),
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
            Text(value, style: TextStyle(fontSize: 16, fontWeight: FontWeight.bold, color: color)),
          ],
        ),
      ),
    );
  }

  Future<void> _confirmClearServers(BuildContext context, DownloadsViewModel vm) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Effacer tous les serveurs ?'),
        content: const Text(
          'Cette opération supprime la liste des serveurs enregistrés dans votre base locale. Aucun fichier de téléchargement n\'est affecté.',
        ),
        actions: [
          TextButton(onPressed: () => Navigator.of(ctx).pop(false), child: const Text('Annuler')),
          ElevatedButton(
            style: ElevatedButton.styleFrom(backgroundColor: Colors.redAccent),
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
}
