import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/kad_controller.dart';
import 'kad_format.dart';

/// Scrollable data table of Kad routing contacts (id, address, version,
/// verification status, failures, last seen, ping action).
class KadContactsTable extends StatelessWidget {
  final KadController controller;
  final List<KadContactModel> contacts;

  const KadContactsTable({
    super.key,
    required this.controller,
    required this.contacts,
  });

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      scrollDirection: Axis.vertical,
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        child: DataTable(
          columnSpacing: 24,
          headingRowHeight: 38,
          dataRowMinHeight: 40,
          dataRowMaxHeight: 44,
          headingTextStyle: const TextStyle(
            fontSize: 12,
            fontWeight: FontWeight.bold,
            color: AppColors.textSecondary,
          ),
          columns: const [
            DataColumn(label: Text('Kad ID (128-bit XOR)')),
            DataColumn(label: Text('Adresse IP & Ports')),
            DataColumn(label: Text('Version')),
            DataColumn(label: Text('Statut')),
            DataColumn(label: Text('Échecs')),
            DataColumn(label: Text('Dernière vue')),
            DataColumn(label: Text('Actions')),
          ],
          rows: contacts.map((c) {
            return DataRow(
              cells: [
                DataCell(
                  Tooltip(
                    message: c.kadId,
                    child: Text(
                      formatTruncatedKadId(c.kadId),
                      style: const TextStyle(
                        fontSize: 12,
                        fontFamily: 'monospace',
                        color: AppColors.accentTeal,
                      ),
                    ),
                  ),
                ),
                DataCell(
                  Text(
                    c.tcpPort != null
                        ? '${c.ip}:${c.udpPort} (TCP: ${c.tcpPort})'
                        : '${c.ip}:${c.udpPort}',
                    style: const TextStyle(fontSize: 12, color: AppColors.textPrimary),
                  ),
                ),
                DataCell(
                  Text(
                    'v${c.version}',
                    style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
                  ),
                ),
                DataCell(
                  Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 6,
                      vertical: 2,
                    ),
                    decoration: BoxDecoration(
                      color: c.verified
                          ? AppColors.tintGreen.withAlpha(20)
                          : AppColors.warning.withAlpha(20),
                      borderRadius: BorderRadius.circular(4),
                      border: Border.all(
                        color: c.verified
                            ? AppColors.positive.withAlpha(80)
                            : AppColors.accentAmber.withAlpha(80),
                        width: 0.5,
                      ),
                    ),
                    child: Text(
                      c.verified ? 'Vérifié' : 'Non vérifié',
                      style: TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.bold,
                        color: c.verified
                            ? AppColors.positive
                            : AppColors.accentAmber,
                      ),
                    ),
                  ),
                ),
                DataCell(
                  Text(
                    '${c.failCount}',
                    style: TextStyle(
                      fontSize: 12,
                      color: c.failCount > 0
                          ? AppColors.accentOrange
                          : AppColors.textMuted,
                    ),
                  ),
                ),
                DataCell(
                  Text(
                    formatKadLastSeen(c.lastSeenUnixMs),
                    style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
                  ),
                ),
                DataCell(
                  IconButton(
                    icon: const Icon(
                      Icons.network_ping_rounded,
                      size: 16,
                      color: AppColors.accentCyan,
                    ),
                    tooltip: 'Envoyer Kad Ping (0x60)',
                    onPressed: () => controller.pingContact(
                      ip: c.ip,
                      port: c.udpPort,
                      kadId: c.kadId,
                    ),
                  ),
                ),
              ],
            );
          }).toList(),
        ),
      ),
    );
  }
}
