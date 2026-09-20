import 'package:flutter/material.dart';

import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../downloads/view_models/downloads_view_model.dart';

class KadView extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const KadView({super.key, required this.viewModel});

  @override
  State<KadView> createState() => _KadViewState();
}

class _KadViewState extends State<KadView> {
  String _searchQuery = '';

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.viewModel,
      builder: (context, _) {
    final vm = widget.viewModel;
    final status = vm.kadStatus;
    final contacts = vm.kadContacts.where((c) {
      if (_searchQuery.isEmpty) return true;
      final q = _searchQuery.toLowerCase();
      return c.kadId.toLowerCase().contains(q) ||
          c.ip.contains(q) ||
          c.udpPort.toString().contains(q);
    }).toList();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _buildToolbar(context, vm),
        _buildSafetyBanner(context, status),
        _buildStatusCard(context, vm, status),
        _buildFilterBar(context, vm),
        const Divider(color: AppTheme.darkBorder, height: 1),
        Expanded(
          child: vm.isKadLoading && contacts.isEmpty
              ? const Center(child: CircularProgressIndicator())
              : contacts.isEmpty
              ? _buildEmptyState(context, vm)
              : _buildContactsTable(context, vm, contacts),
        ),
      ],
    );
      },
    );
  }

  // ── Toolbar ────────────────────────────────────────────────────────────────

  Widget _buildToolbar(BuildContext context, DownloadsViewModel vm) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
      decoration: const BoxDecoration(
        color: AppTheme.darkSurface,
        border: Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isNarrow = constraints.maxWidth < 900;
          return Row(
            children: [
              const Icon(Icons.hub_rounded, color: AppTheme.primary, size: 22),
              const SizedBox(width: 10),
              const Expanded(
                child: Text(
                  'Réseau Kad DHT (Kademlia)',
                  style: TextStyle(
                    fontSize: 16,
                    fontWeight: FontWeight.bold,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 12),
              if (isNarrow) ...[
                IconButton(
                  tooltip: 'Actualiser',
                  icon: const Icon(Icons.refresh_rounded, size: 18),
                  onPressed: () {
                    vm.fetchKadStatus();
                    vm.fetchKadContacts();
                  },
                ),
                IconButton(
                  tooltip: 'Amorcer (Bootstrap)',
                  icon: const Icon(
                    Icons.rocket_launch_outlined,
                    size: 18,
                    color: AppColors.accentPurple,
                  ),
                  onPressed: () => _handleBootstrapDialog(context, vm),
                ),
                IconButton(
                  tooltip: 'Importer nodes.dat',
                  icon: const Icon(
                    Icons.file_upload_outlined,
                    size: 18,
                    color: AppTheme.primary,
                  ),
                  onPressed: () => _handleImportNodesDat(context, vm),
                ),
                IconButton(
                  tooltip: 'Amorcer depuis URL',
                  icon: const Icon(
                    Icons.cloud_download_outlined,
                    size: 18,
                    color: AppColors.accentTeal,
                  ),
                  onPressed: () => _handleBootstrapFromUrl(context, vm),
                ),
              ] else ...[
                OutlinedButton.icon(
                  icon: const Icon(Icons.refresh_rounded, size: 16),
                  label: const Text('Actualiser'),
                  onPressed: () {
                    vm.fetchKadStatus();
                    vm.fetchKadContacts();
                  },
                ),
                const SizedBox(width: 10),
                OutlinedButton.icon(
                  icon: const Icon(
                    Icons.rocket_launch_outlined,
                    size: 16,
                    color: AppColors.accentPurple,
                  ),
                  label: const Text(
                    'Amorcer (Bootstrap)',
                    style: TextStyle(color: AppColors.accentPurple),
                  ),
                  onPressed: () => _handleBootstrapDialog(context, vm),
                ),
                const SizedBox(width: 10),
                ElevatedButton.icon(
                  icon: const Icon(Icons.file_upload_outlined, size: 16),
                  label: const Text('Importer nodes.dat'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppTheme.primary,
                    foregroundColor: AppColors.textPrimary,
                  ),
                  onPressed: () => _handleImportNodesDat(context, vm),
                ),
                const SizedBox(width: 10),
                FilledButton.icon(
                  icon: const Icon(Icons.cloud_download_outlined, size: 16),
                  label: const Text('Amorcer depuis URL'),
                  style: FilledButton.styleFrom(
                    backgroundColor: AppColors.labShade700,
                    foregroundColor: AppColors.textPrimary,
                  ),
                  onPressed: () => _handleBootstrapFromUrl(context, vm),
                ),
              ],
            ],
          );
        },
      ),
    );
  }

  // ── Safety Banner ──────────────────────────────────────────────────────────

  Widget _buildSafetyBanner(BuildContext context, KadStatusModel status) {
    final isPublic = status.networkProfile != 'local-lab';
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: (isPublic ? AppColors.lab : AppColors.tintPurple).withAlpha(20),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: (isPublic ? AppColors.accentTeal : AppColors.accentPurple).withAlpha(
            80,
          ),
        ),
      ),
      child: Row(
        children: [
          Icon(
            isPublic ? Icons.public : Icons.shield_outlined,
            color: isPublic ? AppColors.accentTeal : AppColors.accentPurple,
            size: 18,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              status.isUdpActive
                  ? (isPublic
                        ? 'Réseau Kad DHT Mondial actif sur ${status.udpBindAddress ?? "0.0.0.0:39737"} | Routage Kademlia ouvert | Protection Anti-SSRF active.'
                        : 'Socket UDP actif : ${status.udpBindAddress ?? "127.0.0.1:39737"} (Mode Laboratoire Confiné) | Kill switch instantané.')
                  : (isPublic
                        ? 'Transport UDP Kad : Cliquez sur "Démarrer UDP" pour vous connecter au réseau Kademlia mondial.'
                        : 'Transport UDP Kad confiné : Seul le trafic loopback (127.0.0.1) est autorisé dans le profil LocalLab.'),
              style: const TextStyle(
                fontSize: 12,
                color: AppColors.textSecondary,
                height: 1.3,
              ),
            ),
          ),
        ],
      ),
    );
  }

  // ── Status Card ────────────────────────────────────────────────────

  Widget _buildStatusCard(
    BuildContext context,
    DownloadsViewModel vm,
    KadStatusModel status,
  ) {
    final isPublic = status.networkProfile != 'local-lab';
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
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
              const Icon(
                Icons.fingerprint_rounded,
                size: 16,
                color: AppColors.textSecondary,
              ),
              const SizedBox(width: 6),
              const Text(
                'ID Kad Local :',
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.bold,
                  color: AppColors.textSecondary,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: SelectableText(
                  status.localId.isNotEmpty
                      ? status.localId
                      : '(Généré à l'
                            'initialisation)',
                  style: const TextStyle(
                    fontSize: 12,
                    fontFamily: 'monospace',
                    color: AppColors.accentTeal,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              if (status.isUdpActive)
                OutlinedButton.icon(
                  icon: const Icon(
                    Icons.stop_circle_outlined,
                    size: 14,
                    color: AppColors.danger,
                  ),
                  label: const Text(
                    'Arrêter UDP',
                    style: TextStyle(fontSize: 11, color: AppColors.danger),
                  ),
                  style: OutlinedButton.styleFrom(
                    side: const BorderSide(color: AppColors.danger, width: 0.5),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 4,
                    ),
                  ),
                  onPressed: () => vm.stopKadUdp(),
                )
              else
                FilledButton.icon(
                  icon: const Icon(Icons.play_circle_outline, size: 14),
                  label: Text(
                    isPublic ? 'Démarrer UDP Kad' : 'Démarrer UDP Loopback',
                    style: const TextStyle(fontSize: 11),
                  ),
                  style: FilledButton.styleFrom(
                    backgroundColor: isPublic
                        ? AppTheme.primary
                        : AppColors.accentPurple,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 4,
                    ),
                  ),
                  onPressed: () => vm.startKadUdp(
                    bindAddress: isPublic ? '0.0.0.0:39737' : '127.0.0.1:39737',
                  ),
                ),
            ],
          ),
          const SizedBox(height: 10),
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                _buildStatBadge(
                  label: 'Socket UDP',
                  value: status.isUdpActive
                      ? 'Actif (${status.udpBindAddress ?? (isPublic ? "0.0.0.0:39737" : "127.0.0.1:39737")})'
                      : 'Inactif',
                  color: status.isUdpActive
                      ? AppColors.positive
                      : AppColors.accentOrange,
                ),
                const SizedBox(width: 12),
                _buildStatBadge(
                  label: 'Paquets TX / RX',
                  value: '${status.packetsSent} / ${status.packetsReceived}',
                  color: AppColors.accentCyan,
                ),
                const SizedBox(width: 12),
                _buildStatBadge(
                  label: 'Contacts actifs',
                  value: '${status.totalContacts}',
                  color: AppColors.accentTeal,
                ),
                const SizedBox(width: 12),
                _buildStatBadge(
                  label: 'Buckets actifs',
                  value: '${status.activeBuckets} / 128',
                  color: AppColors.positive,
                ),
                const SizedBox(width: 12),
                _buildStatBadge(
                  label: 'Contacts en base',
                  value: '${status.dbContactsCount}',
                  color: AppColors.textSecondary,
                ),
                const SizedBox(width: 12),
                _buildStatBadge(
                  label: 'Profil',
                  value: status.networkProfile,
                  color: AppColors.accentPurple,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildStatBadge({
    required String label,
    required String value,
    required Color color,
  }) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: color.withAlpha(15),
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: color.withAlpha(40), width: 0.5),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            '$label: ',
            style: const TextStyle(fontSize: 11, color: AppColors.textMuted),
          ),
          Text(
            value,
            style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.bold,
              color: color,
            ),
          ),
        ],
      ),
    );
  }

  // ── Filter Bar ─────────────────────────────────────────────────────

  Widget _buildFilterBar(BuildContext context, DownloadsViewModel vm) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
              decoration: InputDecoration(
                hintText: 'Filtrer par Kad ID, IP ou Port...',
                hintStyle: const TextStyle(fontSize: 13, color: AppColors.textFaint),
                prefixIcon: const Icon(
                  Icons.search_rounded,
                  size: 18,
                  color: AppColors.textFaint,
                ),
                isDense: true,
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 8,
                ),
                filled: true,
                fillColor: AppTheme.darkSurface,
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(6),
                  borderSide: const BorderSide(
                    color: AppTheme.darkBorder,
                    width: 0.5,
                  ),
                ),
                enabledBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(6),
                  borderSide: const BorderSide(
                    color: AppTheme.darkBorder,
                    width: 0.5,
                  ),
                ),
              ),
              onChanged: (val) => setState(() => _searchQuery = val),
            ),
          ),
          const SizedBox(width: 12),
          TextButton.icon(
            icon: const Icon(
              Icons.delete_sweep_outlined,
              size: 16,
              color: AppColors.danger,
            ),
            label: const Text(
              'Effacer la table',
              style: TextStyle(color: AppColors.danger, fontSize: 12),
            ),
            onPressed: () => _confirmClearContacts(context, vm),
          ),
        ],
      ),
    );
  }

  // ── Contacts Table ─────────────────────────────────────────────────

  Widget _buildContactsTable(
    BuildContext context,
    DownloadsViewModel vm,
    List<KadContactModel> contacts,
  ) {
    return SingleChildScrollView(
      scrollDirection: Axis.vertical,
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: DataTable(
          columnSpacing: 24,
          headingRowHeight: 38,
          dataRowMinHeight: 40,
          dataRowMaxHeight: 44,
          headingTextStyle: const TextStyle(
            fontSize: 12,
            fontWeight: FontWeight.bold,
            color: AppColors.textSecondary,
          ),
          columns: const [
            DataColumn(label: Text('Kad ID (128-bit XOR)')),
            DataColumn(label: Text('Adresse IP & Ports')),
            DataColumn(label: Text('Version')),
            DataColumn(label: Text('Statut')),
            DataColumn(label: Text('Échecs')),
            DataColumn(label: Text('Dernière vue')),
            DataColumn(label: Text('Actions')),
          ],
          rows: contacts.map((c) {
            return DataRow(
              cells: [
                DataCell(
                  Tooltip(
                    message: c.kadId,
                    child: Text(
                      _formatTruncatedId(c.kadId),
                      style: const TextStyle(
                        fontSize: 12,
                        fontFamily: 'monospace',
                        color: AppColors.accentTeal,
                      ),
                    ),
                  ),
                ),
                DataCell(
                  Text(
                    c.tcpPort != null
                        ? '${c.ip}:${c.udpPort} (TCP: ${c.tcpPort})'
                        : '${c.ip}:${c.udpPort}',
                    style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
                  ),
                ),
                DataCell(
                  Text(
                    'v${c.version}',
                    style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
                  ),
                ),
                DataCell(
                  Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 6,
                      vertical: 2,
                    ),
                    decoration: BoxDecoration(
                      color: c.verified
                          ? AppColors.tintGreen.withAlpha(20)
                          : AppColors.warning.withAlpha(20),
                      borderRadius: BorderRadius.circular(4),
                      border: Border.all(
                        color: c.verified
                            ? AppColors.positive.withAlpha(80)
                            : AppColors.accentAmber.withAlpha(80),
                        width: 0.5,
                      ),
                    ),
                    child: Text(
                      c.verified ? 'Vérifié' : 'Non vérifié',
                      style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.bold,
                        color: c.verified
                            ? AppColors.positive
                            : AppColors.accentAmber,
                      ),
                    ),
                  ),
                ),
                DataCell(
                  Text(
                    '${c.failCount}',
                    style: TextStyle(
                      fontSize: 12,
                      color: c.failCount > 0
                          ? AppColors.accentOrange
                          : AppColors.textMuted,
                    ),
                  ),
                ),
                DataCell(
                  Text(
                    _formatTime(c.lastSeenUnixMs),
                    style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
                  ),
                ),
                DataCell(
                  IconButton(
                    icon: const Icon(
                      Icons.network_ping_rounded,
                      size: 16,
                      color: AppColors.accentCyan,
                    ),
                    tooltip: 'Envoyer Kad Ping (0x60)',
                    onPressed: () => vm.pingKadContact(
                      ip: c.ip,
                      port: c.udpPort,
                      kadId: c.kadId,
                    ),
                  ),
                ),
              ],
            );
          }).toList(),
        ),
      ),
    );
  }

  String _formatTruncatedId(String id) {
    if (id.length <= 16) return id;
    return '${id.substring(0, 8)}...${id.substring(id.length - 8)}';
  }

  String _formatTime(int unixMs) {
    if (unixMs <= 0) return 'Jamais';
    final dt = DateTime.fromMillisecondsSinceEpoch(unixMs);
    final now = DateTime.now();
    final diff = now.difference(dt);
    if (diff.inMinutes < 1) return 'À l\'instant';
    if (diff.inHours < 1) return 'Il y a ${diff.inMinutes}m';
    if (diff.inDays < 1) return 'Il y a ${diff.inHours}h';
    final day = dt.day.toString().padLeft(2, '0');
    final month = dt.month.toString().padLeft(2, '0');
    final hour = dt.hour.toString().padLeft(2, '0');
    final minute = dt.minute.toString().padLeft(2, '0');
    return '$day/$month $hour:$minute';
  }

  // ── Empty State ────────────────────────────────────────────────────

  Widget _buildEmptyState(BuildContext context, DownloadsViewModel vm) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          const Icon(Icons.hub_outlined, size: 48, color: AppColors.fillMedium),
          const SizedBox(height: 12),
          const Text(
            'Aucun contact Kad',
            style: TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.bold,
              color: AppColors.textMuted,
            ),
          ),
          const SizedBox(height: 6),
          const Text(
            'Importez un fichier nodes.dat pour amorcer la table de routage K-buckets.',
            style: TextStyle(fontSize: 12, color: AppColors.textFaint),
          ),
          const SizedBox(height: 16),
          ElevatedButton.icon(
            icon: const Icon(Icons.file_upload_outlined, size: 16),
            label: const Text('Importer nodes.dat'),
            onPressed: () => _handleImportNodesDat(context, vm),
          ),
        ],
      ),
    );
  }

  // ── Import nodes.dat Dialog ────────────────────────────────────────

  Future<void> _handleImportNodesDat(
    BuildContext context,
    DownloadsViewModel vm,
  ) async {
    final pathController = TextEditingController();
    NodesDatInspectReportModel? report;
    bool isInspecting = false;

    await showDialog(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, setDialogState) {
          return AlertDialog(
            backgroundColor: AppTheme.darkSurface,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(12),
              side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
            ),
            title: Row(
              children: const [
                Icon(
                  Icons.file_upload_outlined,
                  color: AppTheme.primary,
                  size: 20,
                ),
                SizedBox(width: 8),
                Text('Importer un fichier nodes.dat'),
              ],
            ),
            content: SizedBox(
              width: 520,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Text(
                    'Spécifiez le chemin absolu vers un fichier nodes.dat (v0, v1 ou v2).',
                    style: TextStyle(fontSize: 12, color: AppColors.textSecondary),
                  ),
                  const SizedBox(height: 12),
                  Row(
                    children: [
                      Expanded(
                        child: TextField(
                          controller: pathController,
                          style: const TextStyle(
                            fontSize: 13,
                            color: AppColors.textPrimary,
                          ),
                          decoration: InputDecoration(
                            hintText: 'C:\\...\\nodes.dat',
                            hintStyle: const TextStyle(
                              fontSize: 13,
                              color: AppColors.textFaint,
                            ),
                            isDense: true,
                            contentPadding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                            filled: true,
                            fillColor: AppTheme.darkCard,
                            border: OutlineInputBorder(
                              borderRadius: BorderRadius.circular(6),
                              borderSide: const BorderSide(
                                color: AppTheme.darkBorder,
                                width: 0.5,
                              ),
                            ),
                          ),
                        ),
                      ),
                      const SizedBox(width: 8),
                      ElevatedButton(
                        onPressed: isInspecting
                            ? null
                            : () async {
                                final p = pathController.text.trim();
                                if (p.isEmpty) return;
                                setDialogState(() => isInspecting = true);
                                final rep = await vm.inspectNodesDat(p);
                                setDialogState(() {
                                  report = rep;
                                  isInspecting = false;
                                });
                              },
                        child: isInspecting
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Text('Inspecter'),
                      ),
                    ],
                  ),
                  if (report != null) ...[
                    const SizedBox(height: 16),
                    Container(
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: AppTheme.darkCard,
                        borderRadius: BorderRadius.circular(8),
                        border: Border.all(
                          color: AppTheme.darkBorder,
                          width: 0.5,
                        ),
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Format nodes.dat v${report!.version} détecté',
                            style: const TextStyle(
                              fontSize: 13,
                              fontWeight: FontWeight.bold,
                              color: AppColors.accentTeal,
                            ),
                          ),
                          const SizedBox(height: 6),
                          Text(
                            '• Contacts déclarés : ${report!.declaredCount}',
                            style: const TextStyle(
                              fontSize: 12,
                              color: AppColors.textSecondary,
                            ),
                          ),
                          Text(
                            '• Contacts valides & admissibles : ${report!.validContactsCount}',
                            style: const TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.bold,
                              color: AppColors.positive,
                            ),
                          ),
                          if (report!.rejectedByPolicyCount > 0)
                            Text(
                              '• Rejetés par la politique réseau : ${report!.rejectedByPolicyCount}',
                              style: const TextStyle(
                                fontSize: 12,
                                color: AppColors.accentOrange,
                              ),
                            ),
                          if (report!.rejectedByFilterCount > 0)
                            Text(
                              '• Rejetés par le filtre IP : ${report!.rejectedByFilterCount}',
                              style: const TextStyle(
                                fontSize: 12,
                                color: AppColors.danger,
                              ),
                            ),
                        ],
                      ),
                    ),
                  ],
                ],
              ),
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.of(ctx).pop(),
                child: const Text(
                  'Annuler',
                  style: TextStyle(color: AppColors.textSoft),
                ),
              ),
              FilledButton(
                style: FilledButton.styleFrom(
                  backgroundColor: AppTheme.primary,
                ),
                onPressed: report == null || report!.validContactsCount == 0
                    ? null
                    : () async {
                        final p = pathController.text.trim();
                        Navigator.of(ctx).pop();
                        await vm.importNodesDat(p);
                      },
                child: const Text('Importer les contacts'),
              ),
            ],
          );
        },
      ),
    );
  }

  // ── Bootstrap Dialog ───────────────────────────────────────────────

  Future<void> _handleBootstrapDialog(
    BuildContext context,
    DownloadsViewModel vm,
  ) async {
    final isPublic = vm.kadStatus.networkProfile != 'local-lab';
    final ipController = TextEditingController(
      text: isPublic ? '' : '127.0.0.1',
    );
    final portController = TextEditingController(text: '39737');

    await showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
        title: Row(
          children: const [
            Icon(
              Icons.rocket_launch_outlined,
              color: AppColors.accentPurple,
              size: 20,
            ),
            SizedBox(width: 8),
            Text('Amorcer le réseau Kad (Bootstrap)'),
          ],
        ),
        content: SizedBox(
          width: 400,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Text(
                isPublic
                    ? 'Entrez l\'adresse IP publique et le port UDP d\'un nœud Kad pour amorcer la table de routage (ex: 212.83.184.225).'
                    : 'Entrez l\'adresse IP et le port UDP d\'un nœud d\'amorçage (en profil LocalLab, seul 127.0.0.1 est permis).',
                style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: ipController,
                style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
                decoration: InputDecoration(
                  labelText: 'Adresse IP',
                  hintText: isPublic ? 'ex: 212.83.184.225' : '127.0.0.1',
                  isDense: true,
                  filled: true,
                  fillColor: AppTheme.darkCard,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(6),
                  ),
                ),
              ),
              const SizedBox(height: 10),
              TextField(
                controller: portController,
                keyboardType: TextInputType.number,
                style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
                decoration: InputDecoration(
                  labelText: 'Port UDP',
                  hintText: '39737',
                  isDense: true,
                  filled: true,
                  fillColor: AppTheme.darkCard,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(6),
                  ),
                ),
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text(
              'Annuler',
              style: TextStyle(color: AppColors.textSoft),
            ),
          ),
          FilledButton(
            style: FilledButton.styleFrom(backgroundColor: AppColors.accentPurple),
            onPressed: () async {
              final ip = ipController.text.trim();
              final port = int.tryParse(portController.text.trim()) ?? 39737;
              Navigator.of(ctx).pop();
              await vm.bootstrapKad(ip: ip, port: port);
            },
            child: const Text('Lancer l\'amorçage'),
          ),
        ],
      ),
    );
  }

  // ── Confirm Clear Contacts Dialog ──────────────────────────────────

  void _confirmClearContacts(BuildContext context, DownloadsViewModel vm) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
        title: const Text('Effacer la table de routage Kad ?'),
        content: const Text(
          'Tous les contacts Kad en mémoire et dans la base SQLite locale seront supprimés.\n'
          'Aucun paquet réseau ne sera envoyé.',
          style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text(
              'Annuler',
              style: TextStyle(color: AppColors.textSoft),
            ),
          ),
          FilledButton(
            style: FilledButton.styleFrom(backgroundColor: AppColors.danger),
            onPressed: () {
              Navigator.of(ctx).pop();
              vm.clearKadContacts();
            },
            child: const Text('Effacer'),
          ),
        ],
      ),
    );
  }

  // ── Bootstrap From URL Dialog ───────────────────────────────────────

  Future<void> _handleBootstrapFromUrl(
    BuildContext context,
    DownloadsViewModel vm,
  ) async {
    final urlController = TextEditingController(
      text: 'https://upd.emule-security.org/nodes.dat',
    );

    final url = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(12),
          side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
        title: const Row(
          children: [
            Icon(
              Icons.cloud_download_outlined,
              color: AppColors.accentTeal,
              size: 20,
            ),
            SizedBox(width: 8),
            Text('Amorcer Kad depuis une URL'),
          ],
        ),
        content: SizedBox(
          width: 520,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Téléchargez un fichier nodes.dat distant pour amorcer votre table de routage Kad Kademlia (ex: https://upd.emule-security.org/nodes.dat) :',
                style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
              ),
              const SizedBox(height: 12),
              TextField(
                controller: urlController,
                autofocus: true,
                style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
                decoration: InputDecoration(
                  labelText: 'URL du nodes.dat',
                  hintText: 'https://upd.emule-security.org/nodes.dat',
                  isDense: true,
                  filled: true,
                  fillColor: AppTheme.darkCard,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(6),
                  ),
                  prefixIcon: const Icon(Icons.link_rounded, size: 18),
                ),
              ),
              const SizedBox(height: 10),
              const Text(
                'Note : Seuls les protocoles HTTP/HTTPS publics sont acceptés. Le trafic vers les réseaux locaux (RFC1918) et loopback est rejeté par sécurité (anti-SSRF).',
                style: TextStyle(fontSize: 11, color: AppColors.textFaint),
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(null),
            child: const Text(
              'Annuler',
              style: TextStyle(color: AppColors.textSoft),
            ),
          ),
          FilledButton.icon(
            icon: const Icon(Icons.download_rounded, size: 16),
            label: const Text('Télécharger et Amorcer'),
            style: FilledButton.styleFrom(
              backgroundColor: AppColors.labShade700,
              foregroundColor: AppColors.textPrimary,
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
    await vm.bootstrapKadFromUrl(url);
  }
}
