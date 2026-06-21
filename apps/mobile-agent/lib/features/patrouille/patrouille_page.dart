import 'package:flutter/material.dart';
import 'package:geolocator/geolocator.dart';

import '../../core/api/api_client.dart';
import '../../core/auth/session_controller.dart';
import '../../core/theme.dart';
import '../../core/ui/common.dart';
import 'patrouille_tracker.dart';

class PatrouillePage extends StatefulWidget {
  const PatrouillePage({
    super.key,
    required this.controller,
    required this.tracker,
  });

  final SessionController controller;
  final PatrouilleTracker tracker;

  @override
  State<PatrouillePage> createState() => _PatrouillePageState();
}

class _PatrouillePageState extends State<PatrouillePage> {
  bool _sending = false;
  String? _message;

  Future<void> _sendPosition() async {
    final patrouille = widget.controller.activePatrouille.patrouille;
    if (patrouille == null) {
      return;
    }
    setState(() {
      _sending = true;
      _message = null;
    });
    try {
      final serviceEnabled = await Geolocator.isLocationServiceEnabled();
      if (!serviceEnabled) {
        throw ApiException('GPS desactive sur le telephone');
      }
      var permission = await Geolocator.checkPermission();
      if (permission == LocationPermission.denied) {
        permission = await Geolocator.requestPermission();
      }
      if (permission == LocationPermission.denied ||
          permission == LocationPermission.deniedForever) {
        throw ApiException('GPS refuse');
      }
      final position = await Geolocator.getCurrentPosition(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          timeLimit: Duration(seconds: 12),
        ),
      );
      await widget.controller.recordPatrouillePosition(
        patrouilleId: patrouille.id,
        latitude: position.latitude,
        longitude: position.longitude,
        accuracyM: position.accuracy,
      );
      setState(() => _message = 'Position envoyee au serveur');
    } on ApiException catch (error) {
      setState(() => _message = error.message);
    } catch (_) {
      setState(() => _message = 'Envoi position impossible');
    } finally {
      if (mounted) {
        setState(() => _sending = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final patrouille = widget.controller.activePatrouille.patrouille;
    final agents = widget.controller.activePatrouille.agents;
    // Stop tracking if the active patrouille disappeared (ended/unassigned).
    if (patrouille == null && widget.tracker.isTracking) {
      WidgetsBinding.instance.addPostFrameCallback(
        (_) => widget.tracker.stop(),
      );
    }
    return RefreshIndicator(
      onRefresh: widget.controller.refreshData,
      child: ListView(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 96),
        children: [
          Text(
            'Patrouille',
            style: Theme.of(
              context,
            ).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w900),
          ),
          const SizedBox(height: 12),
          if (patrouille == null)
            const EmptyState(
              title: 'Aucune patrouille active',
              message:
                  'Une patrouille en cours doit etre affectee a votre agent.',
              icon: Icons.shield_outlined,
            )
          else ...[
            SectionPanel(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          patrouille.nom,
                          style: Theme.of(context).textTheme.titleLarge
                              ?.copyWith(fontWeight: FontWeight.w900),
                        ),
                      ),
                      StatusPill(status: patrouille.status),
                    ],
                  ),
                  const SizedBox(height: 8),
                  Text(
                    patrouille.description ?? 'Mission terrain en cours',
                    style: const TextStyle(color: apmMuted),
                  ),
                  const SizedBox(height: 12),
                  Text('Debut: ${formatShortDate(patrouille.dateDebut)}'),
                  if (patrouille.dateDebutPrevue != null ||
                      patrouille.dateFinPrevue != null) ...[
                    const SizedBox(height: 8),
                    Text(
                      'Prevu: ${formatShortDate(patrouille.dateDebutPrevue)} '
                      '→ ${formatShortDate(patrouille.dateFinPrevue)}',
                      style: const TextStyle(color: apmMuted),
                    ),
                  ],
                  const SizedBox(height: 12),
                  FilledButton.icon(
                    onPressed: _sending ? null : _sendPosition,
                    icon: _sending
                        ? const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.my_location),
                    label: Text(_sending ? 'Envoi...' : 'Envoyer ma position'),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 12),
            _TrackingCard(tracker: widget.tracker, patrouilleId: patrouille.id),
            const SizedBox(height: 12),
            Text(
              'Equipe',
              style: Theme.of(
                context,
              ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w900),
            ),
            const SizedBox(height: 8),
            SectionPanel(
              padding: EdgeInsets.zero,
              child: Column(
                children: [
                  for (final agent in agents)
                    ListTile(
                      leading: const Icon(Icons.person_outline),
                      title: Text(
                        agent.fullName,
                        style: const TextStyle(fontWeight: FontWeight.w800),
                      ),
                      subtitle: Text(agent.matricule),
                      trailing: Text(agent.rolePatrouille),
                    ),
                ],
              ),
            ),
          ],
          const SizedBox(height: 20),
          Text(
            'Mes patrouilles',
            style: Theme.of(
              context,
            ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w900),
          ),
          const SizedBox(height: 8),
          if (widget.controller.patrouilles.isEmpty)
            const EmptyState(
              title: 'Aucune patrouille affectee',
              message:
                  'Les patrouilles en cours ou planifiees pour votre agent '
                  'apparaitront ici.',
              icon: Icons.event_note_outlined,
            )
          else
            SectionPanel(
              padding: EdgeInsets.zero,
              child: Column(
                children: [
                  for (final item in widget.controller.patrouilles)
                    ListTile(
                      leading: const Icon(Icons.shield_outlined),
                      title: Text(
                        item.nom,
                        style: const TextStyle(fontWeight: FontWeight.w800),
                      ),
                      subtitle:
                          (item.dateDebutPrevue != null ||
                              item.dateFinPrevue != null)
                          ? Text(
                              'Prevu: ${formatShortDate(item.dateDebutPrevue)} '
                              '→ ${formatShortDate(item.dateFinPrevue)}',
                              style: const TextStyle(color: apmMuted),
                            )
                          : null,
                      trailing: StatusPill(status: item.status),
                    ),
                ],
              ),
            ),
          if (_message != null) ...[
            const SizedBox(height: 12),
            Text(
              _message!,
              style: TextStyle(
                color: _message!.contains('envoyee') ? apmGreen : apmRed,
                fontWeight: FontWeight.w800,
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _TrackingCard extends StatelessWidget {
  const _TrackingCard({required this.tracker, required this.patrouilleId});

  final PatrouilleTracker tracker;
  final String patrouilleId;

  String _formatTime(DateTime value) {
    final local = value.toLocal();
    final h = local.hour.toString().padLeft(2, '0');
    final m = local.minute.toString().padLeft(2, '0');
    final s = local.second.toString().padLeft(2, '0');
    return '$h:$m:$s';
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: tracker,
      builder: (context, _) {
        final pending = tracker.pendingCount;
        final lastSentAt = tracker.lastSentAt;
        return SectionPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                value: tracker.isTracking,
                onChanged: (value) {
                  if (value) {
                    tracker.start(patrouilleId);
                  } else {
                    tracker.stop();
                  }
                },
                title: const Text(
                  'Suivi automatique',
                  style: TextStyle(fontWeight: FontWeight.w900),
                ),
                subtitle: Text(
                  tracker.isTracking
                      ? 'Position envoyee automatiquement pendant la patrouille.'
                      : 'Active le suivi GPS continu (avant-plan).',
                  style: const TextStyle(color: apmMuted),
                ),
              ),
              if (tracker.isTracking) ...[
                const SizedBox(height: 4),
                Row(
                  children: [
                    const Icon(Icons.my_location, size: 16, color: apmGreen),
                    const SizedBox(width: 8),
                    Text(
                      lastSentAt == null
                          ? 'En attente du premier point...'
                          : 'Dernier envoi: ${_formatTime(lastSentAt)}',
                      style: const TextStyle(color: apmMuted),
                    ),
                  ],
                ),
                if (pending > 0) ...[
                  const SizedBox(height: 6),
                  Row(
                    children: [
                      const Icon(Icons.cloud_off, size: 16, color: apmGold),
                      const SizedBox(width: 8),
                      Text(
                        '$pending point(s) en attente de reseau',
                        style: const TextStyle(
                          color: apmGold,
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ),
                ],
              ],
              if (tracker.error != null) ...[
                const SizedBox(height: 6),
                Text(
                  tracker.error!,
                  style: const TextStyle(
                    color: apmRed,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ],
            ],
          ),
        );
      },
    );
  }
}
