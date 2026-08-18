import 'package:flutter/material.dart';

import '../models.dart';
import '../theme.dart';

class AgentAvatar extends StatelessWidget {
  const AgentAvatar({
    super.key,
    required this.agent,
    required this.imageUrl,
    required this.headers,
    this.onForbidden,
    this.radius = 26,
  });

  final AgentProfile agent;
  final String? imageUrl;
  final Map<String, String> headers;
  final VoidCallback? onForbidden;
  final double radius;

  @override
  Widget build(BuildContext context) {
    final size = radius * 2;
    final initials = agentInitials(agent.fullName);
    final resolvedImageUrl = imageUrl?.trim();
    final canLoadPhoto =
        agent.photoUrl != null &&
        resolvedImageUrl != null &&
        resolvedImageUrl.isNotEmpty;

    return ClipOval(
      child: SizedBox.square(
        dimension: size,
        child: DecoratedBox(
          decoration: BoxDecoration(color: apmGreen.withValues(alpha: 0.12)),
          child: canLoadPhoto
              ? Image.network(
                  resolvedImageUrl,
                  key: Key('agent-avatar-image-${agent.id}'),
                  headers: headers,
                  fit: BoxFit.cover,
                  width: size,
                  height: size,
                  semanticLabel: 'Photo de profil',
                  loadingBuilder: (context, child, progress) {
                    if (progress == null) {
                      return child;
                    }
                    return Center(
                      child: SizedBox.square(
                        dimension: radius * 0.7,
                        child: const CircularProgressIndicator(strokeWidth: 2),
                      ),
                    );
                  },
                  errorBuilder: (_, error, _) {
                    if (error is NetworkImageLoadException &&
                        error.statusCode == 403) {
                      onForbidden?.call();
                    }
                    return _InitialsFallback(
                      agentId: agent.id,
                      initials: initials,
                    );
                  },
                )
              : _InitialsFallback(agentId: agent.id, initials: initials),
        ),
      ),
    );
  }
}

String agentInitials(String fullName) {
  final initials = fullName
      .trim()
      .split(RegExp(r'\s+'))
      .where((part) => part.isNotEmpty)
      .take(2)
      .map((part) => part.characters.first.toUpperCase())
      .join();
  return initials.isEmpty ? 'APM' : initials;
}

class _InitialsFallback extends StatelessWidget {
  const _InitialsFallback({required this.agentId, required this.initials});

  final String agentId;
  final String initials;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Text(
        initials,
        key: Key('agent-avatar-initials-$agentId'),
        style: const TextStyle(color: apmGreen, fontWeight: FontWeight.w900),
      ),
    );
  }
}
