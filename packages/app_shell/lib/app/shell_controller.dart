// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/foundation.dart';

enum AppNavSection { downloads, uploads, shares, history, servers, search, kad, settings }

enum DownloadFilter {
  all,
  active,
  paused,
  completed,
  failed;

  String get label {
    switch (this) {
      case DownloadFilter.all:
        return 'All Downloads';
      case DownloadFilter.active:
        return 'Active';
      case DownloadFilter.paused:
        return 'Paused';
      case DownloadFilter.completed:
        return 'Completed';
      case DownloadFilter.failed:
        return 'Failed';
    }
  }
}


/// App-shell UI state: current nav section, downloads filter & search
/// query, debug bar visibility, and the global status/error banners.
///
/// [onSectionSelected] is invoked when the section changes so the host can
/// trigger per-section refreshes. [isDevToolsAvailable] gates the debug bar.
class ShellController extends ChangeNotifier {
  /// Whether dev tools (debug bar) are available.
  final bool isDevToolsAvailable;

  /// Called after the section changes (e.g. to fetch that section's data).
  final void Function(AppNavSection section)? onSectionSelected;

  ShellController({
    this.isDevToolsAvailable = false,
    this.onSectionSelected,
  });

  AppNavSection _section = AppNavSection.downloads;
  DownloadFilter _filter = DownloadFilter.all;
  String _searchQuery = '';
  bool _debugBarVisible = false;
  String? _errorMessage;
  String? _protocolMismatchError;
  String? _statusNotification;

  AppNavSection get section => _section;
  DownloadFilter get filter => _filter;
  String get searchQuery => _searchQuery;
  String? get errorMessage => _errorMessage;
  String? get protocolMismatchError => _protocolMismatchError;
  String? get statusNotification => _statusNotification;
  bool get isDebugBarVisible =>
      isDevToolsAvailable && _debugBarVisible;

  void setSection(AppNavSection section) {
    _section = section;
    onSectionSelected?.call(section);
    notifyListeners();
  }

  void setFilter(DownloadFilter filter) {
    _section = AppNavSection.downloads;
    _filter = filter;
    notifyListeners();
  }

  void setSearchQuery(String query) {
    _searchQuery = query;
    notifyListeners();
  }

  void toggleDebugBar() {
    if (!isDevToolsAvailable) return;
    _debugBarVisible = !_debugBarVisible;
    notifyListeners();
  }

  void setStatus(String message) {
    _statusNotification = message;
    notifyListeners();
  }

  void setError(String? message) {
    _errorMessage = message;
    notifyListeners();
  }

  void setProtocolMismatch(String? message) {
    _protocolMismatchError = message;
    notifyListeners();
  }

  void clearStatusNotification() {
    _statusNotification = null;
    notifyListeners();
  }
}
