import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/providers.dart';

/// Shared files view (`/shares`): every file served to remote peers with
/// its eMule upload priority (powershare) selector and enable switch.
/// Mirrors the old eMule "Shared Files" tab.
class SharesView extends ConsumerStatefulWidget {
  const SharesView({super.key});

  @override
  ConsumerState<SharesView> createState() => _SharesViewState();
}

class _SharesViewState extends ConsumerState<SharesView> {
  List<SharedFileModel> _files = const [];
  bool _loading = true;

  static const _priorities = [
    ('verylow', 'Très basse'),
    ('low', 'Basse'),
    ('normal', 'Normale'),
    ('high', 'Haute'),
    ('veryhigh', 'Très haute'),
    ('powershare', 'PowerShare'),
  ];

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final files = await ref.read(muleDaemonClientProvider).listSharedFiles();
      if (mounted) setState(() => _files = files);
    } catch (_) {
      // Non-fatal.
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Row(
            children: [
              const Text(
                'Fichiers partagés',
                style: TextStyle(
                  fontSize: 16,
                  fontWeight: FontWeight.bold,
                  color: AppColors.textPrimary,
                ),
              ),
              const SizedBox(width: 12),
              Text(
                '${_files.length} fichier(s)',
                style: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                ),
              ),
              const Spacer(),
              IconButton(
                icon: const Icon(Icons.refresh, size: 18),
                tooltip: 'Rafraîchir',
                onPressed: _load,
              ),
            ],
          ),
          const SizedBox(height: 8),
          Expanded(
            child: _loading
                ? const Center(child: CircularProgressIndicator())
                : _files.isEmpty
                    ? const Center(
                        child: Text(
                          'Aucun fichier partagé.\nLes fichiers du dossier Incoming et les téléchargements terminés sont partagés automatiquement.',
                          textAlign: TextAlign.center,
                          style: TextStyle(
                            fontSize: 12,
                            color: AppColors.textMuted,
                          ),
                        ),
                      )
                    : ListView.separated(
                        itemCount: _files.length,
                        separatorBuilder: (_, __) => const Divider(
                          height: 1,
                          color: AppTheme.darkBorder,
                        ),
                        itemBuilder: (context, i) =>
                            _ShareRow(file: _files[i], onChanged: _load),
                      ),
          ),
        ],
      ),
    );
  }
}

class _ShareRow extends ConsumerWidget {
  final SharedFileModel file;
  final VoidCallback onChanged;

  const _ShareRow({required this.file, required this.onChanged});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Row(
        children: [
          Expanded(
            flex: 3,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  file.fileName,
                  style: const TextStyle(
                    fontSize: 12,
                    color: AppColors.textPrimary,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
                Tooltip(
                  message: file.filePath,
                  child: Text(
                    '${Formatters.formatBytes(file.fileSize)} — ${file.fileHash}',
                    style: const TextStyle(
                      fontSize: 10,
                      color: AppColors.textMuted,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
            ),
          ),
          SizedBox(
            height: 24,
            child: DropdownButton<String>(
              value: _SharesViewState._priorities
                      .any((p) => p.$1 == file.upPriority)
                  ? file.upPriority
                  : 'normal',
              isDense: true,
              underline: const SizedBox.shrink(),
              style: const TextStyle(
                fontSize: 11,
                color: AppColors.textSecondary,
              ),
              dropdownColor: AppTheme.darkSurface,
              items: _SharesViewState._priorities
                  .map((p) => DropdownMenuItem(value: p.$1, child: Text(p.$2)))
                  .toList(),
              onChanged: (v) async {
                if (v == null) return;
                try {
                  await ref
                      .read(muleDaemonClientProvider)
                      .setSharedFileUploadPriority(file.fileHash, v);
                  onChanged();
                } catch (_) {}
              },
            ),
          ),
          const SizedBox(width: 12),
          Tooltip(
            message: file.enabled ? 'Partagé' : 'Suspendu',
            child: Switch(
              value: file.enabled,
              onChanged: (v) async {
                try {
                  await ref
                      .read(muleDaemonClientProvider)
                      .setSharedFileEnabled(file.fileHash, v);
                  onChanged();
                } catch (_) {}
              },
            ),
          ),
        ],
      ),
    );
  }
}
