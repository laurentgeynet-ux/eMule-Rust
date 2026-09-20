import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Empty server list placeholder with a server.met import call-to-action.
class ServersEmptyState extends StatelessWidget {
  final VoidCallback onImport;

  const ServersEmptyState({super.key, required this.onImport});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(
            Icons.dns_outlined,
            size: 64,
            color: AppColors.textPrimary.withAlpha(40),
          ),
          const SizedBox(height: 16),
          const Text(
            'Aucun serveur eD2K enregistre',
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
              color: AppColors.textSecondary,
            ),
          ),
          const SizedBox(height: 8),
          const Text(
            'Importez un fichier server.met local pour inspecter et gerer vos serveurs.',
            style: TextStyle(fontSize: 13, color: AppColors.textFaint),
          ),
          const SizedBox(height: 20),
          ElevatedButton.icon(
            icon: const Icon(Icons.file_upload_outlined),
            label: const Text('Selectionner server.met'),
            onPressed: onImport,
          ),
        ],
      ),
    );
  }
}
