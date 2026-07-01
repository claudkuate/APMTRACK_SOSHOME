import 'package:apmtrack_agent/core/api/api_client.dart';
import 'package:apmtrack_agent/core/auth/session_controller.dart';
import 'package:apmtrack_agent/core/auth/session_store.dart';
import 'package:apmtrack_agent/core/models.dart';
import 'package:apmtrack_agent/core/offline/offline_store.dart';
import 'package:apmtrack_agent/core/theme.dart';
import 'package:apmtrack_agent/core/ui/common.dart';
import 'package:apmtrack_agent/features/pvs/create_pv_page.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:intl/date_symbol_data_local.dart';

void main() {
  setUpAll(() async {
    await initializeDateFormatting('fr_FR');
  });

  testWidgets('filters infractions without clearing hidden selections', (
    tester,
  ) async {
    final controller = _buildController();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: buildApmtrackTheme(),
        home: CreatePvPage(controller: controller),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text(_defautNom), findsOneWidget);
    expect(find.text(_stationnementNom), findsOneWidget);
    expect(find.text(_depotNom), findsOneWidget);
    expect(find.text(_constructionNom), findsOneWidget);

    await tester.tap(find.text(_stationnementNom));
    await tester.pumpAndSettle();
    expect(_checkboxValue(tester, _stationnementNom), isTrue);

    await tester.enterText(
      find.byKey(const Key('intervention-search-field')),
      'defaut',
    );
    await tester.pumpAndSettle();

    expect(find.text(_defautNom), findsOneWidget);
    expect(find.text(_stationnementNom), findsNothing);
    expect(find.text(formatFcfa(35000)), findsOneWidget);

    await tester.tap(find.byTooltip('Effacer la recherche'));
    await tester.pumpAndSettle();

    expect(find.text(_stationnementNom), findsOneWidget);
    expect(_checkboxValue(tester, _stationnementNom), isTrue);

    await tester.enterText(
      find.byKey(const Key('intervention-search-field')),
      'collecte',
    );
    await tester.pumpAndSettle();
    expect(find.text(_depotNom), findsOneWidget);
    expect(find.text(_defautNom), findsNothing);

    await tester.enterText(
      find.byKey(const Key('intervention-search-field')),
      '50000',
    );
    await tester.pumpAndSettle();
    expect(find.text(_constructionNom), findsOneWidget);
    expect(find.text(_depotNom), findsNothing);

    await tester.enterText(
      find.byKey(const Key('intervention-search-field')),
      'introuvable',
    );
    await tester.pumpAndSettle();
    expect(find.text('Aucun resultat'), findsOneWidget);
    expect(
      find.text('Aucune infraction ne correspond a cette recherche.'),
      findsOneWidget,
    );
  });

  testWidgets('renders selected infractions as a review list', (tester) async {
    final controller = _buildController();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: buildApmtrackTheme(),
        home: CreatePvPage(controller: controller),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text(_stationnementNom));
    await tester.enterText(
      find.byKey(const Key('intervention-search-field')),
      'collecte',
    );
    await tester.pumpAndSettle();
    await tester.tap(
      find.ancestor(
        of: find.text(_depotNom),
        matching: find.byType(CheckboxListTile),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('Continuer'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField).at(0), 'Claude');
    await tester.enterText(find.byType(TextField).at(3), '699000001');
    await tester.enterText(find.byType(TextField).at(5), 'CE42625YD');
    await tester.pumpAndSettle();

    await tester.tap(find.text('Continuer'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField).first, 'Yaounde');
    await tester.pumpAndSettle();

    await tester.tap(find.text('Continuer'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Continuer'));
    await tester.pumpAndSettle();

    expect(find.text('Etape 5/5 - Revue'), findsOneWidget);
    expect(find.text(_defautNom), findsOneWidget);
    expect(find.text(_stationnementNom), findsOneWidget);
    expect(find.text(_depotNom), findsOneWidget);
    expect(
      find.text('$_defautNom / $_stationnementNom / $_depotNom'),
      findsNothing,
    );
    expect(find.text('Montant indicatif'), findsOneWidget);
    expect(find.text(formatFcfa(55000)), findsOneWidget);
  });

  testWidgets('offers retry when referentiel is empty and offline', (
    tester,
  ) async {
    final controller = _buildEmptyController(offline: true);
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: buildApmtrackTheme(),
        home: CreatePvPage(controller: controller),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Referentiel non charge'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Reessayer'), findsOneWidget);
    expect(find.text('Le referentiel mobile est vide.'), findsNothing);
  });

  testWidgets('shows empty message when referentiel is genuinely empty', (
    tester,
  ) async {
    final controller = _buildEmptyController();
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        theme: buildApmtrackTheme(),
        home: CreatePvPage(controller: controller),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('Le referentiel mobile est vide.'), findsOneWidget);
    expect(find.text('Reessayer'), findsNothing);
  });
}

