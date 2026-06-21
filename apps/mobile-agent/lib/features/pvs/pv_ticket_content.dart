import '../../core/config.dart';
import '../../core/models.dart';
import '../../core/ui/common.dart';

class PvTicketContent {
  const PvTicketContent({
    required this.headerLines,
    required this.detailLines,
    required this.infractionLines,
    required this.totalLine,
    required this.verificationUrl,
  });

  final List<String> headerLines;
  final List<String> detailLines;
  final List<String> infractionLines;
  final String totalLine;
  final String verificationUrl;

  List<String> get allTextLines => [
    ...headerLines,
    ...detailLines,
    'Infractions',
    ...infractionLines,
    totalLine,
    'Verification',
    verificationUrl,
  ];
}

class PvTicketContentBuilder {
  const PvTicketContentBuilder();

  PvTicketContent build({
    required Pv pv,
    MobileProfile? profile,
    String? verificationBaseUrl,
  }) {
    final detailLines = <String>[];
    final infractionLines = <String>[];

    void field(String label, String? value) {
      final clean = value?.trim();
      if (clean == null || clean.isEmpty) return;
      detailLines.add('$label: $clean');
    }

    field('PV', pv.pvNumber);
    field('Date', formatShortDate(pv.createdAt));
    field('Statut', statusLabel(pv.status));
    if (profile != null) {
      field('Agent', profile.agent.fullName);
      field('Matricule', profile.agent.matricule);
    }
    field('Type', pv.subjectLabel);
    field('Contrevenant', pv.verbalizedDisplayName);
    field('Piece', pv.verbalizedIdentityLabel);
    field('Telephone', pv.verbalizedPhone);
    field('Adresse', pv.verbalizedAddress);
    field('Vehicule', pv.vehicleIdentityLabel);
    field('Carte grise', pv.vehicleRegistrationCardNumber);
    field('Marque', pv.vehicleMake);
    field('Modele', pv.vehicleModel);
    field('Couleur', pv.vehicleColor);
    field('Lieu', pv.locationDescription);

    if (pv.interventions.isEmpty) {
      infractionLines.add('- ${pv.infractionsLabel}');
    } else {
      for (final item in pv.interventions) {
        infractionLines.add('- ${item.nom}');
        if (item.montantFcfa != null || item.sujetPaiement) {
          infractionLines.add('  ${formatFcfa(item.montantFcfa)}');
        }
      }
    }

    return PvTicketContent(
      headerLines: ['APMTRACK', if (profile != null) profile.commune.nom],
      detailLines: detailLines,
      infractionLines: infractionLines,
      totalLine: 'Montant total: ${formatFcfa(pv.amountInitialFcfa)}',
      verificationUrl: buildVerificationUrl(pv, baseUrl: verificationBaseUrl),
    );
  }

  String buildVerificationUrl(Pv pv, {String? baseUrl}) {
    final root = (baseUrl ?? apiBaseUrl).replaceFirst(RegExp(r'/$'), '');
    return '$root/api/v1/public/pvs/${Uri.encodeComponent(pv.pvNumber)}';
  }
}
