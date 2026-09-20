import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/kad_controller.dart';

/// Kad section toolbar: refresh, bootstrap, nodes.dat import and
/// bootstrap-from-URL actions — icon buttons on narrow layouts, labelled
/// buttons otherwise.
class KadToolbar extends StatelessWidget {
  final KadController controller;
  final VoidCallback onBootstrap;
  final VoidCallback onImportNodesDat;
  final VoidCallback onBootstrapFromUrl;

  const KadToolbar({
    super.key,
    required this.controller,
    required this.onBootstrap,
    required this.onImportNodesDat,
    required this.onBootstrapFromUrl,
  });

  @override
  Widget build(BuildContext context) {
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
                  onPressed: _refresh,
                ),
                IconButton(
                  tooltip: 'Amorcer (Bootstrap)',
                  icon: const Icon(
                    Icons.rocket_launch_outlined,
                    size: 18,
                    color: AppColors.accentPurple,
                  ),
                  onPressed: onBootstrap,
                ),
                IconButton(
                  tooltip: 'Importer nodes.dat',
                  icon: const Icon(
                    Icons.file_upload_outlined,
                    size: 18,
                    color: AppTheme.primary,
                  ),
                  onPressed: onImportNodesDat,
                ),
                IconButton(
                  tooltip: 'Amorcer depuis URL',
                  icon: const Icon(
                    Icons.cloud_download_outlined,
                    size: 18,
                    color: AppColors.accentTeal,
                  ),
                  onPressed: onBootstrapFromUrl,
                ),
              ] else ...[
                OutlinedButton.icon(
                  icon: const Icon(Icons.refresh_rounded, size: 16),
                  label: const Text('Actualiser'),
                  onPressed: _refresh,
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
                  onPressed: onBootstrap,
                ),
                const SizedBox(width: 10),
                ElevatedButton.icon(
                  icon: const Icon(Icons.file_upload_outlined, size: 16),
                  label: const Text('Importer nodes.dat'),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: AppTheme.primary,
                    foregroundColor: AppColors.textPrimary,
                  ),
                  onPressed: onImportNodesDat,
                ),
                const SizedBox(width: 10),
                FilledButton.icon(
                  icon: const Icon(Icons.cloud_download_outlined, size: 16),
                  label: const Text('Amorcer depuis URL'),
                  style: FilledButton.styleFrom(
                    backgroundColor: AppColors.labShade700,
                    foregroundColor: AppColors.textPrimary,
                  ),
                  onPressed: onBootstrapFromUrl,
                ),
              ],
            ],
          );
        },
      ),
    );
  }

  void _refresh() {
    controller.fetchStatus();
    controller.fetchContacts();
  }
}
