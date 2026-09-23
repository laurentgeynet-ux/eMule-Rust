// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

import '../../../../l10n/enum_labels.dart';
import '../../../../l10n/l10n_lookup.dart';

/// Sub-domain controller for the network policy (profile, kill switch,
/// resource budgets), the IP filter and the rate limiter.
///
/// Status and error reporting is delegated to the host via
/// [onStatus]/[onError]; [onAllDisconnected] is invoked after a successful
/// kill switch so the host can reset dependent sessions (server, …).
class NetworkPolicyController extends ChangeNotifier {
  final MuleDaemonClient client;

  final void Function(String message)? onStatus;
  final void Function(String message)? onError;

  /// Called after `disconnectAll` succeeds (kill switch engaged).
  final void Function()? onAllDisconnected;

  StreamSubscription<int>? _policySub;

  NetworkPolicyController({
    required this.client,
    this.onStatus,
    this.onError,
    this.onAllDisconnected,
  }) {
    // Event-driven refresh (`network.policyChanged`) — kill switch /
    // profile / budgets stay in sync across connected clients.
    _policySub = client.networkPolicyEvents.listen((_) => fetchPolicy());
  }

  @override
  void dispose() {
    _policySub?.cancel();
    super.dispose();
  }

  NetworkPolicySnapshot? _policy;
  List<NetworkAuditEntryItem> _auditLog = [];
  FilterStatusModel? _filterStatus;
  RateLimitsModel? _rateLimits;

  NetworkPolicySnapshot? get policy => _policy;
  List<NetworkAuditEntryItem> get auditLog => _auditLog;
  FilterStatusModel? get filterStatus => _filterStatus;
  RateLimitsModel? get rateLimits => _rateLimits;

  NetworkProfile get currentProfile =>
      _policy?.profile ?? NetworkProfile.publicEd2kKadPreview;
  NetworkSubsystemState get currentState =>
      _policy?.state ?? NetworkSubsystemState.previewActive;
  bool get isKillSwitchActive =>
      currentState == NetworkSubsystemState.disabled;

  Future<void> fetchPolicy() async {
    try {
      _policy = await client.getNetworkPolicy();
      notifyListeners();
    } catch (_) {}
  }

  Future<void> setProfile(NetworkProfile profile) async {
    try {
      _policy = await client.setNetworkProfile(profile);
      onStatus?.call(
        L10nLookup.current
            .msgProfileSwitched(profile.labelL10n(L10nLookup.current)),
      );
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrProfileSwitch(e));
      notifyListeners();
    }
  }

  Future<void> disconnectAll() async {
    try {
      await client.disconnectAll();
      await fetchPolicy();
      onAllDisconnected?.call();
      onStatus?.call(L10nLookup.current.msgKillSwitchOn);
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrKillSwitch(e));
      notifyListeners();
    }
  }

  Future<void> enableNetwork() async {
    try {
      await client.enableNetwork();
      await fetchPolicy();
      onStatus?.call(L10nLookup.current.msgNetworkReactivated);
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrNetworkReactivate(e));
      notifyListeners();
    }
  }

  Future<void> fetchAuditLog({int limit = 50}) async {
    try {
      _auditLog = await client.getNetworkAuditLog(limit: limit);
      notifyListeners();
    } catch (_) {}
  }

  Future<void> setResourceBudgets(ResourceBudgetsModel budgets) async {
    try {
      _policy = await client.setResourceBudgets(budgets);
      onStatus?.call(L10nLookup.current.msgBudgetsUpdated);
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrBudgets(e));
      notifyListeners();
    }
  }

  // ── IP Filter ─────────────────────────────────────────────────────────────

  Future<void> fetchFilterStatus() async {
    try {
      _filterStatus = await client.getFilterStatus();
      notifyListeners();
    } catch (_) {}
  }

  Future<bool> importFilterFile(String filePath) async {
    try {
      _filterStatus = await client.importFilterFile(filePath);
      onStatus?.call(
        L10nLookup.current
            .msgIpFilterImported(_filterStatus?.rangeCount ?? 0),
      );
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrIpFilterImport(e));
      notifyListeners();
      return false;
    }
  }

  Future<void> setFilterEnabled(bool enabled) async {
    try {
      _filterStatus = await client.setFilterEnabled(enabled);
      onStatus?.call(
        enabled
            ? L10nLookup.current.msgIpFilterEnabled
            : L10nLookup.current.msgIpFilterDisabled,
      );
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrIpFilterToggle(e));
      notifyListeners();
    }
  }

  Future<void> clearFilter() async {
    try {
      _filterStatus = await client.clearFilter();
      onStatus?.call(L10nLookup.current.msgIpFilterCleared);
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrIpFilterClear(e));
      notifyListeners();
    }
  }

  // ── Rate Limiter ──────────────────────────────────────────────────────────

  Future<void> fetchRateLimits() async {
    try {
      _rateLimits = await client.getRateLimits();
      notifyListeners();
    } catch (_) {}
  }

  Future<void> setRateLimits(RateLimitsModel limits) async {
    try {
      _rateLimits = await client.setRateLimits(limits);
      onStatus?.call(L10nLookup.current.msgRateLimitsUpdated);
      notifyListeners();
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrRateLimits(e));
      notifyListeners();
    }
  }
}