SessionController _buildController() {
  return SessionController(
      api: _NoopApi(),
      store: MemorySessionStore(),
      cache: MemoryOfflineCacheStore(),
    )
    ..session = _session
    ..status = SessionStatus.authenticated
    ..interventions = _interventions;
}

SessionController _buildEmptyController({bool offline = false}) {
  return SessionController(
      api: _NoopApi(),
      store: MemorySessionStore(),
      cache: MemoryOfflineCacheStore(),
    )
    ..session = _session
    ..status = SessionStatus.authenticated
    ..interventions = const []
    ..offline = offline;
}

bool? _checkboxValue(WidgetTester tester, String title) {
  final tile = tester.widget<CheckboxListTile>(
    find.ancestor(
      of: find.text(title),
      matching: find.byType(CheckboxListTile),
    ),
  );
  return tile.value;
}

const _defautNom = 'D\u00e9faut de patente ou de d\u00e9claration';
const _stationnementNom = 'Stationnement interdit';
const _depotNom = 'D\u00e9p\u00f4t sauvage d ordure';
const _constructionNom = 'Construction sans autorisation';

const _interventions = [
  Intervention(
    id: 'intervention-defaut',
    nom: _defautNom,
    sujetPaiement: true,
    active: true,
    montantFcfa: 25000,
  ),
  Intervention(
    id: 'intervention-stationnement',
    nom: _stationnementNom,
    sujetPaiement: true,
    active: true,
    montantFcfa: 10000,
  ),
  Intervention(
    id: 'intervention-depot',
    nom: _depotNom,
    description: 'Infraction liee a la collecte',
    sujetPaiement: true,
    active: true,
    montantFcfa: 20000,
  ),
  Intervention(
    id: 'intervention-construction',
    nom: _constructionNom,
    sujetPaiement: true,
    active: true,
    montantFcfa: 50000,
  ),
];

const _user = UserAccount(
  id: 'user-1',
  email: 'agent@test.local',
  fullName: 'Agent Test',
  communeId: 'commune-1',
  roles: ['APM_AGENT'],
  active: true,
);

const _session = AuthSession(
  accessToken: 'access-token',
  refreshToken: 'refresh-token',
  user: _user,
);

class _NoopApi implements ApmtrackApi {
  @override
  Future<AuthSession> login(String email, String password) async => _session;

  @override
  Future<AuthSession> refresh(String refreshToken) async => _session;

  @override
  Future<void> logout(String token, String refreshToken) async {}

  @override
  Future<MobileProfile> mobileMe(String token) => throw UnimplementedError();

  @override
  Future<List<Intervention>> mobileInterventions(String token) async =>
      const [];

  @override
  Future<PatrouilleActive> activePatrouille(String token) async {
    return const PatrouilleActive(agents: []);
  }

  @override
  Future<List<Patrouille>> mobilePatrouilles(String token) async => const [];

  @override
  Future<Paginated<Pv>> pvs(String token, {int pageSize = 20}) async {
    return const Paginated(items: [], total: 0);
  }

  @override
  Future<Pv> createPv(String token, CreatePvPayload payload) =>
      throw UnimplementedError();

  @override
  Future<Pv> updatePv(String token, String pvId, CreatePvPayload payload) =>
      throw UnimplementedError();

  @override
  Future<void> recordPatrouillePosition(
    String token,
    String patrouilleId, {
    required double latitude,
    required double longitude,
    double? accuracyM,
  }) async {}

  @override
  Future<String> pvQrSvg(String token, String pvId) =>
      throw UnimplementedError();

  @override
  Future<List<int>> pvPdfBytes(String token, String pvId) =>
      throw UnimplementedError();

  @override
  Future<List<PvPhoto>> listPvPhotos(String token, String pvId) =>
      throw UnimplementedError();

  @override
  Future<PvPhoto> uploadPvPhoto(
    String token,
    String pvId, {
    required List<int> bytes,
    required String filename,
    required String contentType,
  }) => throw UnimplementedError();

  @override
  Future<void> deletePvPhoto(String token, String pvId, String photoId) =>
      throw UnimplementedError();

  @override
  String photoContentUrl(String pvId, String photoId) => '';

  @override
  String agentPhotoContentUrl(String agentId) => '';

  @override
  Future<PvPublic> verifyPublicPv(String pvNumber) =>
      throw UnimplementedError();
}
