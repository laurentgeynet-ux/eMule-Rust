// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:collection';

import 'package:flutter/foundation.dart';

/// Niveaux de log applicatifs.
enum AppLogLevel { debug, info, warning, error }

/// Entrée du ring buffer mémoire. Les messages sont épurés à la source
/// par les appelants : jamais de token d'authentification, jamais d'IP
/// externe brute, jamais de secret.
class AppLogEntry {
  final DateTime timestamp;
  final AppLogLevel level;
  final String tag;
  final String message;

  const AppLogEntry({
    required this.timestamp,
    required this.level,
    required this.tag,
    required this.message,
  });

  String get levelName => level.name.toUpperCase();

  @override
  String toString() =>
      '${timestamp.toIso8601String()} $levelName [$tag] $message';
}

/// Façade de log locale « premium » : ring buffer borné (500 entrées)
/// alimentant l'export « Diagnostics », `debugPrint` en mode debug
/// uniquement — en release, rien ne sort sur la console et tout reste
/// dans le buffer (zéro fuite, zéro télémétrie externe).
class AppLogger {
  AppLogger._();

  /// Capacité du ring buffer (entrées les plus récentes conservées).
  static const int capacity = 500;

  /// Stack traces tronquées à cette profondeur dans le buffer.
  static const int _stackLines = 12;

  static final ListQueue<AppLogEntry> _buffer =
      ListQueue<AppLogEntry>(capacity);
  static int _dropped = 0;

  /// Entrées conservées, ordre chronologique (lecture seule).
  static List<AppLogEntry> get entries => List.unmodifiable(_buffer);

  /// Nombre d'entrées évincées par saturation du buffer.
  static int get droppedCount => _dropped;

  static void log(
    AppLogLevel level,
    String tag,
    String message, {
    Object? error,
    StackTrace? stackTrace,
  }) {
    final text = StringBuffer(message);
    if (error != null) text.write(' — $error');
    if (stackTrace != null && level == AppLogLevel.error) {
      final lines = stackTrace.toString().trimRight().split('\n');
      final keep =
          lines.length > _stackLines ? lines.sublist(0, _stackLines) : lines;
      text.write('\n${keep.join('\n')}');
    }
    final entry = AppLogEntry(
      timestamp: DateTime.now(),
      level: level,
      tag: tag,
      message: text.toString(),
    );
    if (_buffer.length == capacity) {
      _buffer.removeFirst();
      _dropped++;
    }
    _buffer.addLast(entry);
    if (kDebugMode) {
      debugPrint(entry.toString());
    }
  }

  static void debug(String tag, String message, {Object? error}) =>
      log(AppLogLevel.debug, tag, message, error: error);

  static void info(String tag, String message, {Object? error}) =>
      log(AppLogLevel.info, tag, message, error: error);

  static void warn(
    String tag,
    String message, {
    Object? error,
    StackTrace? stackTrace,
  }) =>
      log(AppLogLevel.warning, tag, message,
          error: error, stackTrace: stackTrace);

  static void error(
    String tag,
    String message, {
    Object? error,
    StackTrace? stackTrace,
  }) =>
      log(AppLogLevel.error, tag, message,
          error: error, stackTrace: stackTrace);

  /// Rendu texte des `maxEntries` dernières entrées pour l'export
  /// diagnostics. Chaque entrée est déjà épurée à la source.
  static String dump({int maxEntries = 100}) {
    final all = _buffer.toList();
    final start = all.length > maxEntries ? all.length - maxEntries : 0;
    final sb = StringBuffer();
    if (_dropped > 0) {
      sb.writeln('(… $_dropped entrée(s) plus anciennes évincées …)');
    }
    for (final e in all.sublist(start)) {
      sb.writeln(e.toString());
    }
    return sb.toString().trimRight();
  }

  /// Vide le buffer (tests).
  @visibleForTesting
  static void clear() {
    _buffer.clear();
    _dropped = 0;
  }
}
