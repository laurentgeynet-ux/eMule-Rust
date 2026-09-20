import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/kad_controller.dart';

/// Dialog: download a remote nodes.dat over HTTP(S) and bootstrap Kad.
/// Public URLs only — local/loopback targets are rejected by the daemon
/// (anti-SSRF).
Future<void> showKadBootstrapFromUrlDialog(
  BuildContext context,
  KadController controller,
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
  await controller.bootstrapFromUrl(url);
}
