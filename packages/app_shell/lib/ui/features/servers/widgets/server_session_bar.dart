// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/servers_controller.dart';
import 'servers_format.dart';
import '../../../../l10n/l10n.dart';

/// Live eD2K server session bar: connection status, HighID/LowID badge,
/// server name, user/file metrics, welcome message, disconnect action.
class ServerSessionBar extends StatelessWidget {
  final ServersController controller;

  const ServerSessionBar({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    final session = controller.session;
    final status = session.status;
    final isConnected = status == ServerConnectionStatus.connected;
    final isConnecting =
        status == ServerConnectionStatus.connecting || controller.isConnecting;
    final isDisconnected = status == ServerConnectionStatus.disconnected;

    Color barBg;
    Color barBorder;
    Color statusColor;
    if (isConnected) {
      barBg = AppColors.tintGreen.withAlpha(20);
      barBorder = AppColors.positive.withAlpha(80);
      statusColor = AppColors.positive;
    } else if (isConnecting) {
      barBg = AppColors.warning.withAlpha(20);
      barBorder = AppColors.accentAmber.withAlpha(80);
      statusColor = AppColors.accentAmber;
    } else {
      barBg = AppColors.textPrimary.withAlpha(8);
      barBorder = AppTheme.darkBorder;
      statusColor = AppColors.textFaint;
    }

    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: barBg,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: barBorder),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isNarrow = constraints.maxWidth < 750;

          final statusIndicator = Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 8,
                height: 8,
                margin: const EdgeInsets.only(right: 8),
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: statusColor,
                ),
              ),
              Text(
                status.toDisplayString(),
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: statusColor,
                ),
              ),
              if (isConnected) ...[
                const SizedBox(width: 8),
                Container(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 7,
                    vertical: 2,
                  ),
                  decoration: BoxDecoration(
                    color: session.isHighId
                        ? AppColors.positive.withAlpha(40)
                        : AppColors.accentOrange.withAlpha(40),
                    borderRadius: BorderRadius.circular(4),
                    border: Border.all(
                      color: session.isHighId
                          ? AppColors.positive.withAlpha(100)
                          : AppColors.accentOrange.withAlpha(100),
                    ),
                  ),
                  child: Text(
                    session.isHighId ? 'HighID' : 'LowID',
                    style: TextStyle(
                      fontSize: 11,
                      fontWeight: FontWeight.bold,
                      color: session.isHighId
                          ? AppColors.positive
                          : AppColors.accentOrange,
                    ),
                  ),
                ),
              ],
            ],
          );

          final serverTitle =
              session.serverName != null && session.serverName!.isNotEmpty
              ? Text(
                  session.serverName!,
                  style: const TextStyle(
                    fontSize: 13,
                    color: AppColors.textSecondary,
                    fontWeight: FontWeight.w500,
                  ),
                  overflow: TextOverflow.ellipsis,
                )
              : !isDisconnected
              ? Text(
                  session.serverAddress ?? '',
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textMuted,
                    fontFamily: 'monospace',
                  ),
                  overflow: TextOverflow.ellipsis,
                )
              : Text(context.l10n.aucunServeurConnecte,
                  style: TextStyle(fontSize: 12, color: AppColors.textFaint),
                );

          final userMetrics = isConnected && session.currentUsers != null
              ? Text(context.l10n.utilisateurs(
                  formatCompactCount(session.currentUsers!),
                  formatCompactCount(session.currentFiles ?? 0)),
                  style: const TextStyle(
                    fontSize: 11,
                    color: AppColors.textMuted,
                  ),
                )
              : null;

          final actionBtn = !isDisconnected
              ? TextButton.icon(
                  icon: Icon(
                    isConnecting
                        ? Icons.close_rounded
                        : Icons.power_settings_new,
                    size: 16,
                    color: AppColors.danger,
                  ),
                  label: Text(
                    isConnecting
                        ? context.l10n.annuler
                        : context.l10n.serverDisconnectBtn,
                    style: const TextStyle(
                      color: AppColors.danger,
                      fontSize: 12,
                    ),
                  ),
                  style: TextButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 6,
                    ),
                    minimumSize: Size.zero,
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                  ),
                  onPressed: () => controller.disconnect(),
                )
              : null;

          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              if (isNarrow) ...[
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [Expanded(child: statusIndicator), ?actionBtn],
                ),
                const SizedBox(height: 6),
                Row(children: [Expanded(child: serverTitle)]),
                if (userMetrics != null) ...[
                  const SizedBox(height: 4),
                  userMetrics,
                ],
              ] else ...[
                Row(
                  children: [
                    statusIndicator,
                    const SizedBox(width: 10),
                    Expanded(child: serverTitle),
                    if (userMetrics != null) ...[
                      const SizedBox(width: 12),
                      userMetrics,
                    ],
                    if (actionBtn != null) ...[
                      const SizedBox(width: 12),
                      actionBtn,
                    ],
                  ],
                ),
              ],
              if (isConnected &&
                  session.welcomeMessage != null &&
                  session.welcomeMessage!.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    session.welcomeMessage!,
                    maxLines: 3,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(
                      fontSize: 11.5,
                      color: AppColors.textMuted,
                      height: 1.4,
                    ),
                  ),
                ),
            ],
          );
        },
      ),
    );
  }
}
