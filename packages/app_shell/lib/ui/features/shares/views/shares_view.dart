// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

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
///
/// Files are grouped by category (same `categories` as downloads — a
/// share synthesized from a download follows its category) and sorted
/// inside each group by total uploaded bytes, most served first.
class SharesView extends ConsumerStatefulWidget {
  const SharesView({super.key});

  @override
  ConsumerState<SharesView> createState() => _SharesViewState();
}

class _SharesViewState extends ConsumerState<SharesView> {
  bool _rescanning = false;
  final TextEditingController _searchCtrl = TextEditingController();
  String _searchQuery = '';

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

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

  static int _byUploadedDesc(SharedFileModel a, SharedFileModel b) =>
      b.uploadedBytes.compareTo(a.uploadedBytes);

  /// Aplati la liste en sections : `CategoryItem` = en-tête de groupe,
  /// `_UncategorizedHeader` = groupe sans catégorie (en dernier),
  /// `SharedFileModel` = ligne. Ordre des catégories = `sortOrder` ;
  /// à l'intérieur, tri par total uploadé décroissant.
  List<Object> _buildSections(
    List<SharedFileModel> files,
    List<CategoryItem> categories,
  ) {
    final query = _searchQuery.trim().toLowerCase();
    final filtered = files
        .where(
          (f) =>
              query.isEmpty ||
              f.fileName.toLowerCase().contains(query) ||
              f.fileHash.toLowerCase().contains(query),
        )
        .toList();

    final byCat = <String?, List<SharedFileModel>>{};
    for (final f in filtered) {
      byCat.putIfAbsent(f.categoryId, () => []).add(f);
    }
    for (final g in byCat.values) {
      g.sort(_byUploadedDesc);
    }

    final ordered = [...categories]
      ..sort((a, b) => a.sortOrder.compareTo(b.sortOrder));
    final knownIds = ordered.map((c) => c.id).toSet();

    // Ids orphelins (catégorie supprimée) → vers le groupe non classé.
    final uncategorized = <SharedFileModel>[
      ...?byCat.remove(null),
      for (final k in byCat.keys.toList())
        if (k != null && !knownIds.contains(k)) ...?byCat.remove(k),
    ]..sort(_byUploadedDesc);

    final flat = <Object>[];
    for (final c in ordered) {
      final g = byCat.remove(c.id);
      if (g != null && g.isNotEmpty) {
        flat.add(c);
        flat.addAll(g);
      }
    }
    if (uncategorized.isNotEmpty) {
      flat.add(const _UncategorizedHeader());
      flat.addAll(uncategorized);
    }
    return flat;
  }

