import 'package:flutter/material.dart';
import 'package:ui_kit/ui_kit.dart';

/// Security notice: LAN/private-range traffic stays blocked even in
/// public mode (anti-SSRF).
class AntiSsrfNotice extends StatelessWidget {
  const AntiSsrfNotice({super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: AppColors.warning.withValues(alpha: 0.1),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: AppColors.warning.withValues(alpha: 0.3),
        ),
      ),
      child: const Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(Icons.security, color: AppColors.warning, size: 16),
              SizedBox(width: 8),
              Text(
                'Garantie de Sécurité Anti-SSRF',
                style: TextStyle(
                  color: AppColors.warning,
                  fontWeight: FontWeight.bold,
                  fontSize: 12,
                ),
              ),
            ],
          ),
          SizedBox(height: 6),
          Text(
            'Même en mode public, la politique réseau d\'Emule-Sion bloque rigoureusement tout trafic sortant vers les adresses de votre réseau local privé (192.168.x.x, 10.x.x.x, 172.16-31.x.x) pour protéger vos équipements domestiques.',
            style: TextStyle(
              fontSize: 11,
              color: AppColors.textSecondary,
              height: 1.4,
            ),
          ),
        ],
      ),
    );
  }
}
