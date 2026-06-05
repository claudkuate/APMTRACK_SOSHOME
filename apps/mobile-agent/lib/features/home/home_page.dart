import 'package:flutter/material.dart';

import '../../core/auth/session_controller.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';

class HomePage extends StatelessWidget {
  const HomePage({
    super.key,
    required this.controller,
    required this.onCreatePv,
    required this.onOpenPvs,
    required this.onOpenScan,
  });

  final SessionController controller;
  final VoidCallback onCreatePv;
  final VoidCallback onOpenPvs;
  final VoidCallback onOpenScan;

  @override
  Widget build(BuildContext context) {
    final profile = controller.profile;
    final patrouille = controller.activePatrouille.patrouille;
    final recentPvs = controller.pvs.take(3).toList();

    return RefreshIndicator(
      onRefresh: controller.refreshData,
      child: ListView(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 96),
        children: [
          if (profile != null)
            SectionPanel(
              child: Row(
                children: [
                  CircleAvatar(
                    radius: 26,
                    backgroundColor: apmGreen.withValues(alpha: 0.12),
                    child: Text(
                      profile.agent.fullName
                          .split(' ')
                          .where((part) => part.isNotEmpty)
                          .take(2)
                          .map((part) => part[0])
                          .join()
                          .toUpperCase(),
                      style: const TextStyle(
                        color: apmGreen,
                        fontWeight: FontWeight.w900,
                      ),
                    ),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          profile.agent.fullName,
                          style: Theme.of(context).textTheme.titleMedium
                              ?.copyWith(fontWeight: FontWeight.w900),
                        ),
                        Text(
                          '${profile.agent.grade} - ${profile.agent.matricule}',
                          style: const TextStyle(color: apmMuted),
                        ),
                      ],
                    ),
                  ),
                  StatusPill(status: profile.agent.status),
                ],
              ),
            ),
          const SizedBox(height: 12),
          SectionPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text(
                  'Patrouille active',
                  style: TextStyle(fontWeight: FontWeight.w900, fontSize: 16),
                ),
                const SizedBox(height: 10),
                if (patrouille == null)
                  const Text(
                    'Aucune patrouille en cours affectee a votre compte.',
                    style: TextStyle(color: apmMuted),
                  )
                else ...[
                  Text(
                    patrouille.nom,
                    style: Theme.of(context).textTheme.titleLarge?.copyWith(
                      fontWeight: FontWeight.w900,
                    ),
                  ),
                  const SizedBox(height: 4),
                  Text(
                    'Equipe: ${controller.activePatrouille.agents.length} agent(s)',
                    style: const TextStyle(color: apmMuted),
                  ),
                  const SizedBox(height: 8),
                  StatusPill(status: patrouille.status),
                ],
              ],
            ),
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: FilledButton.icon(
                  onPressed: onCreatePv,
                  icon: const Icon(Icons.note_add_outlined),
                  label: const Text('Nouveau PV'),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Expanded(
                child: _QuickAction(
                  icon: Icons.qr_code_scanner,
                  label: 'Scanner QR',
                  onTap: onOpenScan,
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: _QuickAction(
                  icon: Icons.description_outlined,
                  label: 'Mes PV',
                  onTap: onOpenPvs,
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          Text(
            'Derniers PV',
            style: Theme.of(
              context,
            ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w900),
          ),
          const SizedBox(height: 8),
          if (recentPvs.isEmpty)
            const EmptyState(
              title: 'Aucun PV emis',
              message: 'Les PV valides par le serveur apparaitront ici.',
              icon: Icons.description_outlined,
            )
          else
            SectionPanel(
              padding: EdgeInsets.zero,
              child: Column(
                children: [
                  for (final pv in recentPvs)
                    ListTile(
                      title: Text(
                        pv.pvNumber,
                        style: const TextStyle(fontWeight: FontWeight.w800),
                      ),
                      subtitle: Text(
                        [
                          pv.subjectLabel,
                          pv.infractionsLabel,
                          pv.vehicleIdentityLabel,
                          pv.verbalizedDisplayName,
                        ].whereType<String>().join(' - '),
                      ),
                      trailing: StatusPill(status: pv.status),
                    ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}

class _QuickAction extends StatelessWidget {
  const _QuickAction({
    required this.icon,
    required this.label,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(8),
      child: SectionPanel(
        child: Column(
          children: [
            Icon(icon, color: apmGreen),
            const SizedBox(height: 8),
            Text(label, style: const TextStyle(fontWeight: FontWeight.w800)),
          ],
        ),
      ),
    );
  }
}
