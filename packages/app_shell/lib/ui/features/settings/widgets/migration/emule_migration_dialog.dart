// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../app/providers.dart';
import '../../../../../core/folder_picker.dart';
import '../../../../../l10n/l10n.dart';
import 'migration_source.dart';
import 'migration_source_tile.dart';

/// Assistant de migration eMule : pointe vers le dossier de configuration
/// eMule (`config/`), détecte les fichiers importables et exécute les RPC
/// `*.import*` correspondants avec un rapport par source.
class EmuleMigrationDialog extends ConsumerStatefulWidget {
  const EmuleMigrationDialog({super.key});

  @override
  ConsumerState<EmuleMigrationDialog> createState() =>
      _EmuleMigrationDialogState();
}

class _EmuleMigrationDialogState extends ConsumerState<EmuleMigrationDialog> {
  final _dirController = TextEditingController();
  final _incomingController = TextEditingController();

  Set<MigrationSource> _detected = const {};
  final Set<MigrationSource> _selected = {};
  final Map<MigrationSource, MigrationRunResult> _results = {};

  bool _running = false;
  MigrationSource? _current;
  bool _replaceIdentity = false;

  @override
  void initState() {
    super.initState();
    _dirController.addListener(_refreshDetected);
  }

  @override
  void dispose() {
    _dirController.dispose();
    _incomingController.dispose();
    super.dispose();
  }

  void _refreshDetected() {
    final dir = _dirController.text.trim();
    final detected = dir.isEmpty
        ? <MigrationSource>{}
        : MigrationSource.values.where((s) => s.detectedIn(dir)).toSet();
    setState(() {
      _detected = detected;
      _selected.removeWhere((s) => !detected.contains(s));
    });
  }

  Future<void> _browseDir() async {
    final picked = await pickFolder(
      description: context.l10n.migrationConfigDirLabel,
    );
    if (picked != null && mounted) _dirController.text = picked;
  }

  Future<void> _browseIncoming() async {
    final picked = await pickFolder(
      description: context.l10n.migrationIncomingLabel,
    );
    if (picked != null && mounted) _incomingController.text = picked;
  }

