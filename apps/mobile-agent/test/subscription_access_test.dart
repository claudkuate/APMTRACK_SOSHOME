import 'package:apmtrack_agent/app.dart';
import 'package:apmtrack_agent/core/api/api_client.dart';
import 'package:apmtrack_agent/core/auth/session_controller.dart';
import 'package:apmtrack_agent/core/auth/session_store.dart';
import 'package:apmtrack_agent/core/models.dart';
import 'package:apmtrack_agent/core/offline/offline_models.dart';
import 'package:apmtrack_agent/core/offline/offline_store.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('controle local de l’abonnement', () {
    test(
      'une echeance connue et depassee ferme la session sans effacer les brouillons',
      () async {
        final store = MemorySessionStore();
        final cache = MemoryOfflineCacheStore();
        final saved = _session(
          expiresAt: DateTime.now().toUtc().subtract(
            const Duration(minutes: 1),
          ),
        );
        final draft = _draft();
        await store.write(saved);
        await cache.write('offline.drafts', encodeDrafts([draft]));

        final controller = SessionController(
          api: _SubscriptionRejectingApi(saved),
          store: store,
          cache: cache,
        );
        await controller.bootstrap();

        expect(controller.status, SessionStatus.unauthenticated);
        expect(controller.session, isNull);
        expect(
          controller.message,
          SessionController.subscriptionBlockedMessage,
        );
        expect(await store.read(), isNull);
        final persisted = decodeDrafts(await cache.read('offline.drafts'));
        expect(persisted, hasLength(1));
        expect(persisted.single.localId, draft.localId);
        expect(persisted.single.status, PvDraftStatus.pending);
      },
    );

    test(
      'un refus serveur pendant la synchronisation conserve le brouillon en attente',
      () async {
        final store = MemorySessionStore();
        final cache = MemoryOfflineCacheStore();
        final saved = _session(
          expiresAt: DateTime.now().toUtc().add(const Duration(days: 30)),
        );
        final draft = _draft();
        final api = _SubscriptionRejectingApi(saved);
        await store.write(saved);
        await cache.write('offline.drafts', encodeDrafts([draft]));

        final controller = SessionController(
          api: api,
          store: store,
          cache: cache,
        );
        await controller.bootstrap();

        expect(api.createPvCalls, 1);
        expect(controller.status, SessionStatus.unauthenticated);
        expect(controller.session, isNull);
        expect(
          controller.message,
          SessionController.subscriptionBlockedMessage,
        );
        final persisted = decodeDrafts(await cache.read('offline.drafts'));
        expect(persisted, hasLength(1));
        expect(persisted.single.localId, draft.localId);
        expect(persisted.single.status, PvDraftStatus.pending);
      },
    );

    test(
      'une session ouverte se ferme automatiquement a son echeance connue',
      () async {
        final store = MemorySessionStore();
        final cache = MemoryOfflineCacheStore();
        final saved = _session(
          expiresAt: DateTime.now().toUtc().add(
            const Duration(milliseconds: 150),
          ),
        );
        await store.write(saved);

        final controller = SessionController(
          api: _SubscriptionRejectingApi(saved),
          store: store,
          cache: cache,
        );
        addTearDown(controller.dispose);
        await controller.bootstrap();

        expect(controller.status, SessionStatus.authenticated);
        await Future<void>.delayed(const Duration(milliseconds: 300));

        expect(controller.status, SessionStatus.unauthenticated);
        expect(controller.session, isNull);
        expect(
          controller.message,
          SessionController.subscriptionBlockedMessage,
        );
        expect(await store.read(), isNull);
      },
    );

    test(
      'un asset protege refuse revalide puis ferme la session sans perdre les brouillons',
      () async {
        final store = MemorySessionStore();
        final cache = MemoryOfflineCacheStore();
        final saved = _session(
          expiresAt: DateTime.now().toUtc().add(const Duration(days: 30)),
        );
        final draft = _draft();
        final api = _SubscriptionRejectingApi(saved);
        await store.write(saved);
        await cache.write('offline.drafts', encodeDrafts([draft]));

        final controller = SessionController(
          api: api,
          store: store,
          cache: cache,
        );
        addTearDown(controller.dispose);
        await controller.bootstrap();
        api.rejectAuthenticatedReads = true;

        await controller.handleAuthenticatedAssetForbidden();

        expect(controller.status, SessionStatus.unauthenticated);
        expect(controller.session, isNull);
        expect(
          controller.message,
          SessionController.subscriptionBlockedMessage,
        );
        final persisted = decodeDrafts(await cache.read('offline.drafts'));
        expect(persisted, hasLength(1));
        expect(persisted.single.localId, draft.localId);
        expect(persisted.single.status, PvDraftStatus.pending);
      },
    );

    test(
      'le retour en ligne revalide la suspension serveur sans perdre les brouillons',
      () async {
        final store = MemorySessionStore();
        final cache = MemoryOfflineCacheStore();
        final saved = _session(
          expiresAt: DateTime.now().toUtc().add(const Duration(days: 30)),
        );
        final draft = _draft();
        final api = _SubscriptionRejectingApi(saved);
        await store.write(saved);
        await cache.write('offline.drafts', encodeDrafts([draft]));

        final controller = SessionController(
          api: api,
          store: store,
          cache: cache,
        );
        addTearDown(controller.dispose);
        await controller.bootstrap();
        api.rejectAuthenticatedReads = true;

        await controller.revalidateKnownSubscriptionAccess(checkServer: true);

        expect(controller.status, SessionStatus.unauthenticated);
        expect(controller.session, isNull);
        expect(
          controller.message,
          SessionController.subscriptionBlockedMessage,
        );
        final persisted = decodeDrafts(await cache.read('offline.drafts'));
        expect(persisted, hasLength(1));
        expect(persisted.single.localId, draft.localId);
        expect(persisted.single.status, PvDraftStatus.pending);
      },
    );

    testWidgets('le message de suspension est affiche sur la connexion', (
      tester,
    ) async {
      final store = MemorySessionStore();
      final cache = MemoryOfflineCacheStore();
      final saved = _session(
        expiresAt: DateTime.now().toUtc().subtract(const Duration(minutes: 1)),
      );
      await store.write(saved);

      await tester.pumpWidget(
        ApmtrackAgentApp(
          api: _SubscriptionRejectingApi(saved),
          store: store,
          cache: cache,
        ),
      );
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 10));

      expect(
        find.text(SessionController.subscriptionBlockedMessage),
        findsOneWidget,
      );

      await tester.pumpWidget(const SizedBox.shrink());
    });
  });
}

