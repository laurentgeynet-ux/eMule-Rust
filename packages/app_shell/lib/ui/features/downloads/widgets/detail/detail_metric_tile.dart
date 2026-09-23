import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Compact `label / value` metric tile used in the transfer metrics card.
class DetailMetricTile extends StatelessWidget {
  final String label;
  final String value;

  const DetailMetricTile({
    super.key,
    required this.label,
    required this.value,
  });

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: const TextStyle(fontSize: 11, color: AppColors.textFaint),
        ),
        const SizedBox(height: 3),
        Text(
          value,
          style: const TextStyle(
            fontSize: 13,
            fontWeight: FontWeight.bold,
            color: AppColors.textPrimary,
          ),
        ),
      ],
    );
  }
}
