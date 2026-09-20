import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'detail_section_card.dart';

/// Diagnostics card: copies a sanitized diagnostic dump (no auth token,
/// redacted user paths) to the clipboard.
class DownloadDiagnosticsCard extends StatelessWidget {
  final DownloadItem item;
  final String Function(DownloadItem item) diagnosticBuilder;

  const DownloadDiagnosticsCard({
    super.key,
    required this.item,
    required this.diagnosticBuilder,
  });

  @override
  Widget build(BuildContext context) {
    return DetailSectionCard(
      title: 'Diagnostics & Redacted Export',
      icon: Icons.bug_report_outlined,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          const Expanded(
            child: Text(
              'Export sanitized diagnostics (zero auth token secrets, loopback 127.0.0.1 confinement verification, redacted user directories).',
              style: TextStyle(fontSize: 12, color: AppColors.textSoft),
            ),
          ),
          const SizedBox(width: 16),
          FilledButton.icon(
            icon: const Icon(Icons.copy_all_rounded, size: 16),
            label: const Text('Copy Redacted Diagnostic'),
            style: FilledButton.styleFrom(
              backgroundColor: AppTheme.darkCard,
              foregroundColor: AppColors.textPrimary,
              side: const BorderSide(color: AppTheme.darkBorder),
            ),
            onPressed: () {
              final diag = diagnosticBuilder(item);
              Clipboard.setData(ClipboardData(text: diag));
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(
                  content: Text(
                    'Download diagnostic copied to clipboard (sanitized: no secrets)',
                  ),
                  duration: Duration(seconds: 2),
                ),
              );
            },
          ),
        ],
      ),
    );
  }
}
