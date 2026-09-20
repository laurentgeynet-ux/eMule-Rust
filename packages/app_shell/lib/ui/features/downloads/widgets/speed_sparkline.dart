import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/transfer_rate_tracker.dart';

/// Rolling 60-second download-rate sparkline for the top bar.
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
        painter: _SparklinePainter(tracker.downloadHistory),
      ),
    );
  }
}

class _SparklinePainter extends CustomPainter {
  final List<double> samples;

  _SparklinePainter(this.samples);

  @override
  void paint(Canvas canvas, Size size) {
    if (samples.length < 2) return;
    final max = samples.reduce((a, b) => a > b ? a : b);
    if (max <= 0) return;

    final line = Paint()
      ..color = AppTheme.statusDownloading
      ..strokeWidth = 1.2
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;
    final fill = Paint()
      ..color = AppTheme.statusDownloading.withValues(alpha: 0.15)
      ..style = PaintingStyle.fill;

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
    fillPath.lineTo(offset + (samples.length - 1) * step, size.height);
    fillPath.close();

    canvas.drawPath(fillPath, fill);
    canvas.drawPath(path, line);
  }

  @override
  bool shouldRepaint(_SparklinePainter old) => true;
}
