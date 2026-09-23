// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

/// App-level controller holding the latest `event.statsTick` snapshot.
///
/// When a tick has been received, [latest] is the source of truth for
/// aggregate rates and connection counters — clients render the daemon's
/// own numbers instead of re-sampling locally. `null` until the first tick.
class StatsController extends ChangeNotifier {
  final MuleDaemonClient client;

  StatsController({required this.client});

  DaemonStatsModel? _latest;
  StreamSubscription<DaemonStatsModel>? _sub;

  DaemonStatsModel? get latest => _latest;

  /// Subscribes to `statsEvents`. Idempotent.
  void attach() {
    _sub ??= client.statsEvents.listen((s) {
      _latest = s;
      notifyListeners();
    });
  }

  @override
  void dispose() {
    _sub?.cancel();
    super.dispose();
  }
}
