import 'package:flutter/material.dart';

import 'package:mule_client/mule_client.dart';

/// eMule-style availability coloring:
/// red = no known source has this part (download will stall),
/// blue shades = rare → common, teal = verified locally.
Color partColorFor(DownloadPartInfo part) {
  if (part.isVerified) {
    return Colors.tealAccent;
  }
  if (part.state == 'missing') {
    if (part.availableSourceCount == 0) {
      return Colors.redAccent;
    }
    // 1-2 sources → dark blue, ≥8 → light blue.
    final t = (part.availableSourceCount / 8).clamp(0.0, 1.0);
    return Color.lerp(
      const Color(0xFF0D47A1),
      Colors.lightBlueAccent,
      t,
    )!;
  }
  switch (part.state) {
    case 'completed':
      return Colors.blueAccent;
    case 'downloading':
      return Colors.amberAccent;
    default:
      return Colors.white12;
  }
}

/// Continuous per-part strip like eMule's transfer progress bar:
/// each part is drawn at its byte-range position and colored by
/// availability/state. Renders nothing when [parts] is empty.
class PartAvailabilityBar extends StatelessWidget {
  final List<DownloadPartInfo> parts;
  final int totalBytes;
  final double height;

  const PartAvailabilityBar({
    super.key,
    required this.parts,
    required this.totalBytes,
    this.height = 5,
  });

  @override
  Widget build(BuildContext context) {
    if (parts.isEmpty || totalBytes <= 0) {
      return const SizedBox.shrink();
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final width = constraints.maxWidth;
        final children = <Widget>[];

        for (final part in parts) {
          final left = (part.startOffset / totalBytes) * width;
          final partWidth =
              (((part.endOffset - part.startOffset) / totalBytes) * width)
                  .clamp(1.0, width);
          children.add(
            Positioned(
              left: left.clamp(0.0, width),
              top: 0,
              bottom: 0,
              width: partWidth.toDouble(),
              child: Tooltip(
                message:
                    'Part ${part.partIndex} · ${part.state} · '
                    'Sources: ${part.availableSourceCount}',
                preferBelow: false,
                child: Container(color: partColorFor(part)),
              ),
            ),
          );
        }

        return ClipRRect(
          borderRadius: BorderRadius.circular(4),
          child: Container(
            height: height,
            color: Colors.white12,
            child: Stack(children: children),
          ),
        );
      },
    );
  }
}
