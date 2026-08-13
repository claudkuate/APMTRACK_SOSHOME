import 'package:apmtrack_agent/core/api/api_client.dart';
import 'package:apmtrack_agent/core/auth/session_controller.dart';
import 'package:apmtrack_agent/core/auth/session_store.dart';
import 'package:apmtrack_agent/core/models.dart';
import 'package:apmtrack_agent/core/offline/offline_models.dart';
import 'package:apmtrack_agent/core/offline/offline_store.dart';
import 'package:apmtrack_agent/features/pvs/pv_list_page.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:intl/date_symbol_data_local.dart';

void main() {
  setUpAll(() async {
    await initializeDateFormatting('fr_FR');
  });

  testWidgets('distinguishes local drafts from server PV proof queues', (
    tester,
  ) async {
    final controller =
        SessionController(
            api: _NoopApi(),
            store: MemorySessionStore(),
            cache: MemoryOfflineCacheStore(),
          )
          ..session = _session
          ..status = SessionStatus.authenticated
          ..drafts = [_localDraft, _serverProofDraft]
          ..pvs = const [];
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: PvListPage(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('PV officiels'), findsOneWidget);
    expect(find.text('Aucun PV officiel'), findsOneWidget);
    expect(find.text('Brouillons locaux a synchroniser (2)'), findsOneWidget);
    expect(
      find.textContaining(
        'Non officiel - En attente de synchronisation serveur',
      ),
      findsOneWidget,
    );
    expect(
      find.textContaining("PV serveur cree - preuves en attente d'envoi"),
      findsOneWidget,
    );

    await tester.tap(find.byTooltip('Supprimer le brouillon local'));
    await tester.pumpAndSettle();
    expect(find.text('Supprimer le brouillon local ?'), findsOneWidget);
    expect(
      find.text(
        'Cette saisie non officielle sera definitivement perdue. Aucun PV serveur ne sera supprime.',
      ),
      findsOneWidget,
    );
    await tester.tap(find.text('Annuler'));
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Supprimer les preuves en attente'));
    await tester.pumpAndSettle();
    expect(find.text('Supprimer les preuves en attente ?'), findsOneWidget);
    expect(
      find.text(
        'Le PV serveur existe deja. Seuls le brouillon local et les preuves en attente seront supprimes.',
      ),
      findsOneWidget,
    );
  });

  testWidgets('renders separators and opens official PV details', (
    tester,
  ) async {
    final controller =
        SessionController(
            api: _NoopApi(),
            store: MemorySessionStore(),
            cache: MemoryOfflineCacheStore(),
          )
          ..session = _session
          ..status = SessionStatus.authenticated
          ..pvs = _officialPvs;
    addTearDown(controller.dispose);

    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(body: PvListPage(controller: controller)),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.text('PV-YDE1-2026-000001'), findsOneWidget);
    expect(find.text('PV-YDE1-2026-000002'), findsOneWidget);
    expect(find.byType(Divider), findsOneWidget);

    await tester.tap(find.text('PV-YDE1-2026-000001'));
    await tester.pumpAndSettle();

    expect(find.byTooltip('Imprimer'), findsOneWidget);
    expect(find.text('Montant'), findsOneWidget);
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

const _session = AuthSession(
  accessToken: 'access-token',
  refreshToken: 'refresh-token',
  user: _user,
);

const _payload = CreatePvPayload(
  interventionId: 'intervention-1',
  vehiclePlate: 'CE456CD',
  verbalizedName: 'Jean Test',
  locationDescription: 'Avenue Kennedy',
);

final _localDraft = PvDraft(
  localId: 'draft-local',
  payload: _payload,
  createdAt: DateTime(2026, 6, 6),
  interventionName: 'Stationnement interdit',
  amountFcfa: 10000,
);

final _serverProofDraft = PvDraft(
  localId: 'draft-proof',
  payload: _payload,
  createdAt: DateTime(2026, 6, 6),
  interventionName: 'Stationnement interdit',
  amountFcfa: 10000,
  serverPvId: 'pv-2',
  photos: const [
    PvDraftPhoto(
      path: 'proof.jpg',
      filename: 'proof.jpg',
      contentType: 'image/jpeg',
    ),
  ],
);

final _officialPvs = [
  Pv(
    id: 'pv-1',
    pvNumber: 'PV-YDE1-2026-000001',
    interventionId: 'intervention-1',
    status: 'EN_ATTENTE_PAIEMENT',
    createdAt: DateTime(2026, 6, 6),
    vehiclePlate: 'YDE1-137-AA',
    amountInitialFcfa: 25000,
    verbalizedName: 'Marie Ngono',
  ),
  Pv(
    id: 'pv-2',
    pvNumber: 'PV-YDE1-2026-000002',
    interventionId: 'intervention-1',
    status: 'PAYE',
    createdAt: DateTime(2026, 6, 6),
    vehiclePlate: 'YDE1-274-AA',
    amountInitialFcfa: 10000,
    verbalizedName: 'Pierre Fotso',
  ),
];

class _NoopApi implements ApmtrackApi {
  @override
  Future<AuthSession> login(String email, String password) async => _session;

  @override
  Future<AuthSession> refresh(String refreshToken) async => _session;

  @override
  Future<void> logout(String token, String refreshToken) async {}

  @override
  Future<void> changePassword(
    String token,
    String currentPassword,
    String newPassword,
  ) async {}

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
  Future<String> pvQrSvg(String token, String pvId) async =>
      '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"></svg>';

  @override
  Future<List<int>> pvPdfBytes(String token, String pvId) async => const <int>[];

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
