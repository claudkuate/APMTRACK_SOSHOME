import 'package:apmtrack_agent/core/models.dart';
import 'package:apmtrack_agent/features/pvs/pv_ticket_content.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:intl/date_symbol_data_local.dart';

void main() {
  setUpAll(() async {
    await initializeDateFormatting('fr_FR');
  });

  test('builds a complete 58mm PV ticket content', () {
    const builder = PvTicketContentBuilder();
    final content = builder.build(
      pv: _pv,
      profile: _profile,
      verificationBaseUrl: 'https://apmtrack.test',
    );
    final printable = content.allTextLines.join('\n');

    expect(printable, contains('G-APM'));
    expect(printable, contains('Commune d arrondissement de Yaounde Ier'));
    expect(printable, contains('PV-YDE1-2026-000004'));
    expect(printable, contains('Marie Ngono'));
    expect(printable, contains('CE42625YD'));
    expect(printable, contains('Affichage publicitaire'));
    expect(printable, contains('Defaut de patente'));
    expect(printable, contains('55\u202f000 FCFA'));
    expect(
      builder.buildVerificationUrl(_pv, baseUrl: 'https://apmtrack.test/'),
      'https://apmtrack.test/api/v1/public/pvs/PV-YDE1-2026-000004',
    );
    expect(
      printable,
      contains('https://apmtrack.test/api/v1/public/pvs/PV-YDE1-2026-000004'),
    );
  });
}

const _user = UserAccount(
  id: 'user-1',
  email: 'agent@test.local',
  fullName: 'Agent Test',
  communeId: 'commune-1',
  roles: ['APM_AGENT'],
  active: true,
);

const _profile = MobileProfile(
  user: _user,
  commune: Commune(
    id: 'commune-1',
    code: 'YDE1',
    nom: 'Commune d arrondissement de Yaounde Ier',
    region: 'Centre',
    departement: 'Mfoundi',
  ),
  agent: AgentProfile(
    id: 'agent-1',
    matricule: 'APM-YDE1-001',
    fullName: 'Marie Ngono',
    communeId: 'commune-1',
    status: 'ACTIF',
  ),
);

final _pv = Pv(
  id: 'pv-4',
  pvNumber: 'PV-YDE1-2026-000004',
  interventionId: 'intervention-1',
  interventions: const [
    PvIntervention(
      interventionId: 'intervention-1',
      orderIndex: 0,
      nom: 'Affichage publicitaire',
      sujetPaiement: true,
      montantFcfa: 30000,
    ),
    PvIntervention(
      interventionId: 'intervention-2',
      orderIndex: 1,
      nom: 'Defaut de patente',
      sujetPaiement: true,
      montantFcfa: 25000,
    ),
  ],
  subjectType: PvSubjectTypes.personWithVehicle,
  status: 'EN_ATTENTE_PAIEMENT',
  createdAt: DateTime(2026, 6, 6),
  verbalizedFirstName: 'Claude',
  verbalizedLastName: 'Fitz',
  verbalizedIdentityType: 'CNI',
  verbalizedIdentityNumber: '142727252727',
  vehiclePlate: 'CE42625YD',
  vehicleRegistrationCardNumber: 'CG427252526',
  vehicleMake: 'Toyota',
  vehicleModel: 'Rav4',
  vehicleColor: 'Rouge',
  locationDescription: 'Yaounde',
  amountInitialFcfa: 55000,
);
