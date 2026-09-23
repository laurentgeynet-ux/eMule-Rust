// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../l10n/l10n.dart';

/// Bottom-of-sidebar daemon status card: glowing status dot, state label,
/// daemon/protocol version, opens diagnostics on tap.
class SidebarStatusCard extends StatelessWidget {
  final bool isCompact;
  final Color statusColor;
  final String statusLabel;
  final String? versionLine;
  final String tooltipLabel;
  final VoidCallback onTap;
  /// Pastilles réseau (eD2K / Kad) affichées sous la version — chaque
  /// entrée est un libellé + sa couleur d'état.
  final List<(String, Color)> networkBadges;
  /// Nom (ou adresse) du serveur eD2K courant — null quand déconnecté.
  final String? serverLine;

  const SidebarStatusCard({
    super.key,
    required this.isCompact,
    required this.statusColor,
    required this.statusLabel,
    required this.tooltipLabel,
    required this.onTap,
    this.versionLine,
    this.networkBadges = const [],
    this.serverLine,
  });

  @override
  Widget build(BuildContext context) {
    final dot = Container(
      width: isCompact ? 12 : 10,
      height: isCompact ? 12 : 10,
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: statusColor,
        boxShadow: [
          BoxShadow(
            color: statusColor.withValues(alpha: 0.5),
            blurRadius: 6,
            spreadRadius: 1,
          ),
        ],
      ),
    );

    return Padding(
      padding: EdgeInsets.all(isCompact ? 8 : 16),
      child: Material(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Tooltip(
            message: isCompact
                ? '${context.l10n.daemonCardTooltipTitle}: $tooltipLabel\n'
                    '${networkBadges.map((b) => b.$1).join('  ·  ')}'
                    '${serverLine != null ? '\n${context.l10n.daemonCardTooltipServer(serverLine!)}' : ''}'
                    '\n${context.l10n.daemonCardTooltipDiagnostics}'
                : '',
            child: Container(
              padding: EdgeInsets.all(isCompact ? 10 : 14),
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(8),
                border: Border.all(
                  color: AppTheme.darkBorder,
                  width: 0.5,
                ),
              ),
              child: isCompact
                  ? Center(child: dot)
                  : Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Row(
                          children: [
                            dot,
                            const SizedBox(width: 8),
                            Expanded(
                              child: Text(
                                statusLabel,
                                style: const TextStyle(
                                  fontSize: 13,
                                  fontWeight: FontWeight.w600,
                                ),
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            const Icon(
                              Icons.info_outline,
                              size: 14,
                              color: AppColors.textFaint,
                            ),
                          ],
                        ),
                        if (versionLine != null) ...[
                          const SizedBox(height: 6),
                          Text(
                            versionLine!,
                            style: const TextStyle(
                              fontSize: 11,
                              color: AppColors.textMuted,
                            ),
                          ),
                        ],
                        if (networkBadges.isNotEmpty) ...[
                          const SizedBox(height: 6),
                          Row(
                            children: [
                              for (final (i, badge) in networkBadges.indexed) ...[
                                if (i > 0) const SizedBox(width: 10),
                                Container(
                                  width: 7,
                                  height: 7,
                                  decoration: BoxDecoration(
                                    shape: BoxShape.circle,
                                    color: badge.$2,
                                    boxShadow: [
                                      BoxShadow(
                                        color: badge.$2.withValues(alpha: 0.4),
                                        blurRadius: 4,
                                      ),
                                    ],
                                  ),
                                ),
                                const SizedBox(width: 4),
                                Flexible(
                                  child: Text(
                                    badge.$1,
                                    style: TextStyle(
                                      fontSize: 11,
                                      fontWeight: FontWeight.w600,
                                      color: badge.$2,
                                    ),
                                    overflow: TextOverflow.ellipsis,
                                  ),
                                ),
                              ],
                            ],
                          ),
                        ],
                        if (serverLine != null) ...[
                          const SizedBox(height: 4),
                          Text(
                            serverLine!,
                            key: const Key('sidebar_server_line'),
                            style: const TextStyle(
                              fontSize: 11,
                              color: AppColors.textMuted,
                            ),
                            overflow: TextOverflow.ellipsis,
                          ),
                        ],
                      ],
                    ),
            ),
          ),
        ),
      ),
    );
  }
}
