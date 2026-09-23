// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Carte « Réseaux » — deux interrupteurs remplaçant le sélecteur de
/// profil à 3 options. Le contrat de sécurité sous-jacent est inchangé :
/// les switches ne font que projeter le même [NetworkProfile] :
/// eD2K off → `localLab` ; eD2K seul → `publicEd2kPreview` ;
/// eD2K + Kad → `publicEd2kKadPreview`. Activer Kad implique eD2K.
class NetworkTogglesCard extends StatelessWidget {
  final NetworkProfile active;
  final ValueChanged<NetworkProfile> onSelected;

  const NetworkTogglesCard({
    super.key,
    required this.active,
    required this.onSelected,
  });

  bool get _ed2kOn => active != NetworkProfile.localLab;
  bool get _kadOn => active == NetworkProfile.publicEd2kKadPreview;

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Card(
      color: AppTheme.darkSurface,
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(12, 12, 12, 4),
        child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            l10n.networksCardTitle,
            style: const TextStyle(fontSize: 13, fontWeight: FontWeight.bold),
          ),
          SwitchListTile(
            key: const Key('switch_ed2k'),
            contentPadding: EdgeInsets.zero,
            dense: true,
            title: Text(
              l10n.networkEd2kTitle,
              style:
                  const TextStyle(fontSize: 12, color: AppColors.textPrimary),
            ),
            subtitle: Text(
              l10n.netProfileEd2kDesc,
              style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
            ),
            value: _ed2kOn,
            onChanged: (on) => onSelected(
              on
                  ? (_kadOn
                      ? NetworkProfile.publicEd2kKadPreview
                      : NetworkProfile.publicEd2kPreview)
                  : NetworkProfile.localLab,
            ),
          ),
          SwitchListTile(
            key: const Key('switch_kad'),
            contentPadding: EdgeInsets.zero,
            dense: true,
            title: Text(
              l10n.networkKadTitle,
              style:
                  const TextStyle(fontSize: 12, color: AppColors.textPrimary),
            ),
            subtitle: Text(
              l10n.netProfileKadDesc,
              style: const TextStyle(fontSize: 10, color: AppColors.textMuted),
            ),
            value: _kadOn,
            onChanged: (on) => onSelected(
              on
                  ? NetworkProfile.publicEd2kKadPreview
                  : NetworkProfile.publicEd2kPreview,
            ),
          ),
        ],
        ),
      ),
    );
  }
}
