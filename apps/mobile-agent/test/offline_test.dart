import 'dart:io';

import 'package:apmtrack_agent/core/api/api_client.dart';
import 'package:apmtrack_agent/core/auth/session_controller.dart';
import 'package:apmtrack_agent/core/auth/session_store.dart';
import 'package:apmtrack_agent/core/models.dart';
import 'package:apmtrack_agent/core/offline/offline_models.dart';
import 'package:apmtrack_agent/core/offline/offline_store.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('Offline read cache', () {
    test('restores cached data and stays authenticated when offline', () async {
      final cache = MemoryOfflineCacheStore();
      final store = MemorySessionStore();
      await store.write(_session);

      final online = SessionController(
        api: _OfflineFakeApi(),
        store: store,
        cache: cache,
      );
      await online.bootstrap();
      expect(online.status, SessionStatus.authenticated);
      expect(online.pvs, hasLength(1));

      // A fresh controller with the same cache but no network.
      final offline = SessionController(
        api: _OfflineFakeApi()..networkDown = true,
        store: store,
        cache: cache,
      );
      await offline.bootstrap();

      expect(offline.status, SessionStatus.authenticated);
      expect(offline.offline, isTrue);
      expect(offline.profile, isNotNull);
      expect(offline.pvs, hasLength(1));
      expect(offline.interventions, hasLength(1));
    });
  });

  group('Offline PV draft queue', () {
    SessionController build(_OfflineFakeApi api) => SessionController(
      api: api,
      store: MemorySessionStore(),
      cache: MemoryOfflineCacheStore(),
    )..session = _session;

    test('queues a PV as a draft on network failure', () async {
      final api = _OfflineFakeApi()..createBehavior = _CreateBehavior.network;
      final controller = build(api)..interventions = [_intervention];

      final outcome = await controller.createPv(_payload);

      expect(outcome.queued, isTrue);
      expect(controller.drafts, hasLength(1));
      expect(controller.drafts.single.status, PvDraftStatus.pending);
      expect(controller.drafts.single.interventionName, _intervention.nom);
      expect(controller.drafts.single.amountFcfa, _intervention.montantFcfa);
    });

    test(
      'preserves enriched subject and vehicle fields in queued draft',
      () async {
        final api = _OfflineFakeApi()..createBehavior = _CreateBehavior.network;
        final controller = build(api)..interventions = [_intervention];
        const payload = CreatePvPayload(
          interventionId: 'intervention-1',
          subjectType: PvSubjectTypes.personWithVehicle,
          verbalizedName: 'Jean Test',
          verbalizedFirstName: 'Jean',
          verbalizedLastName: 'Test',
          verbalizedIdentityType: 'CNI',
          verbalizedIdentityNumber: 'ID123',
          verbalizedPhone: '699000000',
          vehicleRegistrationCardNumber: 'CG123',
          vehicleMake: 'Toyota',
          locationDescription: 'Avenue Kennedy',
        );

        final outcome = await controller.createPv(payload);

        expect(outcome.queued, isTrue);
        expect(controller.drafts.single.payload.vehiclePlate, isNull);
        expect(
          controller.drafts.single.payload.vehicleRegistrationCardNumber,
          'CG123',
        );
        expect(controller.drafts.single.payload.verbalizedIdentityType, 'CNI');
        expect(controller.drafts.single.payload.verbalizedFirstName, 'Jean');
      },
    );

    test('a business rejection (4xx) is surfaced, not queued', () async {
      final api = _OfflineFakeApi()..createBehavior = _CreateBehavior.reject;
      final controller = build(api);

      await expectLater(
        () => controller.createPv(_payload),
        throwsA(isA<ApiException>()),
      );
      expect(controller.drafts, isEmpty);
    });

    test('syncs a queued draft to the server when back online', () async {
      final api = _OfflineFakeApi()..createBehavior = _CreateBehavior.network;
      final controller = build(api)..interventions = [_intervention];
      await controller.createPv(_payload);
      expect(controller.drafts, hasLength(1));

      api.createBehavior = _CreateBehavior.success;
      await controller.syncDrafts();

      expect(controller.drafts, isEmpty);
      expect(controller.pvs, hasLength(1));
      expect(controller.pvs.single.pvNumber, _createdPv.pvNumber);
    });

    test('marks a draft as failed on definitive server rejection', () async {
      final api = _OfflineFakeApi()..createBehavior = _CreateBehavior.network;
      final controller = build(api)..interventions = [_intervention];
      await controller.createPv(_payload);

      api.createBehavior = _CreateBehavior.reject;
      await controller.syncDrafts();

      expect(controller.drafts, hasLength(1));
      expect(controller.drafts.single.status, PvDraftStatus.failed);
      expect(controller.drafts.single.error, isNotNull);
      expect(controller.pvs, isEmpty);
    });

    test('keeps the draft pending if still offline at sync time', () async {
      final api = _OfflineFakeApi()..createBehavior = _CreateBehavior.network;
      final controller = build(api)..interventions = [_intervention];
      await controller.createPv(_payload);

      // Still offline.
      await controller.syncDrafts();

      expect(controller.drafts, hasLength(1));
      expect(controller.drafts.single.status, PvDraftStatus.pending);
    });

    test('syncs queued draft photos after creating the server PV', () async {
      final api = _OfflineFakeApi()..createBehavior = _CreateBehavior.network;
      final controller = build(api)..interventions = [_intervention];
      final file = File(
        '${Directory.systemTemp.path}${Platform.pathSeparator}apmtrack-proof-${DateTime.now().microsecondsSinceEpoch}.jpg',
      );
      await file.writeAsBytes([1, 2, 3]);
      final photo = PvDraftPhoto(
        path: file.path,
        filename: 'proof.jpg',
        contentType: 'image/jpeg',
      );

      await controller.createPv(_payload, photos: [photo]);
      expect(controller.drafts.single.photos, hasLength(1));

      api.createBehavior = _CreateBehavior.success;
      await controller.syncDrafts();

      expect(controller.drafts, isEmpty);
      expect(api.uploadedPhotos, hasLength(1));
      expect(await file.exists(), isFalse);
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

const _intervention = Intervention(
  id: 'intervention-1',
  nom: 'Stationnement interdit',
  sujetPaiement: true,
  active: true,
  montantFcfa: 10000,
);

final _serverPv = Pv(
  id: 'pv-1',
  pvNumber: 'PV-YDE1-2026-000001',
  interventionId: 'intervention-1',
  status: 'EN_ATTENTE_PAIEMENT',
  createdAt: DateTime(2026, 6, 5),
  vehiclePlate: 'CE123AB',
  amountInitialFcfa: 10000,
);

final _createdPv = Pv(
  id: 'pv-2',
  pvNumber: 'PV-YDE1-2026-000002',
  interventionId: 'intervention-1',
  status: 'EN_ATTENTE_PAIEMENT',
  createdAt: DateTime(2026, 6, 5),
);

const _session = AuthSession(
  accessToken: 'access-token',
  refreshToken: 'refresh-token',
  user: _user,
);

const _payload = CreatePvPayload(
  interventionId: 'intervention-1',
  vehiclePlate: 'CE456CD',
  locationDescription: 'Avenue Kennedy',
);

enum _CreateBehavior { success, network, reject }

class _OfflineFakeApi implements ApmtrackApi {
  bool networkDown = false;
  _CreateBehavior createBehavior = _CreateBehavior.success;
  final List<String> uploadedPhotos = [];

  Never _offline() => throw ApiException('Reseau indisponible');

  @override
  Future<MobileProfile> mobileMe(String token) async {
    if (networkDown) _offline();
    return _profile;
  }

  @override
  Future<List<Intervention>> mobileInterventions(String token) async {
    if (networkDown) _offline();
    return const [_intervention];
  }

  @override
  Future<PatrouilleActive> activePatrouille(String token) async {
    if (networkDown) _offline();
    return const PatrouilleActive(agents: []);
  }

  @override
  Future<List<Patrouille>> mobilePatrouilles(String token) async {
    if (networkDown) _offline();
    return const [];
  }

  @override
  Future<Paginated<Pv>> pvs(String token, {int pageSize = 20}) async {
    if (networkDown) _offline();
    return Paginated(items: [_serverPv], total: 1);
  }

  @override
  Future<Pv> createPv(String token, CreatePvPayload payload) async {
    switch (createBehavior) {
      case _CreateBehavior.success:
        return _createdPv;
      case _CreateBehavior.network:
        throw ApiException('Reseau indisponible');
      case _CreateBehavior.reject:
        throw ApiException('Double verbalisation bloquante', statusCode: 409);
    }
  }

  @override
  Future<Pv> updatePv(
    String token,
    String pvId,
    CreatePvPayload payload,
  ) async {
    return _createdPv;
  }

  @override
  Future<AuthSession> refresh(String refreshToken) async => _session;

  @override
  Future<void> recordPatrouillePosition(
    String token,
    String patrouilleId, {
    required double latitude,
    required double longitude,
    double? accuracyM,
  }) async {}

  @override
  Future<AuthSession> login(String email, String password) =>
      throw UnimplementedError();

  @override
  Future<void> logout(String token, String refreshToken) async {}

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
  }) async {
    uploadedPhotos.add('$pvId/$filename/${bytes.length}/$contentType');
    return PvPhoto(
      id: 'photo-${uploadedPhotos.length}',
      pvId: pvId,
      contentType: contentType,
      sizeBytes: bytes.length,
    );
  }

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
