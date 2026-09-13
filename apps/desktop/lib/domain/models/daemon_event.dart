import 'package:flutter/foundation.dart';
import 'download_item.dart';

@immutable
class DownloadUpdatedEvent {
  final int sequence;
  final DownloadItem download;

  const DownloadUpdatedEvent({
    required this.sequence,
    required this.download,
  });

  @override
  String toString() => 'DownloadUpdatedEvent(seq: $sequence, dl: ${download.id}, progress: ${download.progress})';
}
