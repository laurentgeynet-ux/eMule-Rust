import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../../../domain/models/download_item.dart';
import '../../../core/formatters.dart';
import '../../../core/theme.dart';
import '../view_models/downloads_view_model.dart';
import 'download_detail_dialog.dart';

class DownloadRow extends StatelessWidget {
  final DownloadItem item;
  final DownloadsViewModel viewModel;
  final bool isSelected;
  final VoidCallback onTap;

  const DownloadRow({
    super.key,
    required this.item,
    required this.viewModel,
    required this.isSelected,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final statusColor = AppTheme.getStatusColor(item.status);
    final category = item.categoryId != null
        ? viewModel.categories.where((c) => c.id == item.categoryId).firstOrNull
        : null;

    return Container(
      margin: const EdgeInsets.only(bottom: 8),
      decoration: BoxDecoration(
        color: isSelected
            ? AppTheme.primary.withValues(alpha: 0.1)
            : AppTheme.darkSurface,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: isSelected ? AppTheme.primary : AppTheme.darkBorder,
          width: isSelected ? 1.5 : 0.5,
        ),
      ),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    // File Type Icon
                    Container(
                      padding: const EdgeInsets.all(8),
                      decoration: BoxDecoration(
                        color: Colors.white.withValues(alpha: 0.05),
                        borderRadius: BorderRadius.circular(6),
                      ),
                      child: Icon(
                        _getFileIcon(item.fileName),
                        color: Colors.white70,
                        size: 20,
                      ),
                    ),
                    const SizedBox(width: 14),

                    // File Name and Hash/ID
                    Expanded(
                      flex: 4,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            item.fileName,
                            style: const TextStyle(
                              fontSize: 14,
                              fontWeight: FontWeight.w600,
                              color: Colors.white,
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          const SizedBox(height: 2),
                          Wrap(
                            crossAxisAlignment: WrapCrossAlignment.center,
                            spacing: 8,
                            runSpacing: 4,
                            children: [
                              Row(
                                mainAxisSize: MainAxisSize.min,
                                children: [
                                  Tooltip(
                                    message: 'Full ID: ${item.id}',
                                    preferBelow: false,
                                    child: Text(
                                      'ID: ${_abbreviateId(item.id)}',
                                      style: const TextStyle(
                                        fontSize: 11,
                                        color: Colors.white38,
                                        fontFamily: 'monospace',
                                      ),
                                    ),
                                  ),
                                  const SizedBox(width: 4),
                                  InkWell(
                                    borderRadius: BorderRadius.circular(4),
                                    onTap: () {
                                      Clipboard.setData(ClipboardData(text: item.id));
                                      ScaffoldMessenger.of(context).showSnackBar(
                                        const SnackBar(
                                          content: Text('Download ID copied to clipboard'),
                                          duration: Duration(seconds: 1),
                                        ),
                                      );
                                    },
                                    child: const Padding(
                                      padding: EdgeInsets.all(2.0),
                                      child: Icon(
                                        Icons.copy_rounded,
                                        size: 11,
                                        color: Colors.white38,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                              if (category != null)
                                Container(
                                  padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
                                  decoration: BoxDecoration(
                                    color: category.parsedColor.withValues(alpha: 0.15),
                                    borderRadius: BorderRadius.circular(4),
                                    border: Border.all(
                                      color: category.parsedColor.withValues(alpha: 0.4),
                                      width: 0.8,
                                    ),
                                  ),
                                  child: Row(
                                    mainAxisSize: MainAxisSize.min,
                                    children: [
                                      Container(
                                        width: 6,
                                        height: 6,
                                        decoration: BoxDecoration(
                                          color: category.parsedColor,
                                          shape: BoxShape.circle,
                                        ),
                                      ),
                                      const SizedBox(width: 4),
                                      Text(
                                        category.name,
                                        style: TextStyle(
                                          fontSize: 10,
                                          fontWeight: FontWeight.w600,
                                          color: category.parsedColor,
                                        ),
                                      ),
                                    ],
                                  ),
                                ),
                            ],
                          ),
                        ],
                      ),
                    ),

                    const SizedBox(width: 16),

                    // Size & Progress Ratio
                    Expanded(
                      flex: 3,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            '${Formatters.formatBytes(item.receivedBytes)} / ${Formatters.formatBytes(item.totalBytes)}',
                            style: const TextStyle(
                              fontSize: 12,
                              color: Colors.white70,
                            ),
                          ),
                          const SizedBox(height: 4),
                          // Progress Bar
                          ClipRRect(
                            borderRadius: BorderRadius.circular(4),
                            child: LinearProgressIndicator(
                              value: item.progress,
                              minHeight: 6,
                              backgroundColor: Colors.white12,
                              valueColor:
                                  AlwaysStoppedAnimation<Color>(statusColor),
                            ),
                          ),
                        ],
                      ),
                    ),

                    const SizedBox(width: 16),

                    // Percentage
                    SizedBox(
                      width: 55,
                      child: Text(
                        Formatters.formatPercent(item.progress),
                        style: const TextStyle(
                          fontSize: 13,
                          fontWeight: FontWeight.w600,
                          color: Colors.white,
                        ),
                      ),
                    ),

                    // Status Pill
                    Container(
                      padding: const EdgeInsets.symmetric(
                          horizontal: 10, vertical: 4),
                      decoration: BoxDecoration(
                        color: statusColor.withValues(alpha: 0.15),
                        borderRadius: BorderRadius.circular(12),
                        border: Border.all(
                            color: statusColor.withValues(alpha: 0.3),
                            width: 0.8),
                      ),
                      child: Text(
                        item.status.label,
                        style: TextStyle(
                          fontSize: 11,
                          fontWeight: FontWeight.w600,
                          color: statusColor,
                        ),
                      ),
                    ),

                    const SizedBox(width: 16),

                    // Rate & Sources
                    SizedBox(
                      width: 110,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.end,
                        children: [
                          Text(
                            Formatters.formatSpeed(item.downloadRateBps),
                            style: TextStyle(
                              fontSize: 12,
                              fontWeight: FontWeight.w600,
                              color: item.downloadRateBps > 0
                                  ? AppTheme.statusDownloading
                                  : Colors.white38,
                            ),
                          ),
                          const SizedBox(height: 2),
                          Text(
                            '${item.sourceCount} sources',
                            style: const TextStyle(
                              fontSize: 11,
                              color: Colors.white54,
                            ),
                          ),
                        ],
                      ),
                    ),

                    const SizedBox(width: 16),

                    // Actions
                    Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        PopupMenuButton<String?>(
                          icon: Icon(
                            category != null ? Icons.label_rounded : Icons.label_outline_rounded,
                            color: category != null ? category.parsedColor : Colors.white54,
                            size: 18,
                          ),
                          tooltip: category != null ? 'Category: ${category.name}' : 'Assign Category',
                          onSelected: (catId) {
                            viewModel.setDownloadCategory(item.id, catId);
                          },
                          itemBuilder: (ctx) => [
                            const PopupMenuItem<String?>(
                              value: null,
                              child: Text('Sans catégorie (None)'),
                            ),
                            ...viewModel.categories.map((c) => PopupMenuItem<String?>(
                              value: c.id,
                              child: Row(
                                children: [
                                  Container(
                                    width: 10,
                                    height: 10,
                                    decoration: BoxDecoration(color: c.parsedColor, shape: BoxShape.circle),
                                  ),
                                  const SizedBox(width: 8),
                                  Text(c.name),
                                ],
                              ),
                            )),
                          ],
                        ),
                        IconButton(
                          icon: const Icon(Icons.info_outline_rounded,
                              color: Colors.white70, size: 20),
                          tooltip: 'Inspect Details & Part Map',
                          onPressed: () => _openDetailDialog(context),
                        ),
                        if (item.status.isPaused)
                          IconButton(
                            icon: const Icon(Icons.play_arrow_rounded,
                                color: AppTheme.statusCompleted),
                            tooltip: 'Resume',
                            onPressed: () => viewModel.resumeDownload(item.id),
                          )
                        else if (item.status.isActive)
                          IconButton(
                            icon: const Icon(Icons.pause_rounded,
                                color: AppTheme.statusPaused),
                            tooltip: 'Pause',
                            onPressed: () => viewModel.pauseDownload(item.id),
                          ),
                        IconButton(
                          icon: const Icon(Icons.delete_outline_rounded,
                              color: Colors.white54, size: 20),
                          tooltip: 'Remove',
                          onPressed: () => _confirmRemove(context),
                        ),
                      ],
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  void _openDetailDialog(BuildContext context) {
    showDialog(
      context: context,
      builder: (ctx) => DownloadDetailDialog(
        downloadId: item.id,
        viewModel: viewModel,
      ),
    );
  }

  void _confirmRemove(BuildContext context) {
    bool deleteFilesOnDisk = true;
    showDialog(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, setDialogState) => AlertDialog(
          title: const Text('Remove Download?'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Remove "${item.fileName}" from downloads?'),
              const SizedBox(height: 12),
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                dense: true,
                title: const Text(
                  'Delete downloaded part/temp files from disk',
                  style: TextStyle(fontSize: 13),
                ),
                value: deleteFilesOnDisk,
                onChanged: (val) {
                  setDialogState(() {
                    deleteFilesOnDisk = val ?? true;
                  });
                },
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              style: FilledButton.styleFrom(backgroundColor: AppTheme.statusFailed),
              onPressed: () {
                viewModel.removeDownload(item.id, deleteFiles: deleteFilesOnDisk);
                Navigator.of(ctx).pop();
              },
              child: const Text('Remove'),
            ),
          ],
        ),
      ),
    );
  }

  IconData _getFileIcon(String fileName) {
    final lower = fileName.toLowerCase();
    if (lower.endsWith('.iso') || lower.endsWith('.img')) {
      return Icons.album_rounded;
    }
    if (lower.endsWith('.zip') ||
        lower.endsWith('.rar') ||
        lower.endsWith('.7z') ||
        lower.endsWith('.tar.xz')) {
      return Icons.folder_zip_rounded;
    }
    if (lower.endsWith('.mp4') || lower.endsWith('.mkv') || lower.endsWith('.avi')) {
      return Icons.video_file_rounded;
    }
    if (lower.endsWith('.mp3') || lower.endsWith('.flac') || lower.endsWith('.ogg')) {
      return Icons.audio_file_rounded;
    }
    if (lower.endsWith('.blend')) {
      return Icons.view_in_ar_rounded;
    }
    return Icons.insert_drive_file_rounded;
  }

  static String _abbreviateId(String id) {
    if (id.length <= 8) return id;
    return '${id.substring(0, 8)}…';
  }
}
