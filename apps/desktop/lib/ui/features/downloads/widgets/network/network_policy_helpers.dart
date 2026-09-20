import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// Human description of a [NetworkProfile] shown under its name.
String networkProfileDescription(NetworkProfile profile) {
  switch (profile) {
    case NetworkProfile.localLab:
      return 'Confined to loopback (127.0.0.1 / ::1). Zero external sockets permitted.';
    case NetworkProfile.publicEd2kPreview:
      return 'Outbound TCP to public eD2K peers/servers. Rejects private LAN & loopback (anti-SSRF).';
    case NetworkProfile.publicEd2kKadPreview:
      return 'Preview WAN with UDP support for Kademlia routing. Strict address validation.';
  }
}

/// `label ........ value` quota row used in the limits card.
class NetworkQuotaRow extends StatelessWidget {
  final String label;
  final String value;

  const NetworkQuotaRow({
    super.key,
    required this.label,
    required this.value,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          Expanded(
            child: Text(
              label,
              style: const TextStyle(
                fontSize: 11,
                color: AppColors.textSoft,
              ),
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(width: 6),
          Text(
            value,
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.bold,
              color: AppColors.textPrimary,
            ),
          ),
        ],
      ),
    );
  }
}
