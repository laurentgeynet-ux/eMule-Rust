// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../core/folder_picker.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../../../../app/providers.dart';
import 'category/category_color_picker.dart';
import 'category/category_priority_dropdown.dart';
import '../../../../l10n/l10n.dart';

/// Create or edit a download category (name, color, default destination
/// and priority).
class CategoryEditDialog extends ConsumerStatefulWidget {
  final CategoryItem? category; // null when creating

  const CategoryEditDialog({
    super.key,
    this.category,
  });

  @override
  ConsumerState<CategoryEditDialog> createState() => _CategoryEditDialogState();
}

class _CategoryEditDialogState extends ConsumerState<CategoryEditDialog> {
  late final TextEditingController _nameController;
  late final TextEditingController _colorController;
  late final TextEditingController _destController;
  String? _selectedPriority;
  bool _isSaving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _nameController =
        TextEditingController(text: widget.category?.name ?? '');
    _colorController = TextEditingController(
      text: widget.category?.color ?? '#2196F3',
    );
    _destController = TextEditingController(
      text: widget.category?.defaultDestination ?? '',
    );
    _selectedPriority = widget.category?.defaultPriority;
  }

  @override
  void dispose() {
    _nameController.dispose();
    _colorController.dispose();
    _destController.dispose();
    super.dispose();
  }

  Future<void> _browseFolder() async {
    final chosen = await pickFolder(
      description: context.l10n.catPickDir,
    );
    if (chosen != null && mounted) {
      setState(() {
        _destController.text = chosen;
      });
    }
  }

  Future<void> _save() async {
    final name = _nameController.text.trim();
    if (name.isEmpty) {
      setState(() => _error = context.l10n.catNameRequired);
      return;
    }

    final color = _colorController.text.trim();
    final dest = _destController.text.trim().isNotEmpty
        ? _destController.text.trim()
        : null;
    final prio = _selectedPriority;

    setState(() {
      _isSaving = true;
      _error = null;
    });

    bool success;
    if (widget.category == null) {
      success = await ref.read(categoriesControllerProvider).create(
        name: name,
        color: color,
        defaultDestination: dest,
        defaultPriority: prio,
      );
    } else {
      success = await ref.read(categoriesControllerProvider).update(
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
      setState(() =>
          _error = ref.read(shellControllerProvider).errorMessage ??
              context.l10n.opFailed);
    }
  }

  @override
  Widget build(BuildContext context) {
    final isEditing = widget.category != null;
    final currentColor = parseCategoryColor(_colorController.text);

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
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: currentColor.withValues(alpha: 0.2),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Icon(
                      isEditing
                          ? Icons.edit_rounded
                          : Icons.category_rounded,
                      color: currentColor,
                      size: 20,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Text(
                    isEditing
                        ? context.l10n.catEditTitle
                        : context.l10n.catNewTitle,
                    style: const TextStyle(
                      fontSize: 18,
                      fontWeight: FontWeight.bold,
                      color: AppColors.textPrimary,
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
              if (_error != null) ...[
                Container(
                  padding: const EdgeInsets.all(10),
                  decoration: BoxDecoration(
                    color:
                        AppTheme.statusFailed.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(6),
                    border: Border.all(
                      color:
                          AppTheme.statusFailed.withValues(alpha: 0.4),
                    ),
                  ),
                  child: Row(
                    children: [
                      const Icon(
                        Icons.error_outline,
                        color: AppTheme.statusFailed,
                        size: 16,
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          _error!,
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
              ],
              _FieldLabel(context.l10n.catNameLabel),
              TextField(
                controller: _nameController,
                autofocus: true,
                style: const TextStyle(fontSize: 13),
                decoration: InputDecoration(
                  hintText: context.l10n.eGMoviesMusicLinux,
                  isDense: true,
                ),
              ),
              const SizedBox(height: 16),
              _FieldLabel(context.l10n.catColorLabel),
              CategoryColorPicker(
                colorController: _colorController,
                onChanged: () => setState(() {}),
              ),
              const SizedBox(height: 16),
              _FieldLabel(context.l10n.catDestLabel),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _destController,
                      style: const TextStyle(fontSize: 13),
                      decoration: InputDecoration(
                        hintText: context.l10n.inheritGlobalDestination,
                        isDense: true,
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton.icon(
                    icon: const Icon(Icons.folder_open, size: 16),
                    label: Text(context.l10n.browse,
                      style: TextStyle(fontSize: 12),
                    ),
                    style: OutlinedButton.styleFrom(
                      padding: const EdgeInsets.symmetric(
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
              const SizedBox(height: 16),
              _FieldLabel(context.l10n.catPrioLabel),
              CategoryPriorityDropdown(
                value: _selectedPriority,
                onChanged: (val) =>
                    setState(() => _selectedPriority = val),
              ),
              const SizedBox(height: 24),
              Row(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  TextButton(
                    onPressed: _isSaving
                        ? null
                        : () => Navigator.of(context).pop(),
                    child: Text(context.l10n.cancel,
                      style: TextStyle(color: AppColors.textSoft),
                    ),
                  ),
                  const SizedBox(width: 12),
                  FilledButton.icon(
                    icon: _isSaving
                        ? const SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(
                              strokeWidth: 2,
                              color: AppColors.textPrimary,
                            ),
                          )
                        : Icon(
                            isEditing
                                ? Icons.save_rounded
                                : Icons.add_rounded,
                            size: 18,
                          ),
                    label: Text(
                      isEditing
                          ? context.l10n.catSaveChanges
                          : context.l10n.catCreate,
                    ),
                    style: FilledButton.styleFrom(
                      backgroundColor: currentColor,
                    ),
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

class _FieldLabel extends StatelessWidget {
  final String label;

  const _FieldLabel(this.label);

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Text(
        label,
        style: const TextStyle(
          fontSize: 12,
          fontWeight: FontWeight.w600,
          color: AppColors.textSecondary,
        ),
      ),
    );
  }
}
