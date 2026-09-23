// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../app/providers.dart';
import '../../../../l10n/enum_labels.dart';
import '../../../../l10n/l10n.dart';
import 'detail/peer_source_row.dart';
import 'detail/download_part_map_card.dart';

/// Inline expansion shown under a download row (eMule-style double-click):
/// live connected peers (rate, queue rank), the part progress map and the
/// known peer sources for the file with connect controls.
class DownloadSourcesExpansion extends ConsumerStatefulWidget {
  final DownloadItem item;

  const DownloadSourcesExpansion({super.key, required this.item});

  @override
  ConsumerState<DownloadSourcesExpansion> createState() =>
      _DownloadSourcesExpansionState();
}

class _DownloadSourcesExpansionState
    extends ConsumerState<DownloadSourcesExpansion> {
  Timer? _swarmTimer;
  int _tick = 0;

  String get _hash => widget.item.ed2kHash ?? '';

  @override
  void initState() {
    super.initState();
    final ctl = ref.read(peerSourcesControllerProvider);
    if (_hash.isNotEmpty) {
      unawaited(ctl.loadSourcesForHash(_hash));
      unawaited(ctl.loadSwarmForHash(_hash));
    }
    unawaited(ctl.loadDetailForId(widget.item.id));

    // Live telemetry: swarm sessions every 2 s, source list every 10 s.
    _swarmTimer = Timer.periodic(const Duration(seconds: 2), (_) {
      if (!mounted || _hash.isEmpty) return;
      _tick++;
      final ctl = ref.read(peerSourcesControllerProvider);
      unawaited(ctl.loadSwarmForHash(_hash));
      unawaited(ctl.loadDetailForId(widget.item.id));
      if (_tick % 5 == 0) {
        unawaited(ctl.loadSourcesForHash(_hash));
      }
    });
  }

  @override
  void dispose() {
    _swarmTimer?.cancel();
    super.dispose();
  }

  /// Small dialog to edit the per-file source cap (eMule FT_MAXSOURCES).
  /// Empty input clears the override and restores the global limit.
  Future<void> _editMaxSources(
    BuildContext context,
    WidgetRef ref,
    DownloadItem detail,
  ) async {
    final controller = TextEditingController(
      text: detail.maxSources?.toString() ?? '',
    );
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        title: Text(ctx.l10n.sourcesMaxDialogTitle),
        content: TextField(
          controller: controller,
          keyboardType: TextInputType.number,
          autofocus: true,
          decoration: InputDecoration(
            hintText: ctx.l10n.sourcesMaxHint,
            helperText: ctx.l10n.sourcesMaxHelper,
          ),
          onSubmitted: (_) => Navigator.of(ctx).pop(true),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: Text(ctx.l10n.annuler),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: Text(ctx.l10n.apply),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    final raw = controller.text.trim();
    final value = raw.isEmpty ? null : int.tryParse(raw);
    if (raw.isNotEmpty && (value == null || value < 1 || value > 10000)) {
      return;
    }
    await ref
        .read(peerSourcesControllerProvider)
        .setDownloadMaxSources(widget.item.id, value);
    await ref
        .read(peerSourcesControllerProvider)
        .loadDetailForId(widget.item.id);
  }

  @override
  Widget build(BuildContext context) {
    final ctl = ref.watch(peerSourcesControllerProvider);
    final hash = _hash;
    final sources = ctl.getVisibleSourcesForHash(hash);
    final swarm = ctl.getSwarmForHash(hash);
    final detail = ctl.getDetailForId(widget.item.id) ?? widget.item;

    Widget body;
    if (hash.isEmpty) {
      body = _ExpansionPlaceholder(context.l10n.srcNoHash);
    } else if (sources.isEmpty && swarm.peers.isEmpty) {
      body = _ExpansionPlaceholder(
        ctl.isLoading ? context.l10n.srcLoading : context.l10n.srcNone,
      );
    } else {
      body = Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (swarm.peers.isNotEmpty) ...[
            _SectionHeader(
              icon: Icons.download_rounded,
              label: context.l10n.srcConnected(swarm.peers.length),
            ),
            ...swarm.peers.map(
              (p) => _ConnectedPeerRow(session: p, fileHash: hash),
            ),
            const Divider(height: 1, color: AppTheme.darkBorder),
          ],
          _SectionHeader(
            icon: Icons.list_alt_rounded,
            label: context.l10n.srcKnown(sources.length),
          ),
          ListView.separated(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            itemCount: sources.length,
            separatorBuilder: (context, index) =>
                const Divider(height: 1, color: AppTheme.darkBorder),
            itemBuilder: (context, index) => PeerSourceRow(
              source: sources[index],
              fileHash: hash,
              session: ctl.session,
              isPeerConnecting: ctl.isConnecting,
              onConnect: (address, {force = false}) =>
                  ctl.connect(hash, address, force: force),
              onDisconnect: (address) => ctl.disconnect(address: address),
            ),
          ),
        ],
      );
    }

    return Container(
      margin: const EdgeInsets.only(top: 8),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
            child: Row(
              children: [
                const Icon(Icons.hub_outlined,
                    size: 14, color: AppColors.textMuted),
                const SizedBox(width: 6),
                Text(
                  context.l10n.srcCount(sources.length),
                  style: const TextStyle(
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                    color: AppColors.textSecondary,
                  ),
                ),
                const Spacer(),
                // eMule-style file priority (biases the swarm slot quota).
                SizedBox(
                  height: 22,
                  child: DropdownButton<DownloadPriority>(
                    value: detail.priority,
                    isDense: true,
                    underline: const SizedBox.shrink(),
                    style: const TextStyle(
                      fontSize: 10,
                      color: AppColors.textSecondary,
                    ),
                    dropdownColor: AppTheme.darkSurface,
                    items: DownloadPriority.values
                        .map(
                          (p) => DropdownMenuItem(
                            value: p,
                            child: Text(
                              context.l10n.srcPriorityLabel(
                                p.labelL10n(context.l10n),
                              ),
                            ),
                          ),
                        )
                        .toList(),
                    onChanged: (p) {
                      if (p != null) {
                        ctl.setDownloadPriority(
                          widget.item.id,
                          p.name,
                        );
                      }
                    },
                  ),
                ),
                const SizedBox(width: 4),
                // eMule per-file source cap (FT_MAXSOURCES).
                Tooltip(
                  message: context.l10n.srcMaxTooltip,
                  child: InkWell(
                    onTap: () => _editMaxSources(context, ref, detail),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 4,
                        vertical: 2,
                      ),
                      child: Text(
                        context.l10n.srcMaxShort(
                          detail.maxSources?.toString() ?? '∞',
                        ),
                        style: const TextStyle(
                          fontSize: 10,
                          color: AppColors.textMuted,
                        ),
                      ),
                    ),
                  ),
                ),
                InkWell(
                  onTap: hash.isEmpty
                      ? null
                      : () => ref
                          .read(peerSourcesControllerProvider)
                          .loadSourcesForHash(hash),
                  child: const Padding(
                    padding: EdgeInsets.all(4),
                    child: Icon(Icons.refresh,
                        size: 14, color: AppColors.textMuted),
                  ),
                ),
              ],
            ),
          ),
          const Divider(height: 1, color: AppTheme.darkBorder),
          body,
          if (detail.parts.isNotEmpty) ...[
            const Divider(height: 1, color: AppTheme.darkBorder),
            Padding(
              padding: const EdgeInsets.all(8),
              child: DownloadPartMapCard(item: detail),
            ),
          ],
        ],
      ),
    );
  }
}

