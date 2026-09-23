// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

import '../../../../l10n/l10n_lookup.dart';

/// Sub-domain controller for the History & Statistics section.
///
/// Owns the history entries, stats, filters and retention policy. Status and
/// error reporting is delegated to the host via [onStatus]/[onError] so the
/// global banners keep working.
class HistoryController extends ChangeNotifier {
  final MuleDaemonClient client;

  /// Called with a transient status message (shown in the shell banner).
  final void Function(String message)? onStatus;

  /// Called with an error message on failed RPC calls.
  final void Function(String message)? onError;

  StreamSubscription<int>? _historySub;

  HistoryController({required this.client, this.onStatus, this.onError}) {
    // Event-driven refresh (`history.entryAdded`).
    _historySub = client.historyEvents.listen((_) {
      if (!_isLoading) fetch();
    });
  }

  @override
  void dispose() {
    _historySub?.cancel();
    super.dispose();
  }

  List<HistoryEntry> _entries = [];
  HistoryStats _stats = const HistoryStats();
  bool _isLoading = false;
  String? _eventTypeFilter;
  String? _statusFilter;
  int _retentionDays = 30;

  List<HistoryEntry> get entries => _entries;
  HistoryStats get stats => _stats;
  bool get isLoading => _isLoading;
  String? get eventTypeFilter => _eventTypeFilter;
  String? get statusFilter => _statusFilter;
  int get retentionDays => _retentionDays;

  /// Applies the retention policy found in daemon settings (see
  /// `AppController.fetchSettings`).
  void syncRetentionFromSettings(Map<String, dynamic> settings) {
    _retentionDays =
        (settings['historyRetentionDays'] as num?)?.toInt() ?? 30;
  }

  void setEventTypeFilter(String? eventType) {
    _eventTypeFilter = eventType;
    fetch();
  }

  void setStatusFilter(String? status) {
    _statusFilter = status;
    fetch();
  }

  Future<void> fetch() async {
    _isLoading = true;
    notifyListeners();
    try {
      _entries = await client.listHistory(
        eventType: _eventTypeFilter,
        status: _statusFilter,
      );
      _stats = await client.getHistoryStats();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrFetchHistory(e));
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<bool> clear() async {
    try {
      _isLoading = true;
      notifyListeners();
      final count = await client.clearHistory();
      _entries.clear();
      _stats = await client.getHistoryStats();
      onStatus?.call(L10nLookup.current.msgHistoryCleared(count));
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrClearHistory(e));
      return false;
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  Future<String> export(String format) async {
    try {
      return await client.exportHistory(
        format,
        eventType: _eventTypeFilter,
        status: _statusFilter,
      );
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrExportHistory(e));
      return '';
    }
  }

  Future<bool> setRetentionDays(int days) async {
    try {
      await client.setHistoryRetentionDays(days);
      _retentionDays = days;
      onStatus?.call(
        days == 0
            ? L10nLookup.current.msgRetentionUnlimited
            : L10nLookup.current.msgRetentionDays(days),
      );
      await fetch();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrRetention(e));
      notifyListeners();
      return false;
    }
  }
}
