import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

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
      onStatus?.call('Switched network profile to ${profile.label}');
      notifyListeners();
    } catch (e) {
      onError?.call('Failed to switch network profile: $e');
      notifyListeners();
    }
  }

  Future<void> disconnectAll() async {
    try {
      await client.disconnectAll();
      await fetchPolicy();
      onAllDisconnected?.call();
      onStatus?.call(
        'Kill Switch engaged: All network connections disconnected',
      );
      notifyListeners();
    } catch (e) {
      onError?.call('Kill switch failed: $e');
      notifyListeners();
    }
  }

  Future<void> enableNetwork() async {
    try {
      await client.enableNetwork();
      await fetchPolicy();
      onStatus?.call('Network policy re-enabled');
      notifyListeners();
    } catch (e) {
      onError?.call('Failed to re-enable network: $e');
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
      onStatus?.call('Resource budgets updated');
      notifyListeners();
    } catch (e) {
      onError?.call('Failed to set resource budgets: $e');
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
        'Imported ${_filterStatus?.rangeCount ?? 0} IP filter ranges',
      );
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call('Failed to import IP filter: $e');
      notifyListeners();
      return false;
    }
  }

  Future<void> setFilterEnabled(bool enabled) async {
    try {
      _filterStatus = await client.setFilterEnabled(enabled);
      onStatus?.call(enabled ? 'IP Filter enabled' : 'IP Filter disabled');
      notifyListeners();
    } catch (e) {
      onError?.call('Failed to toggle IP filter: $e');
      notifyListeners();
    }
  }

  Future<void> clearFilter() async {
    try {
      _filterStatus = await client.clearFilter();
      onStatus?.call('IP Filter cleared');
      notifyListeners();
    } catch (e) {
      onError?.call('Failed to clear IP filter: $e');
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
      onStatus?.call('Rate limits updated');
      notifyListeners();
    } catch (e) {
      onError?.call('Failed to set rate limits: $e');
      notifyListeners();
    }
  }
}
