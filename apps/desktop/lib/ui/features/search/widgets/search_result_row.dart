import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';

/// One search result row: file name, size, source count badge, copyable
/// eD2K hash, and actions (download / existing-download status, query
/// sources, copy ed2k link).
class SearchResultRow extends StatelessWidget {
  final SearchResultFileModel file;
  final int index;
  final DownloadItem? existingDownload;
  final Future<bool> Function(SearchResultFileModel file) onDownload;
  final VoidCallback onGoToDownloads;
  final void Function(SearchResultFileModel file) onQuerySources;
  final String? Function() lastError;

  const SearchResultRow({
    super.key,
    required this.file,
    required this.index,
    required this.existingDownload,
    required this.onDownload,
    required this.onGoToDownloads,
    required this.onQuerySources,
    required this.lastError,
  });

  @override
  Widget build(BuildContext context) {
    final isCompleted =
        existingDownload != null && existingDownload!.status.isCompleted;

    return Container(
      height: 44.0,
      decoration: BoxDecoration(
        border: const Border(
          bottom: BorderSide(color: AppTheme.darkBorder, width: 0.3),
        ),
        color: index.isEven
            ? Colors.transparent
            : AppColors.textPrimary.withAlpha(2),
      ),
      child: Material(
        color: Colors.transparent,
        child: InkWell(
          hoverColor: AppTheme.primary.withAlpha(15),
          onTap: () {},
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              children: [
                // Nom du fichier
                Expanded(
                  flex: 4,
                  child: Row(
                    children: [
                      const Icon(
                        Icons.insert_drive_file_outlined,
                        size: 16,
                        color: AppColors.textMuted,
                      ),
                      const SizedBox(width: 8),
                      Flexible(
                        child: Tooltip(
                          message: file.fileName,
                          waitDuration: const Duration(milliseconds: 500),
                          child: Text(
                            file.fileName,
                            style: const TextStyle(
                              color: AppColors.textPrimary,
                              fontWeight: FontWeight.w500,
                              fontSize: 13,
                            ),
                            overflow: TextOverflow.ellipsis,
                            maxLines: 1,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 12),

                // Taille
                SizedBox(
                  width: 95,
                  child: Align(
                    alignment: Alignment.centerRight,
                    child: Text(
                      Formatters.formatBytes(file.fileSize),
                      style: const TextStyle(
                        color: AppColors.textSecondary,
                        fontSize: 12.5,
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 12),

                // Sources
                SizedBox(
                  width: 100,
                  child: Align(
                    alignment: Alignment.centerRight,
                    child: Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 8,
                        vertical: 2,
                      ),
                      decoration: BoxDecoration(
                        color: file.sourceCount > 0
                            ? AppColors.tintGreen.withAlpha(30)
                            : AppColors.fillSubtle,
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        '${file.sourceCount}',
                        style: TextStyle(
                          color: file.sourceCount > 0
                              ? AppColors.positive
                              : AppColors.textFaint,
                          fontWeight: FontWeight.bold,
                          fontSize: 12,
                        ),
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 12),

                // Hachage eD2K
                SizedBox(
                  width: 220,
                  child: Tooltip(
                    message:
                        'Cliquer pour copier le hash : ${file.fileHash}',
                    waitDuration: const Duration(milliseconds: 400),
                    child: InkWell(
                      borderRadius: BorderRadius.circular(4),
                      onTap: () {
                        Clipboard.setData(
                          ClipboardData(text: file.fileHash),
                        );
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(
                            content: Text(
                              'Hachage copié : ${file.fileHash}',
                            ),
                            duration: const Duration(seconds: 2),
                          ),
                        );
                      },
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                          vertical: 4,
                          horizontal: 2,
                        ),
                        child: Text(
                          file.fileHash,
                          style: const TextStyle(
                            fontFamily: 'monospace',
                            fontSize: 11,
                            color: AppColors.textMuted,
                          ),
                          overflow: TextOverflow.ellipsis,
                          maxLines: 1,
                        ),
                      ),
                    ),
                  ),
                ),
                const SizedBox(width: 12),

                // Actions
                SizedBox(
                  width: 280,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (isCompleted)
                        OutlinedButton.icon(
                          icon: const Icon(
                            Icons.check_circle_rounded,
                            size: 14,
                            color: AppTheme.statusCompleted,
                          ),
                          label: const Text(
                            'Terminé',
                            style: TextStyle(
                              fontSize: 12,
                              color: AppTheme.statusCompleted,
                            ),
                          ),
                          style: OutlinedButton.styleFrom(
                            side: const BorderSide(
                              color: AppTheme.statusCompleted,
                            ),
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 6,
                            ),
                            visualDensity: VisualDensity.compact,
                          ),
                          onPressed: onGoToDownloads,
                        )
                      else if (existingDownload != null)
                        OutlinedButton.icon(
                          icon: const Icon(
                            Icons.sync_rounded,
                            size: 14,
                            color: AppTheme.primary,
                          ),
                          label: Text(
                            existingDownload!.status.label,
                            style: const TextStyle(
                              fontSize: 12,
                              color: AppTheme.primary,
                            ),
                          ),
                          style: OutlinedButton.styleFrom(
                            side: const BorderSide(color: AppTheme.primary),
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 6,
                            ),
                            visualDensity: VisualDensity.compact,
                          ),
                          onPressed: onGoToDownloads,
                        )
                      else
                        ElevatedButton.icon(
                          icon: const Icon(Icons.download_rounded, size: 14),
                          label: const Text(
                            'Télécharger',
                            style: TextStyle(fontSize: 12),
                          ),
                          style: ElevatedButton.styleFrom(
                            backgroundColor: AppTheme.primary,
                            foregroundColor: AppColors.textPrimary,
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 6,
                            ),
                            visualDensity: VisualDensity.compact,
                          ),
                          onPressed: () => _download(context),
                        ),
                      const SizedBox(width: 4),
                      IconButton(
                        icon: const Icon(Icons.hub_outlined, size: 16),
                        tooltip: 'Demander sources au serveur',
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints(
                          minWidth: 28,
                          minHeight: 28,
                        ),
                        onPressed: () => onQuerySources(file),
                      ),
                      IconButton(
                        icon: const Icon(Icons.copy_rounded, size: 16),
                        tooltip: 'Copier lien eD2K',
                        padding: EdgeInsets.zero,
                        constraints: const BoxConstraints(
                          minWidth: 28,
                          minHeight: 28,
                        ),
                        onPressed: () {
                          Clipboard.setData(
                            ClipboardData(text: file.ed2kLink),
                          );
                          ScaffoldMessenger.of(context).showSnackBar(
                            const SnackBar(
                              content: Text(
                                'Lien eD2K copié dans le presse-papier',
                              ),
                              duration: Duration(seconds: 2),
                            ),
                          );
                        },
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _download(BuildContext context) async {
    final success = await onDownload(file);
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            success
                ? 'Téléchargement lancé : ${file.fileName}'
                : (lastError() ?? 'Échec de l\'ajout au téléchargement'),
          ),
          duration: const Duration(seconds: 4),
          backgroundColor:
              success ? null : AppColors.accentOrangeShade700,
          action: success
              ? SnackBarAction(
                  label: 'Afficher',
                  onPressed: onGoToDownloads,
                )
              : null,
        ),
      );
    }
  }
}
