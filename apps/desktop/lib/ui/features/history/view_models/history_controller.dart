import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

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

  HistoryController({required this.client, this.onStatus, this.onError});

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
      onError?.call('Failed to fetch history: $e');
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
      onStatus?.call('Cleared $count history record(s)');
      return true;
    } catch (e) {
      onError?.call('Failed to clear history: $e');
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
      onError?.call('Failed to export history: $e');
      return '';
    }
  }

  Future<bool> setRetentionDays(int days) async {
    try {
      await client.setHistoryRetentionDays(days);
      _retentionDays = days;
      onStatus?.call(
        days == 0
            ? 'History retention set to unlimited'
            : 'History retention set to $days days',
      );
      await fetch();
      return true;
    } catch (e) {
      onError?.call('Failed to update retention policy: $e');
      notifyListeners();
      return false;
    }
  }
}
