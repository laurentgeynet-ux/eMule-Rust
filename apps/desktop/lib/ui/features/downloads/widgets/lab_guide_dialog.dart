import 'package:flutter/material.dart';

import 'package:ui_kit/ui_kit.dart';
import 'package:mule_client/mule_client.dart';
import '../view_models/downloads_view_model.dart';
import 'lab/lab_cleanup_dialog.dart';
import 'lab/lab_directory_overview.dart';
import 'lab/lab_error_banner.dart';
import 'lab/lab_step_cleanup.dart';
import 'lab/lab_step_connect.dart';
import 'lab/lab_step_fixture.dart';
import 'lab/lab_step_link.dart';
import 'lab/lab_step_peer.dart';

/// Guided end-to-end loopback test flow: fixture generation, ed2k link,
/// loopback peer, download connect, and test-data cleanup.
class LabGuideDialog extends StatefulWidget {
  final DownloadsViewModel viewModel;

  const LabGuideDialog({super.key, required this.viewModel});

  @override
  State<LabGuideDialog> createState() => _LabGuideDialogState();
}

class _LabGuideDialogState extends State<LabGuideDialog> {
  bool _isLoading = true;
  String? _errorMessage;
  LabStatus? _labStatus;

  // Selected fixture type for generation
  String _selectedFixtureType = 'sample_1mb';
  LabFixtureInfo? _lastCreatedFixture;
  bool _isCreatingFixture = false;
  bool _isAddingToDownloads = false;
  bool _addedToDownloadsSuccess = false;

  // Peer management
  bool _isManagingPeer = false;

  // Cleanup
  bool _isCleaningUp = false;

  // Connect download
  String? _selectedDownloadIdForConnect;
  bool _isConnectingDownload = false;

  @override
  void initState() {
    super.initState();
    _loadStatus();
  }

