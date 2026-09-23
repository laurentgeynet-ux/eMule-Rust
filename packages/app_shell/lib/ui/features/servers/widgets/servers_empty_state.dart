// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

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
          Text(context.l10n.aucunServeurEd2kEnregistre,
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
              color: AppColors.textSecondary,
            ),
          ),
          const SizedBox(height: 8),
          Text(context.l10n.importezUnFichierServerMet,
            style: TextStyle(fontSize: 13, color: AppColors.textFaint),
          ),
          const SizedBox(height: 20),
          ElevatedButton.icon(
            icon: const Icon(Icons.file_upload_outlined),
            label: Text(context.l10n.selectionnerServerMet),
            onPressed: onImport,
          ),
        ],
      ),
    );
  }
}
