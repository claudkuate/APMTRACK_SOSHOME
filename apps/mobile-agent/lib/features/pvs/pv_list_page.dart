import 'package:flutter/material.dart';

import '../../core/auth/session_controller.dart';
import '../../core/offline/offline_models.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';
import 'pv_list_item.dart';

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
                        'Mode hors-ligne : donnees serveur affichees depuis le cache.',
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
                  'PV officiels',
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
              title: 'Aucun PV officiel',
              message:
                  'Les saisies synchronisees et acceptees par le serveur apparaitront ici.',
              icon: Icons.description_outlined,
            )
          else
            SectionPanel(
              padding: EdgeInsets.zero,
              child: PvListItems(
                controller: controller,
                pvs: pvs,
                includeAmountAndDate: true,
                titleWeight: FontWeight.w900,
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
        title: Text(_deleteTitle(draft)),
        content: Text(_deleteMessage(draft)),
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
                'Brouillons locaux a synchroniser (${drafts.length})',
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
                  leading: Icon(_draftIcon(draft), color: _draftColor(draft)),
                  title: Text(
                    _draftTitle(draft),
                    style: const TextStyle(fontWeight: FontWeight.w800),
                  ),
                  subtitle: Text(
                    [
                      draft.payload.vehiclePlate,
                      draft.payload.vehicleRegistrationCardNumber == null
                          ? null
                          : 'CG ${draft.payload.vehicleRegistrationCardNumber}',
                      draft.payload.verbalizedName,
                      draft.payload.verbalizedIdentityNumber,
                      if (draft.photos.isNotEmpty)
                        '${draft.photos.length} preuve(s) en attente',
                      if (draft.amountFcfa != null)
                        'Montant indicatif: ${formatFcfa(draft.amountFcfa)}',
                      _draftStatusText(draft),
                    ].whereType<String>().join(' - '),
                    style: TextStyle(color: _draftColor(draft)),
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
                        tooltip: _deleteTooltip(draft),
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

String _draftTitle(PvDraft draft) {
  final fallback = draft.serverPvId == null
      ? 'Brouillon local'
      : 'PV serveur avec preuves en attente';
  return draft.interventionName ?? fallback;
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

String _deleteTitle(PvDraft draft) {
  if (draft.serverPvId == null) {
    return 'Supprimer le brouillon local ?';
  }
  return 'Supprimer les preuves en attente ?';
}

String _deleteMessage(PvDraft draft) {
  if (draft.serverPvId == null) {
    return 'Cette saisie non officielle sera definitivement perdue. Aucun PV serveur ne sera supprime.';
  }
  return 'Le PV serveur existe deja. Seuls le brouillon local et les preuves en attente seront supprimes.';
}

String _deleteTooltip(PvDraft draft) {
  if (draft.serverPvId == null) {
    return 'Supprimer le brouillon local';
  }
  return 'Supprimer les preuves en attente';
}
