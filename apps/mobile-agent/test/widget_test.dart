import 'package:apmtrack_agent/app.dart';
import 'package:apmtrack_agent/core/api/api_client.dart';
import 'package:apmtrack_agent/core/auth/session_store.dart';
import 'package:apmtrack_agent/core/models.dart';
import 'package:apmtrack_agent/core/offline/offline_store.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:intl/date_symbol_data_local.dart';

void main() {
  setUpAll(() async {
    // Aligne le test sur le runtime (`main.dart`) : les écrans formatent des
    // dates en `fr_FR` via `core/ui/common.dart`.
    await initializeDateFormatting('fr_FR');
  });

  testWidgets('renders login page', (tester) async {
    await tester.pumpWidget(
      ApmtrackAgentApp(
        api: _FakeApi(),
        store: MemorySessionStore(),
        cache: MemoryOfflineCacheStore(),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('APMTRACK Agent'), findsWidgets);
    expect(find.text('Connexion agent terrain'), findsOneWidget);
    expect(find.text('Se connecter'), findsOneWidget);
  });

  testWidgets('logs in and renders agent home', (tester) async {
    await tester.pumpWidget(
      ApmtrackAgentApp(
        api: _FakeApi(),
        store: MemorySessionStore(),
        cache: MemoryOfflineCacheStore(),
      ),
    );
    await tester.pumpAndSettle();

    await tester.enterText(
      find.byType(TextFormField).at(0),
      'agent@test.local',
    );
    await tester.enterText(find.byType(TextFormField).at(1), 'password');
    await tester.tap(find.text('Se connecter'));
    await tester.pumpAndSettle();

    expect(find.text('Patrouille active'), findsOneWidget);
    expect(find.text('Agent Test'), findsOneWidget);
    expect(find.text('Nouveau PV'), findsWidgets);
  });
}

class _FakeApi implements ApmtrackApi {
  static const _user = UserAccount(
    id: 'user-1',
    email: 'agent@test.local',
    fullName: 'Agent Test',
    communeId: 'commune-1',
    roles: ['APM_AGENT'],
    active: true,
  );

  static const _profile = MobileProfile(
    user: _user,
    commune: Commune(
      id: 'commune-1',
      code: 'YDE1',
      nom: 'Yaounde 1',
      region: 'Centre',
      departement: 'Mfoundi',
    ),
    agent: AgentProfile(
      id: 'agent-1',
      matricule: 'APM-YDE1-001',
      fullName: 'Agent Test',
      communeId: 'commune-1',
      grade: 'Agent',
      status: 'ACTIF',
      formationNasla: true,
    ),
  );

  @override
  Future<AuthSession> login(String email, String password) async {
    return const AuthSession(
      accessToken: 'access-token',
      refreshToken: 'refresh-token',
      user: _user,
    );
  }

  @override
  Future<AuthSession> refresh(String refreshToken) async {
    return const AuthSession(
      accessToken: 'access-token-2',
      refreshToken: 'refresh-token-2',
      user: _user,
    );
  }

  @override
  Future<void> logout(String token, String refreshToken) async {}

  @override
  Future<MobileProfile> mobileMe(String token) async => _profile;

  @override
  Future<List<Intervention>> mobileInterventions(String token) async => const [
    Intervention(
      id: 'intervention-1',
      nom: 'Stationnement interdit',
      sujetPaiement: true,
      active: true,
      montantFcfa: 10000,
    ),
  ];

  @override
  Future<PatrouilleActive> activePatrouille(String token) async {
    return const PatrouilleActive(
      patrouille: Patrouille(
        id: 'patrouille-1',
        nom: 'Patrouille centre',
        status: 'EN_COURS',
      ),
      agents: [
        PatrouilleMember(
          agentId: 'agent-1',
          matricule: 'APM-YDE1-001',
          fullName: 'Agent Test',
          grade: 'Agent',
          rolePatrouille: 'CHEF',
        ),
      ],
    );
  }

  @override
  Future<Paginated<Pv>> pvs(String token, {int pageSize = 20}) async {
    return Paginated(
      total: 1,
      items: [
        Pv(
          id: 'pv-1',
          pvNumber: 'PV-YDE1-2026-000001',
          interventionId: 'intervention-1',
          status: 'EN_ATTENTE_PAIEMENT',
          createdAt: DateTime(2026, 6, 5),
          vehiclePlate: 'CE123AB',
          amountInitialFcfa: 10000,
        ),
      ],
    );
  }

  @override
  Future<Pv> createPv(String token, CreatePvPayload payload) async {
    return Pv(
      id: 'pv-2',
      pvNumber: 'PV-YDE1-2026-000002',
      interventionId: payload.interventionId,
      status: 'EN_ATTENTE_PAIEMENT',
      createdAt: DateTime(2026, 6, 5),
    );
  }

  @override
  Future<Pv> updatePv(
    String token,
    String pvId,
    CreatePvPayload payload,
  ) async {
    return Pv(
      id: pvId,
      pvNumber: 'PV-YDE1-2026-000002',
      interventionId: payload.interventionId,
      status: 'EN_ATTENTE_PAIEMENT',
      createdAt: DateTime(2026, 6, 5),
    );
  }

  @override
  Future<String> pvQrSvg(String token, String pvId) async => '<svg></svg>';

  @override
  Future<List<PvPhoto>> listPvPhotos(String token, String pvId) async =>
      const [];

  @override
  Future<PvPhoto> uploadPvPhoto(
    String token,
    String pvId, {
    required List<int> bytes,
    required String filename,
    required String contentType,
  }) async => PvPhoto(
    id: 'photo-1',
    pvId: pvId,
    contentType: contentType,
    sizeBytes: bytes.length,
  );

  @override
  Future<void> deletePvPhoto(String token, String pvId, String photoId) async {}

  @override
  String photoContentUrl(String pvId, String photoId) =>
      'http://test/$pvId/$photoId';

  @override
  Future<PvPublic> verifyPublicPv(String pvNumber) async {
    return PvPublic(pvNumber: pvNumber, status: 'EN_ATTENTE_PAIEMENT');
  }

  @override
  Future<void> recordPatrouillePosition(
    String token,
    String patrouilleId, {
    required double latitude,
    required double longitude,
    double? accuracyM,
  }) async {}
}
