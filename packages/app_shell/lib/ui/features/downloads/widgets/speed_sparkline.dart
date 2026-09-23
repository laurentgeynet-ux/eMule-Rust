// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/transfer_rate_tracker.dart';

/// Rolling 60-second download/upload-rate sparkline for the top bar.
/// Download is drawn filled (status color), upload as a teal line —
/// both share the same scale so rates stay visually comparable.
class SpeedSparkline extends StatelessWidget {
  final TransferRateTracker tracker;
  final double width;
  final double height;

  const SpeedSparkline({
    super.key,
    required this.tracker,
    this.width = 56,
    this.height = 16,
  });

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: tracker,
      builder: (context, _) => CustomPaint(
        size: Size(width, height),
        painter: _SparklinePainter(
          tracker.downloadHistory,
          tracker.uploadHistory,
        ),
      ),
    );
  }
}

class _SparklinePainter extends CustomPainter {
  final List<double> downloadSamples;
  final List<double> uploadSamples;

  _SparklinePainter(this.downloadSamples, this.uploadSamples);

  @override
  void paint(Canvas canvas, Size size) {
    // Subtle baseline track so the graph area reads even when idle.
    canvas.drawLine(
      Offset(0, size.height - 0.5),
      Offset(size.width, size.height - 0.5),
      Paint()
        ..color = Colors.white.withValues(alpha: 0.08)
        ..strokeWidth = 0.5,
    );

    final maxDl = downloadSamples.fold<double>(
        0, (a, b) => a > b ? a : b);
    final maxUl =
        uploadSamples.fold<double>(0, (a, b) => a > b ? a : b);
    final max = maxDl > maxUl ? maxDl : maxUl;
    if (max <= 0) return;

    _drawSeries(
      canvas,
      size,
      downloadSamples,
      max,
      AppTheme.statusDownloading,
      fill: true,
    );
    _drawSeries(
      canvas,
      size,
      uploadSamples,
      max,
      AppColors.accentTeal,
      fill: false,
    );
  }

  void _drawSeries(
    Canvas canvas,
    Size size,
    List<double> samples,
    double max,
    Color color, {
    required bool fill,
  }) {
    if (samples.length < 2) return;

    final line = Paint()
      ..color = color
      ..strokeWidth = 1.2
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;

    final path = Path();
    final fillPath = Path();
    final step = size.width / (TransferRateTracker.windowSize - 1);
    final offset = (TransferRateTracker.windowSize - samples.length) * step;

    for (var i = 0; i < samples.length; i++) {
      final x = offset + i * step;
      final y = size.height - (samples[i] / max) * (size.height - 1);
      if (i == 0) {
        path.moveTo(x, y);
        fillPath.moveTo(x, size.height);
        fillPath.lineTo(x, y);
      } else {
        path.lineTo(x, y);
        fillPath.lineTo(x, y);
      }
    }

    if (fill) {
      fillPath.lineTo(offset + (samples.length - 1) * step, size.height);
      fillPath.close();
      canvas.drawPath(
          fillPath,
          Paint()
            ..shader = LinearGradient(
              begin: Alignment.topCenter,
              end: Alignment.bottomCenter,
              colors: [
                color.withValues(alpha: 0.35),
                color.withValues(alpha: 0.02),
              ],
            ).createShader(Rect.fromLTWH(0, 0, size.width, size.height))
            ..style = PaintingStyle.fill);
    }
    canvas.drawPath(path, line);
  }

  @override
  bool shouldRepaint(_SparklinePainter old) => true;
}
