import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import 'change_destination_dialog.dart';
import 'detail_copy_row.dart';
import 'detail_section_card.dart';
import '../../../../../l10n/l10n.dart';

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
      title: context.l10n.storageTargetDestination,
      icon: Icons.folder_open_rounded,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          DetailCopyRow(
            label: context.l10n.temporaryPartPathConfined,
            value: item.tempPath ?? 'data/temp/${item.id}.part',
            monospace: true,
          ),
          const Divider(color: AppTheme.darkBorder, height: 16),
          Row(
            children: [
              Expanded(
                child: DetailCopyRow(
                  label: context.l10n.finalTargetDestination,
                  value: item.targetPath ?? context.l10n.pendingDestination,
                  monospace: true,
                ),
              ),
              const SizedBox(width: 12),
              OutlinedButton.icon(
                icon: const Icon(Icons.edit_location_alt_outlined, size: 16),
                label: Text(context.l10n.changeDestination),
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
          Text(context.l10n.noteChangingDestinationRedirectsWhere,
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
