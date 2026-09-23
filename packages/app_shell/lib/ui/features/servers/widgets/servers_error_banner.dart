// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// Error banner for the servers section. Network-policy errors get an extra
/// hint and a shortcut to the network profile dialog.
class ServersErrorBanner extends StatelessWidget {
  final String error;
  final VoidCallback onEditProfile;

  const ServersErrorBanner({
    super.key,
    required this.error,
    required this.onEditProfile,
  });

  @override
  Widget build(BuildContext context) {
    final isPolicyError =
        error.toLowerCase().contains('policy') ||
        error.toLowerCase().contains('networkprofile') ||
        error.toLowerCase().contains('réseau') ||
        error.toLowerCase().contains('network');

    return Container(
      margin: const EdgeInsets.fromLTRB(20, 10, 20, 0),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: AppTheme.statusFailed.withAlpha(30),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.statusFailed.withAlpha(120)),
      ),
      child: Row(
        children: [
          const Icon(
            Icons.error_outline_rounded,
            color: AppTheme.statusFailed,
            size: 20,
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  error,
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textPrimary,
                    fontWeight: FontWeight.w500,
                  ),
                ),
                if (isPolicyError) ...[
                  const SizedBox(height: 2),
                  Text(context.l10n.parDefauteMuleRustEst,
                    style: TextStyle(
                      fontSize: 11,
                      color: AppColors.textSecondary,
                    ),
                  ),
                ],
              ],
            ),
          ),
          if (isPolicyError) ...[
            const SizedBox(width: 10),
            ElevatedButton(
              style: ElevatedButton.styleFrom(
                backgroundColor: AppTheme.primary,
                foregroundColor: AppColors.textPrimary,
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 6,
                ),
                minimumSize: Size.zero,
                tapTargetSize: MaterialTapTargetSize.shrinkWrap,
              ),
              onPressed: onEditProfile,
              child: Text(context.l10n.modifierLeProfil,
                style: TextStyle(fontSize: 11),
              ),
            ),
          ],
        ],
      ),
    );
  }
}
