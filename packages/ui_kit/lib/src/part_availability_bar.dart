import 'dart:math' as math;

import 'package:flutter/material.dart';

import 'package:mule_client/mule_client.dart';

/// eMule 0.72a `CPartFile::DrawStatusBar` palette, adapted to the dark
/// theme (eMule's light theme uses black `crHave`; we use a light slate so
/// received bytes stay visible on a dark background).

/// Received bytes (eMule `crHave`).
const Color _haveColor = Color(0xFF90A4AE); // blueGrey 300

/// Hash-verified bytes — extra local state eMule folds into `crHave`.
const Color _verifiedColor = Colors.tealAccent;

/// Missing gap in a part no source advertises (eMule `crMissing` red).
const Color _missingColor = Colors.redAccent;

/// Requested but not yet received block (eMule `crPending` RGB(255,208,0)).
const Color _pendingColor = Color(0xFFFFD000);

/// Overall progress strip at the top of the bar (eMule `crProgress`).
const Color _progressColor = Color(0xFF00E000);

const int _kPartSizeBytes = 9728000; // eD2K ED2K_PART_SIZE
const int _kMaskCells = 64;

/// eMule availability gradient for a missing gap:
/// `RGB(0, 210 - 22*(freq-1), 255)` — rare parts are light cyan-blue,
/// common parts approach pure blue. 0 sources → red.
Color availabilityColor(int sourceCount) {
  if (sourceCount <= 0) {
    return _missingColor;
  }
  final g = (210 - 22 * (sourceCount - 1)).clamp(0, 255);
  return Color.fromARGB(255, 0, g, 255);
}

/// Whole-part color for the part-map card: verified → teal, fully received
/// → "have" slate, otherwise the availability color of the missing gap.
Color partColorFor(DownloadPartInfo part) {
  if (part.isVerified) {
    return _verifiedColor;
  }
  if (part.state == 'completed') {
    return _haveColor;
  }
  return availabilityColor(part.availableSourceCount);
}

/// Byte-accurate eD2K chunk bar like eMule's transfer progress bar.
///
/// Per pixel column, the byte's cell is resolved from
/// [DownloadPartInfo.receivedMask]: received cells are painted slate (teal
/// when the part hash is verified), missing cells are painted with the
/// part's availability color (red = no source, blue gradient rare→common).
/// [pendingRanges] (in-flight block requests) overlay in yellow, and a
/// thin green strip at the top shows overall progress — exactly like
/// eMule 0.72a `DrawStatusBar`.
class PartAvailabilityBar extends StatelessWidget {
  final List<DownloadPartInfo> parts;
  final int totalBytes;
  final List<ByteRange> pendingRanges;
  final double progress;
  final double height;

  /// Muted palette for paused/stopped/failed downloads (eMule `notgray`).
  final bool muted;

  const PartAvailabilityBar({
    super.key,
    required this.parts,
    required this.totalBytes,
    this.pendingRanges = const [],
    this.progress = 0.0,
    this.height = 5,
    this.muted = false,
  });

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: height,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final width = constraints.maxWidth;
          return ClipRRect(
            borderRadius: BorderRadius.circular(4),
            child: Stack(
              children: [
                Positioned.fill(
                  child: CustomPaint(
                    painter: _PartBarPainter(
                      parts: parts,
                      pendingRanges: pendingRanges,
                      totalBytes: totalBytes,
                      progress: progress,
                      muted: muted,
                    ),
                  ),
                ),
                // Per-part tooltips only stay usable while a part is a few
                // px wide; beyond that they would be sub-pixel slivers.
                if (parts.length <= 64 && totalBytes > 0)
                  for (final part in parts)
                    Positioned(
                      left: (part.startOffset / totalBytes * width)
                          .clamp(0.0, width),
                      width: ((part.endOffset - part.startOffset) /
                              totalBytes *
                              width)
                          .clamp(1.0, width),
                      top: 0,
                      bottom: 0,
                      child: Tooltip(
                        message:
                            'Part ${part.partIndex} · ${part.state} · '
                            'Sources: ${part.availableSourceCount}',
                        preferBelow: false,
                        child: const SizedBox.expand(),
                      ),
                    ),
              ],
            ),
          );
        },
      ),
    );
  }
}

