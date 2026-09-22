import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

import '../view_models/kad_controller.dart';
import '../../../../l10n/l10n.dart';

/// Dialog: bootstrap the Kad routing table from a single node (IP + UDP
/// port). In `local-lab` profile only 127.0.0.1 is allowed.
Future<void> showKadBootstrapDialog(
  BuildContext context,
  KadController controller,
) async {
  final isPublic = controller.status.networkProfile != 'local-lab';
  final ipController = TextEditingController(
    text: isPublic ? '' : '127.0.0.1',
  );
  final portController = TextEditingController(text: '39737');

  await showDialog(
    context: context,
    builder: (ctx) => AlertDialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      title: Row(
        children: [
          Icon(
            Icons.rocket_launch_outlined,
            color: AppColors.accentPurple,
            size: 20,
          ),
          SizedBox(width: 8),
          Text(context.l10n.amorcerLeReseauKadBootstrap),
        ],
      ),
      content: SizedBox(
        width: 400,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              isPublic
                  ? context.l10n.kadBootstrapDescPublic
                  : context.l10n.kadBootstrapDescLab,
              style: const TextStyle(fontSize: 12, color: AppColors.textSecondary),
            ),
            const SizedBox(height: 12),
            TextField(
              controller: ipController,
              style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
              decoration: InputDecoration(
                labelText: context.l10n.adresseIp,
                hintText: isPublic ? 'ex: 212.83.184.225' : '127.0.0.1',
                isDense: true,
                filled: true,
                fillColor: AppTheme.darkCard,
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
            ),
            const SizedBox(height: 10),
            TextField(
              controller: portController,
              keyboardType: TextInputType.number,
              style: const TextStyle(fontSize: 13, color: AppColors.textPrimary),
              decoration: InputDecoration(
                labelText: context.l10n.portUdp,
                hintText: '39737',
                isDense: true,
                filled: true,
                fillColor: AppTheme.darkCard,
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(6),
                ),
              ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(ctx).pop(),
          child: Text(context.l10n.annuler,
            style: TextStyle(color: AppColors.textSoft),
          ),
        ),
        FilledButton(
          style: FilledButton.styleFrom(backgroundColor: AppColors.accentPurple),
          onPressed: () async {
            final ip = ipController.text.trim();
            final port = int.tryParse(portController.text.trim()) ?? 39737;
            Navigator.of(ctx).pop();
            await controller.bootstrap(ip: ip, port: port);
          },
          child: Text(context.l10n.lancerLAmorcage),
        ),
      ],
    ),
  );
}
