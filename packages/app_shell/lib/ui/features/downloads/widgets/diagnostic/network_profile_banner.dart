// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../l10n/enum_labels.dart';
import '../../../../../l10n/l10n.dart';
import 'network_profile_colors.dart';

/// Top banner: strict loopback confinement vs public network profile.
class NetworkProfileBanner extends StatelessWidget {
  final NetworkProfile profile;

  const NetworkProfileBanner({super.key, required this.profile});

  @override
  Widget build(BuildContext context) {
    final isLoopbackOnly = profile == NetworkProfile.localLab;
    final cardColor = isLoopbackOnly
        ? NetworkProfileColors.loopbackFill
        : NetworkProfileColors.exposedFill;
    final borderColor = isLoopbackOnly
        ? NetworkProfileColors.loopbackBorder
        : NetworkProfileColors.exposedBorder;
    final iconColor = isLoopbackOnly
        ? NetworkProfileColors.loopbackAccent
        : NetworkProfileColors.exposedAccent;
    final title = isLoopbackOnly
        ? context.l10n.diagLoopbackTitle
        : context.l10n.diagNetworkProfile(profile.labelL10n(context.l10n));
    final subtitle = isLoopbackOnly
        ? context.l10n.diagLoopbackSubtitle
        : context.l10n.diagPublicSubtitle;

    return Container(
      padding: const EdgeInsets.all(AppSpacing.md),
      decoration: BoxDecoration(
        color: cardColor,
        borderRadius: AppRadius.mdAll,
        border: Border.all(color: borderColor, width: 0.5),
      ),
      child: Row(
        children: [
          Icon(
            isLoopbackOnly ? Icons.shield_outlined : Icons.public_rounded,
            color: iconColor,
            size: 22,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: TextStyle(
                    color: iconColor,
                    fontWeight: FontWeight.bold,
                    fontSize: 13,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: const TextStyle(
                    color: AppColors.textSecondary,
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
