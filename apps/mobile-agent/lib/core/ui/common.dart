import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';

import '../theme.dart';

final _fcfaFormatter = NumberFormat.decimalPattern('fr_FR');
final _dateFormatter = DateFormat('dd MMM yyyy', 'fr_FR');

String formatFcfa(int? value) {
  if (value == null) return 'Non payant';
  return '${_fcfaFormatter.format(value)} FCFA';
}

String formatShortDate(DateTime? value) {
  if (value == null) return '-';
  return _dateFormatter.format(value.toLocal());
}

String statusLabel(String value) {
  return value
      .toLowerCase()
      .split('_')
      .map(
        (part) => part.isEmpty
            ? part
            : '${part[0].toUpperCase()}${part.substring(1)}',
      )
      .join(' ');
}

Color statusColor(String status) {
  if (['ACTIF', 'PAYE', 'TRAITE', 'NON_PAYANT', 'CLOTUREE'].contains(status)) {
    return apmGreen;
  }
  if ([
    'SUSPENDU',
    'ANNULE',
    'REJETE',
    'EN_RETARD',
    'RETRAITE',
  ].contains(status)) {
    return apmRed;
  }
  return apmGold;
}

class SectionPanel extends StatelessWidget {
  const SectionPanel({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.all(16),
  });

  final Widget child;
  final EdgeInsetsGeometry padding;

  @override
  Widget build(BuildContext context) {
    // `Material` (et non `DecoratedBox`) afin que les `ListTile` enfants aient
    // une surface où peindre encre/`tileColor` : un fond opaque peint par-dessus
    // le Material ancêtre les rendrait invisibles.
    return Material(
      color: apmPanel,
      shape: RoundedRectangleBorder(
        side: const BorderSide(color: apmBorder),
        borderRadius: BorderRadius.circular(8),
      ),
      clipBehavior: Clip.antiAlias,
      child: Padding(padding: padding, child: child),
    );
  }
}

class StatusPill extends StatelessWidget {
  const StatusPill({super.key, required this.status});

  final String status;

  @override
  Widget build(BuildContext context) {
    final color = statusColor(status);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.11),
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: color.withValues(alpha: 0.3)),
      ),
      child: Text(
        statusLabel(status),
        style: TextStyle(
          color: color,
          fontWeight: FontWeight.w800,
          fontSize: 12,
        ),
      ),
    );
  }
}

class NetworkPill extends StatelessWidget {
  const NetworkPill({super.key});

  @override
  Widget build(BuildContext context) {
    return StreamBuilder<List<ConnectivityResult>>(
      stream: Connectivity().onConnectivityChanged,
      builder: (context, snapshot) {
        final results = snapshot.data;
        final online =
            results == null ||
            results.any((item) => item != ConnectivityResult.none);
        return Container(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
          decoration: BoxDecoration(
            color: online
                ? apmGreen.withValues(alpha: 0.1)
                : apmRed.withValues(alpha: 0.1),
            borderRadius: BorderRadius.circular(999),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(Icons.circle, size: 8, color: online ? apmGreen : apmRed),
              const SizedBox(width: 6),
              Text(
                online ? 'En ligne' : 'Hors ligne',
                style: TextStyle(
                  color: online ? apmGreen : apmRed,
                  fontWeight: FontWeight.w800,
                  fontSize: 12,
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class EmptyState extends StatelessWidget {
  const EmptyState({
    super.key,
    required this.title,
    required this.message,
    this.icon = Icons.inbox_outlined,
  });

  final String title;
  final String message;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    return SectionPanel(
      child: Column(
        children: [
          Icon(icon, color: apmMuted, size: 32),
          const SizedBox(height: 10),
          Text(title, style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 6),
          Text(
            message,
            textAlign: TextAlign.center,
            style: const TextStyle(color: apmMuted),
          ),
        ],
      ),
    );
  }
}
