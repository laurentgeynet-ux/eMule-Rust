// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../app/providers.dart';
import '../../../../../l10n/l10n.dart';
import '../../../settings/views/sections/settings_section_scaffold.dart';

/// Friends section: userhashes granted a dedicated upload slot bypassing
/// the queue (eMule `friends.met` / friend slot) — `friends.list/add/remove`.
class SettingsFriendsSection extends ConsumerStatefulWidget {
  const SettingsFriendsSection({super.key});

  @override
  ConsumerState<SettingsFriendsSection> createState() =>
      _SettingsFriendsSectionState();
}

class _SettingsFriendsSectionState
    extends ConsumerState<SettingsFriendsSection> {
  List<FriendModel> _friends = const [];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final friends = await ref.read(muleDaemonClientProvider).listFriends();
      if (mounted) setState(() => _friends = friends);
    } catch (_) {
      // Non-fatal.
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _addFriend() async {
    final l10n = context.l10n;
    final hashCtl = TextEditingController();
    final nameCtl = TextEditingController();
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        title: Text(l10n.settingsAddFriendTitle),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: hashCtl,
              autofocus: true,
              decoration: InputDecoration(
                labelText: l10n.settingsUserHashLabel,
                hintText: l10n.settingsUserHashHint,
              ),
            ),
            const SizedBox(height: 8),
            TextField(
              controller: nameCtl,
              decoration:
                  InputDecoration(labelText: l10n.settingsFriendNameLabel),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: Text(l10n.btnCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: Text(l10n.ajouter),
          ),
        ],
      ),
    );
    final hash = hashCtl.text.trim().toLowerCase();
    final name = nameCtl.text.trim();
    hashCtl.dispose();
    nameCtl.dispose();
    if (confirmed != true || !mounted) return;
    if (hash.length != 32 ||
        !RegExp(r'^[0-9a-f]{32}$').hasMatch(hash)) {
      return;
    }
    try {
      await ref
          .read(muleDaemonClientProvider)
          .addFriend(hash, name: name);
      _load();
    } catch (_) {}
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return SettingsSectionScaffold(
      title: l10n.settingsSectionFriends,
      subtitle: l10n.settingsFriendsSubtitle,
      icon: Icons.people_alt_outlined,
      children: [
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.tonalIcon(
            icon: const Icon(Icons.person_add_alt_1, size: 16),
            label: Text(l10n.settingsAddFriendTitle),
            onPressed: _addFriend,
          ),
        ),
        const SizedBox(height: 8),
        if (_loading)
          const SizedBox(
            height: 20,
            width: 20,
            child: CircularProgressIndicator(strokeWidth: 2),
          )
        else if (_friends.isEmpty)
          Text(
            l10n.settingsNoFriends,
            style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
          )
        else
          for (final f in _friends)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 2),
              child: Row(
                children: [
                  const Icon(Icons.person_outline,
                      size: 14, color: AppColors.textMuted),
                  const SizedBox(width: 6),
                  Expanded(
                    child: Tooltip(
                      message: f.userHash,
                      child: Text(
                        f.name.isNotEmpty ? f.name : f.userHash,
                        style: const TextStyle(
                          fontSize: 12,
                          color: AppColors.textPrimary,
                        ),
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.close, size: 14),
                    tooltip: l10n.settingsRemoveFriendTooltip,
                    onPressed: () async {
                      try {
                        await ref
                            .read(muleDaemonClientProvider)
                            .removeFriend(f.userHash);
                        _load();
                      } catch (_) {}
                    },
                  ),
                ],
              ),
            ),
      ],
    );
  }
}
