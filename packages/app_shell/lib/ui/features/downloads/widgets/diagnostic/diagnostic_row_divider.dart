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
