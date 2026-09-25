// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Bulle de conversation : messages utilisateur (alignés à droite,
/// accentués), réponses de l'assistant (à gauche, neutres), notices et
/// erreurs de tour. Les tuiles d'outils vivent dans `ToolEventTile`.
class ChatBubble extends StatelessWidget {
  final ChatEntry entry;

  /// Contenu en cours de streaming (bulle assistant provisoire).
  final String? streamingText;

  const ChatBubble({super.key, required this.entry, this.streamingText});

  /// Bulle provisoire affichée pendant l'arrivée des tokens.
  const ChatBubble.streaming({super.key, required this.streamingText})
      : entry = const ChatEntry(kind: ChatEntryKind.assistant, text: '');

  @override
  Widget build(BuildContext context) {
    final isUser = entry.kind == ChatEntryKind.user;
    final isError = entry.kind == ChatEntryKind.error;
    final isNotice = entry.kind == ChatEntryKind.notice;
    final text = streamingText ?? entry.text;

    final color = isUser
        ? AppColors.fillMedium
        : isError
            ? AppColors.tintRed.withValues(alpha: 0.15)
            : AppColors.fillLight;
    final textStyle = isNotice
        ? AppTextStyles.metricLabel
        : AppTextStyles.bodySmall.copyWith(
            color: isError ? AppColors.danger : AppColors.textPrimary,
          );

    return Align(
      alignment: isUser ? Alignment.centerRight : Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 640),
        child: Container(
          margin: const EdgeInsets.symmetric(vertical: AppSpacing.xxs),
          padding: const EdgeInsets.symmetric(
            horizontal: AppSpacing.md,
            vertical: AppSpacing.sm,
          ),
          decoration: BoxDecoration(
            color: color,
            borderRadius: BorderRadius.circular(AppRadius.lg),
          ),
          child: SelectableText(text, style: textStyle),
        ),
      ),
    );
  }
}
