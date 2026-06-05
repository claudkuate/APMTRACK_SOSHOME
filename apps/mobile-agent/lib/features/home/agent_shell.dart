import 'package:flutter/material.dart';

import '../../core/auth/session_controller.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';
import '../patrouille/patrouille_page.dart';
import '../patrouille/patrouille_tracker.dart';
import '../profile/profile_page.dart';
import '../pvs/create_pv_page.dart';
import '../pvs/pv_list_page.dart';
import '../scan/scan_page.dart';
import 'home_page.dart';

class AgentShell extends StatefulWidget {
  const AgentShell({super.key, required this.controller});

  final SessionController controller;

  @override
  State<AgentShell> createState() => _AgentShellState();
}

class _AgentShellState extends State<AgentShell> {
  int _index = 0;
  late final PatrouilleTracker _tracker = PatrouilleTracker(
    controller: widget.controller,
  );

  @override
  void dispose() {
    _tracker.dispose();
    super.dispose();
  }

  Future<void> _openCreatePv() async {
    final created = await Navigator.of(context).push<bool>(
      MaterialPageRoute(
        builder: (_) => CreatePvPage(controller: widget.controller),
      ),
    );
    if (created == true && mounted) {
      setState(() => _index = 1);
    }
  }

  Future<void> _refresh() async {
    try {
      await widget.controller.refreshData();
    } catch (_) {
      if (!mounted) return;
      final message = widget.controller.message ?? 'Actualisation impossible';
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(message)));
    }
  }

  @override
  Widget build(BuildContext context) {
    final pages = [
      HomePage(
        controller: widget.controller,
        onCreatePv: _openCreatePv,
        onOpenPvs: () => setState(() => _index = 1),
        onOpenScan: () => setState(() => _index = 2),
      ),
      PvListPage(controller: widget.controller),
      ScanPage(controller: widget.controller),
      PatrouillePage(controller: widget.controller, tracker: _tracker),
      ProfilePage(controller: widget.controller),
    ];

    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('APMTRACK Agent'),
            if (widget.controller.profile != null)
              Text(
                widget.controller.profile!.commune.nom,
                style: const TextStyle(fontSize: 12, color: apmMuted),
              ),
          ],
        ),
        actions: [
          const Center(child: NetworkPill()),
          IconButton(
            tooltip: 'Actualiser',
            onPressed: widget.controller.loadingData ? null : _refresh,
            icon: const Icon(Icons.refresh),
          ),
        ],
      ),
      body: SafeArea(child: pages[_index]),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _openCreatePv,
        icon: const Icon(Icons.note_add_outlined),
        label: const Text('Nouveau PV'),
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _index,
        onDestinationSelected: (value) => setState(() => _index = value),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.home_outlined),
            selectedIcon: Icon(Icons.home),
            label: 'Accueil',
          ),
          NavigationDestination(
            icon: Icon(Icons.description_outlined),
            selectedIcon: Icon(Icons.description),
            label: 'PV',
          ),
          NavigationDestination(
            icon: Icon(Icons.qr_code_scanner_outlined),
            selectedIcon: Icon(Icons.qr_code_scanner),
            label: 'Scan',
          ),
          NavigationDestination(
            icon: Icon(Icons.shield_outlined),
            selectedIcon: Icon(Icons.shield),
            label: 'Patrouille',
          ),
          NavigationDestination(
            icon: Icon(Icons.person_outline),
            selectedIcon: Icon(Icons.person),
            label: 'Profil',
          ),
        ],
      ),
    );
  }
}
