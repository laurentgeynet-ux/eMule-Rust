import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Second tab of the IP filter dialog: token-bucket bandwidth quotas.
class RateLimitsTab extends StatelessWidget {
  final TextEditingController downloadLimitController;
  final TextEditingController uploadLimitController;
  final TextEditingController connLimitController;
  final VoidCallback onSave;

  /// Appelé sur chaque saisie utilisateur (dirty tracking — les champs
  /// modifiés ne sont pas écrasés par une resync externe).
  final ValueChanged<TextEditingController>? onChanged;

  const RateLimitsTab({
    super.key,
    required this.downloadLimitController,
    required this.uploadLimitController,
    required this.connLimitController,
    required this.onSave,
    this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(context.l10n.bandwidthTokenBucketQuotas,
            style: TextStyle(
              fontWeight: FontWeight.bold,
              color: AppColors.textPrimary,
              fontSize: 13,
            ),
          ),
          const SizedBox(height: 4),
          Text(context.l10n.limitsAreEnforcedAsynchronouslyUsing,
            style: TextStyle(fontSize: 11, color: AppColors.textSoft),
          ),
          const SizedBox(height: 4),
          Text(context.l10n.rateLimitsCapacityNote,
            style: TextStyle(fontSize: 10, color: AppColors.textFaint),
          ),
          const SizedBox(height: 16),
          _LimitField(
            controller: downloadLimitController,
            label: context.l10n.downloadLimitKbS0,
            onChanged: onChanged,
            onSubmit: onSave,
          ),
          const SizedBox(height: 12),
          _LimitField(
            controller: uploadLimitController,
            label: context.l10n.uploadLimitKbS0,
            onChanged: onChanged,
            onSubmit: onSave,
          ),
          const SizedBox(height: 12),
          _LimitField(
            controller: connLimitController,
            label: context.l10n.maxNewConnectionsMinute,
            onChanged: onChanged,
            onSubmit: onSave,
          ),
          const SizedBox(height: 20),
          Align(
            alignment: Alignment.centerRight,
            child: ElevatedButton.icon(
              onPressed: onSave,
              style: ElevatedButton.styleFrom(
                backgroundColor: AppColors.warningShade700,
                foregroundColor: Colors.black,
              ),
              icon: const Icon(Icons.save, size: 16),
              label: Text(context.l10n.saveLimits,
                style: TextStyle(fontWeight: FontWeight.bold),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _LimitField extends StatelessWidget {
  final TextEditingController controller;
  final String label;
  final ValueChanged<TextEditingController>? onChanged;
  final VoidCallback onSubmit;

  const _LimitField({
    required this.controller,
    required this.label,
    required this.onSubmit,
    this.onChanged,
  });

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: controller,
      keyboardType: TextInputType.number,
      style: const TextStyle(color: AppColors.textPrimary, fontSize: 12),
      decoration: InputDecoration(
        labelText: label,
        labelStyle: const TextStyle(
          color: AppColors.textSecondary,
          fontSize: 12,
        ),
        filled: true,
        fillColor: AppTheme.darkBackground,
      ),
      onChanged: (_) => onChanged?.call(controller),
      onSubmitted: (_) => onSubmit(),
    );
  }
}
