// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../l10n/l10n.dart';

/// Status badge for a peer source row (allowed / filtered / rejected /
/// connected / tried / failed / candidate).
class PeerSourceStatusBadge extends StatelessWidget {
  final PeerSourceStatus status;

  const PeerSourceStatusBadge({super.key, required this.status});

  @override
  Widget build(BuildContext context) {
    final (bg, fg, text) = switch (status) {
      PeerSourceStatus.allowed => (
        AppColors.positive.withValues(alpha: 0.15),
        AppColors.positive,
        context.l10n.peerSrcAllowed,
      ),
      PeerSourceStatus.filtered => (
        AppColors.danger.withValues(alpha: 0.15),
        AppColors.danger,
        context.l10n.peerSrcFiltered,
      ),
      PeerSourceStatus.rejected => (
        AppColors.accentOrange.withValues(alpha: 0.15),
        AppColors.accentOrange,
        context.l10n.peerSrcRejected,
      ),
      PeerSourceStatus.connected => (
        AppColors.accentTeal.withValues(alpha: 0.15),
        AppColors.accentTeal,
        context.l10n.connConnected,
      ),
      PeerSourceStatus.tried => (
        AppColors.accentBlue.withValues(alpha: 0.15),
        AppColors.accentBlue,
        context.l10n.peerSrcTried,
      ),
      PeerSourceStatus.failed => (
        AppColors.tintRed.withValues(alpha: 0.15),
        AppColors.tintRed,
        context.l10n.peerSrcFailed,
      ),
      PeerSourceStatus.candidate => (
        AppColors.fillLight,
        AppColors.textSecondary,
        context.l10n.peerSrcCandidate,
      ),
    };

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 1),
      decoration: BoxDecoration(
        color: bg,
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        text,
        style: TextStyle(fontSize: 9, fontWeight: FontWeight.bold, color: fg),
      ),
    );
  }
}
