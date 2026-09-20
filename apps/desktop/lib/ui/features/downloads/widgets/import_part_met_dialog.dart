import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:ui_kit/ui_kit.dart';
import 'package:mule_client/mule_client.dart';
import '../view_models/downloads_view_model.dart';

class ImportPartMetDialog extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const ImportPartMetDialog({super.key, required this.viewModel});

  @override
  State<ImportPartMetDialog> createState() => _ImportPartMetDialogState();
}

class _ImportPartMetDialogState extends State<ImportPartMetDialog> {
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
    final initialCatId = widget.viewModel.selectedCategoryId;
    if (initialCatId != null && initialCatId != 'uncategorized') {
      _selectedCategoryId = initialCatId;
      final cat = widget.viewModel.categories
          .where((c) => c.id == initialCatId)
          .firstOrNull;
      if (cat != null &&
          cat.defaultDestination != null &&
          cat.defaultDestination!.isNotEmpty) {
        _targetDirController =
            TextEditingController(text: cat.defaultDestination!);
      } else {
        _targetDirController = TextEditingController(
          text: widget.viewModel.defaultDownloadDirectory.path,
        );
      }
    } else {
      _targetDirController = TextEditingController(
        text: widget.viewModel.defaultDownloadDirectory.path,
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

  Future<void> _runInspection() async {
    final metPath = _metPathController.text.trim();
    if (metPath.isEmpty) return;

    final partPath = _partPathController.text.trim();

    setState(() {
      _isInspecting = true;
      _inspectError = null;
    });

    try {
      final res = await widget.viewModel.inspectPartMet(
        metPath,
        partPath: partPath.isNotEmpty ? partPath : null,
      );
      if (mounted) {
        setState(() {
          _inspection = res;
          if (_partPathController.text.trim().isEmpty && res.partPath != null) {
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
      final res = await widget.viewModel.importPartMet(
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
                      color: AppTheme.statusPaused.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(
                      Icons.file_download_outlined,
                      color: AppTheme.statusPaused,
                      size: 24,
                    ),
                  ),
                  const SizedBox(width: 12),
                  const Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Import .part.met (Experimental)',
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                            color: Colors.white,
                          ),
                        ),
                        SizedBox(height: 2),
                        Text(
                          'Import an existing partial or completed eMule download safely.',
                          style: TextStyle(
                            fontSize: 12,
                            color: Colors.white70,
                          ),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.close, color: Colors.white60),
                    onPressed: () => Navigator.of(context).pop(false),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              // Confinement and Paused Notice
              if (_importReport == null)
                Container(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: AppTheme.primary.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(8),
                    border: Border.all(color: AppTheme.primary.withValues(alpha: 0.3)),
                  ),
                  child: const Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Icon(Icons.shield_outlined,
                          color: AppTheme.primary, size: 20),
                      SizedBox(width: 10),
                      Expanded(
                        child: Text(
                          'The download will be staged into data/temp/ and initialized in Paused state. L’import ne contacte aucun peer et le téléchargement restera en pause.',
                          style: TextStyle(
                            fontSize: 12,
                            color: Colors.white,
                            height: 1.3,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
              if (_importReport != null)
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          color: AppTheme.statusCompleted.withValues(alpha: 0.15),
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(color: AppTheme.statusCompleted.withValues(alpha: 0.3)),
                        ),
                        child: const Row(
                          children: [
                            Icon(Icons.check_circle_outline, color: AppTheme.statusCompleted, size: 20),
                            SizedBox(width: 10),
                            Expanded(
                              child: Text(
                                'Import successful! The download is paused and verified parts have been staged.',
                                style: TextStyle(color: Colors.white, fontSize: 13, fontWeight: FontWeight.bold),
                              ),
                            ),
                          ],
                        ),
                      ),
                      const SizedBox(height: 12),
                      const Text(
                        'Audit Report',
                        style: TextStyle(fontSize: 13, fontWeight: FontWeight.bold, color: Colors.white),
                      ),
                      const SizedBox(height: 6),
                      Expanded(
                        child: Container(
                          width: double.infinity,
                          padding: const EdgeInsets.all(12),
                          decoration: BoxDecoration(
                            color: AppTheme.darkBackground,
                            borderRadius: BorderRadius.circular(8),
                            border: Border.all(color: AppTheme.darkBorder),
                          ),
                          child: SingleChildScrollView(
                            child: SelectableText(
                              _importReport!,
                              key: const Key('import_audit_report_text'),
                              style: const TextStyle(
                                fontFamily: 'monospace',
                                fontSize: 11,
                                color: Colors.white70,
                                height: 1.4,
                              ),
                            ),
                          ),
                        ),
                      ),
                      const SizedBox(height: 16),
                      Row(
                        mainAxisAlignment: MainAxisAlignment.end,
                        children: [
                          OutlinedButton.icon(
                            key: const Key('copy_report_button'),
                            onPressed: () {
                              Clipboard.setData(ClipboardData(text: _importReport!));
                              ScaffoldMessenger.of(context).showSnackBar(
                                const SnackBar(
                                  content: Text('Audit report copied to clipboard'),
                                  duration: Duration(seconds: 2),
                                ),
                              );
                            },
                            icon: const Icon(Icons.copy, size: 16),
                            label: const Text('Copy Report'),
                          ),
                          const SizedBox(width: 12),
                          ElevatedButton(
                            key: const Key('finish_import_button'),
                            onPressed: () => Navigator.of(context).pop(true),
                            style: ElevatedButton.styleFrom(
                              backgroundColor: AppTheme.primary,
                              foregroundColor: Colors.white,
                            ),
                            child: const Text('Done'),
                          ),
                        ],
                      ),
                    ],
                  ),
                )
              else ...[
                const SizedBox(height: 16),
                Flexible(
                  child: SingleChildScrollView(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        // Path to .part.met
                        const Text(
                          'Path to .part.met file',
                        style: TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: Colors.white,
                        ),
                      ),
                      const SizedBox(height: 6),
                      Row(
                        children: [
                          Expanded(
                            child: TextField(
                              key: const Key('met_path_input'),
                              controller: _metPathController,
                              style: const TextStyle(
                                fontSize: 13,
                                color: Colors.white,
                              ),
                              decoration: InputDecoration(
                                hintText: 'C:\\eMule\\temp\\001.part.met',
                                hintStyle: const TextStyle(
                                  color: Colors.white38,
                                  fontSize: 13,
                                ),
                                filled: true,
                                fillColor: AppTheme.darkBackground,
                                contentPadding: const EdgeInsets.symmetric(
                                  horizontal: 12,
                                  vertical: 10,
                                ),
                                border: OutlineInputBorder(
                                  borderRadius: BorderRadius.circular(8),
                                  borderSide: const BorderSide(
                                    color: AppTheme.darkBorder,
                                  ),
                                ),
                              ),
                              onSubmitted: (_) => _runInspection(),
                            ),
                          ),
                          const SizedBox(width: 8),
                          ElevatedButton.icon(
                            key: const Key('inspect_button'),
                            onPressed: _isInspecting ? null : _runInspection,
                            icon: _isInspecting
                                ? const SizedBox(
                                    width: 14,
                                    height: 14,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                      color: Colors.white,
                                    ),
                                  )
                                : const Icon(Icons.search, size: 16),
                            label: const Text('Inspect'),
                            style: ElevatedButton.styleFrom(
                              backgroundColor: AppTheme.darkCard,
                              foregroundColor: Colors.white,
                              padding: const EdgeInsets.symmetric(
                                horizontal: 12,
                                vertical: 12,
                              ),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),
                      // Path to .part
                      const Text(
                        'Associated .part file path (optional, auto-detected)',
                        style: TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: Colors.white,
                        ),
                      ),
                      const SizedBox(height: 6),
                      TextField(
                        key: const Key('part_path_input'),
                        controller: _partPathController,
                        style: const TextStyle(
                          fontSize: 13,
                          color: Colors.white,
                        ),
                        decoration: InputDecoration(
                          hintText: 'Auto-detected next to .met file',
                          hintStyle: const TextStyle(
                            color: Colors.white38,
                            fontSize: 13,
                          ),
                          filled: true,
                          fillColor: AppTheme.darkBackground,
                          contentPadding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 10,
                          ),
                          border: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(8),
                            borderSide: const BorderSide(
                              color: AppTheme.darkBorder,
                            ),
                          ),
                        ),
                      ),
                      const SizedBox(height: 16),
                      // Inspection Preview Box
                      if (_inspection != null)
                        Container(
                          padding: const EdgeInsets.all(12),
                          decoration: BoxDecoration(
                            color: AppTheme.darkBackground,
                            borderRadius: BorderRadius.circular(8),
                            border: Border.all(
                              color: _inspection!.partFileExists
                                  ? AppTheme.darkBorder
                                  : AppTheme.statusFailed,
                            ),
                          ),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Row(
                                children: [
                                  const Icon(
                                    Icons.check_circle_outline,
                                    color: AppTheme.statusCompleted,
                                    size: 18,
                                  ),
                                  const SizedBox(width: 8),
                                  Expanded(
                                    child: Text(
                                      _inspection!.fileName,
                                      style: const TextStyle(
                                        fontSize: 14,
                                        fontWeight: FontWeight.bold,
                                        color: Colors.white,
                                      ),
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                ],
                              ),
                              const Divider(
                                color: AppTheme.darkBorder,
                                height: 16,
                              ),
                              Row(
                                mainAxisAlignment:
                                    MainAxisAlignment.spaceBetween,
                                children: [
                                  const Text(
                                    'Total Size:',
                                    style: TextStyle(
                                      fontSize: 12,
                                      color: Colors.white70,
                                    ),
                                  ),
                                  Text(
                                    Formatters.formatBytes(_inspection!.totalSize),
                                    style: const TextStyle(
                                      fontSize: 12,
                                      fontWeight: FontWeight.w600,
                                      color: Colors.white,
                                    ),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 4),
                              Row(
                                mainAxisAlignment:
                                    MainAxisAlignment.spaceBetween,
                                children: [
                                  const Text(
                                    'Est. Acquired / Gaps Complement:',
                                    style: TextStyle(
                                      fontSize: 12,
                                      color: Colors.white70,
                                    ),
                                  ),
                                  Text(
                                    Formatters.formatBytes(_inspection!
                                        .estimatedTransferredBytes),
                                    style: const TextStyle(
                                      fontSize: 12,
                                      fontWeight: FontWeight.w600,
                                      color: AppTheme.secondary,
                                    ),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 4),
                              Row(
                                mainAxisAlignment:
                                    MainAxisAlignment.spaceBetween,
                                children: [
                                  const Text(
                                    'Part Count:',
                                    style: TextStyle(
                                      fontSize: 12,
                                      color: Colors.white70,
                                    ),
                                  ),
                                  Text(
                                    '${_inspection!.partCount} part(s)',
                                    style: const TextStyle(
                                      fontSize: 12,
                                      fontWeight: FontWeight.w600,
                                      color: Colors.white,
                                    ),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 4),
                              Row(
                                mainAxisAlignment:
                                    MainAxisAlignment.spaceBetween,
                                children: [
                                  const Text(
                                    '.part File on Disk:',
                                    style: TextStyle(
                                      fontSize: 12,
                                      color: Colors.white70,
                                    ),
                                  ),
                                  Text(
                                    _inspection!.partFileExists
                                        ? 'Present (${Formatters.formatBytes(_inspection!.partFileSize ?? 0)})'
                                        : 'MISSING (.part file not found)',
                                    style: TextStyle(
                                      fontSize: 12,
                                      fontWeight: FontWeight.w600,
                                      color: _inspection!.partFileExists
                                          ? AppTheme.statusCompleted
                                          : AppTheme.statusFailed,
                                    ),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 4),
                              Row(
                                children: [
                                  const Text(
                                    'eD2K Hash: ',
                                    style: TextStyle(
                                      fontSize: 11,
                                      color: Colors.white70,
                                    ),
                                  ),
                                  Expanded(
                                    child: Text(
                                      _inspection!.ed2kHash,
                                      style: const TextStyle(
                                        fontSize: 11,
                                        fontFamily: 'monospace',
                                        color: Colors.white60,
                                      ),
                                      overflow: TextOverflow.ellipsis,
                                    ),
                                  ),
                                ],
                              ),
                            ],
                          ),
                        ),
                      if (_inspectError != null)
                        Container(
                          padding: const EdgeInsets.all(10),
                          decoration: BoxDecoration(
                            color: AppTheme.statusFailed.withValues(alpha: 0.15),
                            borderRadius: BorderRadius.circular(6),
                            border:
                                Border.all(color: AppTheme.statusFailed.withValues(alpha: 0.4)),
                          ),
                          child: Row(
                            children: [
                              const Icon(Icons.error_outline,
                                  color: AppTheme.statusFailed, size: 16),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(
                                  _inspectError!,
                                  style: const TextStyle(
                                    color: AppTheme.statusFailed,
                                    fontSize: 12,
                                  ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      const SizedBox(height: 16),
                      // Destination Directory
                      const Text(
                        'Target Download Directory',
                        style: TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: Colors.white,
                        ),
                      ),
                      const SizedBox(height: 6),
                      TextField(
                        key: const Key('target_dir_input'),
                        controller: _targetDirController,
                        style: const TextStyle(
                          fontSize: 13,
                          color: Colors.white,
                        ),
                        decoration: InputDecoration(
                          hintText: 'Target directory',
                          filled: true,
                          fillColor: AppTheme.darkBackground,
                          contentPadding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 10,
                          ),
                          border: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(8),
                            borderSide: const BorderSide(
                              color: AppTheme.darkBorder,
                            ),
                          ),
                        ),
                      ),
                      const SizedBox(height: 16),
                      // Category Dropdown
                      const Text(
                        'Category (Optional)',
                        style: TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: Colors.white,
                        ),
                      ),
                      const SizedBox(height: 6),
                      DropdownButtonFormField<String?>(
                        key: const Key('category_select'),
                        initialValue: _selectedCategoryId,
                        dropdownColor: AppTheme.darkSurface,
                        style: const TextStyle(
                          color: Colors.white,
                          fontSize: 13,
                        ),
                        decoration: InputDecoration(
                          filled: true,
                          fillColor: AppTheme.darkBackground,
                          contentPadding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 10,
                          ),
                          border: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(8),
                            borderSide: const BorderSide(
                              color: AppTheme.darkBorder,
                            ),
                          ),
                        ),
                        items: [
                          const DropdownMenuItem<String?>(
                            value: null,
                            child: Text('No Category'),
                          ),
                          ...widget.viewModel.categories.map(
                            (c) => DropdownMenuItem<String?>(
                              value: c.id,
                              child: Row(
                                children: [
                                  Container(
                                    width: 10,
                                    height: 10,
                                    decoration: BoxDecoration(
                                      color: Color(int.parse(
                                        c.color.replaceFirst('#', '0xFF'),
                                      )),
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
                        onChanged: (val) {
                          setState(() {
                            _selectedCategoryId = val;
                            if (val != null) {
                              final cat = widget.viewModel.categories
                                  .where((c) => c.id == val)
                                  .firstOrNull;
                              if (cat != null &&
                                  cat.defaultDestination != null &&
                                  cat.defaultDestination!.isNotEmpty) {
                                _targetDirController.text =
                                    cat.defaultDestination!;
                              }
                            }
                          });
                        },
                      ),
                      if (_importError != null) ...[
                        const SizedBox(height: 12),
                        Container(
                          padding: const EdgeInsets.all(10),
                          decoration: BoxDecoration(
                            color: AppTheme.statusFailed.withValues(alpha: 0.15),
                            borderRadius: BorderRadius.circular(6),
                            border: Border.all(
                                color: AppTheme.statusFailed.withValues(alpha: 0.4)),
                          ),
                          child: Text(
                            _importError!,
                            style: const TextStyle(
                              color: AppTheme.statusFailed,
                              fontSize: 12,
                            ),
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ),
                const SizedBox(height: 20),
                Row(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    TextButton(
                      onPressed: _isImporting
                          ? null
                          : () => Navigator.of(context).pop(false),
                      child: const Text('Cancel'),
                    ),
                    const SizedBox(width: 12),
                    ElevatedButton(
                      key: const Key('import_submit_button'),
                      onPressed: canImport ? _submitImport : null,
                      style: ElevatedButton.styleFrom(
                        backgroundColor: AppTheme.primary,
                        foregroundColor: Colors.white,
                        padding: const EdgeInsets.symmetric(
                          horizontal: 20,
                          vertical: 12,
                        ),
                      ),
                      child: _isImporting
                          ? const SizedBox(
                              width: 18,
                              height: 18,
                              child: CircularProgressIndicator(
                                strokeWidth: 2,
                                color: Colors.white,
                              ),
                            )
                          : const Text('Import Download'),
                    ),
                  ],
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
