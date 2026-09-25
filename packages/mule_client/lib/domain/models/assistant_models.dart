// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

/// Connection state of the assistant WebSocket transport.
enum AssistantConnState { disconnected, connecting, connected, error }

/// Kind of an `assistant.event` notification emitted by mule-assistant.
enum AssistantEventType { token, toolStart, toolResult, finalText, notice, error }

/// Parsed `assistant.event` notification.
class AssistantEvent {
  final AssistantEventType type;
  final String? content;
  final String? toolName;
  final Map<String, dynamic>? args;
  final bool isError;

  const AssistantEvent({
    required this.type,
    this.content,
    this.toolName,
    this.args,
    this.isError = false,
  });

  factory AssistantEvent.fromJson(Map<String, dynamic> json) {
    final type = switch (json['type']) {
      'token' => AssistantEventType.token,
      'toolStart' => AssistantEventType.toolStart,
      'toolResult' => AssistantEventType.toolResult,
      'final' => AssistantEventType.finalText,
      'error' => AssistantEventType.error,
      _ => AssistantEventType.notice,
    };
    return AssistantEvent(
      type: type,
      content: (json['content'] ?? json['message']) as String?,
      toolName: json['name'] as String?,
      args: json['args'] as Map<String, dynamic>?,
      isError: json['is_error'] == true,
    );
  }
}

/// Pending user confirmation for a protected tool call
/// (`assistant.confirmRequired` notification).
class AssistantConfirmRequest {
  final String toolName;
  final Map<String, dynamic> args;

  const AssistantConfirmRequest({required this.toolName, required this.args});

  factory AssistantConfirmRequest.fromJson(Map<String, dynamic> json) =>
      AssistantConfirmRequest(
        toolName: json['name'] as String? ?? '',
        args: json['args'] as Map<String, dynamic>? ?? const {},
      );
}

/// One visible entry of the assistant conversation.
enum ChatEntryKind { user, assistant, toolCall, toolResult, notice, error }

class ChatEntry {
  final ChatEntryKind kind;
  final String text;
  final String? toolName;
  final bool isError;

  const ChatEntry({
    required this.kind,
    required this.text,
    this.toolName,
    this.isError = false,
  });
}
