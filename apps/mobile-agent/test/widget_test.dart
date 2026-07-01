import 'package:apmtrack_agent/app.dart';
import 'package:apmtrack_agent/core/api/api_client.dart';
import 'package:apmtrack_agent/core/auth/session_controller.dart';
import 'package:apmtrack_agent/core/auth/session_store.dart';
import 'package:apmtrack_agent/core/models.dart';
import 'package:apmtrack_agent/core/offline/offline_store.dart';
import 'package:apmtrack_agent/core/theme.dart';
import 'package:apmtrack_agent/core/ui/agent_avatar.dart';
import 'package:apmtrack_agent/features/auth/login_page.dart';
import 'package:apmtrack_agent/features/home/home_page.dart';
import 'package:apmtrack_agent/features/profile/profile_page.dart';
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
    await _pumpUntilFound(tester, find.text('Se connecter'));

    expect(find.text('G-APM Agent'), findsWidgets);
    expect(find.byKey(const Key('login-hero-image')), findsOneWidget);
    expect(find.byKey(const Key('cameroon-seal-image')), findsOneWidget);
    expect(find.text('Connexion agent terrain'), findsOneWidget);
    expect(
      find.text(
        'Brouillon local possible hors reseau. Numero, montant et QR sont attribues par le backend apres synchronisation.',
      ),
      findsNothing,
    );
    expect(
      find.text(
        'Les PV officiels sont valides par le serveur. Le mobile ne cree pas de PV hors ligne.',
      ),
      findsNothing,
    );
    expect(find.text('Configuration'), findsNothing);
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
    await _pumpUntilFound(tester, find.text('Se connecter'));

    await tester.enterText(
      find.byType(TextFormField).at(0),
      'agent@test.local',
    );
    await tester.enterText(find.byType(TextFormField).at(1), 'password');
    await tester.tap(find.text('Se connecter'));
    await _pumpUntilFound(tester, find.text('Patrouille active'));

    expect(find.text('Patrouille active'), findsOneWidget);
    expect(find.text('Agent Test'), findsOneWidget);
    expect(find.text('Nouvelle saisie PV'), findsWidgets);
    expect(find.text('PV officiels'), findsOneWidget);
    expect(find.text('Derniers PV officiels'), findsOneWidget);
    expect(find.text('Nouveau PV'), findsNothing);
    expect(
      find.byKey(const Key('agent-avatar-initials-agent-1')),
      findsOneWidget,
    );
    expect(find.byKey(const Key('agent-avatar-image-agent-1')), findsNothing);

    await tester.scrollUntilVisible(find.text('PV-YDE1-2026-000001'), 200);
    await tester.pumpAndSettle();
    await tester.tap(find.text('PV-YDE1-2026-000001'));
    await tester.pumpAndSettle();

    expect(find.byTooltip('Imprimer'), findsOneWidget);
    expect(find.text('Montant'), findsOneWidget);
  });

  testWidgets('renders login page with disabled animations', (tester) async {
    final controller = SessionController(
      api: _FakeApi(),
      store: MemorySessionStore(),
      cache: MemoryOfflineCacheStore(),
    );
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: buildApmtrackTheme(),
        builder: (context, child) {
          return MediaQuery(
            data: MediaQuery.of(context).copyWith(disableAnimations: true),
            child: child!,
          );
        },
        home: LoginPage(controller: controller),
      ),
    );
    await tester.pump();

    expect(find.byKey(const Key('login-hero-image')), findsOneWidget);
    expect(find.text('Se connecter'), findsOneWidget);
  });

  testWidgets('agent avatar uses authenticated photo endpoint', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: AgentAvatar(
            agent: AgentProfile(
              id: 'agent-1',
              matricule: 'APM-YDE1-001',
              fullName: 'Agent Test',
              communeId: 'commune-1',
              status: 'ACTIF',
              photoUrl: 'avatars/agents/agent-1.jpg',
            ),
            imageUrl: 'http://test/api/v1/agents/agent-1/photo',
            headers: {'authorization': 'Bearer access-token'},
          ),
        ),
      ),
    );

    final image = tester.widget<Image>(
      find.byKey(const Key('agent-avatar-image-agent-1')),
    );
    final provider = image.image as NetworkImage;
    expect(provider.url, 'http://test/api/v1/agents/agent-1/photo');
    expect(provider.headers, {'authorization': 'Bearer access-token'});
    expect(
      find.byKey(const Key('agent-avatar-initials-agent-1')),
      findsNothing,
    );
  });

  testWidgets('agent avatar falls back to initials when image load fails', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: AgentAvatar(
            agent: AgentProfile(
              id: 'agent-1',
              matricule: 'APM-YDE1-001',
              fullName: 'Agent Test',
              communeId: 'commune-1',
              status: 'ACTIF',
              photoUrl: 'avatars/agents/agent-1.jpg',
            ),
            imageUrl: 'http://test/api/v1/agents/agent-1/photo',
            headers: {'authorization': 'Bearer access-token'},
          ),
        ),
      ),
    );

    final image = tester.widget<Image>(
      find.byKey(const Key('agent-avatar-image-agent-1')),
    );
    final fallback = image.errorBuilder!(
      tester.element(find.byKey(const Key('agent-avatar-image-agent-1'))),
      Exception('network failed'),
      StackTrace.empty,
    );

    await tester.pumpWidget(MaterialApp(home: Scaffold(body: fallback)));

    expect(
      find.byKey(const Key('agent-avatar-initials-agent-1')),
      findsOneWidget,
    );
    expect(find.text('AT'), findsOneWidget);
  });

  testWidgets('home renders the authenticated agent photo endpoint', (
    tester,
  ) async {
    final controller = _controllerWithProfile(_FakeApi.profileWithPhoto);
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: buildApmtrackTheme(),
        home: Scaffold(
          body: HomePage(
            controller: controller,
            onCreatePv: () {},
            onOpenPvs: () {},
            onOpenScan: () {},
          ),
        ),
      ),
    );

    _expectAgentPhotoImage(tester);
  });

  testWidgets('profile renders the authenticated agent photo endpoint', (
    tester,
  ) async {
    final controller = _controllerWithProfile(_FakeApi.profileWithPhoto);
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: buildApmtrackTheme(),
        home: Scaffold(body: ProfilePage(controller: controller)),
      ),
    );

    _expectAgentPhotoImage(tester);
  });
}

