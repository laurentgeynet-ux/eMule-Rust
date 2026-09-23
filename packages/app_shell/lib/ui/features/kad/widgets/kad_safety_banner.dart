// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Contextual banner describing the Kad transport mode (public vs confined
/// loopback lab) and the current UDP socket state.
class KadSafetyBanner extends StatelessWidget {
  final KadStatusModel status;

  const KadSafetyBanner({super.key, required this.status});

  @override
  Widget build(BuildContext context) {
    final isPublic = status.networkProfile != 'local-lab';
    return Container(
      margin: const EdgeInsets.fromLTRB(20, 12, 20, 0),
      padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
      decoration: BoxDecoration(
        color: (isPublic ? AppColors.lab : AppColors.tintPurple).withAlpha(20),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: (isPublic ? AppColors.accentTeal : AppColors.accentPurple)
              .withAlpha(80),
        ),
      ),
      child: Row(
        children: [
          Icon(
            isPublic ? Icons.public : Icons.shield_outlined,
            color: isPublic ? AppColors.accentTeal : AppColors.accentPurple,
            size: 18,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              status.isUdpActive
                  ? (isPublic
                        ? context.l10n.kadBannerPublic(
                            status.udpBindAddress ?? "0.0.0.0:39737",
                          )
                        : context.l10n.kadBannerLab(
                            status.udpBindAddress ?? "127.0.0.1:39737",
                          ))
                  : (isPublic
                        ? context.l10n.kadBannerStartPublic
                        : context.l10n.kadBannerStartLab),
              style: const TextStyle(
                fontSize: 12,
                color: AppColors.textSecondary,
                height: 1.3,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