AuthSession _session({required DateTime expiresAt}) => AuthSession(
  accessToken: 'access-token',
  refreshToken: 'refresh-token',
  user: UserAccount(
    id: 'agent-user',
    email: 'agent@example.test',
    fullName: 'Agent Test',
    communeId: 'commune-1',
    roles: const ['AGENT'],
    active: true,
    communeAccessExpiresAt: expiresAt,
  ),
);

PvDraft _draft() => PvDraft(
  localId: 'draft-subscription-test',
  payload: const CreatePvPayload(
    interventionId: 'intervention-1',
    verbalizedName: 'Usager Test',
  ),
  createdAt: DateTime.utc(2026, 8, 17),
  ownerUserId: 'agent-user',
);

class _SubscriptionRejectingApi implements ApmtrackApi {
  _SubscriptionRejectingApi(this.currentSession);

  final AuthSession currentSession;
  int createPvCalls = 0;
  bool rejectAuthenticatedReads = false;

  @override
  Future<MobileProfile> mobileMe(String token) async {
    if (rejectAuthenticatedReads) {
      throw ApiException(
        SessionController.subscriptionBlockedMessage,
        statusCode: 403,
        code: 'COMMUNE_SUBSCRIPTION_INACTIVE',
      );
    }
    return MobileProfile(
      user: currentSession.user,
      commune: const Commune(
        id: 'commune-1',
        code: 'TEST',
        nom: 'Commune Test',
        region: 'Centre',
        departement: 'Mfoundi',
      ),
      agent: const AgentProfile(
        id: 'agent-1',
        matricule: 'AG-001',
        fullName: 'Agent Test',
        communeId: 'commune-1',
        status: 'ACTIF',
      ),
    );
  }

  @override
  Future<List<Intervention>> mobileInterventions(String token) async =>
      const [];

  @override
  Future<PatrouilleActive> activePatrouille(String token) async =>
      const PatrouilleActive(agents: []);

  @override
  Future<List<Patrouille>> mobilePatrouilles(String token) async => const [];

  @override
  Future<Paginated<Pv>> pvs(String token, {int pageSize = 20}) async =>
      const Paginated(items: [], total: 0);

  @override
  Future<Pv> createPv(String token, CreatePvPayload payload) async {
    createPvCalls++;
    throw ApiException(
      SessionController.subscriptionBlockedMessage,
      statusCode: 403,
      code: 'COMMUNE_SUBSCRIPTION_INACTIVE',
    );
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError(invocation.memberName.toString());
}
