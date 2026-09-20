import 'package:flutter/material.dart';
import 'package:mule_client/mule_client.dart';
import 'package:ui_kit/ui_kit.dart';
import '../../../../app/app_controller.dart';
import 'network/ip_filter_tab.dart';
import 'network/rate_limits_tab.dart';
import '../../../../l10n/l10n.dart';

/// IP filter (ipfilter.dat) + token-bucket rate limiters dialog.
class IpFilterDialog extends StatefulWidget {
  final AppController viewModel;

  const IpFilterDialog({super.key, required this.viewModel});

  @override
  State<IpFilterDialog> createState() => _IpFilterDialogState();
}

class _IpFilterDialogState extends State<IpFilterDialog>
    with SingleTickerProviderStateMixin {
  late TabController _tabController;
  final TextEditingController _filePathController =
      TextEditingController();
  final TextEditingController _downloadLimitController =
      TextEditingController();
  final TextEditingController _uploadLimitController =
      TextEditingController();
  final TextEditingController _connLimitController =
      TextEditingController();

  bool _isLoading = false;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
    _loadData();
  }

  @override
  void dispose() {
    _tabController.dispose();
    _filePathController.dispose();
    _downloadLimitController.dispose();
    _uploadLimitController.dispose();
    _connLimitController.dispose();
    super.dispose();
  }

  Future<void> _loadData() async {
    setState(() => _isLoading = true);
    await widget.viewModel.fetchFilterStatus();
    await widget.viewModel.fetchRateLimits();

    final limits = widget.viewModel.rateLimits;
    if (limits != null) {
      _downloadLimitController.text =
          (limits.downloadBytesPerSec ~/ 1024).toString();
      _uploadLimitController.text =
          (limits.uploadBytesPerSec ~/ 1024).toString();
      _connLimitController.text =
          limits.connectionsPerMinute.toString();
    }

    if (mounted) {
      setState(() => _isLoading = false);
    }
  }

  Future<void> _toggleFilter(bool enabled) async {
    setState(() => _isLoading = true);
    await widget.viewModel.setFilterEnabled(enabled);
    if (mounted) setState(() => _isLoading = false);
  }

  Future<void> _importFilter(String path) async {
    setState(() => _isLoading = true);
    final success = await widget.viewModel.importFilterFile(path);
    if (mounted) {
      setState(() => _isLoading = false);
      if (success) {
        _filePathController.clear();
      }
    }
  }

  Future<void> _clearFilter() async {
    setState(() => _isLoading = true);
    await widget.viewModel.clearFilter();
    if (mounted) setState(() => _isLoading = false);
  }

  Future<void> _saveLimits() async {
    final dlKb =
        int.tryParse(_downloadLimitController.text.trim()) ?? 0;
    final ulKb =
        int.tryParse(_uploadLimitController.text.trim()) ?? 0;
    final conn =
        int.tryParse(_connLimitController.text.trim()) ?? 120;

    final limits = RateLimitsModel(
      downloadBytesPerSec: dlKb * 1024,
      uploadBytesPerSec: ulKb * 1024,
      connectionsPerMinute: conn,
      serverRequestsPerSec: 10,
      kadPacketsPerSec: 50,
    );

    setState(() => _isLoading = true);
    await widget.viewModel.setRateLimits(limits);
    if (mounted) setState(() => _isLoading = false);
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      backgroundColor: AppTheme.darkSurface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
      ),
      child: ConstrainedBox(
        constraints: const BoxConstraints(
          maxWidth: 720,
          maxHeight: 620,
        ),
        child: Padding(
          padding: const EdgeInsets.all(22),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Container(
                    padding: const EdgeInsets.all(8),
                    decoration: BoxDecoration(
                      color: AppColors.warning.withValues(alpha: 0.15),
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: const Icon(
                      Icons.security,
                      color: AppColors.warning,
                      size: 24,
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(context.l10n.ipFilterRateLimiters,
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.bold,
                            color: AppColors.textPrimary,
                          ),
                        ),
                        SizedBox(height: 2),
                        Text(context.l10n.localDefensiveIpFilterIpfilter,
                          style: TextStyle(
                            fontSize: 12,
                            color: AppColors.textSoft,
                          ),
                        ),
                      ],
                    ),
                  ),
                  IconButton(
                    onPressed: () => Navigator.of(context).pop(),
                    icon: const Icon(
                      Icons.close,
                      color: AppColors.textSecondary,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              TabBar(
                controller: _tabController,
                indicatorColor: AppColors.warning,
                labelColor: AppColors.warning,
                unselectedLabelColor: AppColors.textSoft,
                tabs: const [
                  Tab(
                    icon: Icon(Icons.filter_alt, size: 18),
                    text: 'IP Filter',
                  ),
                  Tab(
                    icon: Icon(Icons.speed, size: 18),
                    text: 'Rate Limits',
                  ),
                ],
              ),
              const SizedBox(height: 16),
              Expanded(
                child: _isLoading
                    ? const Center(
                        child: CircularProgressIndicator(),
                      )
                    : TabBarView(
                        controller: _tabController,
                        children: [
                          IpFilterTab(
                            status: widget.viewModel.filterStatus,
                            filePathController: _filePathController,
                            onToggle: _toggleFilter,
                            onImport: _importFilter,
                            onClear: _clearFilter,
                          ),
                          RateLimitsTab(
                            downloadLimitController:
                                _downloadLimitController,
                            uploadLimitController:
                                _uploadLimitController,
                            connLimitController:
                                _connLimitController,
                            onSave: _saveLimits,
                          ),
                        ],
                      ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