  @override
  Widget build(BuildContext context) {
    final shares = ref.watch(sharesControllerProvider);
    final categories = ref.watch(categoriesControllerProvider).categories;
    final sections = _buildSections(shares.files, categories);
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
                context.l10n.sharesFileCount(shares.files.length),
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                ),
              ),
              const Spacer(),
              SizedBox(
                width: 220,
                height: 32,
                child: TextField(
                  controller: _searchCtrl,
                  onChanged: (v) => setState(() => _searchQuery = v),
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textPrimary,
                  ),
                  decoration: InputDecoration(
                    isDense: true,
                    hintText: context.l10n.searchPlaceholderShares,
                    hintStyle: const TextStyle(
                      fontSize: 12,
                      color: AppColors.textMuted,
                    ),
                    prefixIcon: const Icon(Icons.search, size: 16),
                    prefixIconConstraints:
                        const BoxConstraints(minWidth: 30, minHeight: 30),
                    suffixIcon: _searchQuery.isEmpty
                        ? null
                        : IconButton(
                            icon: const Icon(Icons.close, size: 14),
                            padding: EdgeInsets.zero,
                            constraints: const BoxConstraints(
                                minWidth: 24, minHeight: 24),
                            onPressed: () {
                              _searchCtrl.clear();
                              setState(() => _searchQuery = '');
                            },
                          ),
                    contentPadding:
                        const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                    filled: true,
                    fillColor: AppColors.fillLight.withValues(alpha: 0.4),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(6),
                      borderSide: const BorderSide(
                        color: AppTheme.darkBorder,
                        width: 0.5,
                      ),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(6),
                      borderSide: const BorderSide(
                        color: AppTheme.darkBorder,
                        width: 0.5,
                      ),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(6),
                      borderSide: const BorderSide(
                        color: AppTheme.primary,
                        width: 1,
                      ),
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 8),
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
                : sections.isEmpty
                    ? Center(
                        child: Text(
                          context.l10n.sharesEmptyState,
                          textAlign: TextAlign.center,
                          style: const TextStyle(
                            fontSize: 12,
                            color: AppColors.textMuted,
                          ),
                        ),
                      )
                    : ListView.separated(
                        itemCount: sections.length,
                        separatorBuilder: (_, i) => sections[i] is SharedFileModel
                            ? const Divider(height: 1, color: AppTheme.darkBorder)
                            : const SizedBox(height: 4),
                        itemBuilder: (context, i) {
                          final entry = sections[i];
                          if (entry is CategoryItem) {
                            return _CategoryHeader(category: entry);
                          }
                          if (entry is _UncategorizedHeader) {
                            return _CategoryHeader(category: null);
                          }
                          return _ShareRow(
                            file: entry as SharedFileModel,
                            categories: categories,
                          );
                        },
                      ),
          ),
        ],
      ),
    );
  }
}

/// Marqueur interne pour l'en-tête « Sans catégorie ».
class _UncategorizedHeader {
  const _UncategorizedHeader();
}

/// En-tête de section catégorie : pastille couleur + nom + compteur.
class _CategoryHeader extends StatelessWidget {
  final CategoryItem? category;

  const _CategoryHeader({required this.category});

  @override
  Widget build(BuildContext context) {
    final cat = category;
    return Padding(
      padding: const EdgeInsets.only(top: 10, bottom: 4),
      child: Row(
        children: [
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              color: cat?.parsedColor ?? AppColors.textMuted,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 8),
          Text(
            cat?.name ?? context.l10n.uncategorized,
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              letterSpacing: 0.4,
              color: AppColors.textSecondary,
            ),
          ),
        ],
      ),
    );
  }
}

class _ShareRow extends ConsumerWidget {
  final SharedFileModel file;
  final List<CategoryItem> categories;

  const _ShareRow({required this.file, required this.categories});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final category = file.categoryId == null
        ? null
        : categories
            .where((c) => c.id == file.categoryId)
            .firstOrNull;
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
          PopupMenuButton<String?>(
            icon: Icon(
              category != null
                  ? Icons.label_rounded
                  : Icons.label_outline_rounded,
              color: category != null
                  ? category.parsedColor
                  : AppColors.textMuted,
              size: 18,
            ),
            tooltip: category != null
                ? context.l10n.tooltipCategoryName(category.name)
                : context.l10n.tooltipAssignCategory,
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 24, minHeight: 24),
            onSelected: (categoryId) async {
              try {
                await ref
                    .read(sharesControllerProvider)
                    .setSharedFileCategory(file.fileHash, categoryId);
              } catch (_) {}
            },
            itemBuilder: (ctx) => [
              PopupMenuItem<String?>(
                value: null,
                child: Text(ctx.l10n.sansCategorieNone),
              ),
              ...categories.map(
                (c) => PopupMenuItem<String?>(
                  value: c.id,
                  child: Row(
                    children: [
                      Container(
                        width: 10,
                        height: 10,
                        decoration: BoxDecoration(
                          color: c.parsedColor,
                          shape: BoxShape.circle,
                        ),
                      ),
                      const SizedBox(width: 8),
                      Text(c.name),
                    ],
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(width: 4),
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
  /// disque et retiré des partages (l'entrée de téléchargement complétée
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
