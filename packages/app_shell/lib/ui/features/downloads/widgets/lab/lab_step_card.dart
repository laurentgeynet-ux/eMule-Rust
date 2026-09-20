import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Numbered step card used by the guided loopback test flow.
class LabStepCard extends StatelessWidget {
  final int stepNumber;
  final String title;
  final String description;
  final Widget content;
  final bool isCompleted;

  const LabStepCard({
    super.key,
    required this.stepNumber,
    required this.title,
    required this.description,
    required this.content,
    this.isCompleted = false,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(10),
        border: Border.all(
          color: isCompleted
              ? AppTheme.statusCompleted.withValues(alpha: 0.5)
              : AppTheme.darkBorder,
          width: 0.75,
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Container(
                width: 24,
                height: 24,
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  color: isCompleted
                      ? AppTheme.statusCompleted
                      : AppTheme.primary,
                ),
                alignment: Alignment.center,
                child: isCompleted
                    ? const Icon(Icons.check, size: 14, color: Colors.black)
                    : Text('$stepNumber',
                        style: const TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.bold,
                          color: AppColors.textPrimary,
                        ),
                      ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      title,
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                    Text(
                      description,
                      style: const TextStyle(
                        fontSize: 11,
                        color: AppColors.textMuted,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          content,
        ],
      ),
    );
  }
}
