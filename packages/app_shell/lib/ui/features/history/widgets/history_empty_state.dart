// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../l10n/l10n.dart';

/// Empty history placeholder.
class HistoryEmptyState extends StatelessWidget {
  const HistoryEmptyState({super.key});

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            Icons.history_toggle_off_rounded,
            size: 64,
            color: AppColors.textPrimary.withValues(alpha: 0.15),
          ),
          const SizedBox(height: 16),
          Text(context.l10n.noDownloadHistoryRecordedYet,
            style: TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
              color: AppColors.textSecondary,
            ),
          ),
          const SizedBox(height: 6),
          Text(context.l10n.completedResumedFailedAndRemoved,
            style: TextStyle(fontSize: 13, color: AppColors.textFaint),
          ),
        ],
      ),
    );
  }
}
