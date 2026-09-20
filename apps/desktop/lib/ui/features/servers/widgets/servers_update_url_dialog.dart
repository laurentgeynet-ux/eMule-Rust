import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/servers_controller.dart';

/// Dialog: download a remote server.met over HTTP(S) and update the local
/// server directory. Private/loopback targets are rejected (anti-SSRF).
Future<void> showServersUpdateUrlDialog(
  BuildContext context,
  ServersController controller,
) async {
  final urlController = TextEditingController(
    text: 'http://upd.emule-security.org/server.met',
  );

  final url = await showDialog<String>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: const Row(
        children: [
          Icon(Icons.cloud_download_outlined, color: AppColors.accentTeal),
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
              style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
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
              style: TextStyle(fontSize: 11, color: AppColors.textFaint),
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
  await controller.updateFromUrl(url);
}