class _SectionHeader extends StatelessWidget {
  final IconData icon;
  final String label;

  const _SectionHeader({required this.icon, required this.label});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
      child: Row(
        children: [
          Icon(icon, size: 12, color: AppColors.textFaint),
          const SizedBox(width: 6),
          Text(
            label,
            style: const TextStyle(
              fontSize: 10,
              fontWeight: FontWeight.w600,
              color: AppColors.textFaint,
            ),
          ),
        ],
      ),
    );
  }
}

/// A live connected peer: address, connection state, queue rank, part in
/// progress, bytes received and current rate.
class _ConnectedPeerRow extends StatelessWidget {
  final PeerSessionModel session;
  final String fileHash;

  const _ConnectedPeerRow({required this.session, required this.fileHash});

  Color get _statusColor {
    switch (session.status) {
      case PeerConnectionStatus.transferring:
        return AppColors.accentTeal;
      case PeerConnectionStatus.queued:
        return AppColors.accentAmber;
      case PeerConnectionStatus.connecting:
      case PeerConnectionStatus.handshaking:
        return AppColors.accentBlue;
      default:
        return AppColors.textMuted;
    }
  }

  @override
  Widget build(BuildContext context) {
    final rate = Formatters.formatSpeed(session.transferRateBps);
    final received = Formatters.formatBytes(session.transferredBytes);
    final queue = session.queueRank != null ? ' · QR #${session.queueRank}' : '';
    final part = session.currentPart != null
        ? ' · part ${session.currentPart! + 1}'
        : '';

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
      child: Row(
        children: [
          Icon(Icons.swap_vert_rounded, size: 14, color: _statusColor),
          const SizedBox(width: 8),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Text(
                      session.peerAddress ?? '?',
                      style: const TextStyle(
                        fontSize: 11,
                        fontWeight: FontWeight.w600,
                        color: AppColors.textPrimary,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 5,
                        vertical: 1,
                      ),
                      decoration: BoxDecoration(
                        color: _statusColor.withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        session.status.labelL10n(context.l10n),
                        style: TextStyle(
                          fontSize: 9,
                          fontWeight: FontWeight.w600,
                          color: _statusColor,
                        ),
                      ),
                    ),
                  ],
                ),
                Text(
                  context.l10n.peerReceivedBytes(received) + queue + part,
                  style: const TextStyle(
                    fontSize: 10,
                    color: AppColors.textMuted,
                  ),
                ),
              ],
            ),
          ),
          Text(
            rate,
            style: TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w600,
              color: session.transferRateBps > 0
                  ? AppColors.accentTeal
                  : AppColors.textMuted,
            ),
          ),
        ],
      ),
    );
  }
}

class _ExpansionPlaceholder extends StatelessWidget {
  final String text;
  const _ExpansionPlaceholder(this.text);

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 16),
      child: Center(
        child: Text(
          text,
          style: const TextStyle(fontSize: 11, color: AppColors.textFaint),
        ),
      ),
    );
  }
}
