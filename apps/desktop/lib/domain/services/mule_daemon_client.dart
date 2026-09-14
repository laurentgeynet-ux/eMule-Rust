import '../models/daemon_event.dart';
import '../models/daemon_info.dart';
import '../models/download_item.dart';
import '../models/lab_models.dart';

abstract class MuleDaemonClient {
  /// Connect to the daemon and perform authentication handshake.
  Future<DaemonInfo> connect({required Uri url, String? token});

  /// Disconnect gracefully.
  Future<void> disconnect();

  /// Stream of connection state changes.
  Stream<DaemonConnectionState> get connectionState;

  /// Current connection state snapshot.
  DaemonConnectionState get currentConnectionState;

  /// Cached daemon info snapshot from handshake, if connected.
  DaemonInfo? get daemonInfo;

  /// Stream of download updates emitted by the daemon.
  Stream<DownloadUpdatedEvent> get downloadEvents;

  /// Fetch full snapshot of all downloads and current sequence.
  Future<List<DownloadItem>> listDownloads();

  /// Fetch a single download by ID.
  Future<DownloadItem?> getDownload(String id);

  /// Add a new download via eD2K link.
  Future<String> addEd2kLink(
    String link, {
    String? targetDirectory,
    String? allocationPolicy,
  });

  /// Pause an active download.
  Future<DownloadItem> pauseDownload(String id);

  /// Resume a paused download.
  Future<DownloadItem> resumeDownload(String id);

  /// Remove a download, optionally deleting files.
  Future<void> removeDownload(String id, {bool deleteFiles = false});

  /// Inject data range for simulation and integrity verification tests.
  Future<DownloadItem> injectRange(String id, int offset, String hexData);

  /// Request daemon graceful shutdown (system.stop).
  Future<void> stopDaemon();

  /// Fetch lab status (workdir mode, path, fixtures, active test peer).
  Future<LabStatus> getLabStatus();

  /// Create a deterministic test fixture in `<workdir>/fixtures`.
  Future<LabFixtureInfo> createLabFixture(String fixtureType);

  /// Start the loopback test peer (127.0.0.1) sharing all fixtures.
  Future<LabPeerInfo> startLabPeer();

  /// Stop the running loopback test peer.
  Future<void> stopLabPeer();

  /// Clean up test data and fixtures (allowed only in Portable / dev modes).
  Future<LabCleanupReport> cleanupLabTestData();

  /// Connect a download to an active loopback source.
  Future<void> connectDownloadToSource(String downloadId, String peerAddr);

  /// Clean up any resources/sockets.
  void dispose();
}
