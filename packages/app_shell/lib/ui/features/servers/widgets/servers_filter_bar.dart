// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// Server list filter field + destructive "clear all" action (hidden when
/// the list is empty).
class ServersFilterBar extends StatelessWidget {
  final ValueChanged<String> onQueryChanged;
  final bool hasServers;
  final VoidCallback onClear;

  const ServersFilterBar({
    super.key,
    required this.onQueryChanged,
    required this.hasServers,
    required this.onClear,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 10),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              decoration: InputDecoration(
                hintText: context.l10n.rechercherUnServeurNomIp,
                prefixIcon: const Icon(Icons.search, size: 18),
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 12,
                  vertical: 8,
                ),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(8),
                  borderSide: const BorderSide(color: AppTheme.darkBorder),
                ),
                isDense: true,
              ),
              onChanged: onQueryChanged,
            ),
          ),
          const SizedBox(width: 16),
          if (hasServers)
            TextButton.icon(
              icon: const Icon(
                Icons.delete_sweep_outlined,
                color: AppColors.danger,
                size: 16,
              ),
              label: Text(context.l10n.toutEffacer,
                style: TextStyle(color: AppColors.danger),
              ),
              onPressed: onClear,
            ),
        ],
      ),
    );
  }
}
