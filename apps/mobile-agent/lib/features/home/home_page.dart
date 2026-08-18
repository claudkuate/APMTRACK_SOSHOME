import 'package:flutter/material.dart';

import '../../core/auth/session_controller.dart';
import '../../core/offline/offline_models.dart';
import '../../core/theme.dart';
import '../../core/ui/agent_avatar.dart';
import '../../core/ui/common.dart';
import '../pvs/pv_list_item.dart';

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
    final drafts = controller.drafts;

    return RefreshIndicator(
      onRefresh: controller.refreshData,
      child: ListView(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 96),
        children: [
          if (profile != null)
            SectionPanel(
              child: Row(
                children: [
                  AgentAvatar(
                    agent: profile.agent,
                    imageUrl: profile.agent.photoUrl == null
                        ? null
                        : controller.agentPhotoContentUrl(profile.agent.id),
                    headers: controller.authHeaders,
                    onForbidden: controller.handleAuthenticatedAssetForbidden,
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
                          profile.agent.matricule,
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
                  label: const Text('Nouvelle saisie PV'),
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
                  label: 'PV officiels',
                  onTap: onOpenPvs,
                ),
              ),
            ],
          ),
          if (drafts.isNotEmpty) ...[
            const SizedBox(height: 16),
            Text(
              'Brouillons locaux a synchroniser',
              style: Theme.of(
                context,
              ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w900),
            ),
            const SizedBox(height: 8),
            SectionPanel(
              padding: EdgeInsets.zero,
              child: Column(
                children: [
                  for (final draft in drafts.take(3))
                    ListTile(
                      leading: Icon(
                        _draftIcon(draft),
                        color: _draftColor(draft),
                      ),
                      title: Text(
                        draft.interventionName ?? 'Brouillon local',
                        style: const TextStyle(fontWeight: FontWeight.w800),
                      ),
                      subtitle: Text(
                        _draftStatusText(draft),
                        style: TextStyle(color: _draftColor(draft)),
                      ),
                    ),
                ],
              ),
            ),
          ],
          const SizedBox(height: 16),
          Text(
            'Derniers PV officiels',
            style: Theme.of(
              context,
            ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w900),
          ),
          const SizedBox(height: 8),
          if (recentPvs.isEmpty)
            const EmptyState(
              title: 'Aucun PV officiel',
              message:
                  'Les saisies synchronisees et acceptees par le serveur apparaitront ici.',
              icon: Icons.description_outlined,
            )
          else
            SectionPanel(
              padding: EdgeInsets.zero,
              child: PvListItems(controller: controller, pvs: recentPvs),
            ),
        ],
      ),
    );
  }
}

String _draftStatusText(PvDraft draft) {
  if (draft.status == PvDraftStatus.failed) {
    return draft.error == null
        ? 'Echec serveur - Revoir avant nouvel essai'
        : 'Echec serveur : ${draft.error}';
  }
  if (draft.serverPvId == null) {
    return 'Non officiel - En attente de synchronisation serveur';
  }
  return "PV serveur cree - preuves en attente d'envoi";
}

IconData _draftIcon(PvDraft draft) {
  if (draft.status == PvDraftStatus.failed) {
    return Icons.error_outline;
  }
  if (draft.serverPvId == null) {
    return Icons.cloud_upload_outlined;
  }
  return Icons.photo_library_outlined;
}

Color _draftColor(PvDraft draft) =>
    draft.status == PvDraftStatus.failed ? apmRed : apmGold;

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
