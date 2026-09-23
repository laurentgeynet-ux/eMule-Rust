// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Loulach

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../l10n/l10n.dart';

/// Table of upload sessions. When [isQueue] is true the rows display the
/// waiting-queue rank instead of the live upload rate, and rows are not
/// cancellable.
class UploadsTable extends StatelessWidget {
  final List<UploadSessionModel> sessions;
  final bool isQueue;
  final void Function(String clientAddress)? onCancel;

  const UploadsTable({
    super.key,
    required this.sessions,
    this.isQueue = false,
    this.onCancel,
  });

  @override
  Widget build(BuildContext context) {
    final nowMs = DateTime.now().millisecondsSinceEpoch;
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: DataTable(
        headingRowHeight: 36,
        dataRowMinHeight: 40,
        dataRowMaxHeight: 48,
        headingTextStyle: const TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: AppColors.textSecondary,
          letterSpacing: 0.6,
        ),
        columns: [
          DataColumn(label: Text(context.l10n.uploadsClient)),
          DataColumn(label: Text(context.l10n.uploadsFile)),
          DataColumn(label: Text(context.l10n.transferred)),
          if (isQueue) ...[
            DataColumn(label: Text(context.l10n.uploadsRank)),
            // eMule queue score: wait × credit × file priority.
            DataColumn(label: Text(context.l10n.uploadsScore)),
            DataColumn(label: Text(context.l10n.uploadsCredit)),
          ] else
            DataColumn(label: Text(context.l10n.uploadsRate)),
          DataColumn(label: Text(context.l10n.statut)),
          if (!isQueue) DataColumn(label: Text(context.l10n.actions)),
        ],
        rows: [
          for (final s in sessions)
            DataRow(
              cells: [
                DataCell(
                  Text(
                    s.clientAddress,
                    style: const TextStyle(fontSize: 12),
                  ),
                ),
                DataCell(
                  ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 320),
                    child: Tooltip(
                      message: s.fileHash,
                      child: Text(
                        s.fileName,
                        style: const TextStyle(fontSize: 12),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
                ),
                DataCell(
                  Text(
                    Formatters.formatBytes(s.transferredBytes),
                    style: const TextStyle(fontSize: 12),
                  ),
                ),
                DataCell(
                  Text(
                    isQueue
                        ? '#${s.queueRank ?? 0}'
                        : Formatters.formatSpeed(s.uploadRateBps),
                    style: TextStyle(
                      fontSize: 12,
                      color: isQueue
                          ? AppColors.textSecondary
                          : AppTheme.statusDownloading,
                    ),
                  ),
                ),
                if (isQueue) ...[
                  DataCell(
                    Text(
                      '${s.effectiveScore(nowMs)}',
                      style: TextStyle(
                        fontSize: 12,
                        color: s.banned
                            ? AppTheme.statusFailed
                            : AppColors.textSecondary,
                      ),
                    ),
                  ),
                  DataCell(
                    Text(
                      'x${(s.creditMultiplierBps / 100).toStringAsFixed(2)}',
                      style: const TextStyle(
                        fontSize: 12,
                        color: AppColors.textSecondary,
                      ),
                    ),
                  ),
                ],
                DataCell(
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Text(
                        s.state,
                        style: const TextStyle(
                          fontSize: 12,
                          color: AppColors.textSecondary,
                        ),
                      ),
                      if (s.banned) ...[
                        const SizedBox(width: 4),
                        Tooltip(
                          message: context.l10n.uploadsBanned,
                          child: Icon(
                            Icons.block,
                            size: 12,
                            color: AppTheme.statusFailed,
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
                if (!isQueue)
                  DataCell(
                    IconButton(
                      icon: const Icon(Icons.close_rounded, size: 18),
                      tooltip: context.l10n.uploadsCancelTooltip,
                      onPressed: onCancel == null
                          ? null
                          : () => onCancel!(s.clientAddress),
                    ),
                  ),
              ],
            ),
        ],
      ),
    );
  }
}
