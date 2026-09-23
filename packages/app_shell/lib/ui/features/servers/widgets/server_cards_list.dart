// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/servers_controller.dart';
import 'server_status_badge.dart';
import '../../../../l10n/l10n.dart';

/// Mobile layout (< 650 px): one card per server with status badge,
/// address, connect action and enable/delete icon buttons.
class ServerCardsList extends StatelessWidget {
  final ServersController controller;
  final List<Ed2kServerModel> servers;
  final bool hasServerConnectionCap;

  const ServerCardsList({
    super.key,
    required this.controller,
    required this.servers,
    required this.hasServerConnectionCap,
  });

  @override
  Widget build(BuildContext context) {
    return ListView.separated(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      itemCount: servers.length,
      separatorBuilder: (_, _) => const SizedBox(height: 10),
      itemBuilder: (context, index) {
        final server = servers[index];
        final isDisabled =
            server.filterStatus == ServerFilterStatus.disabled;
        final isAllowed =
            server.filterStatus == ServerFilterStatus.allowed && !isDisabled;

        final session = controller.session;
        final isActiveSession =
            session.serverAddress == server.address &&
            session.status != ServerConnectionStatus.disconnected;

        return Container(
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: AppTheme.darkSurface,
            borderRadius: BorderRadius.circular(8),
            border: Border.all(
              color: isActiveSession
                  ? AppColors.positive.withAlpha(100)
                  : AppTheme.darkBorder,
            ),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  ServerStatusBadge(
                    filterStatus: server.filterStatus,
                    compact: true,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      server.name.isNotEmpty ? server.name : context.l10n.serverUnnamed,
                      style: const TextStyle(
                        fontWeight: FontWeight.bold,
                        fontSize: 13.5,
                        color: AppColors.textPrimary,
                      ),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  if (server.ping > 0) ...[
                    const SizedBox(width: 6),
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 6,
                        vertical: 2,
                      ),
                      decoration: BoxDecoration(
                        color: AppColors.textPrimary.withAlpha(10),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(context.l10n.ms('${server.ping}'),
                        style: const TextStyle(
                          fontSize: 11,
                          color: AppColors.textSecondary,
                          fontWeight: FontWeight.w500,
                        ),
                      ),
                    ),
                  ],
                ],
              ),
              const SizedBox(height: 6),
              Row(
                children: [
                  const Icon(
                    Icons.lan_outlined,
                    size: 13,
                    color: AppColors.textFaint,
                  ),
                  const SizedBox(width: 5),
                  Text(
                    server.address,
                    style: const TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 11.5,
                      color: AppColors.textSecondary,
                    ),
                  ),
                  if (server.version != null &&
                      server.version!.isNotEmpty) ...[
                    const SizedBox(width: 10),
                    Text(context.l10n.v2('${server.version}'),
                      style: const TextStyle(
                        fontSize: 11,
                        color: AppColors.textFaint,
                      ),
                    ),
                  ],
                ],
              ),
              if (server.description.isNotEmpty) ...[
                const SizedBox(height: 4),
                Text(
                  server.description,
                  style: const TextStyle(
                    fontSize: 11,
                    color: AppColors.textMuted,
                  ),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ],
              const SizedBox(height: 8),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  if (hasServerConnectionCap && isAllowed) ...[
                    if (isActiveSession) ...[
                      Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 8,
                          vertical: 4,
                        ),
                        decoration: BoxDecoration(
                          color: AppColors.positive.withAlpha(25),
                          borderRadius: BorderRadius.circular(4),
                          border: Border.all(
                            color: AppColors.positive.withAlpha(80),
                          ),
                        ),
                        child: Text(
                          session.status.toDisplayString(),
                          style: const TextStyle(
                            color: AppColors.positive,
                            fontSize: 11,
                          ),
                        ),
                      ),
                    ] else ...[
                      ElevatedButton.icon(
                        icon: const Icon(Icons.power_rounded, size: 13),
                        label: Text(context.l10n.seConnecter,
                          style: TextStyle(fontSize: 11),
                        ),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: AppTheme.primary,
                          foregroundColor: AppColors.textPrimary,
                          padding: const EdgeInsets.symmetric(
                            horizontal: 10,
                            vertical: 6,
                          ),
                          visualDensity: VisualDensity.compact,
                        ),
                        onPressed:
                            (!controller.isConnecting &&
                                session.status ==
                                    ServerConnectionStatus.disconnected)
                            ? () => controller.connect(server.address)
                            : null,
                      ),
                    ],
                    const SizedBox(width: 8),
                  ],
                  IconButton(
                    icon: Icon(
                      isDisabled
                          ? Icons.play_arrow_rounded
                          : Icons.pause_rounded,
                      size: 18,
                      color: isDisabled
                          ? AppColors.positive
                          : AppColors.accentAmber,
                    ),
                    tooltip: isDisabled
                        ? context.l10n.serverEnable
                        : context.l10n.serverDisable,
                    onPressed: () =>
                        controller.setEnabled(server.address, isDisabled),
                  ),
                  IconButton(
                    icon: const Icon(
                      Icons.delete_outline,
                      size: 18,
                      color: AppColors.textMuted,
                    ),
                    tooltip: context.l10n.supprimer,
                    onPressed: () => controller.delete(server.address),
                  ),
                ],
              ),
            ],
          ),
        );
      },
    );
  }
}
