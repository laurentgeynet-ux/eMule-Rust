import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../../app/providers.dart';

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
    final hashCtl = TextEditingController();
    final nameCtl = TextEditingController();
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTheme.darkSurface,
        title: const Text('Ajouter un ami'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: hashCtl,
              autofocus: true,
              decoration: const InputDecoration(
                labelText: 'UserHash (hex)',
                hintText: '32 caractères hexadécimaux',
              ),
            ),
            const SizedBox(height: 8),
            TextField(
              controller: nameCtl,
              decoration: const InputDecoration(labelText: 'Nom (optionnel)'),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: const Text('Annuler'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: const Text('Ajouter'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    final hash = hashCtl.text.trim().toLowerCase();
    if (hash.length != 32 ||
        !RegExp(r'^[0-9a-f]{32}$').hasMatch(hash)) {
      return;
    }
    try {
      await ref
          .read(muleDaemonClientProvider)
          .addFriend(hash, name: nameCtl.text.trim());
      _load();
    } catch (_) {}
  }

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          padding: const EdgeInsets.all(8),
          decoration: BoxDecoration(
            color: AppTheme.primary.withValues(alpha: 0.15),
            borderRadius: BorderRadius.circular(6),
          ),
          child: const Icon(
            Icons.group_outlined,
            size: 18,
            color: AppTheme.primary,
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Expanded(
                    child: Text(
                      'Amis',
                      style: TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.bold,
                        color: AppColors.textPrimary,
                      ),
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.person_add_alt_1, size: 16),
                    tooltip: 'Ajouter un ami',
                    onPressed: _addFriend,
                  ),
                ],
              ),
              const Text(
                'Slot d\'upload dédié sans passer par la file (friend slot eMule).',
                style: TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                  height: 1.3,
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
                const Text(
                  'Aucun ami configuré.',
                  style: TextStyle(fontSize: 12, color: AppColors.textMuted),
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
                          tooltip: 'Retirer',
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
          ),
        ),
      ],
    );
  }
}
