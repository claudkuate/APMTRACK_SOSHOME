import 'package:flutter/material.dart';

import '../../core/auth/session_controller.dart';
import '../../core/offline/offline_models.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';
import 'pv_detail_page.dart';

class PvListPage extends StatelessWidget {
  const PvListPage({super.key, required this.controller});

  final SessionController controller;

  @override
  Widget build(BuildContext context) {
    final pvs = controller.pvs;
    final drafts = controller.drafts;
    return RefreshIndicator(
      onRefresh: controller.refreshData,
      child: ListView(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 96),
        children: [
          if (controller.offline)
            Padding(
              padding: const EdgeInsets.only(bottom: 12),
              child: SectionPanel(
                child: Row(
                  children: [
                    const Icon(Icons.cloud_off, size: 18, color: apmGold),
                    const SizedBox(width: 8),
                    const Expanded(
                      child: Text(
                        'Mode hors-ligne — donnees en cache.',
                        style: TextStyle(
                          color: apmGold,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          if (drafts.isNotEmpty) ...[
            _DraftsSection(controller: controller, drafts: drafts),
            const SizedBox(height: 16),
          ],
          Row(
            children: [
              Expanded(
                child: Text(
                  'Mes PV',
                  style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                    fontWeight: FontWeight.w900,
                  ),
                ),
              ),
              Text('${pvs.length}', style: const TextStyle(color: apmMuted)),
            ],
          ),
          const SizedBox(height: 12),
          if (pvs.isEmpty)
            const EmptyState(
              title: 'Aucun PV serveur',
              message: 'Cree un PV en ligne pour le retrouver ici.',
              icon: Icons.description_outlined,
            )
          else
            SectionPanel(
              padding: EdgeInsets.zero,
              child: Column(
                children: [
                  for (final pv in pvs)
                    ListTile(
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute(
                          builder: (_) =>
                              PvDetailPage(controller: controller, pv: pv),
                        ),
                      ),
                      title: Text(
                        pv.pvNumber,
                        style: const TextStyle(fontWeight: FontWeight.w900),
                      ),
                      subtitle: Text(
                        [
                          pv.subjectLabel,
                          pv.infractionsLabel,
                          pv.vehiclePlate,
                          pv.verbalizedName,
                          formatFcfa(pv.amountInitialFcfa),
                          formatShortDate(pv.createdAt),
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

class _DraftsSection extends StatelessWidget {
  const _DraftsSection({required this.controller, required this.drafts});

  final SessionController controller;
  final List<PvDraft> drafts;

  Future<void> _confirmDelete(BuildContext context, PvDraft draft) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Supprimer le brouillon ?'),
        content: const Text('Ce PV non synchronise sera definitivement perdu.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Annuler'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Supprimer'),
          ),
        ],
      ),
    );
    if (confirmed == true) {
      await controller.deleteDraft(draft.localId);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                'Brouillons hors-ligne (${drafts.length})',
                style: Theme.of(
                  context,
                ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w900),
              ),
            ),
            if (controller.hasPendingDrafts)
              TextButton.icon(
                onPressed: controller.syncing
                    ? null
                    : () => controller.syncDrafts(),
                icon: controller.syncing
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.sync),
                label: const Text('Synchroniser'),
              ),
          ],
        ),
        const SizedBox(height: 8),
        SectionPanel(
          padding: EdgeInsets.zero,
          child: Column(
            children: [
              for (final draft in drafts)
                ListTile(
                  leading: Icon(
                    draft.status == PvDraftStatus.failed
                        ? Icons.error_outline
                        : Icons.cloud_upload_outlined,
                    color: draft.status == PvDraftStatus.failed
                        ? apmRed
                        : apmGold,
                  ),
                  title: Text(
                    draft.interventionName ?? 'PV brouillon',
                    style: const TextStyle(fontWeight: FontWeight.w800),
                  ),
                  subtitle: Text(
                    [
                      draft.payload.vehiclePlate,
                      draft.payload.verbalizedName,
                      if (draft.photos.isNotEmpty)
                        '${draft.photos.length} preuve(s)',
                      if (draft.amountFcfa != null)
                        formatFcfa(draft.amountFcfa),
                      if (draft.status == PvDraftStatus.failed &&
                          draft.error != null)
                        'Echec: ${draft.error}'
                      else
                        'En attente de synchronisation',
                    ].whereType<String>().join(' - '),
                    style: TextStyle(
                      color: draft.status == PvDraftStatus.failed
                          ? apmRed
                          : apmMuted,
                    ),
                  ),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (draft.status == PvDraftStatus.failed)
                        IconButton(
                          tooltip: 'Reessayer',
                          icon: const Icon(Icons.refresh, color: apmGreen),
                          onPressed: () => controller.retryDraft(draft.localId),
                        ),
                      IconButton(
                        tooltip: 'Supprimer',
                        icon: const Icon(Icons.delete_outline, color: apmRed),
                        onPressed: () => _confirmDelete(context, draft),
                      ),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}