  Future<void> _loadStatus() async {
    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      final status = await widget.viewModel.getLabStatus();
      if (mounted) {
        setState(() {
          _labStatus = status;
          if (status.fixtures.isNotEmpty && _lastCreatedFixture == null) {
            _lastCreatedFixture = status.fixtures.last;
          }
          _isLoading = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = e.toString();
          _isLoading = false;
        });
      }
    }
  }

  Future<void> _createFixture() async {
    setState(() {
      _isCreatingFixture = true;
      _errorMessage = null;
    });

    try {
      final fixture =
          await widget.viewModel.createLabFixture(_selectedFixtureType);
      final updatedStatus = await widget.viewModel.getLabStatus();
      if (mounted) {
        setState(() {
          _lastCreatedFixture = fixture;
          _labStatus = updatedStatus;
          _addedToDownloadsSuccess = false;
          _isCreatingFixture = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Failed to create fixture: $e';
          _isCreatingFixture = false;
        });
      }
    }
  }

  Future<void> _addToDownloads() async {
    final fixture = _lastCreatedFixture;
    if (fixture == null) return;

    setState(() {
      _isAddingToDownloads = true;
      _errorMessage = null;
    });

    try {
      final success = await widget.viewModel.addEd2kLink(fixture.ed2kLink);
      if (mounted) {
        setState(() {
          _addedToDownloadsSuccess = success;
          _isAddingToDownloads = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Failed to add download: $e';
          _isAddingToDownloads = false;
        });
      }
    }
  }

  Future<void> _togglePeer() async {
    final isRunning = _labStatus?.activeLoopbackPeer != null;
    setState(() {
      _isManagingPeer = true;
      _errorMessage = null;
    });

    try {
      if (isRunning) {
        await widget.viewModel.stopLabPeer();
      } else {
        await widget.viewModel.startLabPeer();
      }
      final updatedStatus = await widget.viewModel.getLabStatus();
      if (mounted) {
        setState(() {
          _labStatus = updatedStatus;
          _isManagingPeer = false;
        });
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Peer operation failed: $e';
          _isManagingPeer = false;
        });
      }
    }
  }

  Future<void> _connectDownload() async {
    final peerAddr = _labStatus?.activeLoopbackPeer;
    final downloadId = _selectedDownloadIdForConnect ??
        (widget.viewModel.filteredDownloads.isNotEmpty
            ? widget.viewModel.filteredDownloads.first.id
            : null);

    if (peerAddr == null || downloadId == null) return;

    setState(() {
      _isConnectingDownload = true;
      _errorMessage = null;
    });

    try {
      await widget.viewModel.connectDownloadToSource(downloadId, peerAddr);
      if (mounted) {
        setState(() {
          _isConnectingDownload = false;
        });
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content:
                Text('Connected download to loopback source ($peerAddr)'),
            duration: const Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Failed to connect download: $e';
          _isConnectingDownload = false;
        });
      }
    }
  }

  Future<void> _cleanupTestData() async {
    final fixtureCount = _labStatus?.fixtures.length ?? 0;
    final confirmed = await showLabCleanupDialog(
      context,
      fixtureCount: fixtureCount,
    );
    if (!confirmed) return;

    setState(() {
      _isCleaningUp = true;
      _errorMessage = null;
    });

    try {
      final report = await widget.viewModel.cleanupLabTestData();
      final updatedStatus = await widget.viewModel.getLabStatus();
      if (mounted) {
        setState(() {
          _labStatus = updatedStatus;
          _lastCreatedFixture = null;
          _addedToDownloadsSuccess = false;
          _isCleaningUp = false;
        });
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              'Cleanup completed: ${report.deletedFilesCount} file(s) deleted across '
              '${report.cleanedFolders.join(', ')} (${Formatters.formatBytes(report.freedBytes)} freed).',
            ),
            duration: const Duration(seconds: 4),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _errorMessage = 'Cleanup failed: $e';
          _isCleaningUp = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final workdir = widget.viewModel.daemonInfo?.workdirPath ??
        widget.viewModel.supervisor?.resolveWorkDir().directory.path ??
        'data';
    final downloadsDir = widget.viewModel.defaultDownloadDirectory.path;

    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: const BorderSide(color: AppTheme.darkBorder, width: 0.5),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 720, maxHeight: 820),
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: AppTheme.primary.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: const Icon(
                      Icons.science,
                      color: AppTheme.primary,
                      size: 24,
                    ),
                  ),
                  const SizedBox(width: 12),
                  const Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          'Guided Local Loopback Test Flow',
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                        SizedBox(height: 2),
                        Text(
                          'Full end-to-end eD2K testing on 127.0.0.1 without touching a terminal.',
                          style: TextStyle(
                            fontSize: 12,
                            color: AppColors.textSoft,
                          ),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    icon: const Icon(
                      Icons.close,
                      size: 20,
                      color: AppColors.textMuted,
                    ),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              if (_errorMessage != null)
                LabErrorBanner(message: _errorMessage!),
              Expanded(
                child: _isLoading
                    ? const Center(child: CircularProgressIndicator())
                    : SingleChildScrollView(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            LabDirectoryOverview(
                              workdir: workdir,
                              downloadsDir: downloadsDir,
                              onOpenFolder:
                                  widget.viewModel.openDownloadsFolder,
                            ),
                            LabStepFixture(
                              selectedFixtureType: _selectedFixtureType,
                              onFixtureTypeChanged: (type) => setState(
                                () => _selectedFixtureType = type,
                              ),
                              lastCreatedFixture: _lastCreatedFixture,
                              isCreating: _isCreatingFixture,
                              onCreate: _createFixture,
                            ),
                            const SizedBox(height: 16),
                            LabStepLink(
                              fixture: _lastCreatedFixture,
                              isAdding: _isAddingToDownloads,
                              addedSuccess: _addedToDownloadsSuccess,
                              onAdd: _addToDownloads,
                            ),
                            const SizedBox(height: 16),
                            LabStepPeer(
                              activePeer: _labStatus?.activeLoopbackPeer,
                              fixtureCount:
                                  _labStatus?.fixtures.length ?? 0,
                              isManaging: _isManagingPeer,
                              onToggle: _togglePeer,
                            ),
                            const SizedBox(height: 16),
                            LabStepConnect(
                              downloads:
                                  widget.viewModel.filteredDownloads,
                              selectedDownloadId:
                                  _selectedDownloadIdForConnect,
                              onDownloadChanged: (id) => setState(
                                () =>
                                    _selectedDownloadIdForConnect = id,
                              ),
                              activePeer: _labStatus?.activeLoopbackPeer,
                              fixture: _lastCreatedFixture,
                              isConnecting: _isConnectingDownload,
                              onConnect: _connectDownload,
                              downloadsDir: downloadsDir,
                              onOpenFolder:
                                  widget.viewModel.openDownloadsFolder,
                            ),
                            const SizedBox(height: 16),
                            LabStepCleanup(
                              isPortable:
                                  _labStatus?.isPortable ?? false,
                              isCleaning: _isCleaningUp,
                              onCleanup: _cleanupTestData,
                            ),
                          ],
                        ),
                      ),
              ),
              const SizedBox(height: 16),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: const Text('Close'),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
