import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../core/folder_picker.dart';
import '../../../../app/app_controller.dart';
import 'add_link/add_link_allocation_dropdown.dart';
import 'add_link/add_link_category_dropdown.dart';
import 'add_link/add_link_field_label.dart';
import 'add_link/add_link_preview_card.dart';
import 'add_link/ed2k_link_parser.dart';

/// Dialog adding a download from an `ed2k://|file|…` link with category,
/// target directory and disk allocation options.
class AddLinkDialog extends StatefulWidget {
  final AppController viewModel;

  const AddLinkDialog({super.key, required this.viewModel});

  @override
  State<AddLinkDialog> createState() => _AddLinkDialogState();
}

class _AddLinkDialogState extends State<AddLinkDialog> {
  final _linkController = TextEditingController();
  late final TextEditingController _targetDirController;
  String _allocationPolicy = 'sparse';
  String? _selectedCategoryId;

  Ed2kLinkParseResult _parsed = const Ed2kLinkParseResult.empty();

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
    _linkController.addListener(_validateLink);
  }

  void _validateLink() {
    var result = parseEd2kLink(_linkController.text);

    if (result.isValid) {
      final existing = widget.viewModel.getDownloadByHash(result.hash!);
      if (existing != null) {
        result = Ed2kLinkParseResult.error(
          'Ce fichier est déjà dans la liste des téléchargements ("${existing.fileName}")',
        );
      }
    }

    setState(() => _parsed = result);
  }

  bool get _isValid => _parsed.isValid;

  @override
  void dispose() {
    _linkController.dispose();
    _targetDirController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 580),
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
                      color: AppTheme.primary.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(
                      Icons.add_link_rounded,
                      color: AppTheme.primary,
                      size: 22,
                    ),
                  ),
                  const SizedBox(width: 12),
                  const Text(
                    'Add eD2K Download',
                    style: TextStyle(
                      fontSize: 18,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  const Spacer(),
                  IconButton(
                    icon: const Icon(
                      Icons.close,
                      size: 20,
                      color: AppColors.textMuted,
                    ),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              ),
              const SizedBox(height: 20),
              const AddLinkFieldLabel(label: 'eD2K Link'),
              TextField(
                controller: _linkController,
                autofocus: true,
                style: const TextStyle(
                  fontSize: 13,
                  fontFamily: 'monospace',
                ),
                decoration: InputDecoration(
                  hintText:
                      'ed2k://|file|example.zip|104857600|0123456789abcdef0123456789abcdef|/',
                  hintStyle: const TextStyle(
                    fontSize: 12,
                    color: AppColors.iconFaint,
                  ),
                  errorText: _parsed.error,
                ),
              ),
              if (_isValid) ...[
                const SizedBox(height: 12),
                AddLinkPreviewCard(
                  fileName: _parsed.fileName!,
                  sizeBytes: _parsed.size!,
                  hash: _parsed.hash!,
                ),
              ],
              const SizedBox(height: 16),
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const AddLinkFieldLabel(label: 'Category'),
                  AddLinkCategoryDropdown(
                    categories: widget.viewModel.categories,
                    selectedId: _selectedCategoryId,
                    onChanged: _onCategoryChanged,
                  ),
                ],
              ),
              const SizedBox(height: 16),
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(
                    flex: 2,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const AddLinkFieldLabel(
                          label: 'Target Directory',
                        ),
                        Row(
                          children: [
                            Expanded(
                              child: TextField(
                                controller: _targetDirController,
                                style: const TextStyle(fontSize: 13),
                                decoration: const InputDecoration(
                                  isDense: true,
                                  hintText:
                                      'Default downloads folder',
                                ),
                              ),
                            ),
                            const SizedBox(width: 8),
                            OutlinedButton.icon(
                              icon: const Icon(
                                Icons.folder_open,
                                size: 16,
                              ),
                              label: const Text(
                                'Browse...',
                                style: TextStyle(fontSize: 12),
                              ),
                              style: OutlinedButton.styleFrom(
                                padding:
                                    const EdgeInsets.symmetric(
                                  horizontal: 10,
                                  vertical: 12,
                                ),
                                side: const BorderSide(
                                  color: AppTheme.darkBorder,
                                ),
                              ),
                              onPressed: _browseFolder,
                            ),
                          ],
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 16),
                  Expanded(
                    flex: 2,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const AddLinkFieldLabel(
                          label: 'Disk Allocation',
                        ),
                        AddLinkAllocationDropdown(
                          value: _allocationPolicy,
                          onChanged: (val) => setState(
                            () => _allocationPolicy = val,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 24),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: () => Navigator.of(context).pop(),
                    child: const Text(
                      'Cancel',
                      style: TextStyle(color: AppColors.textSoft),
                    ),
                  ),
                  const SizedBox(width: 12),
                  FilledButton.icon(
                    icon: const Icon(Icons.add_rounded, size: 18),
                    label: const Text('Add Download'),
                    style: FilledButton.styleFrom(
                      backgroundColor: AppTheme.primary,
                    ),
                    onPressed: _isValid ? _submit : null,
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  void _onCategoryChanged(String? val) {
    setState(() {
      _selectedCategoryId = val;
      if (val != null) {
        final cat = widget.viewModel.categories
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

  Future<void> _browseFolder() async {
    final chosen =
        await pickFolder(description: 'Select Download Directory');
    if (chosen != null && mounted) {
      setState(() {
        _targetDirController.text = chosen;
      });
    }
  }

  void _submit() async {
    final link = _linkController.text.trim();
    final targetDir = _targetDirController.text.trim();

    final destination = targetDir.isNotEmpty
        ? targetDir
        : widget.viewModel.defaultDownloadDirectory.path;

    final success = await widget.viewModel.addEd2kLink(
      link,
      targetDirectory: destination,
      allocationPolicy: _allocationPolicy,
      categoryId: _selectedCategoryId,
    );

    if (success && mounted) {
      Navigator.of(context).pop();
    } else if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            widget.viewModel.errorMessage ??
                'Erreur lors de l\'ajout du téléchargement',
          ),
          backgroundColor: AppColors.accentOrangeShade700,
          duration: const Duration(seconds: 4),
        ),
      );
    }
  }
}
