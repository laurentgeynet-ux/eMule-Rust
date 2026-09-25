// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/providers.dart';
import '../../../../l10n/l10n.dart';
import '../widgets/assistant_offline.dart';
import '../widgets/assistant_status_chip.dart';
import '../widgets/chat_bubble.dart';
import '../widgets/chat_input_bar.dart';
import '../widgets/confirm_banner.dart';
import '../widgets/tool_event_tile.dart';

/// Assistant local view: composition only — status header, message list,
/// confirmation banner, input bar. State lives in
/// [assistantControllerProvider].
class AssistantView extends ConsumerStatefulWidget {
  const AssistantView({super.key});

  @override
  ConsumerState<AssistantView> createState() => _AssistantViewState();
}

class _AssistantViewState extends ConsumerState<AssistantView> {
  final _scrollController = ScrollController();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(assistantControllerProvider).connect();
    });
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.jumpTo(_scrollController.position.maxScrollExtent);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final ctl = ref.watch(assistantControllerProvider);
    final l10n = context.l10n;
    _scrollToBottom();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Container(
          padding: AppSpacing.toolbarPadding,
          decoration: AppDecorations.headerBar,
          child: Row(
            children: [
              Text(l10n.assistantTitle, style: AppTextStyles.sectionTitle),
              const Spacer(),
              AssistantStatusChip(
                state: ctl.state,
                visibleTools: ctl.visibleTools,
              ),
            ],
          ),
        ),
        Expanded(
          child: ctl.isConnected || ctl.turnActive || ctl.entries.isNotEmpty
              ? ListView(
                  controller: _scrollController,
                  padding: AppSpacing.pagePadding,
                  children: [
                    if (ctl.entries.isEmpty && ctl.streamingText.isEmpty)
                      _EmptyHint(text: l10n.assistantEmpty),
                    for (final entry in ctl.entries)
                      switch (entry.kind) {
                        ChatEntryKind.toolCall ||
                        ChatEntryKind.toolResult =>
                          ToolEventTile(entry: entry),
                        _ => ChatBubble(entry: entry),
                      },
                    if (ctl.streamingText.isNotEmpty)
                      ChatBubble.streaming(streamingText: ctl.streamingText),
                  ],
                )
              : AssistantOffline(
                  detail: ctl.lastError,
                  onRetry: ctl.connect,
                ),
        ),
        if (ctl.pendingConfirm != null)
          ConfirmBanner(request: ctl.pendingConfirm!, onDecision: ctl.confirm),
        ChatInputBar(
          turnActive: ctl.turnActive,
          enabled: ctl.isConnected,
          onSend: ctl.send,
          onCancel: ctl.cancel,
        ),
      ],
    );
  }
}

class _EmptyHint extends StatelessWidget {
  final String text;

  const _EmptyHint({required this.text});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(AppSpacing.xxl),
      child: Text(
        text,
        style: AppTextStyles.bodySmall.copyWith(color: AppColors.textMuted),
        textAlign: TextAlign.center,
      ),
    );
  }
}
