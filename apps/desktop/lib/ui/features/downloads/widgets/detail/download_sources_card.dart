import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../view_models/peer_sources_controller.dart';
import 'detail_section_card.dart';
import 'peer_source_row.dart';

/// eD2K peer sources card: active peer-session banner, manual-source input,
/// "ask server / ask Kad" actions and the per-source list with connect /
/// force / disconnect controls.
class DownloadSourcesCard extends StatefulWidget {
  final DownloadItem item;
  final PeerSourcesController controller;
  final void Function(String fileHash, int fileSize) onQueryServerSources;
  final void Function(String fileHash, int fileSize) onQueryKadSources;

  const DownloadSourcesCard({
    super.key,
    required this.item,
    required this.controller,
    required this.onQueryServerSources,
    required this.onQueryKadSources,
  });

  @override
  State<DownloadSourcesCard> createState() => _DownloadSourcesCardState();
}

class _DownloadSourcesCardState extends State<DownloadSourcesCard> {
  final TextEditingController _manualSourceController =
      TextEditingController();

  @override
  void dispose() {
    _manualSourceController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ctl = widget.controller;
    final hash = widget.item.ed2kHash ?? '';
    final sources = ctl.getSourcesForHash(hash);
    final session = ctl.session;
    final isConnectedToThis =
        session.fileHash == hash &&
        session.status != PeerConnectionStatus.disconnected;

    return DetailSectionCard(
      title: 'SOURCES ED2K & SESSION PEER',
      icon: Icons.hub_outlined,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (isConnectedToThis) ...[
            _PeerSessionBanner(
              session: session,
              onDisconnect: () => ctl.disconnect(),
            ),
            const SizedBox(height: 12),
          ],

          // Toolbar: manual source, ask server, ask Kad, refresh
          Row(
            children: [
              Expanded(
                child: SizedBox(
                  height: 32,
                  child: TextField(
                    controller: _manualSourceController,
                    style: const TextStyle(
                      fontSize: 12,
                      color: AppColors.textPrimary,
                    ),
                    decoration: InputDecoration(
                      hintText:
                          'Ajouter source IP:port (ex: 127.0.0.1:39735)',
                      hintStyle: const TextStyle(
                        fontSize: 11,
                        color: AppColors.textFaint,
                      ),
                      contentPadding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 8,
                      ),
                      filled: true,
                      fillColor: AppTheme.darkSurface,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(6),
                        borderSide: const BorderSide(
                          color: AppTheme.darkBorder,
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
                  ),
                ),
              ),
              const SizedBox(width: 8),
              SizedBox(
                height: 32,
                child: ElevatedButton.icon(
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppTheme.primary,
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                  ),
                  icon: const Icon(
                    Icons.add,
                    size: 14,
                    color: AppColors.textPrimary,
                  ),
                  label: const Text(
                    'Ajouter',
                    style: TextStyle(
                      fontSize: 11,
                      color: AppColors.textPrimary,
                    ),
                  ),
                  onPressed: () async {
                    final addr = _manualSourceController.text.trim();
                    if (addr.isNotEmpty && hash.isNotEmpty) {
                      final ok = await ctl.addManualSource(hash, addr);
                      if (ok) {
                        _manualSourceController.clear();
                      }
                    }
                  },
                ),
              ),
              const SizedBox(width: 8),
              SizedBox(
                height: 32,
                child: OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(
                    side: const BorderSide(color: AppTheme.darkBorder),
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                  ),
                  icon: const Icon(
                    Icons.search,
                    size: 14,
                    color: AppColors.textSecondary,
                  ),
                  label: const Text(
                    'Demander au serveur',
                    style: TextStyle(
                      fontSize: 11,
                      color: AppColors.textSecondary,
                    ),
                  ),
                  onPressed: hash.isNotEmpty
                      ? () => widget.onQueryServerSources(
                          hash,
                          widget.item.totalBytes,
                        )
                      : null,
                ),
              ),
              const SizedBox(width: 8),
              SizedBox(
                height: 32,
                child: OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(
                    side: const BorderSide(color: AppColors.accentAmber),
                    foregroundColor: AppColors.accentAmber,
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                  ),
                  icon: const Icon(
                    Icons.hub_rounded,
                    size: 14,
                    color: AppColors.accentAmber,
                  ),
                  label: const Text(
                    'Demander au réseau Kad',
                    style: TextStyle(
                      fontSize: 11,
                      color: AppColors.accentAmber,
                    ),
                  ),
                  onPressed: hash.isNotEmpty
                      ? () => widget.onQueryKadSources(
                          hash,
                          widget.item.totalBytes,
                        )
                      : null,
                ),
              ),
              const SizedBox(width: 6),
              IconButton(
                icon: const Icon(
                  Icons.refresh,
                  size: 16,
                  color: AppColors.textMuted,
                ),
                tooltip: 'Actualiser la liste',
                onPressed: hash.isNotEmpty
                    ? () => ctl.loadSourcesForHash(hash)
                    : null,
              ),
            ],
          ),
          const SizedBox(height: 12),

