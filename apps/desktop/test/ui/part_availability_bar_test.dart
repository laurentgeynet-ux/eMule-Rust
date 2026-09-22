import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

void main() {
  group('PartAvailabilityBar colors (eMule 0.72a DrawStatusBar)', () {
    test('availabilityColor: red when no source, blue gradient rare→common', () {
      expect(availabilityColor(0), Colors.redAccent);

      int g(Color c) => (c.g * 255.0).round().clamp(0, 255);

      // eMule formula RGB(0, 210-22*(freq-1), 255): rare = light cyan-blue.
      final rare = availabilityColor(1);
      expect(g(rare), 210);
      expect(rare.b, 1.0);

      // More sources → greener channel decreases → approaches pure blue.
      final common = availabilityColor(10);
      expect(g(common), lessThan(g(rare)));
      expect(common.b, 1.0);

      // Very common clamps at 0 (pure blue).
      expect(g(availabilityColor(100)), 0);
    });

    test('partColorFor maps verified/completed/missing states', () {
      const base = DownloadPartInfo(
        partIndex: 0,
        startOffset: 0,
        endOffset: 9728000,
        size: 9728000,
        receivedBytes: 0,
        isVerified: false,
        state: 'missing',
        availableSourceCount: 0,
      );

      expect(partColorFor(base), Colors.redAccent);
      expect(
        partColorFor(base.copyWith3(availableSourceCount: 4)),
        availabilityColor(4),
      );
      expect(
        partColorFor(base.copyWith3(state: 'completed')),
        const Color(0xFF90A4AE),
      );
      expect(
        partColorFor(base.copyWith3(isVerified: true, state: 'verified')),
        Colors.tealAccent,
      );
    });
  });

  group('PartAvailabilityBar widget', () {
    DownloadPartInfo part(
      int index, {
      required int start,
      required int end,
      int received = 0,
      bool verified = false,
      String state = 'missing',
      int sources = 0,
      int mask = 0,
    }) {
      return DownloadPartInfo(
        partIndex: index,
        startOffset: start,
        endOffset: end,
        size: end - start,
        receivedBytes: received,
        isVerified: verified,
        state: state,
        availableSourceCount: sources,
        receivedMask: mask,
      );
    }

    Future<void> pumpBar(
      WidgetTester tester, {
      required List<DownloadPartInfo> parts,
      int totalBytes = 9728000 * 2,
      List<ByteRange> pendingRanges = const [],
      double progress = 0.0,
      bool muted = false,
    }) async {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 400,
              child: PartAvailabilityBar(
                parts: parts,
                totalBytes: totalBytes,
                pendingRanges: pendingRanges,
                progress: progress,
                muted: muted,
                height: 9,
              ),
            ),
          ),
        ),
      );
      await tester.pump();
    }

    testWidgets('renders CustomPaint with parts and pending ranges', (tester) async {
      await pumpBar(
        tester,
        parts: [
          // Part 0: half received (first 32 of 64 cells), 3 sources.
          part(0, start: 0, end: 9728000, received: 4864000,
              state: 'downloading', sources: 3, mask: 0xFFFFFFFF),
          // Part 1: missing, no source → red.
          part(1, start: 9728000, end: 19456000),
        ],
        pendingRanges: const [ByteRange(4864000, 5048320)],
        progress: 0.25,
      );

      expect(find.byType(PartAvailabilityBar), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(PartAvailabilityBar),
          matching: find.byType(CustomPaint),
        ),
        findsOneWidget,
      );
      expect(find.byType(Tooltip), findsNWidgets(2));
    });

    testWidgets('renders an empty track when no parts are known', (tester) async {
      await pumpBar(tester, parts: const [], totalBytes: 0);
      expect(find.byType(PartAvailabilityBar), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(PartAvailabilityBar),
          matching: find.byType(CustomPaint),
        ),
        findsOneWidget,
      );
    });

    testWidgets('skips per-part tooltips beyond 64 parts', (tester) async {
      final many = List.generate(
        70,
        (i) => part(i,
            start: i * 9728000, end: (i + 1) * 9728000, sources: 2),
      );
      await pumpBar(tester, parts: many, totalBytes: 70 * 9728000);
      expect(find.byType(Tooltip), findsNothing);
    });

    testWidgets('muted palette renders without error', (tester) async {
      await pumpBar(
        tester,
        parts: [part(0, start: 0, end: 9728000, sources: 1)],
        muted: true,
      );
      expect(find.byType(PartAvailabilityBar), findsOneWidget);
    });
  });
}

/// Test-only convenience copy helper (DownloadPartInfo has no copyWith).
extension on DownloadPartInfo {
  DownloadPartInfo copyWith3({
    int? availableSourceCount,
    bool? isVerified,
    String? state,
  }) {
    return DownloadPartInfo(
      partIndex: partIndex,
      startOffset: startOffset,
      endOffset: endOffset,
      size: size,
      receivedBytes: receivedBytes,
      isVerified: isVerified ?? this.isVerified,
      state: state ?? this.state,
      availableSourceCount: availableSourceCount ?? this.availableSourceCount,
      receivedMask: receivedMask,
    );
  }
}
