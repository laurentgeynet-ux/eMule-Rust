// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Uppercase section label used between sidebar groups (full mode only).
class SidebarSectionLabel extends StatelessWidget {
  final String label;

  const SidebarSectionLabel({super.key, required this.label});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
      child: Text(
        label,
        style: const TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: AppColors.textFaint,
          letterSpacing: 1.0,
        ),
      ),
    );
  }
}