Future<void> _pumpUntilFound(WidgetTester tester, Finder finder) async {
  for (var i = 0; i < 20; i += 1) {
    await tester.pump(const Duration(milliseconds: 50));
    if (finder.evaluate().isNotEmpty) {
      return;
    }
  }
  expect(finder, findsOneWidget);
}

SessionController _controllerWithProfile(MobileProfile profile) {
  final controller = SessionController(
    api: _FakeApi(profile: profile),
    store: MemorySessionStore(),
    cache: MemoryOfflineCacheStore(),
  );
  controller
    ..session = AuthSession(
      accessToken: 'access-token',
      refreshToken: 'refresh-token',
      user: profile.user,
    )
    ..status = SessionStatus.authenticated
    ..profile = profile;
  return controller;
}

void _expectAgentPhotoImage(WidgetTester tester) {
  final image = tester.widget<Image>(
    find.byKey(const Key('agent-avatar-image-agent-1')),
  );
  final provider = image.image as NetworkImage;
  expect(provider.url, 'http://test/api/v1/agents/agent-1/photo');
  expect(provider.headers, {'authorization': 'Bearer access-token'});
}

class _FakeApi implements ApmtrackApi {
  _FakeApi({MobileProfile? profile})
    : _profile = profile ?? profileWithoutPhoto;

  static const _user = UserAccount(
    id: 'user-1',
    email: 'agent@test.local',
    fullName: 'Agent Test',
    communeId: 'commune-1',
    roles: ['APM_AGENT'],
    active: true,
  );

  static const profileWithoutPhoto = MobileProfile(
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
      status: 'ACTIF',
    ),
  );

  static const profileWithPhoto = MobileProfile(
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
      status: 'ACTIF',
      photoUrl: 'avatars/agents/agent-1.jpg',
    ),
  );

  final MobileProfile _profile;

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
          rolePatrouille: 'CHEF',
        ),
      ],
    );
  }

  @override
  Future<List<Patrouille>> mobilePatrouilles(String token) async => const [];

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
  Future<String> pvQrSvg(String token, String pvId) async =>
      '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>';

  @override
  Future<List<int>> pvPdfBytes(String token, String pvId) async => const <int>[];

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
  String agentPhotoContentUrl(String agentId) =>
      'http://test/api/v1/agents/$agentId/photo';

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