class _PartBarPainter extends CustomPainter {
  final List<DownloadPartInfo> parts;
  final List<ByteRange> pendingRanges;
  final int totalBytes;
  final double progress;
  final bool muted;

  _PartBarPainter({
    required this.parts,
    required this.pendingRanges,
    required this.totalBytes,
    required this.progress,
    required this.muted,
  });

  /// Mask fallback for snapshots that predate `receivedMask`: assume the
  /// received bytes fill the part contiguously from its start.
  int _effectiveMask(DownloadPartInfo part) {
    if (part.receivedMask != 0 || part.receivedBytes == 0) {
      return part.receivedMask;
    }
    if (part.receivedBytes >= part.size) {
      return -1; // all 64 bits
    }
    final fullCells = part.receivedBytes * _kMaskCells ~/ part.size;
    return fullCells >= _kMaskCells ? -1 : ((1 << fullCells) - 1);
  }

  Color _colorAtByte(int byte) {
    var idx = byte ~/ _kPartSizeBytes;
    if (idx < 0) idx = 0;
    if (idx >= parts.length) idx = parts.length - 1;
    final part = parts[idx];

    if (part.size <= 0) {
      return _haveColor;
    }
    final cell = ((byte - part.startOffset) * _kMaskCells ~/ part.size)
        .clamp(0, _kMaskCells - 1);
    final received = (_effectiveMask(part) >> cell) & 1 == 1;
    if (received) {
      return part.isVerified ? _verifiedColor : _haveColor;
    }
    return availabilityColor(part.availableSourceCount);
  }

  @override
  void paint(Canvas canvas, Size size) {
    final w = size.width;
    final h = size.height;
    if (w <= 0 || h <= 0) {
      return;
    }

    final paint = Paint();
    // No parts known yet (queued, size unknown): flat neutral track so the
    // bar keeps its footprint instead of collapsing.
    if (parts.isEmpty || totalBytes <= 0) {
      canvas.drawRect(
        Rect.fromLTRB(0, 0, w, h),
        paint..color = Colors.white10,
      );
      return;
    }

    final dim = muted ? 0.45 : 1.0;

    // 1. Chunk colors — one column per pixel, coalesced into runs.
    var runStart = 0.0;
    Color? runColor;
    for (var x = 0; x < w; x++) {
      final byte = ((x + 0.5) / w * totalBytes).floor();
      var color = _colorAtByte(byte);
      if (muted) {
        color = color.withValues(alpha: dim);
      }
      if (color != runColor) {
        if (runColor != null) {
          canvas.drawRect(
            Rect.fromLTRB(runStart, 0, x.toDouble(), h),
            paint..color = runColor!,
          );
        }
        runStart = x.toDouble();
        runColor = color;
      }
    }
    if (runColor != null) {
      canvas.drawRect(
        Rect.fromLTRB(runStart, 0, w, h),
        paint..color = runColor!,
      );
    }

    // 2. Pending (requested, in-flight) blocks — eMule yellow.
    paint.color = muted ? _pendingColor.withValues(alpha: dim) : _pendingColor;
    for (final r in pendingRanges) {
      final left = (r.start / totalBytes * w).clamp(0.0, w);
      final right = (r.end / totalBytes * w).clamp(0.0, w);
      if (right > left) {
        canvas.drawRect(Rect.fromLTRB(left, 0, right, h), paint);
      }
    }

    // 3. Thin progress strip on top — eMule green load bar.
    final stripH = math.min(3.0, h * 0.4);
    final doneW = (w * progress.clamp(0.0, 1.0));
    canvas.drawRect(
      Rect.fromLTRB(0, 0, doneW, stripH),
      paint..color = _progressColor,
    );
    canvas.drawRect(
      Rect.fromLTRB(doneW, 0, w, stripH),
      paint..color = Colors.white10,
    );
  }

  @override
  bool shouldRepaint(_PartBarPainter old) =>
      old.parts != parts ||
      old.pendingRanges != pendingRanges ||
      old.totalBytes != totalBytes ||
      old.progress != progress ||
      old.muted != muted;
}
