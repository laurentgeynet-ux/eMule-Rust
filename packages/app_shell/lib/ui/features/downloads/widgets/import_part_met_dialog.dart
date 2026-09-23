// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';

import 'package:ui_kit/ui_kit.dart';
import 'package:mule_client/mule_client.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import '../../../../core/file_picker_helper.dart';
import '../../../../core/folder_picker.dart';
import 'import/import_category_dropdown.dart';
import 'import/import_field_label.dart';
import 'import/import_inline_error.dart';
import 'import/import_inspection_card.dart';
import 'import/import_path_field.dart';
import 'import/import_report_view.dart';
import 'import/import_safety_notice.dart';
import '../../../../l10n/l10n.dart';

/// Import an existing eMule `.part.met` (inspect → preview → stage paused).
class ImportPartMetDialog extends ConsumerStatefulWidget {
    const ImportPartMetDialog({super.key, });

  @override
  ConsumerState<ImportPartMetDialog> createState() => _ImportPartMetDialogState();
}

class _ImportPartMetDialogState extends ConsumerState<ImportPartMetDialog> {
  final _metPathController = TextEditingController();
  final _partPathController = TextEditingController();
  late final TextEditingController _targetDirController;
  String? _selectedCategoryId;

  bool _isInspecting = false;
  String? _inspectError;
  PartMetInspectionResult? _inspection;
  bool _isImporting = false;
  String? _importError;
  String? _importReport;

  @override
  void initState() {
    super.initState();
    final initialCatId = ref.read(categoriesControllerProvider).selectedId;
    if (initialCatId != null && initialCatId != 'uncategorized') {
      _selectedCategoryId = initialCatId;
      final cat = ref.read(categoriesControllerProvider).categories
          .where((c) => c.id == initialCatId)
          .firstOrNull;
      if (cat != null &&
          cat.defaultDestination != null &&
          cat.defaultDestination!.isNotEmpty) {
        _targetDirController =
            TextEditingController(text: cat.defaultDestination!);
      } else {
        _targetDirController = TextEditingController(
          text: ref.read(defaultDownloadDirectoryProvider),
        );
      }
    } else {
      _targetDirController = TextEditingController(
        text: ref.read(defaultDownloadDirectoryProvider),
      );
    }
  }

  @override
  void dispose() {
    _metPathController.dispose();
    _partPathController.dispose();
    _targetDirController.dispose();
    super.dispose();
  }

  /// Ouvre l'explorateur de fichiers natif et remplit [ctrl] avec le
  /// chemin choisi (filtre d'extensions optionnel, ex. `['met']`).
  Future<void> _browseFile(
    TextEditingController ctrl,
    String description, {
    List<String>? allowedExtensions,
  }) async {
    final picked = await pickFile(
      description: description,
      allowedExtensions: allowedExtensions,
    );
    if (picked != null && mounted) ctrl.text = picked;
  }

  /// Ouvre l'explorateur de dossiers natif pour [ctrl].
  Future<void> _browseFolder(
    TextEditingController ctrl,
    String description,
  ) async {
    final picked = await pickFolder(description: description);
    if (picked != null && mounted) ctrl.text = picked;
  }

  Future<void> _runInspection() async {
    final metPath = _metPathController.text.trim();
    if (metPath.isEmpty) return;

    final partPath = _partPathController.text.trim();

    setState(() {
      _isInspecting = true;
      _inspectError = null;
    });

    try {
      final res = await ref.read(downloadsStoreControllerProvider).inspectPartMet(
        metPath,
        partPath: partPath.isNotEmpty ? partPath : null,
      );
      if (mounted) {
        setState(() {
          _inspection = res;
          if (_partPathController.text.trim().isEmpty &&
              res.partPath != null) {
            _partPathController.text = res.partPath!;
          }
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _inspectError = e.toString();
          _inspection = null;
        });
      }
    } finally {
      if (mounted) {
        setState(() {
          _isInspecting = false;
        });
      }
    }
  }

