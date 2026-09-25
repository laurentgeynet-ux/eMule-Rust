// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/providers.dart';
import '../../../../l10n/l10n.dart';

/// Shows the manual share dialog (`shares.add`): registers an
/// already-hashed file as a share. The daemon does not re-hash — the
/// 32-char eD2K hash must be provided (e.g. restored from a `.met`
/// backup or a previous listing).
Future<void> showShareAddDialog(BuildContext context) {
  return showDialog(
    context: context,
    builder: (_) => const _ShareAddDialog(),
  );
}

class _ShareAddDialog extends ConsumerStatefulWidget {
  const _ShareAddDialog();

  @override
  ConsumerState<_ShareAddDialog> createState() => _ShareAddDialogState();
}

class _ShareAddDialogState extends ConsumerState<_ShareAddDialog> {
  static final _hashRe = RegExp(r'^[0-9a-fA-F]{32}$');

  final _hashCtrl = TextEditingController();
  final _nameCtrl = TextEditingController();
  final _sizeCtrl = TextEditingController();
  final _pathCtrl = TextEditingController();
  bool _saving = false;

  @override
  void dispose() {
    _hashCtrl.dispose();
    _nameCtrl.dispose();
    _sizeCtrl.dispose();
    _pathCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final l10n = context.l10n;
    final hash = _hashCtrl.text.trim();
    final name = _nameCtrl.text.trim();
    final size = int.tryParse(_sizeCtrl.text.trim());
    if (!_hashRe.hasMatch(hash) || name.isEmpty || size == null || size <= 0) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.sharesAddInvalid)),
      );
      return;
    }
    setState(() => _saving = true);
    try {
      await ref.read(sharesControllerProvider).addSharedFile(
            fileHash: hash,
            fileName: name,
            fileSize: size,
            filePath: _pathCtrl.text.trim(),
          );
      if (mounted) {
        final messenger = ScaffoldMessenger.of(context);
        Navigator.of(context).pop();
        messenger.showSnackBar(
          SnackBar(content: Text(l10n.msgShareAdded)),
        );
      }
    } catch (e) {
      if (mounted) {
        setState(() => _saving = false);
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.msgErrShareAdd('$e'))),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    InputDecoration deco(String label) => InputDecoration(
          labelText: label,
          isDense: true,
          filled: true,
          fillColor: AppTheme.darkCard,
          border: OutlineInputBorder(borderRadius: BorderRadius.circular(6)),
        );
    const fieldStyle = TextStyle(fontSize: 12, color: AppColors.textPrimary);

    return AlertDialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      title: Row(
        children: [
          const Icon(
            Icons.playlist_add_rounded,
            color: AppColors.accentTeal,
            size: 20,
          ),
          const SizedBox(width: 8),
          Text(l10n.sharesAddTitle),
        ],
      ),
      content: SizedBox(
        width: 420,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              controller: _hashCtrl,
              style: fieldStyle,
              decoration: deco(l10n.sharesAddHashLabel),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _nameCtrl,
              style: fieldStyle,
              decoration: deco(l10n.sharesAddNameLabel),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _sizeCtrl,
              keyboardType: TextInputType.number,
              style: fieldStyle,
              decoration: deco(l10n.sharesAddSizeLabel),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: _pathCtrl,
              style: fieldStyle,
              decoration: deco(l10n.sharesAddPathLabel),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(
            l10n.btnCancel,
            style: const TextStyle(color: AppColors.textSoft),
          ),
        ),
        FilledButton(
          onPressed: _saving ? null : _submit,
          child: _saving
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : Text(l10n.sharesAddAction),
        ),
      ],
    );
  }
}