          // Sources list
          if (sources.isEmpty)
            Container(
              width: double.infinity,
              padding: const EdgeInsets.symmetric(vertical: 16),
              alignment: Alignment.center,
              child: Text(
                ctl.isLoading
                    ? 'Chargement des sources...'
                    : 'Aucune source enregistrée pour ce fichier.',
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textFaint,
                ),
              ),
            )
          else
            Container(
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(6),
                border: Border.all(color: AppTheme.darkBorder, width: 0.5),
              ),
              child: ListView.separated(
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                itemCount: sources.length,
                separatorBuilder: (context, index) =>
                    const Divider(height: 1, color: AppTheme.darkBorder),
                itemBuilder: (context, index) {
                  return PeerSourceRow(
                    source: sources[index],
                    fileHash: hash,
                    session: session,
                    isPeerConnecting: ctl.isConnecting,
                    onConnect: (address, {force = false}) =>
                        ctl.connect(hash, address, force: force),
                    onDisconnect: (address) =>
                        ctl.disconnect(address: address),
                  );
                },
              ),
            ),
        ],
      ),
    );
  }
}

class _PeerSessionBanner extends StatelessWidget {
  final PeerSessionModel session;
  final VoidCallback onDisconnect;

  const _PeerSessionBanner({
    required this.session,
    required this.onDisconnect,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppTheme.primary.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.primary.withValues(alpha: 0.3)),
      ),
      child: Row(
        children: [
          const Icon(
            Icons.wifi_tethering,
            color: AppTheme.primary,
            size: 20,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      'Session active : ${session.peerAddress ?? ""}',
                      style: const TextStyle(
                        fontSize: 12,
                        fontWeight: FontWeight.bold,
                        color: AppColors.textPrimary,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 6,
                        vertical: 1,
                      ),
                      decoration: BoxDecoration(
                        color: AppColors.accentTeal.withValues(alpha: 0.2),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        session.status.toDisplayString(),
                        style: const TextStyle(
                          fontSize: 10,
                          fontWeight: FontWeight.w600,
                          color: AppColors.accentTeal,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 2),
                Text(
                  'Reçu : ${Formatters.formatBytes(session.transferredBytes)} '
                  '(${Formatters.formatSpeed(session.transferRateBps)})',
                  style: const TextStyle(
                    fontSize: 11,
                    color: AppColors.textSecondary,
                  ),
                ),
              ],
            ),
          ),
          OutlinedButton.icon(
            style: OutlinedButton.styleFrom(
              foregroundColor: AppColors.danger,
              side: const BorderSide(color: AppColors.danger, width: 0.8),
              padding: const EdgeInsets.symmetric(
                horizontal: 10,
                vertical: 6,
              ),
            ),
            icon: const Icon(Icons.link_off, size: 14),
            label: const Text('Déconnecter', style: TextStyle(fontSize: 11)),
            onPressed: onDisconnect,
          ),
        ],
      ),
    );
  }
}
