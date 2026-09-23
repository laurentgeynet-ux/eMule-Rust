// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import 'lab_step_card.dart';
import '../../../../../l10n/l10n.dart';

/// Step 3: start/stop the loopback eD2K peer serving the test fixtures.
class LabStepPeer extends StatelessWidget {
  final String? activePeer;
  final int fixtureCount;
  final bool isManaging;
  final VoidCallback onToggle;

  const LabStepPeer({
    super.key,
    required this.activePeer,
    required this.fixtureCount,
    required this.isManaging,
    required this.onToggle,
  });

  @override
  Widget build(BuildContext context) {
    final isRunning = activePeer != null;
    return LabStepCard(
      stepNumber: 3,
      title: context.l10n.loopbackTestPeer1270,
      description: context.l10n.labStep3Desc,
      isCompleted: isRunning,
      content: Wrap(
        spacing: 16,
        runSpacing: 8,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          FilledButton.icon(
            style: FilledButton.styleFrom(
              backgroundColor:
                  isRunning ? AppTheme.statusPaused : AppTheme.secondary,
            ),
            icon: isManaging
                ? const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: AppColors.textPrimary,
                    ),
                  )
                : Icon(isRunning ? Icons.stop : Icons.play_arrow, size: 16),
            label: Text(
              isRunning
                  ? context.l10n.labStopPeer
                  : context.l10n.labStartPeer,
            ),
            onPressed: isManaging ? null : onToggle,
          ),
          Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Container(
                width: 10,
                height: 10,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: isRunning
                      ? AppTheme.statusCompleted
                      : AppColors.fillMedium,
                ),
              ),
              const SizedBox(width: 8),
              Text(
                isRunning
                    ? context.l10n.labPeerActive(activePeer!, fixtureCount)
                    : context.l10n.labPeerOffline,
                style: TextStyle(
                  fontSize: 12,
                  fontWeight:
                      isRunning ? FontWeight.bold : FontWeight.normal,
                  color: isRunning
                      ? AppColors.textPrimary
                      : AppColors.textFaint,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
