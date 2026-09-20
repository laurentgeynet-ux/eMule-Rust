import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'change_destination_dialog.dart';
import 'detail_copy_row.dart';
import 'detail_section_card.dart';

/// Storage card: confined temp .part path, final destination and the
/// "Change Destination" action.
class DownloadPathsCard extends StatelessWidget {
  final DownloadItem item;
  final Future<bool> Function(String newDirectory) onChangeDestination;

  const DownloadPathsCard({
    super.key,
    required this.item,
    required this.onChangeDestination,
  });

  @override
  Widget build(BuildContext context) {
    return DetailSectionCard(
      title: 'Storage & Target Destination',
      icon: Icons.folder_open_rounded,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          DetailCopyRow(
            label: 'Temporary Part Path (Confined)',
            value: item.tempPath ?? 'data/temp/${item.id}.part',
            monospace: true,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          Row(
            children: [
              Expanded(
                child: DetailCopyRow(
                  label: 'Final Target Destination',
                  value: item.targetPath ?? 'Pending destination...',
                  monospace: true,
                ),
              ),
              const SizedBox(width: 12),
              OutlinedButton.icon(
                icon: const Icon(Icons.edit_location_alt_outlined, size: 16),
                label: const Text('Change Destination'),
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppTheme.primary,
                  side: const BorderSide(color: AppTheme.primary),
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 8,
                  ),
                ),
                onPressed: () => showChangeDestinationDialog(
                  context,
                  item,
                  onSave: onChangeDestination,
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          const Text(
            'Note: Changing destination redirects where the completed file is assembled. '
            'Ongoing .part chunks remain safely confined to data/temp/ until final verification.',
            style: TextStyle(
              fontSize: 11,
              color: AppColors.textFaint,
              fontStyle: FontStyle.italic,
            ),
          ),
        ],
      ),
    );
  }
}
