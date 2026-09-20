import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Top banner: strict loopback confinement vs public network profile.
class NetworkProfileBanner extends StatelessWidget {
  final NetworkProfile profile;

  const NetworkProfileBanner({super.key, required this.profile});

  @override
  Widget build(BuildContext context) {
    final isLoopbackOnly = profile == NetworkProfile.localLab;
    final cardColor =
        isLoopbackOnly ? const Color(0x2210B981) : const Color(0x223B82F6);
    final borderColor =
        isLoopbackOnly ? const Color(0x6610B981) : const Color(0x663B82F6);
    final iconColor =
        isLoopbackOnly ? const Color(0xFF10B981) : const Color(0xFF60A5FA);
    final title = isLoopbackOnly
        ? 'Strict Loopback Confinement (127.0.0.1)'
        : 'Network Profile: ${profile.label}';
    final subtitle = isLoopbackOnly
        ? 'Zero WAN traffic. No public eD2K servers, Kad nodes, or external peers.'
        : 'Public network traffic enabled under active network policy.';

    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: cardColor,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: borderColor, width: 0.5),
      ),
      child: Row(
        children: [
          Icon(
            isLoopbackOnly ? Icons.shield_outlined : Icons.public_rounded,
            color: iconColor,
            size: 22,
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: TextStyle(
                    color: iconColor,
                    fontWeight: FontWeight.bold,
                    fontSize: 13,
                  ),
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: const TextStyle(
                    color: AppColors.textSecondary,
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
