// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Chip for a daemon capability name — lab capabilities are highlighted.
class CapabilityChip extends StatelessWidget {
  final String capability;

  const CapabilityChip({super.key, required this.capability});

  @override
  Widget build(BuildContext context) {
    final isLab = capability == 'lab-tools';
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: isLab
            ? AppColors.lab.withValues(alpha: 0.2)
            : AppColors.fillSubtle,
        borderRadius: BorderRadius.circular(4),
        border: Border.all(
          color: isLab ? AppColors.lab : AppColors.fillMedium,
          width: 0.5,
        ),
      ),
      child: Text(
        capability,
        style: TextStyle(
          fontSize: 10,
          fontFamily: 'monospace',
          color: isLab ? AppColors.accentTeal : AppColors.textSecondary,
          fontWeight: isLab ? FontWeight.bold : FontWeight.normal,
        ),
      ),
    );
  }
}
