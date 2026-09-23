// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n_lookup.dart';

/// Sub-domain controller for the loopback lab / dev tools.
///
/// Only reachable when lab mode is available (portable workdir + daemon
/// `lab-tools` capability, or dev tools enabled). Status and error
/// reporting is delegated via [onStatus]/[onError]; [onDownloadUpdated]
/// and [onRefreshDownloads] keep the downloads store in sync.
class LabController extends ChangeNotifier {
  final MuleDaemonClient client;

  final void Function(String message)? onStatus;
  final void Function(String message)? onError;
  final void Function(DownloadItem item)? onDownloadUpdated;
  final Future<void> Function()? onRefreshDownloads;

  LabController({
    required this.client,
    this.onStatus,
    this.onError,
    this.onDownloadUpdated,
    this.onRefreshDownloads,
  });

  Future<void> injectSimulationChunk(
    String id,
    int offset,
    String hexData,
  ) async {
    try {
      final updated = await client.injectRange(id, offset, hexData);
      onDownloadUpdated?.call(updated);
      onStatus?.call(
        L10nLookup.current
            .msgBytesInjected(hexData.length ~/ 2, offset),
      );
      notifyListeners();
    } catch (e) {
      AppLogger.warn('lab', 'injectBytes failed', error: e);
      onError?.call(L10nLookup.current.msgErrInject(e));
      notifyListeners();
    }
  }

  Future<LabStatus> getLabStatus() => client.getLabStatus();

  Future<LabFixtureInfo> createLabFixture(String fixtureType) async {
    final fixture = await client.createLabFixture(fixtureType);
    onStatus?.call(L10nLookup.current.msgFixtureCreated(fixture.fileName));
    notifyListeners();
    return fixture;
  }

  Future<LabPeerInfo> startLabPeer() async {
    final peer = await client.startLabPeer();
    onStatus?.call(L10nLookup.current.msgPeerStarted(peer.peerAddr));
    notifyListeners();
    return peer;
  }

  Future<void> stopLabPeer() async {
    await client.stopLabPeer();
    onStatus?.call(L10nLookup.current.msgPeerStopped);
    notifyListeners();
  }

  Future<LabCleanupReport> cleanupLabTestData() async {
    final report = await client.cleanupLabTestData();
    onStatus?.call(report.message);
    await onRefreshDownloads?.call();
    return report;
  }

  Future<void> connectDownloadToSource(
    String downloadId,
    String peerAddr,
  ) async {
    await client.connectDownloadToSource(downloadId, peerAddr);
    onStatus?.call(L10nLookup.current.msgConnectedToPeer(peerAddr));
    await onRefreshDownloads?.call();
  }
}
