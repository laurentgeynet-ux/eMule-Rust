// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/kad_controller.dart';
import 'kad_stat_badge.dart';
import '../../../../l10n/l10n.dart';

/// Local Kad identity + UDP socket control + metrics (packets, contacts,
/// buckets, profile).
class KadStatusCard extends StatelessWidget {
  final KadController controller;
  final KadStatusModel status;

  const KadStatusCard({
    super.key,
    required this.controller,
    required this.status,
  });

  @override
  Widget build(BuildContext context) {
    final isPublic = status.networkProfile != 'local-lab';
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(
                Icons.fingerprint_rounded,
                size: 16,
                color: AppColors.textSecondary,
              ),
              const SizedBox(width: 6),
              Text(context.l10n.idKadLocal,
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.bold,
                  color: AppColors.textSecondary,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: SelectableText(
                  status.localId.isNotEmpty
                      ? status.localId
                      : context.l10n.kadIdGenerated,
                  style: const TextStyle(
                    fontSize: 12,
                    fontFamily: 'monospace',
                    color: AppColors.accentTeal,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              if (status.isUdpActive)
                OutlinedButton.icon(
                  icon: const Icon(
                    Icons.stop_circle_outlined,
                    size: 14,
                    color: AppColors.danger,
                  ),
                  label: Text(context.l10n.arreterUdp,
                    style: TextStyle(fontSize: 11, color: AppColors.danger),
                  ),
                  style: OutlinedButton.styleFrom(
                    side: const BorderSide(color: AppColors.danger, width: 0.5),
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 4,
                    ),
                  ),
                  onPressed: controller.stopUdp,
                )
              else
                FilledButton.icon(
                  icon: const Icon(Icons.play_circle_outline, size: 14),
                  label: Text(
                    isPublic
                        ? context.l10n.kadStartUdpPublic
                        : context.l10n.kadStartUdpLab,
                    style: const TextStyle(fontSize: 11),
                  ),
                  style: FilledButton.styleFrom(
                    backgroundColor: isPublic
                        ? AppTheme.primary
                        : AppColors.accentPurple,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 4,
                    ),
                  ),
                  onPressed: () => controller.startUdp(
                    bindAddress: isPublic ? '0.0.0.0:39737' : '127.0.0.1:39737',
                  ),
                ),
            ],
          ),
          const SizedBox(height: 10),
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: [
                KadStatBadge(
                  label: context.l10n.socketUdp,
                  value: status.isUdpActive
                      ? context.l10n.kadUdpActive(
                          status.udpBindAddress ??
                              (isPublic
                                  ? "0.0.0.0:39737"
                                  : "127.0.0.1:39737"),
                        )
                      : context.l10n.kadUdpInactive,
                  color: status.isUdpActive
                      ? AppColors.positive
                      : AppColors.accentOrange,
                ),
                const SizedBox(width: 12),
                KadStatBadge(
                  label: context.l10n.paquetsTxRx,
                  value: '${status.packetsSent} / ${status.packetsReceived}',
                  color: AppColors.accentCyan,
                ),
                const SizedBox(width: 12),
                KadStatBadge(
                  label: context.l10n.contactsActifs,
                  value: '${status.totalContacts}',
                  color: AppColors.accentTeal,
                ),
                const SizedBox(width: 12),
                KadStatBadge(
                  label: context.l10n.bucketsActifs,
                  value: '${status.activeBuckets} / 128',
                  color: AppColors.positive,
                ),
                const SizedBox(width: 12),
                KadStatBadge(
                  label: context.l10n.contactsEnBase,
                  value: '${status.dbContactsCount}',
                  color: AppColors.textSecondary,
                ),
                const SizedBox(width: 12),
                KadStatBadge(
                  label: context.l10n.profil,
                  value: status.networkProfile,
                  color: AppColors.accentPurple,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
