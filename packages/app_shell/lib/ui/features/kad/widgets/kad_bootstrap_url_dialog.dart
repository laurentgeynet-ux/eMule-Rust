// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/kad_controller.dart';
import '../../../../l10n/l10n.dart';

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
      title: Row(
        children: [
          Icon(
            Icons.cloud_download_outlined,
            color: AppColors.accentTeal,
            size: 20,
          ),
          SizedBox(width: 8),
          Text(context.l10n.amorcerKadDepuisUneUrl),
        ],
      ),
      content: SizedBox(
        width: 520,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(context.l10n.telechargezUnFichierNodesDat,
              style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: urlController,
              autofocus: true,
              style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
              decoration: InputDecoration(
                labelText: context.l10n.urlDuNodesDat,
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
            Text(context.l10n.noteSeulsLesProtocolesHttp,
              style: TextStyle(fontSize: 11, color: AppColors.textFaint),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(null),
          child: Text(context.l10n.annuler,
            style: TextStyle(color: AppColors.textSoft),
          ),
        ),
        FilledButton.icon(
          icon: const Icon(Icons.download_rounded, size: 16),
          label: Text(context.l10n.telechargerEtAmorcer),
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
