// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:app_shell/l10n/l10n.dart';

/// Connect form shown when no daemon URL is configured. Validates the
/// WebSocket address and hands it plus the optional auth token to
/// [onConnect].
class DaemonConnectScreen extends StatefulWidget {
  final void Function(String url, String? token) onConnect;
  final String? initialUrl;

  const DaemonConnectScreen({
    super.key,
    required this.onConnect,
    this.initialUrl,
  });

  @override
  State<DaemonConnectScreen> createState() => _DaemonConnectScreenState();
}

class _DaemonConnectScreenState extends State<DaemonConnectScreen> {
  late final TextEditingController _urlController =
      TextEditingController(text: widget.initialUrl ?? '');
  late final TextEditingController _tokenController = TextEditingController();
  String? _error;

  String? _normalize(String raw) {
    var text = raw.trim();
    if (text.isEmpty) return null;
    if (!text.contains('://')) text = 'ws://$text';
    final uri = Uri.tryParse(text);
    if (uri == null || !uri.hasScheme || uri.host.isEmpty) return null;
    if (uri.scheme != 'ws' && uri.scheme != 'wss') return null;
    return uri.toString();
  }

  void _submit() {
    final url = _normalize(_urlController.text);
    if (url == null) {
      setState(() => _error = context.l10n.daemonUrlInvalid);
      return;
    }
    final token = _tokenController.text.trim();
    widget.onConnect(url, token.isEmpty ? null : token);
  }

  @override
  void dispose() {
    _urlController.dispose();
    _tokenController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppTheme.darkBackground,
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: Padding(
            padding: AppSpacing.pagePadding,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                const Icon(Icons.cloud_outlined,
                    size: 40, color: AppColors.accentTeal),
                const SizedBox(height: AppSpacing.lg),
                Text(
                  context.l10n.daemonConnectTitle,
                  textAlign: TextAlign.center,
                  style: AppTextStyles.sectionTitle.copyWith(fontSize: 22),
                ),
                const SizedBox(height: AppSpacing.sm),
                Text(
                  context.l10n.daemonConnectDescription,
                  textAlign: TextAlign.center,
                  style: AppTextStyles.bodySmall.copyWith(color: AppColors.textSecondary),
                ),
                const SizedBox(height: AppSpacing.xl),
                TextField(
                  controller: _urlController,
                  autofocus: true,
                  style: const TextStyle(color: AppColors.textPrimary),
                  decoration: InputDecoration(
                    labelText: context.l10n.daemonUrlLabel,
                    hintText: context.l10n.daemonUrlHint,
                    errorText: _error,
                  ),
                  onChanged: (_) {
                    if (_error != null) setState(() => _error = null);
                  },
                  onSubmitted: (_) => _submit(),
                ),
                const SizedBox(height: AppSpacing.md),
                TextField(
                  controller: _tokenController,
                  obscureText: true,
                  style: const TextStyle(color: AppColors.textPrimary),
                  decoration: InputDecoration(
                    labelText: context.l10n.daemonTokenLabel,
                    hintText: context.l10n.daemonTokenHint,
                  ),
                  onSubmitted: (_) => _submit(),
                ),
                const SizedBox(height: AppSpacing.lg),
                FilledButton.icon(
                  onPressed: _submit,
                  icon: const Icon(Icons.link, size: 18),
                  label: Text(context.l10n.connect),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
