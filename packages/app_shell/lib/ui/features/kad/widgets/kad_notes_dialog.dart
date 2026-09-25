// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/providers.dart';
import '../../../../l10n/l10n.dart';

/// Shows the Kad file-notes dialog for [fileHash]: existing comments/
/// ratings collected from the DHT (`kad.searchNotes`) plus a form to
/// publish our own note (`kad.publishNote`) — the eMule « file comments »
/// equivalent.
Future<void> showKadNotesDialog(
  BuildContext context, {
  required String fileHash,
  required String fileName,
  required int fileSize,
}) {
  return showDialog(
    context: context,
    builder: (_) => KadNotesDialog(
      fileHash: fileHash,
      fileName: fileName,
      fileSize: fileSize,
    ),
  );
}

/// Notes list + publish form. Goes through [kadControllerProvider] —
/// never calls the daemon directly (Clean Architecture MVVM).
class KadNotesDialog extends ConsumerStatefulWidget {
  final String fileHash;
  final String fileName;
  final int fileSize;

  const KadNotesDialog({
    super.key,
    required this.fileHash,
    required this.fileName,
    required this.fileSize,
  });

  @override
  ConsumerState<KadNotesDialog> createState() => _KadNotesDialogState();
}

class _KadNotesDialogState extends ConsumerState<KadNotesDialog> {
  List<KadNoteModel>? _notes;
  String? _error;
  bool _publishing = false;
  final _commentCtrl = TextEditingController();
  int _rating = 0;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    _commentCtrl.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _notes = null;
      _error = null;
    });
    try {
      final notes = await ref
          .read(kadControllerProvider)
          .searchNotes(widget.fileHash, widget.fileSize);
      if (mounted) setState(() => _notes = notes);
    } catch (e) {
      if (mounted) {
        setState(() => _error = context.l10n.kadNotesLoadError('$e'));
      }
    }
  }

  Future<void> _publish() async {
    final l10n = context.l10n;
    final comment = _commentCtrl.text.trim();
    if (comment.isEmpty) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(l10n.kadNotesCommentRequired)),
      );
      return;
    }
    setState(() => _publishing = true);
    try {
      final count = await ref.read(kadControllerProvider).publishNote(
            fileHash: widget.fileHash,
            fileName: widget.fileName,
            fileSize: widget.fileSize,
            comment: comment,
            rating: _rating > 0 ? _rating : null,
          );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.kadNotesPublished(count))),
        );
        _commentCtrl.clear();
        setState(() => _rating = 0);
        await _load();
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(l10n.kadNotesPublishError('$e'))),
        );
      }
    } finally {
      if (mounted) setState(() => _publishing = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.l10n;
    return AlertDialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      title: Row(
        children: [
          const Icon(
            Icons.rate_review_outlined,
            color: AppColors.accentPurple,
            size: 20,
          ),
          const SizedBox(width: 8),
          Expanded(
            child: Tooltip(
              message: widget.fileName,
              child: Text(
                l10n.kadNotesTitle,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ),
        ],
      ),
      content: SizedBox(
        width: 460,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Flexible(child: _buildNotesList(context)),
            const SizedBox(height: 12),
            const Divider(color: AppTheme.darkBorder, height: 1),
            const SizedBox(height: 10),
            Text(
              l10n.kadNotesYourComment,
              style: const TextStyle(
                fontSize: 12,
                fontWeight: FontWeight.w600,
                color: AppColors.textPrimary,
              ),
            ),
            const SizedBox(height: 6),
            Row(
              children: [
                Text(
                  l10n.kadNotesRating,
                  style: const TextStyle(
                    fontSize: 11,
                    color: AppColors.textMuted,
                  ),
                ),
                const SizedBox(width: 8),
                for (var i = 1; i <= 5; i++)
                  InkWell(
                    onTap: () => setState(
                      () => _rating = _rating == i ? 0 : i,
                    ),
                    child: Icon(
                      i <= _rating ? Icons.star : Icons.star_border,
                      size: 18,
                      color: i <= _rating
                          ? AppColors.accentAmber
                          : AppColors.textMuted,
                    ),
                  ),
              ],
            ),
            const SizedBox(height: 6),
            TextField(
              controller: _commentCtrl,
              maxLines: 2,
              maxLength: 512,
              style: const TextStyle(
                fontSize: 12,
                color: AppColors.textPrimary,
              ),
              decoration: InputDecoration(
                hintText: l10n.kadNotesCommentHint,
                hintStyle: const TextStyle(
                  fontSize: 12,
                  color: AppColors.textMuted,
                ),
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
          onPressed: () => Navigator.of(context).pop(),
          child: Text(
            l10n.btnClose,
            style: const TextStyle(color: AppColors.textSoft),
          ),
        ),
        FilledButton.icon(
          style: FilledButton.styleFrom(
            backgroundColor: AppColors.accentPurple,
          ),
          icon: _publishing
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.send_rounded, size: 14),
          label: Text(l10n.kadNotesPublish),
          onPressed: _publishing ? null : _publish,
        ),
      ],
    );
  }

  Widget _buildNotesList(BuildContext context) {
    final l10n = context.l10n;
    if (_error != null) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              _error!,
              textAlign: TextAlign.center,
              style: const TextStyle(
                fontSize: 12,
                color: AppTheme.statusFailed,
              ),
            ),
            const SizedBox(height: 8),
            TextButton(
              onPressed: _load,
              child: Text(l10n.btnRetry),
            ),
          ],
        ),
      );
    }
    final notes = _notes;
    if (notes == null) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 24),
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const SizedBox(
                width: 18,
                height: 18,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              const SizedBox(height: 8),
              Text(
                l10n.kadNotesLoading,
                style: const TextStyle(
                  fontSize: 11,
                  color: AppColors.textMuted,
                ),
              ),
            ],
          ),
        ),
      );
    }
    if (notes.isEmpty) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 16),
        child: Text(
          l10n.kadNotesEmpty,
          textAlign: TextAlign.center,
          style: const TextStyle(fontSize: 12, color: AppColors.textMuted),
        ),
      );
    }
    return ConstrainedBox(
      constraints: const BoxConstraints(maxHeight: 220),
      child: ListView.separated(
        shrinkWrap: true,
        itemCount: notes.length,
        separatorBuilder: (_, __) =>
            const Divider(height: 1, color: AppTheme.darkBorder),
        itemBuilder: (context, i) => _NoteTile(note: notes[i]),
      ),
    );
  }
}

class _NoteTile extends StatelessWidget {
  final KadNoteModel note;

  const _NoteTile({required this.note});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              if (note.rating > 0)
                Row(
                  children: [
                    for (var i = 1; i <= 5; i++)
                      Icon(
                        i <= note.rating ? Icons.star : Icons.star_border,
                        size: 12,
                        color: i <= note.rating
                            ? AppColors.accentAmber
                            : AppColors.textFaint,
                      ),
                    const SizedBox(width: 6),
                  ],
                ),
              Expanded(
                child: Text(
                  context.l10n.kadNotesPublisher(
                    note.sourceId.length > 8
                        ? note.sourceId.substring(0, 8)
                        : note.sourceId,
                  ),
                  style: const TextStyle(
                    fontSize: 10,
                    color: AppColors.textMuted,
                  ),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
          if (note.comment.isNotEmpty) ...[
            const SizedBox(height: 2),
            Text(
              note.comment,
              style: const TextStyle(
                fontSize: 12,
                color: AppColors.textPrimary,
              ),
            ),
          ],
        ],
      ),
    );
  }
}
