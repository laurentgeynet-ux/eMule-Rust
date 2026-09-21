import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../app/providers.dart';
import 'detail/peer_source_row.dart';

/// Inline expansion shown under a download row (eMule-style double-click):
/// lists the known peer sources for the file with connect controls.
class DownloadSourcesExpansion extends ConsumerWidget {
  final DownloadItem item;

  const DownloadSourcesExpansion({super.key, required this.item});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final ctl = ref.watch(peerSourcesControllerProvider);
    final hash = item.ed2kHash ?? '';
    final sources = ctl.getSourcesForHash(hash);

    Widget body;
    if (hash.isEmpty) {
      body = const _ExpansionPlaceholder('Pas de hash eD2K pour ce fichier.');
    } else if (sources.isEmpty) {
      body = _ExpansionPlaceholder(
        ctl.isLoading
            ? 'Chargement des sources...'
            : 'Aucune source enregistrée pour ce fichier.',
      );
    } else {
      body = ListView.separated(
        shrinkWrap: true,
        physics: const NeverScrollableScrollPhysics(),
        itemCount: sources.length,
        separatorBuilder: (context, index) =>
            const Divider(height: 1, color: AppTheme.darkBorder),
        itemBuilder: (context, index) => PeerSourceRow(
          source: sources[index],
          fileHash: hash,
          session: ctl.session,
          isPeerConnecting: ctl.isConnecting,
          onConnect: (address, {force = false}) =>
              ctl.connect(hash, address, force: force),
          onDisconnect: (address) => ctl.disconnect(address: address),
        ),
      );
    }

    return Container(
      margin: const EdgeInsets.only(top: 8),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            child: Row(
              children: [
                const Icon(Icons.hub_outlined,
                    size: 14, color: AppColors.textMuted),
                const SizedBox(width: 6),
                Text(
                  'Sources (${sources.length})',
                  style: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                    color: AppColors.textSecondary,
                  ),
                ),
                const Spacer(),
                InkWell(
                  onTap: hash.isEmpty
                      ? null
                      : () => ref
                          .read(peerSourcesControllerProvider)
                          .loadSourcesForHash(hash),
                  child: const Padding(
                    padding: EdgeInsets.all(4),
                    child: Icon(Icons.refresh,
                        size: 14, color: AppColors.textMuted),
                  ),
                ),
              ],
            ),
          ),
          const Divider(height: 1, color: AppTheme.darkBorder),
          body,
        ],
      ),
    );
  }
}

class _ExpansionPlaceholder extends StatelessWidget {
  final String text;
  const _ExpansionPlaceholder(this.text);

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 16),
      child: Center(
        child: Text(
          text,
          style: const TextStyle(fontSize: 11, color: AppColors.textFaint),
        ),
      ),
    );
  }
}
