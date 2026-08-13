import 'package:apmtrack_agent/core/api/api_client.dart';
import 'package:apmtrack_agent/core/auth/session_controller.dart';
import 'package:apmtrack_agent/core/auth/session_store.dart';
import 'package:apmtrack_agent/core/models.dart';
import 'package:apmtrack_agent/core/offline/offline_store.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('SessionController token refresh', () {
    test('refreshes on 401 and persists the rotated session', () async {
      final api = _ScriptedApi(
        validToken: 'new-access',
        refreshResult: _rotatedSession,
      );
      final store = MemorySessionStore();
      await store.write(_initialSession);
      final controller = SessionController(
        api: api,
        store: store,
        cache: MemoryOfflineCacheStore(),
      );

      await controller.bootstrap();

      expect(controller.status, SessionStatus.authenticated);
      expect(controller.profile, isNotNull);
      // A single refresh is shared across the concurrent loads.
      expect(api.refreshCalls, 1);
      // The rotated pair must be persisted, not the revoked one.
      final stored = await store.read();
      expect(stored?.accessToken, 'new-access');
      expect(stored?.refreshToken, 'new-refresh');
      expect(controller.token, 'new-access');
    });

    test(
      'keeps the session on a transient network error at bootstrap',
      () async {
        final api = _ScriptedApi(validToken: 'old-access', networkDown: true);
        final store = MemorySessionStore();
        await store.write(_initialSession);
        final controller = SessionController(
          api: api,
          store: store,
          cache: MemoryOfflineCacheStore(),
        );

        await controller.bootstrap();

        // Online-first: a timeout must not destroy the saved session.
        expect(controller.status, SessionStatus.authenticated);
        expect(controller.session, isNotNull);
        expect(api.refreshCalls, 0);
        expect(await store.read(), isNotNull);
        expect(controller.message, isNotNull);
      },
    );

    test('signs out when the refresh is definitively rejected', () async {
      final api = _ScriptedApi(validToken: 'never', refreshUnauthorized: true);
      final store = MemorySessionStore();
      await store.write(_initialSession);
      final controller = SessionController(
        api: api,
        store: store,
        cache: MemoryOfflineCacheStore(),
      );

      await controller.bootstrap();

      expect(controller.status, SessionStatus.unauthenticated);
      expect(controller.session, isNull);
      expect(await store.read(), isNull);
    });
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

const _initialSession = AuthSession(
  accessToken: 'old-access',
  refreshToken: 'old-refresh',
  user: _user,
);

const _rotatedSession = AuthSession(
  accessToken: 'new-access',
  refreshToken: 'new-refresh',
  user: _user,
);

class _ScriptedApi implements ApmtrackApi {
  _ScriptedApi({
    required this.validToken,
    this.refreshResult,
    this.refreshUnauthorized = false,
    this.networkDown = false,
  });

  final String validToken;
  final AuthSession? refreshResult;
  final bool refreshUnauthorized;
  final bool networkDown;
  int refreshCalls = 0;

  Never _fail(int? code) => throw ApiException('err', statusCode: code);

  void _auth(String token) {
    if (networkDown) {
      _fail(null);
    }
    if (token != validToken) {
      _fail(401);
    }
  }

  @override
  Future<AuthSession> refresh(String refreshToken) async {
    refreshCalls += 1;
    if (networkDown) {
      _fail(null);
    }
    if (refreshUnauthorized) {
      _fail(401);
    }
    return refreshResult!;
  }

  @override
  Future<MobileProfile> mobileMe(String token) async {
    _auth(token);
    return _profile;
  }

  @override
  Future<List<Intervention>> mobileInterventions(String token) async {
    _auth(token);
    return const [];
  }

  @override
  Future<PatrouilleActive> activePatrouille(String token) async {
    _auth(token);
    return const PatrouilleActive(agents: []);
  }

  @override
  Future<List<Patrouille>> mobilePatrouilles(String token) async {
    _auth(token);
    return const [];
  }

  @override
  Future<Paginated<Pv>> pvs(String token, {int pageSize = 20}) async {
    _auth(token);
    return const Paginated(items: [], total: 0);
  }

  @override
  Future<AuthSession> login(String email, String password) =>
      throw UnimplementedError();

  @override
  Future<void> logout(String token, String refreshToken) async {}

  @override
  Future<void> changePassword(
    String token,
    String currentPassword,
    String newPassword,
  ) async {}

  @override
  Future<Pv> createPv(String token, CreatePvPayload payload) =>
      throw UnimplementedError();

  @override
  Future<Pv> updatePv(String token, String pvId, CreatePvPayload payload) =>
      throw UnimplementedError();

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

  @override
  Future<void> recordPatrouillePosition(
    String token,
    String patrouilleId, {
    required double latitude,
    required double longitude,
    double? accuracyM,
  }) async {
    _auth(token);
  }
}
