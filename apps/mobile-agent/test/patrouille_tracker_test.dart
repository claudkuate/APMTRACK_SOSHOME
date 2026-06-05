import 'dart:async';

import 'package:apmtrack_agent/core/api/api_client.dart';
import 'package:apmtrack_agent/core/auth/session_controller.dart';
import 'package:apmtrack_agent/core/auth/session_store.dart';
import 'package:apmtrack_agent/core/models.dart';
import 'package:apmtrack_agent/features/patrouille/location_source.dart';
import 'package:apmtrack_agent/features/patrouille/patrouille_tracker.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> _settle() =>
    Future<void>.delayed(const Duration(milliseconds: 20));

void main() {
  group('PatrouilleTracker', () {
    late _RecorderApi api;
    late SessionController controller;
    late _FakeLocationSource source;
    late PatrouilleTracker tracker;

    setUp(() {
      api = _RecorderApi();
      controller = SessionController(api: api, store: MemorySessionStore())
        ..session = _session;
      source = _FakeLocationSource();
      tracker = PatrouilleTracker(
        controller: controller,
        locationSource: source,
        distanceFilterM: 0,
      );
    });

    tearDown(() => tracker.dispose());

    test('pushes fixes automatically while tracking', () async {
      await tracker.start('pat-1');
      expect(tracker.isTracking, isTrue);

      source.emit(const GpsFix(latitude: 3.8, longitude: 11.5, accuracyM: 5));
      await _settle();

      expect(api.recorded, hasLength(1));
      expect(api.recorded.single.latitude, 3.8);
      expect(tracker.pendingCount, 0);
      expect(tracker.lastSentAt, isNotNull);
      expect(tracker.error, isNull);
    });

    test('queues fixes offline and replays them once back online', () async {
      await tracker.start('pat-1');

      api.offline = true;
      source.emit(const GpsFix(latitude: 1, longitude: 1));
      await _settle();

      expect(api.recorded, isEmpty);
      expect(tracker.pendingCount, 1);
      expect(tracker.error, isNotNull);

      api.offline = false;
      source.emit(const GpsFix(latitude: 2, longitude: 2));
      await _settle();

      // Both the queued fix and the new one are flushed in order.
      expect(api.recorded, hasLength(2));
      expect(api.recorded.first.latitude, 1);
      expect(api.recorded.last.latitude, 2);
      expect(tracker.pendingCount, 0);
      expect(tracker.error, isNull);
    });

    test('does not start when location permission is refused', () async {
      source.ready = false;

      await tracker.start('pat-1');

      expect(tracker.isTracking, isFalse);
      expect(tracker.error, isNotNull);
      expect(api.recorded, isEmpty);
    });

    test('stop clears the pending queue', () async {
      await tracker.start('pat-1');
      api.offline = true;
      source.emit(const GpsFix(latitude: 1, longitude: 1));
      await _settle();
      expect(tracker.pendingCount, 1);

      await tracker.stop();

      expect(tracker.isTracking, isFalse);
      expect(tracker.pendingCount, 0);
    });
  });
}

const _user = UserAccount(
  id: 'user-1',
  email: 'agent@test.local',
  fullName: 'Agent Test',
  roles: ['APM_AGENT'],
  active: true,
);

const _session = AuthSession(
  accessToken: 'access-token',
  refreshToken: 'refresh-token',
  user: _user,
);

class _FakeLocationSource implements LocationSource {
  final StreamController<GpsFix> _controller =
      StreamController<GpsFix>.broadcast();
  bool ready = true;

  void emit(GpsFix fix) => _controller.add(fix);

  @override
  Future<void> ensureReady() async {
    if (!ready) {
      throw ApiException('GPS refuse');
    }
  }

  @override
  Stream<GpsFix> watch({required int distanceFilterM}) => _controller.stream;

  @override
  Future<GpsFix> current() async => const GpsFix(latitude: 0, longitude: 0);
}

class _RecorderApi implements ApmtrackApi {
  final List<GpsFix> recorded = [];
  bool offline = false;

  @override
  Future<void> recordPatrouillePosition(
    String token,
    String patrouilleId, {
    required double latitude,
    required double longitude,
    double? accuracyM,
  }) async {
    if (offline) {
      throw ApiException('reseau indisponible');
    }
    recorded.add(
      GpsFix(latitude: latitude, longitude: longitude, accuracyM: accuracyM),
    );
  }

  @override
  Future<AuthSession> login(String email, String password) =>
      throw UnimplementedError();

  @override
  Future<AuthSession> refresh(String refreshToken) =>
      throw UnimplementedError();

  @override
  Future<void> logout(String token, String refreshToken) async {}

  @override
  Future<MobileProfile> mobileMe(String token) => throw UnimplementedError();

  @override
  Future<List<Intervention>> mobileInterventions(String token) =>
      throw UnimplementedError();

  @override
  Future<PatrouilleActive> activePatrouille(String token) =>
      throw UnimplementedError();

  @override
  Future<Paginated<Pv>> pvs(String token, {int pageSize = 20}) =>
      throw UnimplementedError();

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
  Future<PvPublic> verifyPublicPv(String pvNumber) =>
      throw UnimplementedError();
}
