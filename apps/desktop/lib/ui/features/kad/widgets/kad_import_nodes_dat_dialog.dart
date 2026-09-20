import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/kad_controller.dart';
import '../../../../l10n/l10n.dart';

/// Dialog: pick a nodes.dat path, inspect it (version, declared/valid/
/// rejected counts), then import valid contacts.
Future<void> showKadImportNodesDatDialog(
  BuildContext context,
  KadController controller,
) async {
  final pathController = TextEditingController();
  NodesDatInspectReportModel? report;
  var isInspecting = false;

  await showDialog(
    context: context,
    builder: (ctx) => StatefulBuilder(
      builder: (ctx, setDialogState) {
        return AlertDialog(
          backgroundColor: AppTheme.darkSurface,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(12),
            side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
          ),
          title: Row(
            children: [
              Icon(
                Icons.file_upload_outlined,
                color: AppTheme.primary,
                size: 20,
              ),
              SizedBox(width: 8),
              Text(context.l10n.importerUnFichierNodesDat),
            ],
          ),
          content: SizedBox(
            width: 520,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(context.l10n.specifiezLeCheminAbsoluVers,
                  style: TextStyle(fontSize: 12, color: AppColors.textSecondary),
                ),
                const SizedBox(height: 12),
                Row(
                  children: [
                    Expanded(
                      child: TextField(
                        controller: pathController,
                        style: const TextStyle(
                          fontSize: 13,
                          color: AppColors.textPrimary,
                        ),
                        decoration: InputDecoration(
                          hintText: 'C:\\...\\nodes.dat',
                          hintStyle: const TextStyle(
                            fontSize: 13,
                            color: AppColors.textFaint,
                          ),
                          isDense: true,
                          contentPadding: const EdgeInsets.symmetric(
                            horizontal: 10,
                            vertical: 10,
                          ),
                          filled: true,
                          fillColor: AppTheme.darkCard,
                          border: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(6),
                            borderSide: const BorderSide(
                              color: AppTheme.darkBorder,
                              width: 0.5,
                            ),
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    ElevatedButton(
                      onPressed: isInspecting
                          ? null
                          : () async {
                              final p = pathController.text.trim();
                              if (p.isEmpty) return;
                              setDialogState(() => isInspecting = true);
                              final rep = await controller.inspectNodesDat(p);
                              setDialogState(() {
                                report = rep;
                                isInspecting = false;
                              });
                            },
                      child: isInspecting
                          ? const SizedBox(
                              width: 14,
                              height: 14,
                              child: CircularProgressIndicator(
                                strokeWidth: 2,
                              ),
                            )
                          : Text(context.l10n.inspecter),
                    ),
                  ],
                ),
                if (report != null) ...[
                  const SizedBox(height: 16),
                  Container(
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                      color: AppTheme.darkCard,
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(
                        color: AppTheme.darkBorder,
                        width: 0.5,
                      ),
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(context.l10n.formatNodesDatVDetecte('${report!.version}'),
                          style: const TextStyle(
                            fontSize: 13,
                            fontWeight: FontWeight.bold,
                            color: AppColors.accentTeal,
                          ),
                        ),
                        const SizedBox(height: 6),
                        Text(context.l10n.contactsDeclares('${report!.declaredCount}'),
                          style: const TextStyle(
                            fontSize: 12,
                            color: AppColors.textSecondary,
                          ),
                        ),
                        Text(context.l10n.contactsValidesAdmissibles('${report!.validContactsCount}'),
                          style: const TextStyle(
                            fontSize: 12,
                            fontWeight: FontWeight.bold,
                            color: AppColors.positive,
                          ),
                        ),
                        if (report!.rejectedByPolicyCount > 0)
                          Text(context.l10n.rejetesParLaPolitiqueReseau('${report!.rejectedByPolicyCount}'),
                            style: const TextStyle(
                              fontSize: 12,
                              color: AppColors.accentOrange,
                            ),
                          ),
                        if (report!.rejectedByFilterCount > 0)
                          Text(context.l10n.rejetesParLeFiltreIp('${report!.rejectedByFilterCount}'),
                            style: const TextStyle(
                              fontSize: 12,
                              color: AppColors.danger,
                            ),
                          ),
                      ],
                    ),
                  ),
                ],
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(),
              child: Text(context.l10n.annuler,
                style: TextStyle(color: AppColors.textSoft),
              ),
            ),
            FilledButton(
              style: FilledButton.styleFrom(
                backgroundColor: AppTheme.primary,
              ),
              onPressed: report == null || report!.validContactsCount == 0
                  ? null
                  : () async {
                      final p = pathController.text.trim();
                      Navigator.of(ctx).pop();
                      await controller.importNodesDat(p);
                    },
              child: Text(context.l10n.importerLesContacts),
            ),
          ],
        );
      },
    ),
  );
}
