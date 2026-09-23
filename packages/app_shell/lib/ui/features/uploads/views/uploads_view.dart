// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

import '../../../../app/providers.dart';
import '../widgets/uploads_table.dart';

/// Uploads view: active upload sessions + waiting queue, fed live by
/// `upload.uploadsUpdated` events (see [UploadsController]).
class UploadsView extends ConsumerStatefulWidget {
  const UploadsView({super.key});

  @override
  ConsumerState<UploadsView> createState() => _UploadsViewState();
}

class _UploadsViewState extends ConsumerState<UploadsView> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback(
      (_) => ref.read(uploadsControllerProvider).fetch(),
    );
  }

  @override
  Widget build(BuildContext context) {
    final ctl = ref.watch(uploadsControllerProvider);
    final l10n = context.l10n;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 16, 20, 8),
          child: Row(
            children: [
              Text(
                l10n.sectionUploads,
                style: const TextStyle(
                  fontSize: 18,
                  fontWeight: FontWeight.w600,
                ),
              ),
              const Spacer(),
              Icon(
                Icons.upload_rounded,
                size: 16,
                color: AppTheme.statusDownloading,
              ),
              const SizedBox(width: 6),
              Text(
                Formatters.formatSpeed(ctl.totalUploadRateBps),
                style: const TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: AppTheme.statusDownloading,
                ),
              ),
            ],
          ),
        ),
        Expanded(
          child: ListView(
            padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
            children: [
              _SectionLabel(
                label: l10n.uploadsActive,
                count: ctl.active.length,
              ),
              _TableCard(
                child: ctl.active.isEmpty
                    ? _EmptyState(label: l10n.uploadsActiveEmpty)
                    : UploadsTable(
                        sessions: ctl.active,
                        onCancel: ctl.cancelUpload,
                      ),
              ),
              const SizedBox(height: 16),
              _SectionLabel(label: l10n.uploadsQueue, count: ctl.queue.length),
              _TableCard(
                child: ctl.queue.isEmpty
                    ? _EmptyState(label: l10n.uploadsQueueEmpty)
                    : UploadsTable(sessions: ctl.queue, isQueue: true),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _SectionLabel extends StatelessWidget {
  final String label;
  final int count;

  const _SectionLabel({required this.label, required this.count});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Text(
        '$label ($count)',
        style: const TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w700,
          letterSpacing: 0.8,
          color: AppColors.textSecondary,
        ),
      ),
    );
  }
}

class _TableCard extends StatelessWidget {
  final Widget child;

  const _TableCard({required this.child});

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: child,
    );
  }
}

class _EmptyState extends StatelessWidget {
  final String label;

  const _EmptyState({required this.label});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 24),
      child: Center(
        child: Text(
          label,
          style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
        ),
      ),
    );
  }
}
