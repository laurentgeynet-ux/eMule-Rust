import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Uppercase-ish section heading inside the diagnostics dialog.
class DiagnosticSectionTitle extends StatelessWidget {
  final String title;

  const DiagnosticSectionTitle({super.key, required this.title});

  @override
  Widget build(BuildContext context) {
    return Text(
      title,
      style: const TextStyle(
        fontSize: 13,
        fontWeight: FontWeight.w600,
        color: AppColors.textSecondary,
        letterSpacing: 0.5,
      ),
    );
  }
}
