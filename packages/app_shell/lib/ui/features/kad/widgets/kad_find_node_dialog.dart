// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';
import '../view_models/kad_controller.dart';

/// Dialog: iterative Kad `findNode` lookup for a target KadID — sends
/// `KADEMLIA2_FIND_NODE` rounds and lists the closest contacts returned.
Future<void> showKadFindNodeDialog(
  BuildContext context,
  KadController controller,
) {
  return showDialog(
    context: context,
    builder: (_) => _KadFindNodeDialog(controller: controller),
  );
}

class _KadFindNodeDialog extends StatefulWidget {
  final KadController controller;

  const _KadFindNodeDialog({required this.controller});

  @override
  State<_KadFindNodeDialog> createState() => _KadFindNodeDialogState();
}

class _KadFindNodeDialogState extends State<_KadFindNodeDialog> {
  static final _kadIdRe = RegExp(r'^[0-9a-fA-F]{32}$');

  final _idCtrl = TextEditingController();
  List<KadContactModel>? _contacts;
  bool _searching = false;
  String? _error;

  @override
  void dispose() {
    _idCtrl.dispose();
    super.dispose();
  }

  Future<void> _search() async {
    final l10n = context.l10n;
    final id = _idCtrl.text.trim();
    if (!_kadIdRe.hasMatch(id)) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.kadFindNodeInvalid)),
      );
      return;
    }
    setState(() {
      _searching = true;
      _error = null;
      _contacts = null;
    });
    try {
      final contacts = await widget.controller.findNode(id);
      if (mounted) setState(() => _contacts = contacts);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _searching = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return AlertDialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      title: Row(
        children: [
          const Icon(
            Icons.travel_explore_rounded,
            color: AppColors.accentTeal,
            size: 20,
          ),
          const SizedBox(width: 8),
          Text(l10n.kadFindNodeTitle),
        ],
      ),
      content: SizedBox(
        width: 440,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              controller: _idCtrl,
              style: const TextStyle(
                fontFamily: 'monospace',
                fontSize: 12,
                color: AppColors.textPrimary,
              ),
              decoration: InputDecoration(
                labelText: l10n.kadFindNodeIdLabel,
                isDense: true,
                filled: true,
                fillColor: AppTheme.darkCard,
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(6),
                ),
                suffixIcon: IconButton(
                  icon: const Icon(Icons.search_rounded, size: 18),
                  tooltip: l10n.kadFindNodeAction,
                  onPressed: _searching ? null : _search,
                ),
              ),
              onSubmitted: (_) => _search(),
            ),
            const SizedBox(height: 12),
            if (_searching)
              const Center(
                child: Padding(
                  padding: EdgeInsets.all(16),
                  child: SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                ),
              )
            else if (_error != null)
              Text(
                _error!,
                style: const TextStyle(
                  fontSize: 12,
                  color: AppTheme.statusFailed,
                ),
              )
            else if (_contacts != null)
              if (_contacts!.isEmpty)
                Text(
                  l10n.kadFindNodeEmpty,
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textMuted,
                  ),
                )
              else ...[
                Text(
                  l10n.kadFindNodeResults(_contacts!.length),
                  style: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                    color: AppColors.textSecondary,
                  ),
                ),
                const SizedBox(height: 6),
                ConstrainedBox(
                  constraints: const BoxConstraints(maxHeight: 220),
                  child: ListView.separated(
                    shrinkWrap: true,
                    itemCount: _contacts!.length,
                    separatorBuilder: (_, __) =>
                        const Divider(height: 1, color: AppTheme.darkBorder),
                    itemBuilder: (context, i) => _ContactLine(
                      contact: _contacts![i],
                    ),
                  ),
                ),
              ],
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(
            l10n.btnClose,
            style: const TextStyle(color: AppColors.textSoft),
          ),
        ),
      ],
    );
  }
}

class _ContactLine extends StatelessWidget {
  final KadContactModel contact;

  const _ContactLine({required this.contact});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Row(
        children: [
          Expanded(
            child: Tooltip(
              message: contact.kadId,
              child: Text(
                contact.kadId,
                style: const TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 10,
                  color: AppColors.textSecondary,
                ),
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ),
          const SizedBox(width: 8),
          Text(
            '${contact.ip}:${contact.udpPort}',
            style: const TextStyle(
              fontFamily: 'monospace',
              fontSize: 10,
              color: AppColors.textMuted,
            ),
          ),
        ],
      ),
    );
  }
}