  Future<void> _submitImport() async {
    final metPath = _metPathController.text.trim();
    if (metPath.isEmpty) return;

    final partPath = _partPathController.text.trim();
    final targetDir = _targetDirController.text.trim();

    setState(() {
      _isImporting = true;
      _importError = null;
    });

    try {
      final res = await ref.read(downloadsStoreControllerProvider).importPartMet(
        metPath,
        partPath: partPath.isNotEmpty ? partPath : null,
        targetDirectory: targetDir.isNotEmpty ? targetDir : null,
        categoryId: _selectedCategoryId,
      );

      if (res != null && mounted) {
        if (res.report != null && res.report!.isNotEmpty) {
          setState(() {
            _importReport = res.report;
          });
        } else {
          Navigator.of(context).pop(true);
        }
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _importError = e.toString();
        });
      }
    } finally {
      if (mounted) {
        setState(() {
          _isImporting = false;
        });
      }
    }
  }

  void _onCategoryChanged(String? val) {
    setState(() {
      _selectedCategoryId = val;
      if (val != null) {
        final cat = ref.read(categoriesControllerProvider).categories
            .where((c) => c.id == val)
            .firstOrNull;
        if (cat != null &&
            cat.defaultDestination != null &&
            cat.defaultDestination!.isNotEmpty) {
          _targetDirController.text = cat.defaultDestination!;
        }
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final canImport = _inspection != null &&
        _inspection!.partFileExists &&
        !_isInspecting &&
        !_isImporting;

    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 580, maxHeight: 720),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color:
                          AppTheme.statusPaused.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(
                      Icons.file_download_outlined,
                      color: AppTheme.statusPaused,
                      size: 24,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(context.l10n.importPartMetExperimental,
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                            color: AppColors.textPrimary,
                          ),
                        ),
                        SizedBox(height: 2),
                        Text(context.l10n.importAnExistingPartialOr,
                          style: TextStyle(
                            fontSize: 12,
                            color: AppColors.textSecondary,
                          ),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    icon: const Icon(
                      Icons.close,
                      color: AppColors.textSoft,
                    ),
                    onPressed: () => Navigator.of(context).pop(false),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              if (_importReport == null) const ImportSafetyNotice(),
              if (_importReport != null)
                Expanded(
                  child: ImportReportView(report: _importReport!),
                )
              else ...[
                const SizedBox(height: 16),
                Flexible(
                  child: SingleChildScrollView(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        ImportFieldLabel(
                          label: context.l10n.pathToPartMetFile,
                        ),
                        Row(
                          children: [
                            Expanded(
                              child: ImportPathField(
                                key: const Key('met_path_input'),
                                controller: _metPathController,
                                hintText:
                                    'C:\\eMule\\temp\\001.part.met',
                                onSubmitted: (_) => _runInspection(),
                              ),
                            ),
                            const SizedBox(width: 8),
                            IconButton(
                              icon: const Icon(
                                Icons.folder_open_rounded,
                                size: 18,
                              ),
                              tooltip: context.l10n.browse,
                              onPressed: () => _browseFile(
                                _metPathController,
                                context.l10n.pathToPartMetFile,
                                allowedExtensions: const ['met'],
                              ),
                            ),
                            ElevatedButton.icon(
                              key: const Key('inspect_button'),
                              onPressed: _isInspecting
                                  ? null
                                  : _runInspection,
                              icon: _isInspecting
                                  ? const SizedBox(
                                      width: 14,
                                      height: 14,
                                      child:
                                          CircularProgressIndicator(
                                        strokeWidth: 2,
                                        color:
                                            AppColors.textPrimary,
                                      ),
                                    )
                                  : const Icon(
                                      Icons.search,
                                      size: 16,
                                    ),
                              label: Text(context.l10n.inspect),
                              style: ElevatedButton.styleFrom(
                                backgroundColor: AppTheme.darkCard,
                                foregroundColor:
                                    AppColors.textPrimary,
                                padding:
                                    const EdgeInsets.symmetric(
                                  horizontal: 12,
                                  vertical: 12,
                                ),
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 12),
                        ImportFieldLabel(
                          label: context.l10n.associatedPartFilePathOptional,
                        ),
                        Row(
                          children: [
                            Expanded(
                              child: ImportPathField(
                                key: const Key('part_path_input'),
                                controller: _partPathController,
                                hintText: context.l10n.autoDetectedNextToMet,
                              ),
                            ),
                            const SizedBox(width: 8),
                            IconButton(
                              icon: const Icon(
                                Icons.folder_open_rounded,
                                size: 18,
                              ),
                              tooltip: context.l10n.browse,
                              onPressed: () => _browseFile(
                                _partPathController,
                                context.l10n.associatedPartFilePathOptional,
                                allowedExtensions: const ['part'],
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 16),
                        if (_inspection != null)
                          ImportInspectionCard(
                            inspection: _inspection!,
                          ),
                        if (_inspectError != null)
                          ImportInlineError(
                            message: _inspectError!,
                          ),
                        const SizedBox(height: 16),
                        ImportFieldLabel(
                          label: context.l10n.targetDownloadDirectory,
                        ),
                        Row(
                          children: [
                            Expanded(
                              child: ImportPathField(
                                key: const Key('target_dir_input'),
                                controller: _targetDirController,
                                hintText: context.l10n.targetDirectory2,
                              ),
                            ),
                            const SizedBox(width: 8),
                            IconButton(
                              icon: const Icon(
                                Icons.folder_open_rounded,
                                size: 18,
                              ),
                              tooltip: context.l10n.browse,
                              onPressed: () => _browseFolder(
                                _targetDirController,
                                context.l10n.targetDownloadDirectory,
                              ),
                            ),
                          ],
                        ),
                        const SizedBox(height: 16),
                        ImportFieldLabel(
                          label: context.l10n.categoryOptional,
                        ),
                        ImportCategoryDropdown(
                          categories: ref.read(categoriesControllerProvider).categories,
                          selectedId: _selectedCategoryId,
                          onChanged: _onCategoryChanged,
                        ),
                        if (_importError != null) ...[
                          const SizedBox(height: 12),
                          ImportInlineError(
                            message: _importError!,
                            withIcon: false,
                          ),
                        ],
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 20),
                FittedBox(
                  fit: BoxFit.scaleDown,
                  alignment: Alignment.centerRight,
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      TextButton(
                        onPressed: _isImporting
                            ? null
                            : () => Navigator.of(context).pop(false),
                        child: Text(context.l10n.cancel),
                      ),
                      const SizedBox(width: 12),
                      ElevatedButton(
                        key: const Key('import_submit_button'),
                        onPressed: canImport ? _submitImport : null,
                        style: ElevatedButton.styleFrom(
                          backgroundColor: AppTheme.primary,
                          foregroundColor: AppColors.textPrimary,
                          padding: const EdgeInsets.symmetric(
                            horizontal: 20,
                            vertical: 12,
                          ),
                        ),
                        child: _isImporting
                            ? const SizedBox(
                                width: 18,
                                height: 18,
                                child:
                                    CircularProgressIndicator(
                                  strokeWidth: 2,
                                  color: AppColors.textPrimary,
                                ),
                              )
                            : Text(context.l10n.importDownload),
                      ),
                    ],
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
