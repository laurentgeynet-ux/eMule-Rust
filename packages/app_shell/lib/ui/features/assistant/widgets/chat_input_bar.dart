// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Barre de saisie du chat : champ texte + envoi, remplacé par un
/// bouton d'annulation pendant un tour en cours.
class ChatInputBar extends StatefulWidget {
  final bool turnActive;
  final bool enabled;
  final ValueChanged<String> onSend;
  final VoidCallback onCancel;

  const ChatInputBar({
    super.key,
    required this.turnActive,
    required this.enabled,
    required this.onSend,
    required this.onCancel,
  });

  @override
  State<ChatInputBar> createState() => _ChatInputBarState();
}

class _ChatInputBarState extends State<ChatInputBar> {
  final TextEditingController _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _submit() {
    final text = _controller.text.trim();
    if (text.isEmpty) return;
    _controller.clear();
    widget.onSend(text);
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return Container(
      padding: AppSpacing.pagePadding,
      decoration: AppDecorations.footerBar,
      child: Row(
        children: [
          Expanded(
            child: TextField(
              controller: _controller,
              enabled: widget.enabled && !widget.turnActive,
              style: AppTextStyles.bodySmall
                  .copyWith(color: AppColors.textPrimary),
              decoration: InputDecoration(
                hintText: l10n.assistantInputHint,
                hintStyle: AppTextStyles.bodySmall
                    .copyWith(color: AppColors.textFaint),
                filled: true,
                fillColor: AppColors.fillSubtle,
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(AppRadius.md),
                  borderSide: BorderSide.none,
                ),
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: AppSpacing.md,
                  vertical: AppSpacing.sm,
                ),
              ),
              onSubmitted: (_) => _submit(),
            ),
          ),
          const SizedBox(width: AppSpacing.sm),
          if (widget.turnActive)
            IconButton(
              onPressed: widget.onCancel,
              tooltip: l10n.assistantCancel,
              icon: const Icon(Icons.stop_circle_outlined,
                  color: AppColors.danger),
            )
          else
            IconButton(
              onPressed: widget.enabled ? _submit : null,
              tooltip: l10n.assistantSend,
              icon: const Icon(Icons.send_rounded, color: AppColors.accent),
            ),
        ],
      ),
    );
  }
}
