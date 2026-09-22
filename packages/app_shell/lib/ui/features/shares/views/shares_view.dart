import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/providers.dart';
import '../../../../l10n/l10n.dart';
import '../../downloads/widgets/row/rename_file_dialog.dart';

/// Shared files view (`/shares`): every file served to remote peers with
/// its eMule upload priority (powershare) selector and enable switch.
/// Mirrors the old eMule "Shared Files" tab.
class SharesView extends ConsumerStatefulWidget {
  const SharesView({super.key});

  @override
  ConsumerState<SharesView> createState() => _SharesViewState();
}

class _SharesViewState extends ConsumerState<SharesView> {
  bool _rescanning = false;

  /// Re-scans the Incoming directory on the daemon (eMule "reload shared
  /// files" semantics), then refreshes the list.
  Future<void> _rescan() async {
    setState(() => _rescanning = true);
    try {
      await ref.read(sharesControllerProvider).rescan();
    } catch (_) {
      // Non-fatal : la liste ne change pas, le prochain rescan reste possible.
    }
    if (mounted) setState(() => _rescanning = false);
  }

  static const _priorityIds = [
    'verylow',
    'low',
    'normal',
    'high',
    'veryhigh',
    'powershare',
  ];

  static String _priorityLabelOf(BuildContext context, String id) =>
      switch (id) {
        'verylow' => context.l10n.sharePrioVeryLow,
        'low' => context.l10n.low,
        'normal' => context.l10n.normal,
        'high' => context.l10n.high,
        'veryhigh' => context.l10n.sharePrioVeryHigh,
        _ => 'PowerShare',
      };

  @override
  Widget build(BuildContext context) {
    final shares = ref.watch(sharesControllerProvider);
    final files = shares.files;
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              Text(
                context.l10n.sharesTitle,
                style: const TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.bold,
                  color: AppColors.textPrimary,
                ),
              ),
              const SizedBox(width: 12),
              Text(
                context.l10n.sharesFileCount(files.length),
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                ),
              ),
              const Spacer(),
              IconButton(
                icon: _rescanning
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.refresh, size: 18),
                tooltip: context.l10n.sharesRescanTooltip,
                onPressed: _rescanning ? null : _rescan,
              ),
            ],
          ),
          const SizedBox(height: 8),
          Expanded(
            child: shares.loading
                ? const Center(child: CircularProgressIndicator())
                : files.isEmpty
                    ? Center(
                        child: Text(
                          context.l10n.sharesEmptyState,
                          textAlign: TextAlign.center,
                          style: TextStyle(
                            fontSize: 12,
                            color: AppColors.textMuted,
                          ),
                        ),
                      )
                    : ListView.separated(
                        itemCount: files.length,
                        separatorBuilder: (_, __) => const Divider(
                          height: 1,
                          color: AppTheme.darkBorder,
                        ),
                        itemBuilder: (context, i) =>
                            _ShareRow(file: files[i]),
                      ),
          ),
        ],
      ),
    );
  }
}

class _ShareRow extends ConsumerWidget {
  final SharedFileModel file;

