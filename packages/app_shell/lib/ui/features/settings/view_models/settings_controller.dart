// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n_lookup.dart';

/// Sub-domain controller for daemon settings.
///
/// Owns the raw settings map. Status and error reporting is delegated to
/// the host via [onStatus]/[onError]; [onSettingsLoaded] lets the host
/// propagate settings to other controllers (e.g. history retention).
class SettingsController extends ChangeNotifier {
  final MuleDaemonClient client;

  final void Function(String message)? onStatus;
  final void Function(String message)? onError;

  /// Called after every successful settings fetch.
  final void Function(Map<String, dynamic> settings)? onSettingsLoaded;

  StreamSubscription<SettingsChangedEvent>? _settingsSub;

  SettingsController({
    required this.client,
    this.onStatus,
    this.onError,
    this.onSettingsLoaded,
  }) {
    // Event-driven refresh (`settings.changed`) — multi-client sync.
    _settingsSub = client.settingsEvents.listen((_) => fetch());
  }

  @override
  void dispose() {
    _settingsSub?.cancel();
    super.dispose();
  }

  Map<String, dynamic> _settings = {};

  Map<String, dynamic> get settings => _settings;

  Future<void> fetch() async {
    try {
      _settings = await client.getSettings();
      onSettingsLoaded?.call(_settings);
      notifyListeners();
    } catch (e) {
      // Non-critical in older or restricted daemons
      AppLogger.debug('settings', 'Settings fetch failed', error: e);
    }
  }

  /// Applies a raw `settings.set` patch then refreshes the map.
  Future<bool> updateSettings(Map<String, dynamic> patch) async {
    try {
      await client.updateSettings(patch);
      await fetch();
      onStatus?.call(L10nLookup.current.msgSettingsUpdated);
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrSettingsUpdate(e));
      return false;
    }
  }

  Future<bool> setDefaultDownloadDirectory(String path) async {
    try {
      await client.setDefaultDownloadDirectory(path);
      await fetch();
      onStatus?.call(L10nLookup.current.msgDownloadDirUpdated);
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrDownloadDir(e));
      notifyListeners();
      return false;
    }
  }

  Future<bool> resetDefaultDownloadDirectory() async {
    try {
      await client.resetDefaultDownloadDirectory();
      await fetch();
      onStatus?.call(L10nLookup.current.msgDownloadDirReset);
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrDownloadDirReset(e));
      notifyListeners();
      return false;
    }
  }
}
