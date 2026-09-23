// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../l10n/l10n.dart';

/// Sidebar brand block: app icon + name/tagline (hidden in compact mode).
class SidebarBrandHeader extends StatelessWidget {
  final bool isCompact;

  const SidebarBrandHeader({super.key, required this.isCompact});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: EdgeInsets.symmetric(
        horizontal: isCompact ? 10 : 20,
        vertical: 14,
      ),
      decoration: const BoxDecoration(
        border: Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.5),
        ),
      ),
      child: Row(
        mainAxisAlignment: isCompact
            ? MainAxisAlignment.center
            : MainAxisAlignment.start,
        children: [
          Tooltip(
            message: '${context.l10n.appName} - ${context.l10n.appTagline}',
            child: ClipRRect(
              borderRadius: BorderRadius.circular(10),
              child: Image.asset(
                'assets/brand_icon.png',
                package: 'app_shell',
                width: 38,
                height: 38,
                fit: BoxFit.cover,
              ),
            ),
          ),
          if (!isCompact) ...[
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    context.l10n.appName,
                    style: const TextStyle(
                      fontSize: 18,
                      fontWeight: FontWeight.bold,
                      letterSpacing: 0.5,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                  Text(
                    context.l10n.appTagline,
                    style: const TextStyle(
                      fontSize: 12,
                      color: AppColors.textMuted,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
              ),
            ),
          ],
        ],
      ),
    );
  }
}
