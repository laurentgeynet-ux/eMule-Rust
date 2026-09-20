import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../../l10n/l10n.dart';

/// Scrollable list of redacted network audit decisions.
class NetworkAuditLogList extends StatelessWidget {
  final List<NetworkAuditEntryItem> entries;
  final bool isLoading;

  const NetworkAuditLogList({
    super.key,
    required this.entries,
    required this.isLoading,
  });

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: BoxDecoration(
        color: AppTheme.darkCard,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: AppTheme.darkBorder),
      ),
      child: isLoading
          ? const Center(child: CircularProgressIndicator())
          : entries.isEmpty
              ? Center(
                  child: Text(context.l10n.noNetworkEventsRecordedYet,
                    style: TextStyle(
                      color: AppColors.textFaint,
                      fontSize: 12,
                    ),
                  ),
                )
              : ListView.separated(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 10,
                    vertical: 6,
                  ),
                  itemCount: entries.length,
                  separatorBuilder: (_, _) => const Divider(
                    height: 1,
                    color: AppTheme.darkBorder,
                  ),
                  itemBuilder: (context, index) =>
                      _AuditLogRow(entry: entries[index]),
                ),
    );
  }
}

class _AuditLogRow extends StatelessWidget {
  final NetworkAuditEntryItem entry;

  const _AuditLogRow({required this.entry});

  @override
  Widget build(BuildContext context) {
    final isAllowed = entry.decision.toLowerCase() == 'allowed';
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        children: [
          Icon(
            isAllowed
                ? Icons.check_circle_outline
                : Icons.block_flipped,
            size: 15,
            color: isAllowed ? AppColors.positive : AppColors.danger,
          ),
          const SizedBox(width: 8),
          Text(
            entry.eventType,
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.bold,
            ),
          ),
          const SizedBox(width: 8),
          if (entry.redactedTarget != null)
            Container(
              padding: const EdgeInsets.symmetric(
                horizontal: 5,
                vertical: 2,
              ),
              decoration: BoxDecoration(
                color: Colors.black26,
                borderRadius: BorderRadius.circular(4),
              ),
              child: Text(
                entry.redactedTarget!,
                style: const TextStyle(
                  fontSize: 10,
                  fontFamily: 'monospace',
                  color: AppColors.textSecondary,
                ),
              ),
            ),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              entry.reason ?? '',
              style: const TextStyle(
                fontSize: 10,
                color: AppColors.textMuted,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}
