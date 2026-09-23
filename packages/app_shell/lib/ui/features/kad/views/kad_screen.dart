import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:ui_kit/ui_kit.dart';

import '../../../../app/providers.dart';
import '../widgets/kad_bootstrap_dialog.dart';
import '../widgets/kad_bootstrap_url_dialog.dart';
import '../widgets/kad_clear_contacts_dialog.dart';
import '../widgets/kad_contacts_table.dart';
import '../widgets/kad_empty_state.dart';
import '../widgets/kad_filter_bar.dart';
import '../widgets/kad_import_nodes_dat_dialog.dart';
import '../widgets/kad_safety_banner.dart';
import '../widgets/kad_status_card.dart';
import '../widgets/kad_toolbar.dart';

/// Kad DHT view: composition only — toolbar, safety banner, status card,
/// contacts table. State lives in [kadControllerProvider].
class KadView extends ConsumerStatefulWidget {
  const KadView({super.key});

  @override
  ConsumerState<KadView> createState() => _KadViewState();
}

class _KadViewState extends ConsumerState<KadView> {
  final _searchController = TextEditingController();
  String _filter = '';

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final ctl = ref.read(kadControllerProvider);
      ctl.fetchStatus();
      ctl.fetchContacts();
    });
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final ctl = ref.watch(kadControllerProvider);
    final status = ctl.status;
    final contacts = ctl.contacts.where((c) {
      if (_filter.isEmpty) return true;
      return c.kadId.toLowerCase().contains(_filter.toLowerCase()) ||
          c.ip.contains(_filter) ||
          c.udpPort.toString().contains(_filter);
    }).toList();

    return Column(
      children: [
        KadToolbar(
          controller: ctl,
          onBootstrap: () => showKadBootstrapDialog(context, ctl),
          onImportNodesDat: () => showKadImportNodesDatDialog(context, ctl),
          onBootstrapFromUrl: () => showKadBootstrapFromUrlDialog(context, ctl),
        ),
        KadSafetyBanner(status: status),
        KadStatusCard(controller: ctl, status: status),
        const SizedBox(height: 12),
        KadFilterBar(
          onQueryChanged: (v) => setState(() => _filter = v),
          onClearContacts: () => showKadClearContactsDialog(context, ctl),
        ),
        Expanded(
          child: Container(
            margin: const EdgeInsets.fromLTRB(20, 0, 20, 20),
            decoration: BoxDecoration(
              color: AppTheme.darkCard,
              borderRadius: BorderRadius.circular(8),
              border: Border.all(color: AppTheme.darkBorder, width: 0.5),
            ),
            child: contacts.isEmpty
                ? KadEmptyState(
                    onImport: () => showKadImportNodesDatDialog(context, ctl),
                  )
                : KadContactsTable(controller: ctl, contacts: contacts),
          ),
        ),
      ],
    );
  }
}
