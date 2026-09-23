// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/servers_controller.dart';
import '../../../../core/file_picker_helper.dart';
import '../../../../l10n/l10n.dart';

/// Flow: pick a server.met path → inspect (report of allowed/blocked
/// servers) → confirm import.
Future<void> showServerMetImportDialog(
  BuildContext context,
  ServersController controller,
) async {
  final pathController = TextEditingController();

  final path = await showDialog<String>(
    context: context,
    builder: (ctx) => AlertDialog(
      title: Row(
        children: [
          Icon(Icons.file_upload_outlined, color: AppTheme.primary),
          SizedBox(width: 8),
          Text(context.l10n.importerUnFichierServerMet),
        ],
      ),
      content: SizedBox(
        width: 500,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(context.l10n.indiquezLeCheminAbsoluVers,
              style: TextStyle(fontSize: 13, color: AppColors.textSecondary),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: pathController,
              autofocus: true,
              decoration: InputDecoration(
                labelText: context.l10n.cheminDuFichierServerMet,
                hintText: 'C:\\path\\to\\server.met ou /path/to/server.met',
                border: OutlineInputBorder(),
                prefixIcon: Icon(Icons.description_outlined, size: 18),
                suffixIcon: IconButton(
                  icon: const Icon(Icons.folder_open_rounded, size: 18),
                  tooltip: context.l10n.browse,
                  onPressed: () async {
                    final title = context.l10n.importerUnFichierServerMet;
                    final picked = await pickFile(
                      description: title,
                      allowedExtensions: const ['met'],
                    );
                    if (picked != null) pathController.text = picked;
                  },
                ),
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(null),
          child: Text(context.l10n.annuler),
        ),
        ElevatedButton.icon(
          icon: const Icon(Icons.preview_rounded, size: 16),
          label: Text(context.l10n.inspecter),
          onPressed: () {
            final text = pathController.text.trim();
            if (text.isNotEmpty) {
              Navigator.of(ctx).pop(text);
            }
          },
        ),
      ],
    ),
  );

  if (path == null || path.isEmpty) return;

  if (!context.mounted) return;
  final report = await controller.inspectServerMet(path);
  if (report == null) return;

  if (!context.mounted) return;
  final shouldImport = await showDialog<bool>(
    context: context,
    builder: (ctx) => _ServerMetInspectDialog(report: report),
  );

  if (shouldImport == true) {
    await controller.importServerMet(path);
  }
}

class _ServerMetInspectDialog extends StatelessWidget {
  final ServerInspectReport report;

  const _ServerMetInspectDialog({required this.report});

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Row(
        children: [
          Icon(Icons.preview_rounded, color: AppTheme.primary),
          SizedBox(width: 8),
          Text(context.l10n.apercuDuFichierServerMet),
        ],
      ),
      content: SizedBox(
        width: 600,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(context.l10n.fichier(report.filePath),
              style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
            ),
            const SizedBox(height: 14),
            Row(
              children: [
                _SummaryStat(
                  label: context.l10n.total,
                  value: '${report.totalCount}',
                  color: AppColors.textPrimary,
                ),
                const SizedBox(width: 12),
                _SummaryStat(
                  label: context.l10n.autorises,
                  value: '${report.allowedCount}',
                  color: AppColors.positive,
                ),
                const SizedBox(width: 12),
                _SummaryStat(
                  label: context.l10n.bloquesFiltreIp,
                  value: '${report.blockedCount}',
                  color: AppColors.danger,
                ),
              ],
            ),
            const SizedBox(height: 16),
            Text(context.l10n.lesServeursMarquesEnRouge,
              style: TextStyle(fontSize: 12, color: AppColors.textSecondary),
            ),
            const SizedBox(height: 12),
            Container(
              height: 200,
              decoration: BoxDecoration(
                border: Border.all(color: AppTheme.darkBorder),
                borderRadius: BorderRadius.circular(6),
              ),
              child: ListView.separated(
                itemCount: report.servers.length,
                separatorBuilder: (_, _) =>
                    const Divider(height: 1, color: AppTheme.darkBorder),
                itemBuilder: (ctx, idx) {
                  final s = report.servers[idx];
                  final isBlocked =
                      s.filterStatus == ServerFilterStatus.blockedByFilter;
                  return ListTile(
                    dense: true,
                    title: Text(
                      s.name,
                      style: const TextStyle(fontWeight: FontWeight.w600),
                    ),
                    subtitle: Text(
                      s.address,
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 11,
                      ),
                    ),
                    trailing: isBlocked
                        ? Text(context.l10n.bloque,
                            style: TextStyle(
                              color: AppColors.danger,
                              fontWeight: FontWeight.bold,
                              fontSize: 11,
                            ),
                          )
                        : Text(context.l10n.autorise,
                            style: TextStyle(
                              color: AppColors.positive,
                              fontSize: 11,
                            ),
                          ),
                  );
                },
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(context.l10n.annuler),
        ),
        ElevatedButton.icon(
          icon: const Icon(Icons.download_done_rounded, size: 16),
          label: Text(context.l10n.confirmerImportation),
          onPressed: () => Navigator.of(context).pop(true),
        ),
      ],
    );
  }
}

class _SummaryStat extends StatelessWidget {
  final String label;
  final String value;
  final Color color;

  const _SummaryStat({
    required this.label,
    required this.value,
    required this.color,
  });

  @override
  Widget build(BuildContext context) {
    return Expanded(
      child: Container(
        padding: const EdgeInsets.symmetric(vertical: 8, horizontal: 10),
        decoration: BoxDecoration(
          color: color.withAlpha(20),
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: color.withAlpha(60)),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(label, style: TextStyle(fontSize: 10.5, color: color)),
            const SizedBox(height: 2),
            Text(
              value,
              style: TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.bold,
                color: color,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
