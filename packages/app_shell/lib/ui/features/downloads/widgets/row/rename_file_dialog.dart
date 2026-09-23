// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../l10n/l10n.dart';

/// Dialogue de renommage de fichier partagé entre l'onglet
/// Téléchargements (`downloads.rename`) et l'onglet Partages
/// (`shares.rename`) — équivalent du renommage F2 d'eMule.
///
/// [onSubmit] effectue l'appel RPC ; le dialogue se ferme quand le
/// callback renvoie `true`, sinon affiche une erreur en ligne.
Future<void> showRenameFileDialog(
  BuildContext context, {
  required String currentName,
  required Future<bool> Function(String newName) onSubmit,
}) {
  return showDialog(
    context: context,
    builder: (ctx) =>
        _RenameFileDialog(currentName: currentName, onSubmit: onSubmit),
  );
}

class _RenameFileDialog extends StatefulWidget {
  final String currentName;
  final Future<bool> Function(String newName) onSubmit;

  const _RenameFileDialog({required this.currentName, required this.onSubmit});

  @override
  State<_RenameFileDialog> createState() => _RenameFileDialogState();
}

class _RenameFileDialogState extends State<_RenameFileDialog> {
  late final TextEditingController _controller;
  String? _error;
  bool _busy = false;

  /// Caractères interdits Windows + séparateurs de chemin + contrôle.
  static final _illegal = RegExp(r'[<>:"/\\|?*\x00-\x1F]');

  static bool _isValidName(String name) {
    return name.isNotEmpty &&
        name != '.' &&
        name != '..' &&
        !name.endsWith('.') &&
        !name.endsWith(' ') &&
        !_illegal.hasMatch(name);
  }

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController(text: widget.currentName);
    // Pré-sélectionne le nom sans l'extension (convention eMule).
    final dot = widget.currentName.lastIndexOf('.');
    _controller.selection = TextSelection(
      baseOffset: 0,
      extentOffset: dot > 0 ? dot : widget.currentName.length,
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_busy) return;
    final name = _controller.text.trim();
    if (!_isValidName(name)) {
      setState(() => _error = context.l10n.renameFileInvalid);
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    final ok = await widget.onSubmit(name);
    if (!mounted) return;
    if (ok) {
      Navigator.of(context).pop();
    } else {
      setState(() {
        _busy = false;
        _error = context.l10n.renameFileFailed;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return AlertDialog(
      title: Text(l10n.renameFileTitle),
      content: SizedBox(
        width: 420,
        child: TextField(
          controller: _controller,
          autofocus: true,
          enabled: !_busy,
          decoration: InputDecoration(
            labelText: l10n.renameFileLabel,
            errorText: _error,
          ),
          onSubmitted: (_) => _submit(),
        ),
      ),
      actions: [
        TextButton(
          onPressed: _busy ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: _busy ? null : _submit,
          child: Text(l10n.renameFileApply),
        ),
      ],
    );
  }
}
