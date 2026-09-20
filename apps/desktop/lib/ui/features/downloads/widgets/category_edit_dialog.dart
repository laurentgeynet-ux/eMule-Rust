import 'dart:io';
import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../view_models/downloads_view_model.dart';

class CategoryEditDialog extends StatefulWidget {
  final DownloadsViewModel viewModel;
  final CategoryItem? category; // null when creating

  const CategoryEditDialog({
    super.key,
    required this.viewModel,
    this.category,
  });

  @override
  State<CategoryEditDialog> createState() => _CategoryEditDialogState();
}

class _CategoryEditDialogState extends State<CategoryEditDialog> {
  late final TextEditingController _nameController;
  late final TextEditingController _colorController;
  late final TextEditingController _destController;
  String? _selectedPriority;
  bool _isSaving = false;
  String? _error;

  static const List<String> _presetColors = [
    '#2196F3', // Blue
    '#4CAF50', // Green
    '#9C27B0', // Purple
    '#FF9800', // Orange
    '#E91E63', // Pink
    '#00BCD4', // Cyan
    '#009688', // Teal
    '#FF5722', // Deep Orange
    '#3F51B5', // Indigo
    '#607D8B', // Blue Grey
  ];

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController(text: widget.category?.name ?? '');
    _colorController = TextEditingController(text: widget.category?.color ?? '#2196F3');
    _destController = TextEditingController(text: widget.category?.defaultDestination ?? '');
    _selectedPriority = widget.category?.defaultPriority;
  }

  @override
  void dispose() {
    _nameController.dispose();
    _colorController.dispose();
    _destController.dispose();
    super.dispose();
  }

  Color _parseColor(String hex) {
    final clean = hex.replaceAll('#', '').trim();
    if (clean.length == 6) {
      final val = int.tryParse('FF$clean', radix: 16);
      if (val != null) return Color(val);
    }
    return AppTheme.primary;
  }

  Future<void> _browseFolder() async {
    if (Platform.isWindows) {
      try {
        final result = await Process.run('powershell', [
          '-NoProfile',
          '-Command',
          r'''Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.FolderBrowserDialog; $f.Description = 'Select Default Category Download Directory'; if ($f.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $f.SelectedPath }'''
        ]);
        final chosen = result.stdout.toString().trim();
        if (chosen.isNotEmpty && Directory(chosen).existsSync()) {
          setState(() {
            _destController.text = chosen;
          });
        }
      } catch (_) {}
    }
  }

  Future<void> _save() async {
    final name = _nameController.text.trim();
    if (name.isEmpty) {
      setState(() => _error = 'Category name cannot be empty');
      return;
    }

    final color = _colorController.text.trim();
    final dest = _destController.text.trim().isNotEmpty ? _destController.text.trim() : null;
    final prio = _selectedPriority;

    setState(() {
      _isSaving = true;
      _error = null;
    });

    bool success;
    if (widget.category == null) {
      success = await widget.viewModel.createCategory(
        name: name,
        color: color,
        defaultDestination: dest,
        defaultPriority: prio,
      );
    } else {
      success = await widget.viewModel.updateCategory(
        widget.category!.id,
        name: name,
        color: color,
        defaultDestination: dest,
        defaultPriority: prio,
      );
    }

    if (!mounted) return;
    setState(() => _isSaving = false);

    if (success) {
      Navigator.of(context).pop();
    } else {
      setState(() => _error = widget.viewModel.errorMessage ?? 'Operation failed');
    }
  }

  @override
  Widget build(BuildContext context) {
    final isEditing = widget.category != null;
    final currentColor = _parseColor(_colorController.text);

    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 520),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              // Header
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: currentColor.withValues(alpha: 0.2),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Icon(
                      isEditing ? Icons.edit_rounded : Icons.category_rounded,
                      color: currentColor,
                      size: 20,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Text(
                    isEditing ? 'Edit Category' : 'New Category',
                    style: const TextStyle(
                      fontSize: 18,
                      fontWeight: FontWeight.bold,
                      color: Colors.white,
                    ),
                  ),
                  const Spacer(),
                  IconButton(
                    icon: const Icon(Icons.close, size: 20, color: Colors.white54),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              ),
              const SizedBox(height: 20),

              if (_error != null) ...[
                Container(
                  padding: const EdgeInsets.all(10),
                  decoration: BoxDecoration(
                    color: AppTheme.statusFailed.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(6),
                    border: Border.all(color: AppTheme.statusFailed.withValues(alpha: 0.4)),
                  ),
                  child: Row(
                    children: [
                      const Icon(Icons.error_outline, color: AppTheme.statusFailed, size: 16),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          _error!,
                          style: const TextStyle(color: AppTheme.statusFailed, fontSize: 12),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 16),
              ],

              // Name Field
              const Text(
                'Category Name *',
                style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white70),
              ),
              const SizedBox(height: 6),
              TextField(
                controller: _nameController,
                autofocus: true,
                style: const TextStyle(fontSize: 13),
                decoration: const InputDecoration(
                  hintText: 'e.g. Movies, Music, Linux ISOs',
                  isDense: true,
                ),
              ),
              const SizedBox(height: 16),

              // Color Palette & Hex Input
              const Text(
                'Color Tag',
                style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white70),
              ),
              const SizedBox(height: 8),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: _presetColors.map((hex) {
                  final c = _parseColor(hex);
                  final isSelected = _colorController.text.trim().toUpperCase() == hex.toUpperCase();
                  return InkWell(
                    borderRadius: BorderRadius.circular(16),
                    onTap: () {
                      setState(() {
                        _colorController.text = hex;
                      });
                    },
                    child: Container(
                      width: 28,
                      height: 28,
                      decoration: BoxDecoration(
                        color: c,
                        shape: BoxShape.circle,
                        border: Border.all(
                          color: isSelected ? Colors.white : Colors.white24,
                          width: isSelected ? 2.5 : 1.0,
                        ),
                        boxShadow: isSelected
                            ? [
                                BoxShadow(
                                  color: c.withValues(alpha: 0.5),
                                  blurRadius: 6,
                                  spreadRadius: 1,
                                )
                              ]
                            : null,
                      ),
                      child: isSelected
                          ? const Icon(Icons.check, size: 16, color: Colors.white)
                          : null,
                    ),
                  );
                }).toList(),
              ),
              const SizedBox(height: 10),
              Row(
                children: [
                  Container(
                    width: 20,
                    height: 20,
                    decoration: BoxDecoration(
                      color: currentColor,
                      borderRadius: BorderRadius.circular(4),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: TextField(
                      controller: _colorController,
                      style: const TextStyle(fontSize: 12, fontFamily: 'monospace'),
                      decoration: const InputDecoration(
                        hintText: '#RRGGBB',
                        isDense: true,
                        contentPadding: EdgeInsets.symmetric(horizontal: 10, vertical: 8),
                      ),
                      onChanged: (_) => setState(() {}),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 16),

              // Default Destination Field (Optional)
              const Text(
                'Default Destination (Optional)',
                style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white70),
              ),
              const SizedBox(height: 6),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _destController,
                      style: const TextStyle(fontSize: 13),
                      decoration: const InputDecoration(
                        hintText: 'Inherit global destination',
                        isDense: true,
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton.icon(
                    icon: const Icon(Icons.folder_open, size: 16),
                    label: const Text('Browse...', style: TextStyle(fontSize: 12)),
                    style: OutlinedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 12),
                      side: const BorderSide(color: AppTheme.darkBorder),
                    ),
                    onPressed: _browseFolder,
                  ),
                ],
              ),
              const SizedBox(height: 16),

              // Default Priority Dropdown (Optional)
              const Text(
                'Default Priority (Optional)',
                style: TextStyle(fontSize: 12, fontWeight: FontWeight.w600, color: Colors.white70),
              ),
              const SizedBox(height: 6),
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 12),
                decoration: BoxDecoration(
                  color: AppTheme.darkSurface,
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: AppTheme.darkBorder),
                ),
                child: DropdownButtonHideUnderline(
                  child: DropdownButton<String?>(
                    value: _selectedPriority,
                    isExpanded: true,
                    dropdownColor: AppTheme.darkCard,
                    items: const [
                      DropdownMenuItem(
                        value: null,
                        child: Text('Normal (Default)', style: TextStyle(fontSize: 12)),
                      ),
                      DropdownMenuItem(
                        value: 'low',
                        child: Text('Low', style: TextStyle(fontSize: 12)),
                      ),
                      DropdownMenuItem(
                        value: 'normal',
                        child: Text('Normal', style: TextStyle(fontSize: 12)),
                      ),
                      DropdownMenuItem(
                        value: 'high',
                        child: Text('High', style: TextStyle(fontSize: 12)),
                      ),
                    ],
                    onChanged: (val) => setState(() => _selectedPriority = val),
                  ),
                ),
              ),
              const SizedBox(height: 24),

              // Actions
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: _isSaving ? null : () => Navigator.of(context).pop(),
                    child: const Text('Cancel', style: TextStyle(color: Colors.white60)),
                  ),
                  const SizedBox(width: 12),
                  FilledButton.icon(
                    icon: _isSaving
                        ? const SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(strokeWidth: 2, color: Colors.white),
                          )
                        : Icon(isEditing ? Icons.save_rounded : Icons.add_rounded, size: 18),
                    label: Text(isEditing ? 'Save Changes' : 'Create Category'),
                    style: FilledButton.styleFrom(backgroundColor: currentColor),
                    onPressed: _isSaving ? null : _save,
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
