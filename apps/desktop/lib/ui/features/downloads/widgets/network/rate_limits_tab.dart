import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Second tab of the IP filter dialog: token-bucket bandwidth quotas.
class RateLimitsTab extends StatelessWidget {
  final TextEditingController downloadLimitController;
  final TextEditingController uploadLimitController;
  final TextEditingController connLimitController;
  final VoidCallback onSave;

  const RateLimitsTab({
    super.key,
    required this.downloadLimitController,
    required this.uploadLimitController,
    required this.connLimitController,
    required this.onSave,
  });

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Text(
            'Bandwidth & Token Bucket Quotas',
            style: TextStyle(
              fontWeight: FontWeight.bold,
              color: AppColors.textPrimary,
              fontSize: 13,
            ),
          ),
          const SizedBox(height: 4),
          const Text(
            'Limits are enforced asynchronously using Token Buckets. 0 means unlimited.',
            style: TextStyle(fontSize: 11, color: AppColors.textSoft),
          ),
          const SizedBox(height: 16),
          _LimitField(
            controller: downloadLimitController,
            label: 'Download Limit (KB/s) [0 = unlimited]',
          ),
          const SizedBox(height: 12),
          _LimitField(
            controller: uploadLimitController,
            label: 'Upload Limit (KB/s) [0 = unlimited]',
          ),
          const SizedBox(height: 12),
          _LimitField(
            controller: connLimitController,
            label: 'Max New Connections / Minute',
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
              label: const Text(
                'Save Limits',
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

  const _LimitField({required this.controller, required this.label});

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
    );
  }
}
