import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

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
      onStatus?.call('Injected ${hexData.length ~/ 2} bytes at offset $offset');
      notifyListeners();
    } catch (e) {
      onError?.call('Injection failed: $e');
      notifyListeners();
    }
  }

  Future<LabStatus> getLabStatus() => client.getLabStatus();

  Future<LabFixtureInfo> createLabFixture(String fixtureType) async {
    final fixture = await client.createLabFixture(fixtureType);
    onStatus?.call('Created fixture ${fixture.fileName}');
    notifyListeners();
    return fixture;
  }

  Future<LabPeerInfo> startLabPeer() async {
    final peer = await client.startLabPeer();
    onStatus?.call('Started loopback peer at ${peer.peerAddr}');
    notifyListeners();
    return peer;
  }

  Future<void> stopLabPeer() async {
    await client.stopLabPeer();
    onStatus?.call('Stopped loopback test peer');
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
    onStatus?.call('Connected download to $peerAddr');
    await onRefreshDownloads?.call();
  }
}
