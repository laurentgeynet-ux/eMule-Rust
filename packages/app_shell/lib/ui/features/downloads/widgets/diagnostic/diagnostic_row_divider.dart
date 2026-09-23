// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Divider used between diagnostic info rows.
class DiagnosticRowDivider extends StatelessWidget {
  const DiagnosticRowDivider({super.key});

  @override
  Widget build(BuildContext context) {
    return const Divider(height: 16, color: AppTheme.darkBorder);
  }
}