  Future<void> _runImports() async {
    final dir = _dirController.text.trim();
    if (dir.isEmpty || _selected.isEmpty) return;
    final client = ref.read(muleDaemonClientProvider);
    final incoming = _incomingController.text.trim();

    setState(() {
      _running = true;
      _results.clear();
    });

    try {
      for (final source in MigrationSource.values) {
        if (!_selected.contains(source)) continue;
        setState(() => _current = source);
        try {
          final raw = await source.run(
            client,
            dir,
            incomingDir: incoming.isEmpty ? null : incoming,
            replaceIdentity: _replaceIdentity,
          );
          if (mounted) {
            _results[source] = (
              summary: MigrationSource.describeResult(
                source,
                raw,
                context.l10n,
              ),
              error: null,
            );
          }
        } catch (e) {
          if (mounted) {
            _results[source] = (summary: null, error: e.toString());
          }
        }
        if (!mounted) return;
        setState(() {});
      }
    } finally {
      if (mounted) {
        setState(() {
          _running = false;
          _current = null;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    final dirEntered = _dirController.text.trim().isNotEmpty;
    final detected = MigrationSource.values
        .where(_detected.contains)
        .toList(growable: false);
    final identitySelected = _selected.contains(MigrationSource.identity);
    final allSelected =
        detected.isNotEmpty && _selected.length == detected.length;

    return AlertDialog(
      backgroundColor: AppTheme.darkSurface,
      title: Text(l10n.migrationDialogTitle),
      content: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              // Dossier de configuration eMule.
              _DirField(
                controller: _dirController,
                label: l10n.migrationConfigDirLabel,
                hint: l10n.migrationConfigDirHint,
                onBrowse: _browseDir,
                browseLabel: l10n.browse,
              ),
              const SizedBox(height: AppSpacing.sm),
              // Dossier Incoming optionnel (matching known.met).
              _DirField(
                controller: _incomingController,
                label: l10n.migrationIncomingLabel,
                hint: l10n.migrationIncomingHint,
                onBrowse: _browseIncoming,
                browseLabel: l10n.browse,
              ),
              const SizedBox(height: AppSpacing.md),
              if (dirEntered && detected.isEmpty)
                Text(
                  l10n.migrationNoneDetected,
                  style: AppTextStyles.toolbarLabel
                      .copyWith(color: AppColors.textMuted),
                ),
              if (detected.isNotEmpty) ...[
                InkWell(
                  onTap: _running
                      ? null
                      : () => setState(() {
                            if (allSelected) {
                              _selected.clear();
                            } else {
                              _selected
                                ..clear()
                                ..addAll(detected);
                            }
                          }),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      vertical: AppSpacing.xxs,
                    ),
                    child: Text(
                      l10n.migrationSelectAll,
                      style: AppTextStyles.toolbarLabel
                          .copyWith(color: AppColors.accent),
                    ),
                  ),
                ),
                for (final source in detected)
                  MigrationSourceTile(
                    source: source,
                    checked: _selected.contains(source),
                    enabled: !_running,
                    running: _running && _current == source,
                    result: _results[source],
                    onChanged: (v) => setState(() {
                      if (v) {
                        _selected.add(source);
                      } else {
                        _selected.remove(source);
                      }
                    }),
                  ),
              ],
              if (identitySelected) ...[
                const SizedBox(height: AppSpacing.sm),
                Container(
                  padding: const EdgeInsets.all(AppSpacing.sm),
                  decoration: BoxDecoration(
                    color: AppColors.fillSubtle,
                    borderRadius:
                        BorderRadius.circular(AppRadius.md),
                    border: Border.all(color: AppTheme.darkBorder),
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          const Icon(
                            Icons.warning_amber_rounded,
                            size: 16,
                            color: AppColors.accentAmber,
                          ),
                          const SizedBox(width: AppSpacing.sm),
                          Expanded(
                            child: Text(
                              l10n.migrationReplaceIdentityWarn,
                              style: AppTextStyles.badgeLabel.copyWith(
                                color: AppColors.accentAmber,
                                fontWeight: FontWeight.normal,
                              ),
                            ),
                          ),
                        ],
                      ),
                      InkWell(
                        onTap: _running
                            ? null
                            : () => setState(
                                () => _replaceIdentity = !_replaceIdentity),
                        child: Row(
                          children: [
                            SizedBox(
                              height: 20,
                              width: 20,
                              child: Checkbox(
                                value: _replaceIdentity,
                                onChanged: _running
                                    ? null
                                    : (v) => setState(() =>
                                        _replaceIdentity = v ?? false),
                              ),
                            ),
                            const SizedBox(width: AppSpacing.sm),
                            Expanded(
                              child: Text(
                                l10n.migrationReplaceIdentity,
                                style: AppTextStyles.toolbarLabel.copyWith(
                                  color: AppColors.textPrimary,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: _running ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.close),
        ),
        FilledButton.icon(
          onPressed: _running || _selected.isEmpty
              ? null
              : _runImports,
          icon: _running
              ? const SizedBox(
                  height: 14,
                  width: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.download_done_outlined, size: 16),
          label: Text(
            _running ? l10n.migrationRunning : l10n.migrationRunSelected,
          ),
        ),
      ],
    );
  }
}

/// Champ dossier : label + TextField + bouton « Parcourir… ».
class _DirField extends StatelessWidget {
  const _DirField({
    required this.controller,
    required this.label,
    required this.hint,
    required this.onBrowse,
    required this.browseLabel,
  });

  final TextEditingController controller;
  final String label;
  final String hint;
  final VoidCallback onBrowse;
  final String browseLabel;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          label,
          style: AppTextStyles.toolbarLabel
              .copyWith(color: AppColors.textSecondary),
        ),
        const SizedBox(height: AppSpacing.xxs),
        Row(
          children: [
            Expanded(
              child: TextField(
                controller: controller,
                style: AppTextStyles.toolbarLabel
                    .copyWith(color: AppColors.textPrimary),
                decoration: InputDecoration(
                  hintText: hint,
                  hintStyle: AppTextStyles.toolbarLabel
                      .copyWith(color: AppColors.textFaint),
                  isDense: true,
                  filled: true,
                  fillColor: AppTheme.darkBackground,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(AppRadius.md),
                    borderSide:
                        const BorderSide(color: AppTheme.darkBorder),
                  ),
                ),
              ),
            ),
            const SizedBox(width: AppSpacing.sm),
            TextButton(onPressed: onBrowse, child: Text(browseLabel)),
          ],
        ),
      ],
    );
  }
}
