import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Empty routing-table placeholder with a nodes.dat import call-to-action.
class KadEmptyState extends StatelessWidget {
  final VoidCallback onImport;

  const KadEmptyState({super.key, required this.onImport});

  @override
  Widget build(BuildContext context) {
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
            onPressed: onImport,
          ),
        ],
      ),
    );
  }
}
