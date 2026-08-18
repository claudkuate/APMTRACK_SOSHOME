import 'package:flutter/material.dart';

import '../../core/auth/session_controller.dart';
import '../../core/config.dart';
import '../../core/theme.dart';
import '../../core/ui/agent_avatar.dart';
import '../../core/ui/common.dart';

class ProfilePage extends StatelessWidget {
  const ProfilePage({super.key, required this.controller});

  final SessionController controller;

  @override
  Widget build(BuildContext context) {
    final profile = controller.profile;
    if (profile == null) {
      // Un profil absent n'est pas forcement un chargement en cours : c'est aussi
      // le cas quand /mobile/me a echoue (API injoignable, agent non rattache a
      // une commune, abonnement suspendu). Un spinner nu laisse alors l'agent
      // devant un ecran fige sans diagnostic — on affiche l'erreur et on offre
      // une relance.
      if (controller.loadingData) {
        return const Center(child: CircularProgressIndicator());
      }
      return _ProfileUnavailable(controller: controller);
    }

    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 96),
      children: [
        Text(
          'Profil',
          style: Theme.of(
            context,
          ).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w900),
        ),
        const SizedBox(height: 12),
        SectionPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  AgentAvatar(
                    agent: profile.agent,
                    imageUrl: profile.agent.photoUrl == null
                        ? null
                        : controller.agentPhotoContentUrl(profile.agent.id),
                    headers: controller.authHeaders,
                    onForbidden: controller.handleAuthenticatedAssetForbidden,
                    radius: 28,
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          profile.agent.fullName,
                          style: Theme.of(context).textTheme.titleLarge
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
              const SizedBox(height: 16),
              _Row(label: 'Matricule', value: profile.agent.matricule),
              _Row(label: 'Commune', value: profile.commune.nom),
              _Row(label: 'Region', value: profile.commune.region),
              _Row(label: 'Email', value: profile.user.email),
            ],
          ),
        ),
        const SizedBox(height: 12),
        const SectionPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Mode terrain',
                style: TextStyle(fontWeight: FontWeight.w900),
              ),
              SizedBox(height: 8),
              Text(
                'G-APM mobile est online-first. Les brouillons locaux peuvent etre saisis sans reseau ; seuls les PV synchronises par le serveur sont officiels, avec numero, montant et QR.',
                style: TextStyle(color: apmMuted),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        SectionPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Diagnostic',
                style: TextStyle(fontWeight: FontWeight.w900),
              ),
              const SizedBox(height: 8),
              Text('API: $apiBaseUrl', style: const TextStyle(color: apmMuted)),
              Text(
                'Env: $appEnvironment',
                style: const TextStyle(color: apmMuted),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        OutlinedButton.icon(
          onPressed: () => controller.signOut(),
          icon: const Icon(Icons.logout),
          label: const Text('Deconnexion'),
        ),
      ],
    );
  }
}

/// Ecran de repli quand `/mobile/me` n'a rien rendu : il nomme la cause et donne
/// l'adresse d'API reellement interrogee, seul moyen pour un agent de terrain de
/// signaler un APK construit sur la mauvaise URL.
class _ProfileUnavailable extends StatelessWidget {
  const _ProfileUnavailable({required this.controller});

  final SessionController controller;

  @override
  Widget build(BuildContext context) {
    final reason = controller.message ?? 'Profil indisponible';
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 12, 16, 96),
      children: [
        Text(
          'Profil',
          style: Theme.of(
            context,
          ).textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.w900),
        ),
        const SizedBox(height: 12),
        SectionPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Row(
                children: [
                  Icon(Icons.error_outline, color: apmRed),
                  SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Profil non charge',
                      style: TextStyle(fontWeight: FontWeight.w900),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Text(reason, style: const TextStyle(color: apmMuted)),
              const SizedBox(height: 12),
              FilledButton.icon(
                onPressed: () => controller.refreshData(),
                icon: const Icon(Icons.refresh),
                label: const Text('Reessayer'),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        SectionPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Diagnostic',
                style: TextStyle(fontWeight: FontWeight.w900),
              ),
              const SizedBox(height: 8),
              Text('API: $apiBaseUrl', style: const TextStyle(color: apmMuted)),
              Text(
                'Env: $appEnvironment',
                style: const TextStyle(color: apmMuted),
              ),
              const SizedBox(height: 8),
              const Text(
                'Communiquez ces deux lignes au support avec le matricule concerne.',
                style: TextStyle(color: apmMuted, fontSize: 12),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        OutlinedButton.icon(
          onPressed: () => controller.signOut(),
          icon: const Icon(Icons.logout),
          label: const Text('Deconnexion'),
        ),
      ],
    );
  }
}

class _Row extends StatelessWidget {
  const _Row({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 94,
            child: Text(
              label,
              style: const TextStyle(
                color: apmMuted,
                fontWeight: FontWeight.w700,
              ),
            ),
          ),
          Expanded(child: Text(value)),
        ],
      ),
    );
  }
}