  const _ShareRow({required this.file});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        children: [
          Expanded(
            flex: 3,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  file.fileName,
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
                Tooltip(
                  message: file.filePath,
                  child: Text(
                    '${Formatters.formatBytes(file.fileSize)} — ${file.fileHash}'
                    '${file.isPartial ? context.l10n.sharesPartialParts(file.completedParts.length) : ''}',
                    style: const TextStyle(
                      fontSize: 10,
                      color: AppColors.textMuted,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ),
          ),
          // Débit mesuré courant + total cumulé uploadé pour ce fichier
          // (eMule colonnes « Upload speed » / « Transferred »).
          SizedBox(
            width: 120,
            child: Column(
              key: Key('share_upload_stats_${file.fileHash}'),
              crossAxisAlignment: CrossAxisAlignment.end,
              children: [
                Text(
                  '↑ ${Formatters.formatSpeed(file.uploadRateBps)}',
                  style: TextStyle(
                    fontSize: 11,
                    color: file.uploadRateBps > 0
                        ? AppColors.accentTeal
                        : AppColors.textMuted,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
                Tooltip(
                  message: context.l10n.sharesTotalSentTooltip,
                  child: Text(
                    'Σ ${Formatters.formatBytes(file.uploadedBytes)}',
                    style: const TextStyle(
                      fontSize: 10,
                      color: AppColors.textMuted,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(width: 8),
          SizedBox(
            height: 24,
            child: DropdownButton<String>(
              value: _SharesViewState._priorityIds
                      .any((p) => p == file.upPriority)
                  ? file.upPriority
                  : 'normal',
              isDense: true,
              underline: const SizedBox.shrink(),
              style: const TextStyle(
                fontSize: 11,
                color: AppColors.textSecondary,
              ),
              dropdownColor: AppTheme.darkSurface,
              items: _SharesViewState._priorityIds
                  .map(
                    (p) => DropdownMenuItem(
                      value: p,
                      child: Text(_SharesViewState._priorityLabelOf(context, p)),
                    ),
                  )
                  .toList(),
              onChanged: file.isPartial
                  ? null
                  : (v) async {
                      if (v == null) return;
                      try {
                        await ref
                            .read(sharesControllerProvider)
                            .setUploadPriority(file.fileHash, v);
                      } catch (_) {}
                    },
            ),
          ),
          const SizedBox(width: 12),
          Tooltip(
            message: file.enabled
                ? context.l10n.sharesEnabledTooltip
                : context.l10n.sharesDisabledTooltip,
            child: Switch(
              value: file.enabled,
              onChanged: file.isPartial
                  ? null
                  : (v) async {
                      try {
                        await ref
                            .read(sharesControllerProvider)
                            .setEnabled(file.fileHash, v);
                      } catch (_) {}
                    },
            ),
          ),
          const SizedBox(width: 4),
          Tooltip(
            message: file.isPartial
                ? context.l10n.sharesRenameDisabledTooltip
                : context.l10n.sharesRenameTooltip,
            child: IconButton(
              key: Key('rename_share_${file.fileHash}'),
              constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
              padding: EdgeInsets.zero,
              visualDensity: VisualDensity.compact,
              icon: Icon(
                Icons.edit_rounded,
                size: 18,
                color: file.isPartial
                    ? AppColors.textFaint
                    : AppColors.textSecondary,
              ),
              onPressed:
                  file.isPartial ? null : () => _rename(context, ref),
            ),
          ),
          Tooltip(
            message: file.isPartial
                ? context.l10n.sharesDeleteDisabledTooltip
                : context.l10n.sharesDeleteTooltip,
            child: IconButton(
              key: Key('delete_share_${file.fileHash}'),
              constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
              padding: EdgeInsets.zero,
              visualDensity: VisualDensity.compact,
              icon: Icon(
                Icons.delete_outline_rounded,
                size: 18,
                color: file.isPartial
                    ? AppColors.textFaint
                    : AppColors.danger.withValues(alpha: 0.85),
              ),
              onPressed:
                  file.isPartial ? null : () => _confirmDelete(context, ref),
            ),
          ),
        ],
      ),
    );
  }

  /// Renommage d'un fichier partagé complet (`shares.rename`) : le
  /// fichier disque et l'entrée `shared_files` sont renommés (et le
  /// download complété associé s'il existe).
  void _rename(BuildContext context, WidgetRef ref) {
    showRenameFileDialog(
      context,
      currentName: file.fileName,
      onSubmit: (newName) async {
        try {
          await ref
              .read(sharesControllerProvider)
              .renameSharedFile(file.fileHash, newName);
          return true;
        } catch (_) {
          return false;
        }
      },
    );
  }

  /// Confirmation avant suppression définitive : le fichier est effacé du
  /// disque et retiré des partages (l'entrée de téléchargement complété
  /// associée, s'il y en a une, est supprimée avec lui côté daemon).
  void _confirmDelete(BuildContext context, WidgetRef ref) {
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text(ctx.l10n.sharesDeleteTitle),
        content: Text(
          ctx.l10n.sharesDeleteBody(file.fileName),
          style: const TextStyle(fontSize: 13),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: Text(ctx.l10n.annuler),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: AppTheme.statusFailed,
            ),
            onPressed: () async {
              Navigator.of(ctx).pop();
              try {
                await ref
                    .read(sharesControllerProvider)
                    .removeSharedFile(file.fileHash);
              } catch (_) {}
            },
            child: Text(ctx.l10n.supprimer),
          ),
        ],
      ),
    );
  }
}
