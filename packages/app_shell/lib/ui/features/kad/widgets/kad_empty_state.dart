// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

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
          Text(context.l10n.aucunContactKad,
            style: TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.bold,
              color: AppColors.textMuted,
            ),
          ),
          const SizedBox(height: 6),
          Text(context.l10n.importezUnFichierNodesDat,
            style: TextStyle(fontSize: 12, color: AppColors.textFaint),
          ),
          const SizedBox(height: 16),
          ElevatedButton.icon(
            icon: const Icon(Icons.file_upload_outlined, size: 16),
            label: Text(context.l10n.importerNodesDat),
            onPressed: onImport,
          ),
        ],
      ),
    );
  }
}
