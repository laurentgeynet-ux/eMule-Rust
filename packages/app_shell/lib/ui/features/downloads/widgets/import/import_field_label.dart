// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Small section label above an import form field.
class ImportFieldLabel extends StatelessWidget {
  final String label;

  const ImportFieldLabel({super.key, required this.label});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Text(
        label,
        style: const TextStyle(
          fontSize: 13,
          fontWeight: FontWeight.w600,
          color: AppColors.textPrimary,
        ),
      ),
    );
  }
}
