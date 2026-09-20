import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Shield notice explaining the import is staged + paused and never
/// contacts peers.
class ImportSafetyNotice extends StatelessWidget {
  const ImportSafetyNotice({super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppTheme.primary.withValues(alpha: 0.15),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: AppTheme.primary.withValues(alpha: 0.3),
        ),
      ),
      child: const Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(
            Icons.shield_outlined,
            color: AppTheme.primary,
            size: 20,
          ),
          SizedBox(width: 10),
          Expanded(
            child: Text(
              'The download will be staged into data/temp/ and initialized in Paused state. L’import ne contacte aucun peer et le téléchargement restera en pause.',
              style: TextStyle(
                fontSize: 12,
                color: AppColors.textPrimary,
                height: 1.3,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
