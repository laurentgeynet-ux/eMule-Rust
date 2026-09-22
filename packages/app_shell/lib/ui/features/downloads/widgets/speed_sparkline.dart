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
            ..color = color.withValues(alpha: 0.15)
            ..style = PaintingStyle.fill);
    }
    canvas.drawPath(path, line);
  }

  @override
  bool shouldRepaint(_SparklinePainter old) => true;
}
