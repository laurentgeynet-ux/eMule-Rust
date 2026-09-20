import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Section 2: history retention policy dropdown.
class SettingsRetentionSection extends StatelessWidget {
  final int retentionDays;
  final ValueChanged<int> onChanged;

  const SettingsRetentionSection({
    super.key,
    required this.retentionDays,
    required this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            color: AppTheme.primary.withValues(alpha: 0.15),
            borderRadius: BorderRadius.circular(6),
          ),
          child: const Icon(
            Icons.history_rounded,
            size: 18,
            color: AppTheme.primary,
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'History Retention Policy',
                style: TextStyle(
                  fontSize: 14,
                  fontWeight: FontWeight.bold,
                  color: AppColors.textPrimary,
                ),
              ),
              const SizedBox(height: 4),
              const Text(
                'Historical transition events older than the retention period are automatically pruned on daemon startup.',
                style: TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                  height: 1.3,
                ),
              ),
              const SizedBox(height: 12),
              Wrap(
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: 8,
                runSpacing: 8,
                children: [
                  const Text(
                    'Keep history for: ',
                    style: TextStyle(
                      fontSize: 13,
                      color: AppColors.textSecondary,
                    ),
                  ),
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 10),
                    decoration: BoxDecoration(
                      color: AppTheme.darkCard,
                      borderRadius: BorderRadius.circular(6),
                      border: Border.all(
                        color: AppTheme.darkBorder,
                        width: 0.5,
                      ),
                    ),
                    child: DropdownButtonHideUnderline(
                      child: DropdownButton<int>(
                        value: retentionDays,
                        dropdownColor: AppTheme.darkCard,
                        style: const TextStyle(
                          fontSize: 13,
                          color: AppColors.textPrimary,
                        ),
                        items: const [
                          DropdownMenuItem(
                            value: 7,
                            child: Text('7 Days'),
                          ),
                          DropdownMenuItem(
                            value: 30,
                            child: Text('30 Days (Default)'),
                          ),
                          DropdownMenuItem(
                            value: 90,
                            child: Text('90 Days'),
                          ),
                          DropdownMenuItem(
                            value: 0,
                            child:
                                Text('Indefinitely (No pruning)'),
                          ),
                        ],
                        onChanged: (days) {
                          if (days != null) onChanged(days);
                        },
                      ),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}
