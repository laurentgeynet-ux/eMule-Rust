import 'dart:async';
import 'dart:collection';

import 'package:flutter/foundation.dart';

/// Samples the aggregate download/upload rates once per second and keeps a
/// rolling window for sparkline rendering.
class TransferRateTracker extends ChangeNotifier {
  static const int windowSize = 60;

  final int Function() _downloadRate;
  final int Function() _uploadRate;
  Timer? _timer;

  final Queue<double> _download = Queue<double>();
  final Queue<double> _upload = Queue<double>();

  TransferRateTracker({
    required this._downloadRate,
    required this._uploadRate,
  });

  /// Rolling download-rate samples (bytes/s), oldest first.
  List<double> get downloadHistory => _download.toList();

  /// Rolling upload-rate samples (bytes/s), oldest first.
  List<double> get uploadHistory => _upload.toList();

  void start() {
    _timer ??= Timer.periodic(const Duration(seconds: 1), (_) => _sample());
  }

  void _stop() {
    _timer?.cancel();
    _timer = null;
  }

  @override
  void addListener(VoidCallback listener) {
    super.addListener(listener);
    if (hasListeners) start();
  }

  @override
  void removeListener(VoidCallback listener) {
    super.removeListener(listener);
    if (!hasListeners) _stop();
  }

  void _sample() {
    _push(_download, _downloadRate().toDouble());
    _push(_upload, _uploadRate().toDouble());
    notifyListeners();
  }

  void _push(Queue<double> q, double v) {
    q.addLast(v);
    while (q.length > windowSize) {
      q.removeFirst();
    }
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }
}
